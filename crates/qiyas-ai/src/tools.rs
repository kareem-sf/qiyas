//! The tool catalogue: names, descriptions written for the model, and strict input schemas.
//!
//! Every schema is an object with `additionalProperties: false` and a `required` list, and uses
//! only what the API's strict tool mode accepts (no numeric, length or item-count limits); the
//! executor checks those limits itself.

use serde_json::{json, Value};

/// One tool the model can call.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema of the input: an object, `additionalProperties: false`, with `required`.
    pub input_schema: Value,
    /// Whether the tool changes the take-off or the BOQ work (an undo step, and the take-off
    /// panel must refresh). Showing another sheet or hiding layers is not counted.
    pub mutating: bool,
}

/// The tools, in the order the model sees them.
pub fn catalogue() -> Vec<ToolSpec> {
    vec![
        tool(
            "list_sheets",
            "List the drawing's sheets (model space and paper layouts), the sheet shown now and \
             the drawing units. Coordinates, lengths and areas read from the drawing are in \
             drawing units; measurements also give their quantity in metres.",
            json!({}),
            &[],
            false,
        ),
        tool(
            "set_sheet",
            "Show another sheet. Queries and measurements that take no sheet work on the sheet \
             shown.",
            json!({ "sheet": sheet("The sheet name or block name, from list_sheets.") }),
            &["sheet"],
            false,
        ),
        tool(
            "list_layers",
            "List the drawing's layers with their colour and whether they are off, frozen, \
             locked, or hidden by the user in Qiyas.",
            json!({ "offset": offset() }),
            &[],
            false,
        ),
        tool(
            "show_layers",
            "Show or hide layers in Qiyas; the drawing file is not changed. Entities on hidden \
             layers are not drawn and not counted by count_blocks or block_summary.",
            json!({
                "names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Layer names, from list_layers."
                },
                "visible": { "type": "boolean", "description": "true shows them, false hides them." }
            }),
            &["names", "visible"],
            false,
        ),
        tool(
            "find_entities",
            "Find entities on a sheet by layer, type, block name, text or box; every filter given \
             must match, names match in any case. Each row gives the entity's handle (an integer \
             id for get_entity, measure_length, measure_area, add_measurement and zoom_to), type, \
             layer, block name, text, length and area in drawing units, and bounds \
             [[min_x, min_y], [max_x, max_y]]. Includes entities on hidden layers. Paged: total \
             counts every match; pass next_offset as offset for the next page.",
            json!({
                "sheet": sheet("Sheet name or block name; the sheet shown when left out."),
                "layer": { "type": "string", "description": "Only entities on this layer." },
                "kind": {
                    "type": "string",
                    "description": "Only this entity type, e.g. Line, LwPolyline, Polyline, Arc, \
                                    Circle, Hatch, Insert (a block reference), Text, MText."
                },
                "block_name": { "type": "string", "description": "Only references of this block." },
                "text_contains": {
                    "type": "string",
                    "description": "Only Text, MText or block references whose text or attribute \
                                    values contain this, in any case."
                },
                "within": bbox("Only entities with at least one point inside this box."),
                "offset": offset(),
                "limit": { "type": "integer", "description": "Rows per page, at most 500. Default 100." }
            }),
            &[],
            false,
        ),
        tool(
            "get_entity",
            "One entity by handle: its type, layer, block name, text, and length and area in \
             drawing units.",
            json!({ "handle": handle("The entity's handle, from find_entities.") }),
            &["handle"],
            false,
        ),
        tool(
            "read_text",
            "All text on a sheet, including text inside blocks and block attributes, with its \
             position and height in drawing units. Use it to find room names, labels, notes and \
             titles.",
            json!({
                "sheet": sheet("Sheet name or block name; the sheet shown when left out."),
                "within": bbox("Only text placed inside this box."),
                "offset": offset()
            }),
            &[],
            false,
        ),
        tool(
            "read_tables",
            "Schedules on a sheet: table entities, and grids of loose text laid out as a table \
             (door, window and finish schedules). Each has a name, columns and rows of text.",
            json!({
                "sheet": sheet("Sheet name or block name; the sheet shown when left out."),
                "offset": offset()
            }),
            &[],
            false,
        ),
        tool(
            "read_dimensions",
            "Dimensions on a sheet with the value each one measures (drawing units, or degrees \
             for angles) and any text written over it. Use them to check measured lengths.",
            json!({
                "sheet": sheet("Sheet name or block name; the sheet shown when left out."),
                "offset": offset()
            }),
            &[],
            false,
        ),
        tool(
            "block_summary",
            "How many references of each block sit on each layer of a sheet, visible layers \
             only, most first. Use it to see what can be counted before count_blocks.",
            json!({
                "sheet": sheet("Sheet name or block name; the sheet shown when left out."),
                "offset": offset()
            }),
            &[],
            false,
        ),
        tool(
            "measure_length",
            "Measure a length and add it to the take-off. Give handle (a line, polyline or arc \
             from find_entities; a closed shape gives its perimeter) or points, not both. Prefer \
             handle over points: it measures the drawn geometry exactly. Returns the measurement \
             with its id; shown is in metres.",
            json!({
                "handle": handle("The entity to measure, from find_entities."),
                "points": points("A polyline in drawing coordinates, 2 points or more.")
            }),
            &[],
            true,
        ),
        tool(
            "measure_area",
            "Measure an area and add it to the take-off. Give handle (a closed polyline, circle \
             or hatch from find_entities) or points, not both. Prefer handle over points: it \
             measures the drawn geometry exactly. Returns the measurement with its id; shown is \
             in square metres.",
            json!({
                "handle": handle("The closed entity to measure, from find_entities."),
                "points": points("The corners of the ring in drawing coordinates, 3 points or \
                                  more; it is closed for you.")
            }),
            &[],
            true,
        ),
        tool(
            "count_blocks",
            "Count the references of a block on the sheet shown, on visible layers only, and add \
             the count to the take-off in pieces. Block names come from block_summary or \
             find_entities.",
            json!({
                "block_name": { "type": "string", "description": "The block to count, in any case." },
                "layer": { "type": "string", "description": "Only count references on this layer." }
            }),
            &["block_name"],
            true,
        ),
        tool(
            "add_measurement",
            "Add a take-off line you describe. Geometry is points (drawing coordinates) or \
             handles (entities from find_entities, summed), not both. A length, area or count is \
             worked out from the geometry: a count is one piece per point or per handle, and \
             count handles must be block references. A volume is the area times height. An item \
             is a quantity in its own unit (e.g. 5 bags) and may have no geometry. For one shape \
             prefer measure_length, measure_area or count_blocks; use this to sum several \
             entities, for volumes and items, or to set the description at once.",
            json!({
                "kind": {
                    "type": "string",
                    "enum": ["length", "area", "volume", "count", "item"],
                    "description": "What is measured."
                },
                "description": {
                    "type": "string",
                    "description": "What was measured and where, e.g. 'Block wall 200 mm, ground floor'."
                },
                "points": points("Geometry in drawing coordinates."),
                "handles": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "Entities from find_entities; for a count, block references only."
                },
                "height": { "type": "number", "description": "A volume's height in metres, above 0." },
                "value": { "type": "number", "description": "An item's quantity, in its unit." },
                "unit": { "type": "string", "description": "An item's unit, e.g. bags, t, nr." },
                "note": { "type": "string", "description": "A note for the user." }
            }),
            &["kind", "description"],
            true,
        ),
        tool(
            "set_height",
            "Turn an area into a volume of that area times height, or change a volume's height.",
            json!({
                "id": id(),
                "height": { "type": "number", "description": "Height in metres, above 0." }
            }),
            &["id", "height"],
            true,
        ),
        tool(
            "list_takeoff",
            "The take-off: every measurement with its id, kind, value (drawing units), shown in \
             unit (m, m2, m3, pcs, or an item's own unit), description, layer, sheet, handles, \
             source (user, ai or mcp), note and BOQ code; then totals per kind and unit.",
            json!({ "offset": offset() }),
            &[],
            false,
        ),
        tool(
            "remove_measurement",
            "Remove a measurement from the take-off. The user can undo it.",
            json!({ "id": id() }),
            &["id"],
            true,
        ),
        tool(
            "set_description",
            "Change a measurement's description. Say what was measured and where.",
            json!({
                "id": id(),
                "text": { "type": "string", "description": "The new description." }
            }),
            &["id", "text"],
            true,
        ),
        tool(
            "import_boq",
            "Import a bill of quantities from an Excel file on the user's computer; it replaces \
             the BOQ imported before (the user can undo it). Headers may be English or Arabic. \
             Returns the number of items, the sections, and how many item rows were left out \
             because their quantity is not a number; list_boq gives the items.",
            json!({ "path": { "type": "string", "description": "Full path of the .xlsx file." } }),
            &["path"],
            true,
        ),
        tool(
            "list_boq",
            "The imported BOQ items: code, description, unit (m, m2, m3, pcs, kg, t, or as \
             written), quantity, rate, section, tolerance in percent (null: the user's setting) \
             and the row in the workbook. Paged: total counts every item; pass next_offset as \
             offset for the next page.",
            json!({ "offset": offset() }),
            &[],
            false,
        ),
        tool(
            "link_boq",
            "Link a measurement to a BOQ item, so its quantity counts towards that item.",
            json!({
                "id": id(),
                "code": { "type": "string", "description": "The BOQ item code; empty unlinks it." }
            }),
            &["id", "code"],
            true,
        ),
        tool(
            "compare_boq",
            "Compare the take-off with the BOQ and list the conflicts: quantity mismatches, work \
             missing from the BOQ or from the drawings, schedules that disagree with the plans.",
            json!({
                "tolerance": {
                    "type": "number",
                    "description": "Allowed difference in percent; the user's setting when left out."
                }
            }),
            &[],
            true,
        ),
        tool(
            "list_conflicts",
            "The conflicts from the last comparison, with their status and notes.",
            json!({}),
            &[],
            false,
        ),
        tool(
            "resolve_conflict",
            "Set a conflict's status, with a note saying why.",
            json!({
                "id": { "type": "string", "description": "The conflict's id, from list_conflicts." },
                "status": { "type": "string", "enum": ["open", "resolved", "ignored"] },
                "note": { "type": "string", "description": "Why." }
            }),
            &["id", "status", "note"],
            true,
        ),
        tool(
            "add_markup",
            "Put a markup on the drawing for the user: a cloud around something (a ring of 3 \
             points or more) or a note at a point (1 point or more), with text. Markups are kept \
             with the take-off but are not quantities.",
            json!({
                "points": points("Where, in drawing coordinates."),
                "text": { "type": "string", "description": "What the user should look at, and why." },
                "kind": { "type": "string", "enum": ["cloud", "note"] }
            }),
            &["points", "text", "kind"],
            true,
        ),
        tool(
            "zoom_to",
            "Zoom the user's view to entities or to a box on the sheet shown. Give handles or \
             bounds, not both.",
            json!({
                "handles": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "Entities from find_entities, drawn on the sheet shown."
                },
                "bounds": bbox("The box to show.")
            }),
            &[],
            false,
        ),
        tool(
            "view_image",
            "A PNG picture of the sheet shown, so you can look at the drawing: lines in their \
             colours on white, the take-off in violet. The caption gives the bounds used.",
            json!({
                "bounds": bbox("The part to show; the whole sheet when left out."),
                "width": { "type": "integer", "description": "Width in pixels, at most 1600. Default 1024." }
            }),
            &[],
            false,
        ),
    ]
}

fn tool(
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &[&str],
    mutating: bool,
) -> ToolSpec {
    ToolSpec {
        name,
        description,
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }),
        mutating,
    }
}

fn sheet(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn handle(description: &str) -> Value {
    json!({ "type": "integer", "description": description })
}

fn id() -> Value {
    json!({ "type": "string", "description": "The measurement's id, from the take-off." })
}

fn offset() -> Value {
    json!({
        "type": "integer",
        "description": "Rows to skip; next_offset from the previous page. Default 0."
    })
}

/// An `[x, y]` point in drawing coordinates.
fn point() -> Value {
    json!({ "type": "array", "items": { "type": "number" }, "description": "[x, y]" })
}

fn points(description: &str) -> Value {
    json!({ "type": "array", "items": point(), "description": description })
}

fn bbox(description: &str) -> Value {
    json!({
        "type": "array",
        "items": point(),
        "description": format!("{description} [[min_x, min_y], [max_x, max_y]] in drawing coordinates.")
    })
}

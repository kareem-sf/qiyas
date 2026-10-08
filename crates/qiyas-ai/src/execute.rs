//! Runs a tool call on the session: parses the input, dispatches to a query or to `run_as`, and
//! keeps the JSON result under 40 KB. Nothing from a tool's input is ever logged.

use std::collections::{HashMap, HashSet};

use qiyas_core::{Command, CoreError, EntityFilter, Kind, Outcome, Session, Source};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::image::{height_for, normalized, render_png, MAX_SIDE};

/// Largest JSON result, in bytes. Longer results lose rows (or have their strings cut).
const MAX_OUTPUT: usize = 40_960;
/// `find_entities` rows per page when the model gives no limit.
const DEFAULT_LIMIT: usize = 100;
const DEFAULT_WIDTH: u32 = 1024;
const MIN_WIDTH: u32 = 16;
/// Margin around the whole sheet in `view_image`, as a share of its size.
const MARGIN: f64 = 0.02;
const BOQ_LATER: &str = "This tool arrives with the BOQ tasks; it is not available yet.";

/// What a tool returns: JSON for the model, or a picture of the drawing.
#[derive(Debug, Clone)]
pub enum ToolOutput {
    Json(Value),
    Image { png: Vec<u8>, caption: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("No tool named {0}")]
    UnknownTool(String),
    /// The input is wrong: bad JSON shape, a missing entity, sheet, block or measurement.
    #[error("{0}")]
    BadInput(String),
    /// The tool is in the catalogue but its backing is not built yet.
    #[error("{0}")]
    NotAvailable(String),
    #[error("{0}")]
    Failed(String),
}

impl From<CoreError> for ToolError {
    fn from(e: CoreError) -> Self {
        match e {
            // Handles are integers in the tools' JSON, not hex as in the core's message.
            CoreError::NoSuchEntity(h) => ToolError::BadInput(format!("No entity with handle {h}")),
            CoreError::NoSuchSheet(_) | CoreError::InvalidCommand(_) => {
                ToolError::BadInput(e.to_string())
            }
            other => ToolError::Failed(other.to_string()),
        }
    }
}

/// Runs the tool `name` with `input` on `session` for `source` (new measurements are marked as
/// made by it). Drawing coordinates are in drawing units (`session.drawing.units`); heights
/// are given in metres.
pub fn execute(
    session: &mut Session,
    name: &str,
    input: &Value,
    source: Source,
) -> Result<ToolOutput, ToolError> {
    let empty = json!({});
    let input = if input.is_null() { &empty } else { input };
    let (value, offset) = match name {
        "list_sheets" => {
            parse::<NoInput>(input)?;
            let info = session.info();
            let out = json!({
                "sheets": info.sheets,
                "current": session.sheet,
                "units": info.units,
            });
            (out, 0)
        }
        "set_sheet" => (set_sheet(session, parse(input)?, source)?, 0),
        "list_layers" => {
            let Paged { offset } = parse(input)?;
            (list_layers(session, offset), offset)
        }
        "show_layers" => (show_layers(session, parse(input)?, source)?, 0),
        "find_entities" => {
            let input: FindInput = parse(input)?;
            let offset = input.offset.unwrap_or(0);
            (find_entities(session, input)?, offset)
        }
        "get_entity" => {
            let Handle { handle } = parse(input)?;
            let entity = session.entity_info(handle)?;
            (json!({ "entity": entity, "units": units(session) }), 0)
        }
        "read_text" => {
            let ReadInput {
                sheet,
                within,
                offset,
            } = parse(input)?;
            let block = sheet_block(session, sheet.as_deref())?;
            let rows = session.read_text(Some(&block), within);
            (with_units(session, page(rows, offset)), offset)
        }
        "read_tables" => {
            let SheetInput { sheet, offset } = parse(input)?;
            let block = sheet_block(session, sheet.as_deref())?;
            (page(session.read_tables(Some(&block)), offset), offset)
        }
        "read_dimensions" => {
            let SheetInput { sheet, offset } = parse(input)?;
            let block = sheet_block(session, sheet.as_deref())?;
            let rows = session.read_dimensions(Some(&block));
            (with_units(session, page(rows, offset)), offset)
        }
        "block_summary" => {
            let SheetInput { sheet, offset } = parse(input)?;
            let block = sheet_block(session, sheet.as_deref())?;
            (page(session.block_summary(Some(&block)), offset), offset)
        }
        "measure_length" => (measure(session, parse(input)?, Kind::Length, source)?, 0),
        "measure_area" => (measure(session, parse(input)?, Kind::Area, source)?, 0),
        "count_blocks" => (count_blocks(session, parse(input)?, source)?, 0),
        "add_measurement" => (add_measurement(session, parse(input)?, source)?, 0),
        "set_height" => {
            let SetHeight { id, height } = parse(input)?;
            let height = height / session.drawing.units.to_metres();
            let cmd = Command::SetHeight {
                id: id.clone(),
                height,
            };
            (edit(session, cmd, &id, source)?, 0)
        }
        "list_takeoff" => {
            let Paged { offset } = parse(input)?;
            let mut out = page(session.takeoff.items.clone(), offset);
            out["totals"] = json!(session.takeoff.totals());
            (out, offset)
        }
        "remove_measurement" => {
            let Id { id } = parse(input)?;
            match session.run_as(Command::RemoveMeasurement { id: id.clone() }, source)? {
                Outcome::Removed { id } => (json!({ "removed": id }), 0),
                _ => return Err(no_measurement(&id)),
            }
        }
        "set_description" => {
            let SetText { id, text } = parse(input)?;
            let cmd = Command::SetDescription {
                id: id.clone(),
                text,
            };
            (edit(session, cmd, &id, source)?, 0)
        }
        "link_boq" => {
            let LinkBoq { id, code } = parse(input)?;
            let code = Some(code.trim().to_string()).filter(|c| !c.is_empty());
            let cmd = Command::LinkBoq {
                id: id.clone(),
                code,
            };
            (edit(session, cmd, &id, source)?, 0)
        }
        "import_boq" => (import_boq(session, parse(input)?, source)?, 0),
        "list_boq" => {
            let Paged { offset } = parse(input)?;
            let boq = session.boq.as_ref().ok_or_else(|| {
                bad("No BOQ is imported yet; import_boq imports one from an Excel file")
            })?;
            let mut out = page(boq.items.clone(), offset);
            out["source"] = json!(boq.source);
            out["skipped_rows"] = json!(boq.skipped_rows);
            (out, offset)
        }
        "compare_boq" | "list_conflicts" | "resolve_conflict" => {
            return Err(ToolError::NotAvailable(BOQ_LATER.to_string()));
        }
        "add_markup" => (add_markup(session, parse(input)?, source)?, 0),
        "zoom_to" => (zoom_to(session, parse(input)?)?, 0),
        "view_image" => return view_image(session, parse(input)?),
        _ => return Err(ToolError::UnknownTool(name.to_string())),
    };
    Ok(ToolOutput::Json(capped(value, offset)))
}

// ---- Inputs. Unknown fields are refused, so a misspelt filter is never silently ignored. ----

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoInput {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paged {
    #[serde(default)]
    offset: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SheetName {
    sheet: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShowLayers {
    names: Vec<String>,
    visible: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FindInput {
    sheet: Option<String>,
    layer: Option<String>,
    kind: Option<String>,
    block_name: Option<String>,
    text_contains: Option<String>,
    within: Option<[[f64; 2]; 2]>,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Handle {
    handle: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadInput {
    sheet: Option<String>,
    within: Option<[[f64; 2]; 2]>,
    #[serde(default)]
    offset: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SheetInput {
    sheet: Option<String>,
    #[serde(default)]
    offset: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasureInput {
    handle: Option<u64>,
    points: Option<Vec<[f64; 2]>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountInput {
    block_name: String,
    layer: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddInput {
    kind: Kind,
    description: String,
    #[serde(default)]
    points: Vec<[f64; 2]>,
    #[serde(default)]
    handles: Vec<u64>,
    height: Option<f64>,
    value: Option<f64>,
    unit: Option<String>,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetHeight {
    id: String,
    height: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetText {
    id: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportBoq {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkBoq {
    id: String,
    code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum MarkupKind {
    Cloud,
    Note,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkupInput {
    points: Vec<[f64; 2]>,
    text: String,
    kind: MarkupKind,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoomInput {
    handles: Option<Vec<u64>>,
    bounds: Option<[[f64; 2]; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageInput {
    bounds: Option<[[f64; 2]; 2]>,
    width: Option<u32>,
}

fn parse<T: DeserializeOwned>(input: &Value) -> Result<T, ToolError> {
    T::deserialize(input).map_err(|e| ToolError::BadInput(format!("Bad input: {e}")))
}

// ---- Tools ----

fn set_sheet(session: &mut Session, input: SheetName, source: Source) -> Result<Value, ToolError> {
    let block = sheet_block(session, Some(&input.sheet))?;
    session.run_as(
        Command::SetSheet {
            block: block.clone(),
        },
        source,
    )?;
    Ok(json!({ "sheet": block, "ui": { "action": "show_sheet", "sheet": block } }))
}

fn list_layers(session: &Session, offset: usize) -> Value {
    let hidden: HashSet<String> = session
        .hidden_layers()
        .iter()
        .map(|n| n.to_uppercase())
        .collect();
    let rows: Vec<Value> = session
        .drawing
        .layers()
        .into_iter()
        .map(|l| {
            let [r, g, b] = l.color;
            json!({
                "name": l.name,
                "color": format!("#{r:02X}{g:02X}{b:02X}"),
                "off": l.off,
                "frozen": l.frozen,
                "locked": l.locked,
                "hidden": hidden.contains(&l.name.to_uppercase()),
            })
        })
        .collect();
    page(rows, offset)
}

fn show_layers(
    session: &mut Session,
    input: ShowLayers,
    source: Source,
) -> Result<Value, ToolError> {
    let unknown: Vec<&str> = input
        .names
        .iter()
        .filter(|n| session.drawing.doc.layers.get(n).is_none())
        .map(String::as_str)
        .collect();
    if !unknown.is_empty() {
        return Err(ToolError::BadInput(format!(
            "No layer named {}; list_layers gives the names",
            unknown.join(", ")
        )));
    }
    for name in input.names {
        session.run_as(
            Command::SetLayerHidden {
                name,
                hidden: !input.visible,
            },
            source,
        )?;
    }
    let hidden = session.hidden_layers();
    Ok(json!({ "hidden": hidden, "ui": { "action": "redraw", "hidden": hidden } }))
}

fn find_entities(session: &Session, input: FindInput) -> Result<Value, ToolError> {
    let sheet = match input.sheet {
        Some(name) => Some(sheet_block(session, Some(&name))?),
        None => None,
    };
    let page = session.find_entities(&EntityFilter {
        sheet,
        layer: input.layer,
        kind: input.kind,
        block_name: input.block_name,
        text_contains: input.text_contains,
        within: input.within,
        offset: input.offset.unwrap_or(0),
        limit: input.limit.unwrap_or(DEFAULT_LIMIT),
    });
    Ok(json!({
        "rows": page.rows,
        "total": page.total,
        "next_offset": page.next_offset,
        "units": units(session),
    }))
}

/// `measure_length` or `measure_area`: an entity by handle, or a shape by points.
fn measure(
    session: &mut Session,
    input: MeasureInput,
    kind: Kind,
    source: Source,
) -> Result<Value, ToolError> {
    let cmd = match (input.handle, input.points) {
        (Some(handle), None) => {
            let entity = session.entity_info(handle)?;
            Command::AddMeasurement {
                kind,
                value: None,
                unit: None,
                height: None,
                points: Vec::new(),
                handles: vec![handle],
                description: format!("{} on {}", entity.kind, entity.layer),
                note: String::new(),
            }
        }
        (None, Some(points)) if kind == Kind::Length => Command::MeasureDistance { points },
        (None, Some(points)) => Command::MeasureArea { points },
        (Some(_), Some(_)) => return Err(bad("Give handle or points, not both")),
        (None, None) => return Err(bad("Give handle or points")),
    };
    measured(session.run_as(cmd, source)?)
}

fn count_blocks(
    session: &mut Session,
    input: CountInput,
    source: Source,
) -> Result<Value, ToolError> {
    let doc = &session.drawing.doc;
    if doc.block_records.get(&input.block_name).is_none() {
        return Err(bad(&format!(
            "No block named {}; block_summary lists the blocks",
            input.block_name
        )));
    }
    if let Some(layer) = &input.layer {
        if doc.layers.get(layer).is_none() {
            return Err(bad(&format!("No layer named {layer}")));
        }
    }
    let cmd = Command::CountBlock {
        block_name: input.block_name,
        layer: input.layer,
    };
    measured(session.run_as(cmd, source)?)
}

fn add_measurement(
    session: &mut Session,
    input: AddInput,
    source: Source,
) -> Result<Value, ToolError> {
    if input.unit.as_deref() == Some("markup") {
        return Err(bad("Use add_markup for markups"));
    }
    let mut seen = HashSet::new();
    for &h in &input.handles {
        if !seen.insert(h) {
            return Err(bad(&format!("Handle {h} is given twice")));
        }
        let entity = session.entity_info(h)?;
        if input.kind == Kind::Count && entity.kind != "Insert" {
            return Err(bad(&format!(
                "Handle {h} is a {}, not a block reference; a count takes block references only",
                entity.kind
            )));
        }
    }
    let to_drawing_units = session.drawing.units.to_metres();
    let cmd = Command::AddMeasurement {
        kind: input.kind,
        value: input.value,
        unit: input.unit,
        height: input.height.map(|h| h / to_drawing_units),
        points: input.points,
        handles: input.handles,
        description: input.description,
        note: input.note,
    };
    measured(session.run_as(cmd, source)?)
}

/// Reads the workbook at `path` and makes it the session's BOQ (an undo step). Returns a summary,
/// not the items: list_boq pages through those.
fn import_boq(session: &mut Session, input: ImportBoq, source: Source) -> Result<Value, ToolError> {
    // A wrong path or a sheet with no header is the model's to fix, so both are bad input.
    let boq = qiyas_boq::import_boq(std::path::Path::new(&input.path))
        .map_err(|e| ToolError::BadInput(e.to_string()))?;
    let mut sections: Vec<&str> = Vec::new();
    for item in &boq.items {
        if !item.section.is_empty() && sections.last() != Some(&item.section.as_str()) {
            sections.push(&item.section);
        }
    }
    let out = json!({
        "source": boq.source,
        "items": boq.items.len(),
        "skipped_rows": boq.skipped_rows,
        "sections": sections,
    });
    session.run_as(Command::SetBoq { boq }, source)?;
    Ok(out)
}

fn add_markup(
    session: &mut Session,
    input: MarkupInput,
    source: Source,
) -> Result<Value, ToolError> {
    let (note, least) = match input.kind {
        MarkupKind::Cloud => ("cloud", 3),
        MarkupKind::Note => ("note", 1),
    };
    if input.points.len() < least {
        return Err(bad(&format!("A {note} needs at least {least} point(s)")));
    }
    let bounds = bounds_of(&input.points);
    let cmd = Command::AddMeasurement {
        kind: Kind::Item,
        value: Some(0.0),
        unit: Some("markup".to_string()),
        height: None,
        points: input.points,
        handles: Vec::new(),
        description: input.text,
        note: note.to_string(),
    };
    let mut out = measured(session.run_as(cmd, source)?)?;
    let id = out["measurement"]["id"].clone();
    out["ui"] = json!({ "action": "show_markup", "id": id, "bounds": bounds });
    Ok(out)
}

fn zoom_to(session: &Session, input: ZoomInput) -> Result<Value, ToolError> {
    let (bounds, handles) = match (input.handles, input.bounds) {
        (None, Some(bounds)) => (normalized(bounds), Vec::new()),
        (Some(handles), None) if !handles.is_empty() => (drawn_bounds(session, &handles)?, handles),
        (Some(_), None) => return Err(bad("Give at least one handle")),
        (Some(_), Some(_)) => return Err(bad("Give handles or bounds, not both")),
        (None, None) => return Err(bad("Give handles or bounds")),
    };
    Ok(json!({
        "bounds": bounds,
        "ui": { "action": "zoom", "bounds": bounds, "handles": handles },
    }))
}

fn view_image(session: &Session, input: ImageInput) -> Result<ToolOutput, ToolError> {
    let width = input
        .width
        .unwrap_or(DEFAULT_WIDTH)
        .clamp(MIN_WIDTH, MAX_SIDE);
    let batch = session.batch();
    let bounds = match input.bounds {
        Some(b) => normalized(b),
        None if batch.segments.is_empty() => {
            return Err(bad("Nothing is drawn on the sheet shown"));
        }
        None => with_margin(batch.bounds),
    };
    let bounds = fit(bounds, width);
    let measurements: Vec<_> = session
        .takeoff
        .items
        .iter()
        .filter(|m| m.sheet == session.sheet)
        .cloned()
        .collect();
    let png = render_png(&batch, &measurements, bounds, width);
    let [lo, hi] = bounds;
    let height = height_for(hi[0] - lo[0], hi[1] - lo[1], width);
    let caption = format!(
        "Sheet {}, bounds [[{:.3}, {:.3}], [{:.3}, {:.3}]] in drawing units ({}), {width} x {height} px. \
         Lines in their colours on white (white lines drawn black); the take-off in violet.",
        session.sheet,
        lo[0],
        lo[1],
        hi[0],
        hi[1],
        units(session),
    );
    Ok(ToolOutput::Image { png, caption })
}

// ---- Helpers ----

/// The block of the sheet named by its name or block name (any case); the current sheet when
/// `None`.
fn sheet_block(session: &Session, name: Option<&str>) -> Result<String, ToolError> {
    let Some(name) = name else {
        return Ok(session.sheet.clone());
    };
    session
        .drawing
        .sheets()
        .into_iter()
        .find(|s| s.block.eq_ignore_ascii_case(name) || s.name.eq_ignore_ascii_case(name))
        .map(|s| s.block)
        .ok_or_else(|| {
            bad(&format!(
                "No sheet named {name}; list_sheets gives the names"
            ))
        })
}

fn units(session: &Session) -> String {
    session.info().units
}

fn with_units(session: &Session, mut out: Value) -> Value {
    out["units"] = json!(units(session));
    out
}

/// `rows` from `offset` on, with the total before paging.
fn page<T: serde::Serialize>(rows: Vec<T>, offset: usize) -> Value {
    let total = rows.len();
    let rows: Vec<T> = rows.into_iter().skip(offset).collect();
    json!({ "rows": rows, "total": total })
}

fn measured(outcome: Outcome) -> Result<Value, ToolError> {
    match outcome {
        Outcome::Measured { measurement } => Ok(json!({ "measurement": measurement })),
        _ => Err(ToolError::Failed("Nothing was measured".to_string())),
    }
}

/// Runs an edit of the measurement `id` and returns it as it is now.
fn edit(session: &mut Session, cmd: Command, id: &str, source: Source) -> Result<Value, ToolError> {
    match session.run_as(cmd, source)? {
        Outcome::Nothing => Err(no_measurement(id)),
        _ => Ok(json!({ "measurement": session.takeoff.get(id) })),
    }
}

fn no_measurement(id: &str) -> ToolError {
    bad(&format!(
        "No measurement with id {id}; list_takeoff gives the ids"
    ))
}

fn bad(why: &str) -> ToolError {
    ToolError::BadInput(why.to_string())
}

/// The box around the drawn segments of `handles` on the current sheet.
fn drawn_bounds(session: &Session, handles: &[u64]) -> Result<[[f64; 2]; 2], ToolError> {
    let batch = session.batch();
    let ranges: HashMap<u64, (u32, u32)> = batch
        .ranges
        .iter()
        .map(|&(h, first, count)| (h, (first, count)))
        .collect();
    let mut points = Vec::new();
    for &h in handles {
        session.entity_info(h)?;
        let (first, count) = ranges
            .get(&h)
            .ok_or_else(|| bad(&format!("Entity {h} is not drawn on the sheet shown")))?;
        let [ox, oy] = batch.origin;
        let segments = &batch.segments[*first as usize * 4..(*first + *count) as usize * 4];
        points.extend(
            segments
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[x, y]| [f64::from(*x) + ox, f64::from(*y) + oy]),
        );
    }
    Ok(bounds_of(&points))
}

fn bounds_of(points: &[[f64; 2]]) -> [[f64; 2]; 2] {
    let first = points.first().copied().unwrap_or_default();
    points.iter().fold([first, first], |[lo, hi], p| {
        [
            [lo[0].min(p[0]), lo[1].min(p[1])],
            [hi[0].max(p[0]), hi[1].max(p[1])],
        ]
    })
}

fn with_margin([lo, hi]: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let m = (hi[0] - lo[0]).max(hi[1] - lo[1]) * MARGIN;
    [[lo[0] - m, lo[1] - m], [hi[0] + m, hi[1] + m]]
}

/// Widens or heightens `bounds` about its centre so a picture `width` pixels wide shows all of
/// it undistorted within 1600 pixels of height. A box with no size becomes one drawing unit.
fn fit([lo, hi]: [[f64; 2]; 2], width: u32) -> [[f64; 2]; 2] {
    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let (mut dx, mut dy) = (hi[0] - lo[0], hi[1] - lo[1]);
    if dx <= 0.0 && dy <= 0.0 {
        (dx, dy) = (1.0, 1.0);
    }
    let w = f64::from(width);
    // Height = width * dy / dx must stay within 1 and 1600 pixels.
    dx = dx.max(dy * w / f64::from(MAX_SIDE));
    dy = dy.max(dx / w);
    [
        [centre[0] - dx / 2.0, centre[1] - dy / 2.0],
        [centre[0] + dx / 2.0, centre[1] + dy / 2.0],
    ]
}

fn size(v: &Value) -> usize {
    serde_json::to_vec(v).map_or(usize::MAX, |b| b.len())
}

/// `value` cut to fit `MAX_OUTPUT` bytes: a `rows` array loses its last rows, with
/// `"truncated": true` and `next_offset` (`offset` plus the rows kept); otherwise strings are
/// shortened. As a last resort the result is replaced by a note saying it was too large.
fn capped(mut value: Value, offset: usize) -> Value {
    if size(&value) <= MAX_OUTPUT {
        return value;
    }
    if let Some(Value::Array(rows)) = value.get_mut("rows").map(Value::take) {
        // Size of everything but the rows, with the widest next_offset it could get.
        value["truncated"] = json!(true);
        value["next_offset"] = json!(offset + rows.len());
        value["rows"] = json!([]);
        let base = size(&value);
        let mut used = base;
        let mut kept = 0;
        for row in &rows {
            // A comma before every row but the first.
            let add = size(row) + usize::from(kept > 0);
            if used + add > MAX_OUTPUT {
                break;
            }
            used += add;
            kept += 1;
        }
        if kept > 0 {
            value["next_offset"] = json!(offset + kept);
            value["rows"] = Value::Array(rows.into_iter().take(kept).collect());
            return value;
        }
        value["rows"] = Value::Array(rows);
    }
    let mut limit = 1024;
    while limit >= 16 {
        let mut cut = value.clone();
        cut_strings(&mut cut, limit);
        if let Some(map) = cut.as_object_mut() {
            map.insert("truncated".to_string(), json!(true));
        }
        if size(&cut) <= MAX_OUTPUT {
            return cut;
        }
        limit /= 2;
    }
    json!({ "truncated": true, "note": "The result was too large to return; ask for less." })
}

fn cut_strings(v: &mut Value, limit: usize) {
    match v {
        Value::String(s) if s.chars().count() > limit => {
            *s = s.chars().take(limit).chain(['…']).collect();
        }
        Value::Array(items) => items.iter_mut().for_each(|v| cut_strings(v, limit)),
        Value::Object(map) => map.values_mut().for_each(|v| cut_strings(v, limit)),
        _ => {}
    }
}

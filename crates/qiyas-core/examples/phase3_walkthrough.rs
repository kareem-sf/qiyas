//! The Phase 3 check through the core, no window:
//! `cargo run --release -p qiyas-core --example phase3_walkthrough -- [drawing] [--keep <dir>]`
//!
//! `--room x,y[,box]` hatches the room round that point first (a box `box` drawing units across
//! to begin with, about 50 screen pixels).
//!
//! Copies the drawing (default `samples/test-plan.dwg`) into a temp folder (or `<dir>`, kept, so
//! the saved `check.dwg` and `check-dxf.dxf` can be opened in another CAD program) and never
//! writes to the drawing given. On the copy it redraws a wall (a polyline on the A-WALL layer),
//! undoes and redoes it, hides and shows a layer, adds a linear dimension and a revision cloud,
//! trims a line, moves a block, makes a block with an attribute and inserts it, edits a text,
//! hatches by a pick point, runs find and replace, saves and restores a layer state, adds a
//! table, explodes the biggest block (and undoes it), picks a window over the whole sheet,
//! copies and pastes with a second drawing both ways, autosaves and recovers, then saves as DWG
//! and as DXF, reopens both and checks every entity (counted by kind) and take-off row is back.
//! Prints each step's time in milliseconds.

use std::collections::{BTreeMap, HashSet};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::Instant;

use opencadcodec::EntityType;
use qiyas_core::{discard_autosave, Boundary, Command, Outcome, Session};
use serde_json::{json, Value};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// The `EntityType` variant name, e.g. "LwPolyline", without formatting the entity's data.
fn kind(e: &EntityType) -> String {
    struct UpToParen(String);
    impl fmt::Write for UpToParen {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            match s.find('(') {
                Some(i) => {
                    self.0.push_str(&s[..i]);
                    Err(fmt::Error)
                }
                None => {
                    self.0.push_str(s);
                    Ok(())
                }
            }
        }
    }
    let mut name = UpToParen(String::new());
    let _ = write!(name, "{e:?}");
    name.0
}

/// Entities on the sheet on screen by kind, leaving out the take-off Save embeds (`QIYAS-*`).
fn kinds(s: &Session) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for e in s.drawing.entities_in(&s.sheet) {
        if e.common().layer.to_ascii_uppercase().starts_with("QIYAS-") {
            continue;
        }
        *out.entry(kind(e)).or_insert(0) += 1;
    }
    out
}

fn handles(s: &Session) -> HashSet<u64> {
    s.drawing
        .entities_in(&s.sheet)
        .map(|e| e.common().handle.value())
        .collect()
}

/// Take-off rows, and those whose every handle is still on the sheet.
fn linked(s: &Session) -> (usize, usize) {
    let on_sheet = handles(s);
    let rows = &s.takeoff.items;
    let linked = rows
        .iter()
        .filter(|m| !m.handles.is_empty() && m.handles.iter().all(|h| on_sheet.contains(h)))
        .count();
    (rows.len(), linked)
}

fn cmd(v: &Value) -> Command {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"))
}

fn brief(o: &Outcome) -> String {
    let mut text = serde_json::to_string(o).unwrap_or_default();
    if text.len() > 150 {
        let mut cut = 150;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push('…');
    }
    text
}

/// Runs one command, prints its time and outcome; `None` (printed) when it is refused.
fn run(s: &mut Session, label: &str, v: Value) -> Option<Outcome> {
    let t = Instant::now();
    let out = s.run(cmd(&v));
    let took = ms(t);
    match out {
        Ok(o) => {
            println!("{label:30} {took:9.1} ms  {}", brief(&o));
            Some(o)
        }
        Err(e) => {
            println!("{label:30} {took:9.1} ms  REFUSED: {e}");
            None
        }
    }
}

/// What the window does after a drawing change: build and send the sheet again.
fn redraw(s: &Session, label: &str) -> f64 {
    let t = Instant::now();
    let bytes = s.batch_bytes_if(None, None).map_or(0, |b| b.len());
    let took = ms(t);
    println!(
        "{:30} {took:9.1} ms  {:.1} MB",
        format!("  redraw after {label}"),
        bytes as f64 / 1e6
    );
    took
}

fn added(o: &Option<Outcome>) -> Vec<u64> {
    match o {
        Some(Outcome::Added { handles }) => handles.clone(),
        Some(Outcome::Exploded { handles, .. }) => handles.clone(),
        Some(Outcome::Edited { added, .. }) => added.clone(),
        _ => Vec::new(),
    }
}

fn target(h: u64) -> Value {
    json!({ "handle": h, "chain": [] })
}

fn measure(s: &mut Session, label: &str, h: u64) -> Option<String> {
    match run(s, label, json!({ "type": "measure_entity", "handle": h }))? {
        Outcome::Measured { measurement } => Some(measurement.id),
        _ => None,
    }
}

fn compare(label: &str, before: &BTreeMap<String, usize>, after: &BTreeMap<String, usize>) {
    let total = |m: &BTreeMap<String, usize>| m.values().sum::<usize>();
    let keys: HashSet<&String> = before.keys().chain(after.keys()).collect();
    let mut diffs: Vec<String> = keys
        .into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .map(|k| {
            format!(
                "{k} {} -> {}",
                before.get(k).unwrap_or(&0),
                after.get(k).unwrap_or(&0)
            )
        })
        .collect();
    diffs.sort();
    if diffs.is_empty() {
        println!(
            "{label:30} all {} entities back, same kinds ({} kinds)",
            total(after),
            after.len()
        );
    } else {
        println!(
            "{label:30} DIFFERENT: {} before, {} after: {}",
            total(before),
            total(after),
            diffs.join(", ")
        );
    }
}

/// Hatch by a pick point: the region round `at` (a box `start` across to begin with), hatched
/// with ANSI31 and measured. The hatch's handle, when there was a region.
fn try_hatch(s: &mut Session, label: &str, at: [f64; 2], start: f64) -> Option<u64> {
    let t = Instant::now();
    let found = s.boundary_at(at, start);
    let took = ms(t);
    match found {
        Boundary::Ring {
            ring,
            holes,
            bulges,
        } => {
            let area = |r: &[[f64; 2]]| {
                let n = r.len();
                (0..n)
                    .map(|i| {
                        let (a, b) = (r[i], r[(i + 1) % n]);
                        a[0] * b[1] - b[0] * a[1]
                    })
                    .sum::<f64>()
                    .abs()
                    / 2.0
            };
            let bounds = ring
                .iter()
                .fold([[f64::MAX; 2], [f64::MIN; 2]], |[lo, hi], p| {
                    [
                        [lo[0].min(p[0]), lo[1].min(p[1])],
                        [hi[0].max(p[0]), hi[1].max(p[1])],
                    ]
                });
            println!(
                "{label:30} {took:9.1} ms  ring of {} points ({:.1}, {:.1}) to ({:.1}, {:.1}), area {:.3} less {} islands {:.3} (drawing units)",
                ring.len(),
                bounds[0][0],
                bounds[0][1],
                bounds[1][0],
                bounds[1][1],
                area(&ring),
                holes.len(),
                holes.iter().map(|h| area(h)).sum::<f64>()
            );
            let h = *added(&run(
                s,
                "  hatch it (ANSI31)",
                json!({ "type": "draw", "kind": "hatch", "points": ring, "holes": holes,
                        "bulges": bulges, "pattern": "ansi31" }),
            ))
            .first()?;
            measure(s, "  measure the hatch", h);
            Some(h)
        }
        other => {
            println!("{label:30} {took:9.1} ms  {other:?}");
            None
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let keep = args
        .iter()
        .position(|a| a == "--keep")
        .map(|i| PathBuf::from(&args[i + 1]));
    let samples = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples"));
    let source = args
        .iter()
        .enumerate()
        .find(|(i, a)| {
            !a.starts_with("--")
                && (*i == 0 || !matches!(args[i - 1].as_str(), "--keep" | "--room"))
        })
        .map(|(_, a)| PathBuf::from(a))
        .unwrap_or_else(|| samples.join("test-plan.dwg"));
    let room_arg = args
        .iter()
        .position(|a| a == "--room")
        .and_then(|i| args.get(i + 1))
        .map(|v| {
            v.split(',')
                .map(|n| n.parse::<f64>().expect("--room x,y[,box]"))
                .collect::<Vec<_>>()
        });
    let temp = tempfile::tempdir().unwrap();
    let work = keep.clone().unwrap_or_else(|| temp.path().to_path_buf());
    std::fs::create_dir_all(&work).unwrap();
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("dwg");
    let dwg = work.join(format!("walk.{ext}"));
    std::fs::copy(&source, &dwg).unwrap();
    let other_path = work.join("other.dwg");
    std::fs::copy(samples.join("test-plan.dwg"), &other_path).unwrap();
    println!("drawing  {}", source.display());

    // Open and first draw.
    let t = Instant::now();
    let mut s = Session::open(&dwg).expect("open");
    println!("{:30} {:9.1} ms", "open", ms(t));
    redraw(&s, "open (first draw)");
    let batch = s.batch();
    let [[x0, y0], [x1, y1]] = batch.bounds;
    println!(
        "segments {}  sheet {}  bounds ({x0:.1}, {y0:.1}) to ({x1:.1}, {y1:.1})",
        batch.segments.len() / 4,
        s.sheet
    );
    drop(batch);
    let start_kinds = kinds(&s);
    println!(
        "entities {} on the sheet, {} kinds",
        start_kinds.values().sum::<usize>(),
        start_kinds.len()
    );
    // Hatch a real room by a pick point, before anything is drawn: `--room x,y`, the test
    // plan's living room, or the first room-like label found.
    let room = room_arg.as_ref().map(|r| [r[0], r[1]]).or_else(|| {
        if source.ends_with("test-plan.dwg") {
            return Some([1.0, 1.0]);
        }
        let words = ["OFFICE", "MEETING", "LOBBY", "ROOM", "STORE", "TOILET"];
        let row = s.read_text(None, None).into_iter().find(|t| {
            words
                .iter()
                .any(|w| t.text.to_ascii_uppercase().contains(w))
        })?;
        println!(
            "room label {:?} at ({:.1}, {:.1})",
            row.text, row.at[0], row.at[1]
        );
        Some(row.at)
    });
    let mut hatch = Vec::new();
    if let Some(room) = room {
        // About 50 screen pixels with the room's surroundings on screen.
        let start = room_arg
            .as_ref()
            .and_then(|r| r.get(2).copied())
            .unwrap_or(((x1 - x0).min(y1 - y0) / 200.0).max(1e-3));
        hatch.extend(try_hatch(&mut s, "boundary in a real room", room, start));
    }
    // The working area: the middle of the sheet, a unit a twentieth of its smaller side.
    let u = ((x1 - x0).min(y1 - y0) / 20.0).max(1e-3);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let p = |x: f64, y: f64| json!([cx + x * u, cy + y * u]);
    let pt = |x: f64, y: f64| [cx + x * u, cy + y * u];

    // The wall layer: A-WALL, else one that reads A-WALL (a bound xref's), else one with WALL.
    let layers: Vec<String> = s.info().layers.into_iter().map(|l| l.name).collect();
    let wall_layer = layers
        .iter()
        .find(|n| n.eq_ignore_ascii_case("A-WALL"))
        .or_else(|| {
            layers
                .iter()
                .find(|n| qiyas_core::short_layer_name(n).eq_ignore_ascii_case("A-WALL"))
        })
        .or_else(|| {
            layers
                .iter()
                .find(|n| n.to_ascii_uppercase().contains("WALL"))
        })
        .cloned();
    let wall_layer = match wall_layer {
        Some(l) => l,
        None => {
            run(
                &mut s,
                "new layer A-WALL",
                json!({ "type": "new_layer", "name": "A-WALL" }),
            );
            "A-WALL".to_string()
        }
    };
    println!("wall layer {wall_layer}");
    run(
        &mut s,
        "make the wall layer current",
        json!({ "type": "set_current", "current": { "layer": wall_layer } }),
    );

    // Redraw a missing wall, undo, redo.
    let wall = added(&run(
        &mut s,
        "draw wall polyline",
        json!({ "type": "draw", "kind": "polyline",
                "points": [p(-4.0, -3.0), p(4.0, -3.0), p(4.0, 3.0)] }),
    ))[0];
    redraw(&s, "draw");
    let wall_row = measure(&mut s, "measure the new wall", wall);
    let t = Instant::now();
    let undone = s.undo();
    println!("{:30} {:9.1} ms  {undone}", "undo (the measure)", ms(t));
    let t = Instant::now();
    let undone = s.undo();
    println!("{:30} {:9.1} ms  {undone}", "undo (the wall)", ms(t));
    let gone = !handles(&s).contains(&wall);
    redraw(&s, "undo");
    let t = Instant::now();
    s.redo();
    s.redo();
    println!(
        "{:30} {:9.1} ms  wall gone after undo: {gone}, back after redo: {}",
        "redo twice",
        ms(t),
        handles(&s).contains(&wall)
    );

    // A layer click: hide the wall layer, draw, show it again.
    let t = Instant::now();
    s.set_layer_hidden(&wall_layer, true);
    let bytes = s.batch_bytes_if(None, None).map_or(0, |b| b.len());
    println!(
        "{:30} {:9.1} ms  {:.1} MB",
        "layer click (hide + redraw)",
        ms(t),
        bytes as f64 / 1e6
    );
    let t = Instant::now();
    s.set_layer_hidden(&wall_layer, false);
    s.batch_bytes_if(None, None);
    println!("{:30} {:9.1} ms", "layer click (show + redraw)", ms(t));

    // Dimension and revision cloud.
    let dim = added(&run(
        &mut s,
        "linear dimension",
        json!({ "type": "dimension", "kind": "linear",
                "p1": p(-4.0, -3.0), "p2": p(4.0, -3.0), "line_point": p(0.0, -4.5) }),
    ));
    redraw(&s, "dimension");
    if let Some(&h) = dim.first() {
        measure(&mut s, "measure the dimension", h);
    }
    let cloud = added(&run(
        &mut s,
        "revision cloud",
        json!({ "type": "draw", "kind": "revcloud",
                "points": [p(-5.0, -5.5), p(7.0, -5.5), p(7.0, 4.0), p(-5.0, 4.0)],
                "arc_len": u / 2.0 }),
    ));

    // Trim: a line cut by a short cross line.
    let a = added(&run(
        &mut s,
        "draw line to trim",
        json!({ "type": "draw", "kind": "line", "a": p(-3.5, -2.0), "b": p(-0.5, -2.0) }),
    ))[0];
    let b = added(&run(
        &mut s,
        "draw cutting line",
        json!({ "type": "draw", "kind": "line", "a": p(-2.0, -2.5), "b": p(-2.0, -1.5) }),
    ))[0];
    let a_row = measure(&mut s, "measure the line", a);
    run(
        &mut s,
        "trim",
        json!({ "type": "edit", "kind": "trim", "cutters": [target(b)], "target": target(a),
                "at_point": p(-1.0, -2.0) }),
    );
    if let Some(id) = &a_row {
        let row = s.takeoff.items.iter().find(|m| &m.id == id);
        println!(
            "  trimmed row now {:?} (the piece left is {:.3} drawing units long)",
            row.map(|m| m.shown),
            1.5 * u
        );
    }

    // Move a block reference already in the drawing.
    let first_insert = s.drawing.entities_in(&s.sheet).find_map(|e| match e {
        EntityType::Insert(i) if !i.common.layer.to_ascii_uppercase().starts_with("QIYAS") => {
            Some((i.common.handle.value(), i.block_name.clone()))
        }
        _ => None,
    });
    if let Some((h, name)) = &first_insert {
        measure(&mut s, &format!("count block {name}"), *h);
        run(
            &mut s,
            "move a block",
            json!({ "type": "modify", "kind": "move", "targets": [target(*h)],
                    "delta": [u, 0.0] }),
        );
    } else {
        println!("move a block                   no block reference on the sheet");
    }

    // A block from entities, with an attribute, then inserted.
    let rect = added(&run(
        &mut s,
        "draw rectangle for a block",
        json!({ "type": "draw", "kind": "rectangle", "a": p(5.0, -1.0), "b": p(6.0, 0.0) }),
    ));
    let attdef = added(&run(
        &mut s,
        "define attribute MARK",
        json!({ "type": "att_def", "tag": "MARK", "prompt": "Mark", "default": "W0",
                "at": p(5.2, -0.8), "height": u / 5.0 }),
    ));
    let parts: Vec<Value> = rect.iter().chain(&attdef).map(|&h| target(h)).collect();
    run(
        &mut s,
        "create block QIYAS_CHECK",
        json!({ "type": "create_block", "name": "QIYAS_CHECK", "base": p(5.0, -1.0),
                "targets": parts, "convert_to_insert": false }),
    );
    let inserted = added(&run(
        &mut s,
        "insert QIYAS_CHECK",
        json!({ "type": "insert_block", "name": "QIYAS_CHECK", "at": p(5.0, 1.0),
                "attribute_values": { "MARK": "W1" } }),
    ));

    // Edit a text of the drawing (a Latin one), or one added for the check.
    let text = s.drawing.entities_in(&s.sheet).find_map(|e| match e {
        EntityType::Text(t)
            if !t.value.trim().is_empty()
                && t.value.is_ascii()
                && !t.common.layer.to_ascii_uppercase().starts_with("QIYAS") =>
        {
            Some((t.common.handle.value(), t.value.clone()))
        }
        _ => None,
    });
    let (text, value) = match text {
        Some(t) => t,
        None => {
            let h = added(&run(
                &mut s,
                "add a text",
                json!({ "type": "text", "at": p(0.5, -2.0), "height": u / 4.0, "rotation": 0.0,
                        "value": "CHECK" }),
            ))[0];
            (h, "CHECK".to_string())
        }
    };
    run(
        &mut s,
        &format!("edit text {value:?}"),
        json!({ "type": "edit_text", "target": target(text), "value": format!("{value} (Q)") }),
    );

    // Hatch by a pick point: inside a rectangle drawn for it, then at a real point.
    run(
        &mut s,
        "draw rectangle to hatch",
        json!({ "type": "draw", "kind": "rectangle", "a": p(-3.5, 0.0), "b": p(-0.5, 2.5) }),
    );
    redraw(&s, "rectangle");
    hatch.extend(try_hatch(
        &mut s,
        "boundary in the rectangle",
        pt(-2.0, 1.25),
        u / 2.0,
    ));
    println!(
        "  rectangle is {:.3} x {:.3} drawing units",
        3.0 * u,
        2.5 * u
    );
    redraw(&s, "hatch");

    // Find and replace.
    run(
        &mut s,
        "find \"(Q)\" (whole drawing)",
        json!({ "type": "find_text", "find": "(Q)", "scope": "drawing" }),
    );
    run(
        &mut s,
        "replace \"(Q)\" (whole drawing)",
        json!({ "type": "find_replace", "find": "(Q)", "replace": "(QIYAS)",
                "scope": "drawing" }),
    );
    run(
        &mut s,
        "find \"A\" (whole word, drawing)",
        json!({ "type": "find_text", "find": "A", "scope": "drawing", "whole_word": true }),
    );

    // Layer state.
    run(
        &mut s,
        "save layer state",
        json!({ "type": "save_layer_state", "name": "QIYAS CHECK" }),
    );
    run(
        &mut s,
        "turn the wall layer off",
        json!({ "type": "set_layer", "name": wall_layer, "prop": "on", "value": false }),
    );
    run(
        &mut s,
        "restore layer state",
        json!({ "type": "restore_layer_state", "name": "QIYAS CHECK" }),
    );
    let wall_on = s
        .layers()
        .into_iter()
        .find(|l| l.name == wall_layer)
        .map(|l| !l.off);
    println!("  wall layer on after restore: {wall_on:?}");

    // Table.
    let table = added(&run(
        &mut s,
        "table 3 x 2",
        json!({ "type": "table", "at": p(0.5, 2.5), "rows": 3, "cols": 2,
                "col_width": 1.5 * u, "row_height": 0.5 * u,
                "data": [["Item", "Qty"], ["Wall", "8"], ["Door", "3"]] }),
    ));

    // Explode the biggest block reference on the sheet, then undo it.
    let biggest = s
        .drawing
        .entities_in(&s.sheet)
        .filter_map(|e| match e {
            EntityType::Insert(i) => Some((
                i.common.handle.value(),
                i.block_name.clone(),
                s.drawing.entities_in(&i.block_name).count(),
            )),
            _ => None,
        })
        .max_by_key(|(_, _, n)| *n);
    if let Some((h, name, n)) = biggest {
        let before = linked(&s);
        let o = run(
            &mut s,
            &format!("explode {name} ({n} inside)"),
            json!({ "type": "modify", "kind": "explode", "targets": [target(h)] }),
        );
        if o.is_some() {
            redraw(&s, "explode");
            println!("  rows (all, linked) {:?} -> {:?}", before, linked(&s));
            let t = Instant::now();
            s.undo();
            println!("{:30} {:9.1} ms", "undo the explode", ms(t));
            redraw(&s, "undo");
            println!("  rows after undo {:?}", linked(&s));
        }
    }

    // A window and a crossing over the whole sheet.
    let (dx, dy) = ((x1 - x0) * 0.01, (y1 - y0) * 0.01);
    let whole = [[x0 - dx, y0 - dy], [x1 + dx, y1 + dy]];
    for crossing in [false, true] {
        let t = Instant::now();
        let picked = s.pick_window(whole, crossing);
        println!(
            "{:30} {:9.1} ms  {} entities",
            if crossing {
                "crossing over the whole sheet"
            } else {
                "window over the whole sheet"
            },
            ms(t),
            picked.len()
        );
    }

    // Copy and paste with a second drawing, both ways.
    let mut ours: Vec<u64> = vec![wall];
    ours.extend(&dim);
    ours.extend(&cloud);
    ours.extend(&hatch);
    ours.extend(&inserted);
    ours.extend(&table);
    let mut other = Session::open(&other_path).expect("open the second drawing");
    let copied = run(
        &mut s,
        "copy (wall, dim, cloud, hatch...)",
        json!({ "type": "copy_to_clipboard",
                "targets": ours.iter().map(|&h| target(h)).collect::<Vec<_>>() }),
    );
    if let Some(Outcome::Clipboard { text, count }) = copied {
        println!("  clipboard {count} entities, {} bytes of DXF", text.len());
        let before = kinds(&other);
        run(
            &mut other,
            "paste into the second drawing",
            json!({ "type": "paste_from_clipboard", "at": [20.0, 0.0], "text": text }),
        );
        compare("  second drawing, what came", &before, &kinds(&other));
        run(
            &mut s,
            "paste as block into this one",
            json!({ "type": "paste_from_clipboard", "at": p(9.0, 0.0), "text": text,
                    "as_block": true }),
        );
    }
    let door = other
        .drawing
        .entities_in(&other.sheet)
        .find_map(|e| match e {
            EntityType::Insert(i) if i.block_name == "DOOR" => Some(i.common.handle.value()),
            _ => None,
        });
    if let Some(door) = door {
        if let Some(Outcome::Clipboard { text, .. }) = run(
            &mut other,
            "copy a DOOR from the second",
            json!({ "type": "copy_to_clipboard", "targets": [target(door)] }),
        ) {
            run(
                &mut s,
                "paste it here",
                json!({ "type": "paste_from_clipboard", "at": p(9.0, -3.0), "text": text }),
            );
        }
    }
    drop(other);

    // Autosave and recover.
    let rows_now = linked(&s);
    let kinds_now = kinds(&s);
    let t = Instant::now();
    let saved = s.autosave().expect("autosave");
    let size = saved
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map_or(0, |m| m.len());
    println!(
        "{:30} {:9.1} ms  {:?} ({:.2} MB)",
        "autosave",
        ms(t),
        saved.as_ref().and_then(|p| p.file_name()),
        size as f64 / 1e6
    );
    println!(
        "  offered for recovery: {}",
        qiyas_core::recoverable(&dwg).is_some()
    );
    let t = Instant::now();
    match Session::open_autosave(&dwg) {
        Ok(r) => {
            println!(
                "{:30} {:9.1} ms  as {:?}, unsaved: {}",
                "recover",
                ms(t),
                r.drawing.path.file_name(),
                r.is_dirty()
            );
            compare("  recovered drawing", &kinds_now, &kinds(&r));
            println!("  rows (all, linked) {:?} -> {:?}", rows_now, linked(&r));
        }
        Err(e) => println!("{:30} {:9.1} ms  FAILED: {e}", "recover", ms(t)),
    }
    discard_autosave(&dwg).unwrap();

    // Save as DWG and DXF, reopen both.
    let wall_value = wall_row
        .as_ref()
        .and_then(|id| s.takeoff.items.iter().find(|m| &m.id == id))
        .map(|m| m.shown);
    for (name, label) in [
        ("check.dwg", "save as DWG"),
        ("check-dxf.dxf", "save as DXF"),
    ] {
        let path = work.join(name);
        let t = Instant::now();
        let result = s.save_as(&path);
        let took = ms(t);
        let size = std::fs::metadata(&path).map_or(0, |m| m.len());
        match result {
            Ok(()) => println!("{label:30} {took:9.1} ms  {:.2} MB", size as f64 / 1e6),
            Err(e) => {
                println!("{label:30} {took:9.1} ms  FAILED: {e}");
                continue;
            }
        }
        let t = Instant::now();
        let mut back = match Session::open(&path) {
            Ok(b) => b,
            Err(e) => {
                println!("  reopen FAILED: {e}");
                continue;
            }
        };
        println!(
            "{:30} {:9.1} ms  {} problems",
            format!("  reopen {name}"),
            ms(t),
            back.info().problems.len()
        );
        compare("  entities", &kinds_now, &kinds(&back));
        println!("  rows (all, linked) {:?} -> {:?}", rows_now, linked(&back));
        let again = measure(&mut back, "  measure the wall again", wall).and_then(|id| {
            back.takeoff
                .items
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.shown)
        });
        println!("  wall before save {wall_value:?}, after reopen {again:?}");
        // The attribute value, a table cell and the edited text read back.
        for find in ["W1", "Door", "(QIYAS)"] {
            let found = back.run(cmd(&json!({ "type": "find_text", "find": find,
                                               "scope": "drawing", "match_case": true })));
            let count = match found {
                Ok(Outcome::Found { count, .. }) => count,
                _ => 0,
            };
            println!("  text {find:?} found {count} time(s)");
        }
    }
    if let Some(dir) = keep {
        println!("kept in {}", dir.display());
    }
}

//! Dimensions, leaders and tolerances on screen: a dimension draws its `*D` block when it has one,
//! else the geometry Qiyas generates for it from its definition points and its style; every path
//! belongs to the dimension itself. Leaders draw their line and arrowhead, tolerances their frame.

use opencadcodec::entities::{
    Dimension, DimensionAligned, DimensionLinear, DimensionRadius, EntityType, Insert, Leader,
    Tolerance,
};
use opencadcodec::tables::{BlockRecord, DimStyle, TableEntry};
use opencadcodec::{CadDocument, Color, Layer, Line, Vector3};
use qiyas_geom::{
    dimension_entities, dimension_text, flatten, flatten_within, format_measure, set_dimscale,
    DimLook, FlatPath, Paint, Xform,
};

fn v(x: f64, y: f64) -> Vector3 {
    Vector3::new(x, y, 0.0)
}

/// A drawing whose Standard style has 3 decimals, no zero suppression and arrow and text sizes of
/// 0.2, on layer DIM (blue).
fn doc() -> CadDocument {
    let mut doc = CadDocument::new();
    let mut layer = Layer::new("DIM");
    layer.color = Color::Index(5);
    doc.layers.add(layer).unwrap();
    let style = doc.dim_styles.get_mut("Standard").unwrap();
    style.dimdec = 3;
    style.dimzin = 0;
    style.dimasz = 0.2;
    style.dimtxt = 0.2;
    style.dimexo = 0.1;
    style.dimexe = 0.15;
    style.dimgap = 0.05;
    style.dimscale = 1.0;
    doc
}

/// A linear dimension of (0,0)-(10,0) with its line at y = 2, on layer DIM, in `doc`.
fn linear(doc: &mut CadDocument, block: Option<&str>) -> (u64, EntityType) {
    let mut d = DimensionLinear::new(v(0.0, 0.0), v(10.0, 0.0));
    d.definition_point = v(10.0, 2.0);
    d.base.definition_point = v(10.0, 2.0);
    d.base.common.layer = "DIM".to_string();
    if let Some(name) = block {
        d.base.block_name = name.to_string();
    }
    let h = doc
        .add_entity(EntityType::Dimension(Dimension::Linear(d)))
        .unwrap();
    (h.value(), doc.get_entity(h).unwrap().clone())
}

fn outlines(paths: &[FlatPath]) -> Vec<&FlatPath> {
    paths
        .iter()
        .filter(|p| p.paint == Paint::Outline && p.text.is_none())
        .collect()
}

fn texts(paths: &[FlatPath]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| p.text.as_ref().map(|t| t.value.clone()))
        .collect()
}

#[test]
fn a_dimension_with_a_block_draws_its_block_as_itself() {
    let mut doc = doc();
    let mut record = BlockRecord::new("*D1");
    record.set_handle(doc.allocate_handle());
    record.flags.anonymous = true;
    let owner = record.handle;
    doc.block_records.add(record).unwrap();
    for mut e in [
        EntityType::Line(Line::from_points(v(0.0, 0.5), v(0.0, 2.5))),
        EntityType::Line(Line::from_points(v(10.0, 0.5), v(10.0, 2.5))),
        EntityType::Line(Line::from_points(v(0.0, 2.0), v(10.0, 2.0))),
    ] {
        e.common_mut().owner_handle = owner;
        e.common_mut().color = Color::ByBlock;
        doc.add_entity(e).unwrap();
    }
    let (h, dim) = linear(&mut doc, Some("*D1"));
    let out = flatten(&doc, &dim, &Xform::identity(), 1e-3);
    assert_eq!(
        out.paths.len(),
        3,
        "the block's three lines, nothing generated"
    );
    for p in &out.paths {
        assert_eq!(p.entity, h, "picked as the dimension");
        assert!(p.chain.is_empty(), "a dimension is no block reference");
        assert_eq!(p.layer, "DIM", "layer 0 inside takes the dimension's");
        assert_eq!(p.color, Color::Index(5), "ByLayer DIM, through ByBlock");
    }

    // Inside a block reference, the chain is the reference's.
    let mut holder = BlockRecord::new("HOLDER");
    holder.set_handle(doc.allocate_handle());
    let holder_handle = holder.handle;
    doc.block_records.add(holder).unwrap();
    let mut inner = dim.clone();
    inner.common_mut().handle = opencadcodec::Handle::NULL;
    inner.common_mut().owner_handle = holder_handle;
    let inner = doc.add_entity(inner).unwrap().value();
    let insert = Insert::new("HOLDER", v(100.0, 0.0));
    let ih = doc.add_entity(EntityType::Insert(insert)).unwrap();
    let insert = doc.get_entity(ih).unwrap().clone();
    let out = flatten(&doc, &insert, &Xform::identity(), 1e-3);
    assert_eq!(out.paths.len(), 3);
    assert!(out
        .paths
        .iter()
        .all(|p| p.entity == inner && p.chain == vec![ih.value()]));
    assert!((out.paths[2].path.points[0][0] - 100.0).abs() < 1e-9);
}

#[test]
fn a_dimension_without_a_block_is_generated_from_its_points_and_style() {
    let mut doc = doc();
    let (h, dim) = linear(&mut doc, None);
    let mut budget = 1000;
    let out = flatten_within(
        &doc,
        &Default::default(),
        &dim,
        &Xform::identity(),
        1e-3,
        &mut budget,
    );
    assert!(out.paths.iter().all(|p| p.entity == h && p.layer == "DIM"));
    assert_eq!(texts(&out.paths), vec!["10.000".to_string()]);
    let lines = outlines(&out.paths);
    // Two extension lines, the dimension line, two arrowheads (closed triangles).
    let open: Vec<_> = lines.iter().filter(|p| !p.path.closed).collect();
    let arrows: Vec<_> = lines.iter().filter(|p| p.path.closed).collect();
    assert_eq!(open.len(), 3, "{open:?}");
    assert_eq!(arrows.len(), 2);
    let first = &open[0].path.points;
    assert!(
        (first[0][1] - 0.1).abs() < 1e-9,
        "starts DIMEXO off the point"
    );
    assert!(
        (first[1][1] - 2.15).abs() < 1e-9,
        "ends DIMEXE past the line"
    );
    let fills = out
        .paths
        .iter()
        .filter(|p| matches!(p.paint, Paint::Fill(_)))
        .count();
    assert_eq!(fills, 2, "the arrowheads are filled");
}

#[test]
fn the_look_follows_the_style_and_its_scale() {
    let doc = doc();
    let style = doc.dim_styles.get("Standard").unwrap();
    let look = DimLook::new(style, 50.0);
    assert!((look.arrow - 10.0).abs() < 1e-9);
    assert!((look.text_height - 10.0).abs() < 1e-9);
    assert_eq!(format_measure(10.0, &look), "10.000");

    let mut s = DimStyle::new("S");
    s.dimdec = 2;
    s.dimzin = 8;
    s.dimdsep = b',' as i16;
    s.dimpost = "<> m".to_string();
    let look = DimLook::new(&s, 1.0);
    assert_eq!(
        format_measure(10.0, &look),
        "10 m",
        "trailing zeros suppressed"
    );
    assert_eq!(format_measure(2.456, &look), "2,46 m");
    s.dimzin = 4;
    s.dimpost = String::new();
    assert_eq!(format_measure(0.5, &DimLook::new(&s, 1.0)), ",50");
    s.dimlfac = 1000.0;
    s.dimzin = 0;
    assert_eq!(format_measure(0.5, &DimLook::new(&s, 1.0)), "500,00");
}

#[test]
fn text_overrides_and_kinds() {
    let doc = doc();
    let look = DimLook::new(doc.dim_styles.get("Standard").unwrap(), 1.0);
    let mut d = DimensionAligned::new(v(0.0, 0.0), v(3.0, 4.0));
    d.base.set_text_override(Some("<> TYP".to_string()));
    assert_eq!(
        dimension_text(&Dimension::Aligned(d.clone()), &look),
        "5.000 TYP"
    );
    d.base.set_text_override(Some("{\\C1;WALL}".to_string()));
    assert_eq!(dimension_text(&Dimension::Aligned(d), &look), "WALL");
    let r = DimensionRadius::new(v(0.0, 0.0), v(2.0, 0.0));
    assert_eq!(dimension_text(&Dimension::Radius(r), &look), "R2.000");
}

#[test]
fn a_dimension_scale_override_scales_the_generated_geometry() {
    let mut doc = doc();
    let (_, dim) = linear(&mut doc, None);
    let EntityType::Dimension(mut dim) = dim else {
        unreachable!()
    };
    let style = doc.dim_styles.get("Standard").unwrap().clone();
    let small = dimension_entities(&doc, &dim);
    set_dimscale(&mut dim.base_mut().common, 50.0);
    let look = DimLook::of(&doc, &dim);
    assert!((look.text_height - style.dimtxt * 50.0).abs() < 1e-9);
    let big = dimension_entities(&doc, &dim);
    let height = |g: &qiyas_geom::DimGeometry| {
        g.entities
            .iter()
            .find_map(|e| match e {
                EntityType::Text(t) => Some(t.height),
                _ => None,
            })
            .unwrap()
    };
    assert!((height(&big) - height(&small) * 50.0).abs() < 1e-9);
    assert_eq!(big.entities.len(), small.entities.len());
}

#[test]
fn leaders_draw_their_line_and_arrow_and_tolerances_their_frame() {
    let mut doc = doc();
    let mut leader = Leader::new();
    leader.vertices = vec![v(0.0, 0.0), v(2.0, 2.0), v(4.0, 2.0)];
    leader.common.layer = "DIM".to_string();
    let lh = doc.add_entity(EntityType::Leader(leader)).unwrap();
    let leader = doc.get_entity(lh).unwrap().clone();
    let mut budget = 1000;
    let out = flatten_within(
        &doc,
        &Default::default(),
        &leader,
        &Xform::identity(),
        1e-3,
        &mut budget,
    );
    assert!(out.paths.iter().all(|p| p.entity == lh.value()));
    let lines = outlines(&out.paths);
    assert!(lines.iter().any(|p| p.path.points.len() == 3), "{lines:?}");
    assert!(lines.iter().any(|p| p.path.closed), "an arrowhead");
    assert!(out.paths.iter().any(|p| matches!(p.paint, Paint::Fill(_))));

    let tol = Tolerance::with_text(v(0.0, 0.0), "{\\Fgdt;p}%%v0.5%%vA");
    let th = doc.add_entity(EntityType::Tolerance(tol)).unwrap();
    let tol = doc.get_entity(th).unwrap().clone();
    let out = flatten(&doc, &tol, &Xform::identity(), 1e-3);
    assert_eq!(texts(&out.paths), vec!["p", "0.5", "A"]);
    let frames = out
        .paths
        .iter()
        .filter(|p| p.text.is_none() && p.path.closed)
        .count();
    assert_eq!(frames, 3, "a box per field");
}

#[test]
fn dimtad_4_puts_the_text_below_the_line() {
    let mut doc = doc();
    doc.dim_styles.get_mut("Standard").unwrap().dimtad = 4;
    let (_, dim) = linear(&mut doc, None);
    let EntityType::Dimension(dim) = dim else {
        unreachable!()
    };
    let at = dimension_entities(&doc, &dim).text_at.unwrap();
    // The line is at y 2: the text's middle is a gap and half a height below it.
    assert!((at[1] - (2.0 - 0.05 - 0.1)).abs() < 1e-9, "{at:?}");
}

#[test]
fn a_dimension_own_dimpost_overrides_its_styles_and_keeps_the_other_overrides() {
    use qiyas_geom::{clear_dimpost, dimpost, dimscale, dimtxt, set_dimpost, set_dimtxt};

    let mut doc = doc();
    let (_, dim) = linear(&mut doc, None);
    let EntityType::Dimension(mut d) = dim else {
        panic!("a dimension")
    };
    assert_eq!(dimpost(&d.base().common), None);
    set_dimscale(&mut d.base_mut().common, 50.0);
    set_dimtxt(&mut d.base_mut().common, 2.5);
    set_dimpost(&mut d.base_mut().common, "<> m");
    let common = &d.base().common;
    assert_eq!(dimpost(common).as_deref(), Some("<> m"));
    assert_eq!(dimscale(common), Some(50.0));
    assert_eq!(dimtxt(common), Some(2.5));

    // The look takes it over the style's own (empty) one.
    let look = DimLook::named(&doc, "Standard", common);
    assert_eq!(look.post, "<> m");
    assert_eq!(format_measure(10.0, &look), "10.000 m");

    // Setting it again replaces it; another override set later keeps it.
    set_dimpost(&mut d.base_mut().common, "<> cm");
    set_dimtxt(&mut d.base_mut().common, 3.0);
    assert_eq!(dimpost(&d.base().common).as_deref(), Some("<> cm"));
    // An empty suffix is an override too: no suffix, whatever the style says.
    set_dimpost(&mut d.base_mut().common, "");
    assert_eq!(dimpost(&d.base().common).as_deref(), Some(""));
    let look = DimLook::named(&doc, "Standard", &d.base().common);
    assert_eq!(look.post, "");

    // Cleared, the style's applies and the others stay; with nothing left the record goes.
    clear_dimpost(&mut d.base_mut().common);
    assert_eq!(dimpost(&d.base().common), None);
    assert_eq!(dimscale(&d.base().common), Some(50.0));
    assert_eq!(dimtxt(&d.base().common), Some(3.0));
    let mut bare = d.base().common.clone();
    bare.extended_data.clear();
    set_dimpost(&mut bare, "<> m");
    clear_dimpost(&mut bare);
    assert!(bare.extended_data.is_empty());
}

#[test]
fn dstyle_overrides_are_read_as_code_value_pairs() {
    use opencadcodec::xdata::{ExtendedDataRecord, XDataValue};
    use qiyas_geom::{clear_dimpost, dimpost, dimscale, dimtxt, set_dimpost};

    // As AutoCAD writes them: DIMTAD (271) first, whose value 3 is the code of DIMPOST.
    let mut doc = doc();
    let (_, dim) = linear(&mut doc, None);
    let EntityType::Dimension(mut d) = dim else {
        panic!("a dimension")
    };
    let mut record = ExtendedDataRecord::new("ACAD");
    for v in [
        XDataValue::String("DSTYLE".to_string()),
        XDataValue::ControlString("{".to_string()),
        XDataValue::Integer16(271),
        XDataValue::Integer16(3),
        XDataValue::Integer16(3),
        XDataValue::String("<> ft".to_string()),
        XDataValue::Integer16(40),
        XDataValue::Real(50.0),
        XDataValue::Integer16(140),
        XDataValue::Real(2.5),
        XDataValue::ControlString("}".to_string()),
    ] {
        record.add_value(v);
    }
    d.base_mut().common.extended_data.upsert_record(record);
    let common = &d.base().common;
    assert_eq!(dimpost(common).as_deref(), Some("<> ft"));
    assert_eq!(dimscale(common), Some(50.0));
    assert_eq!(dimtxt(common), Some(2.5));
    // An override with no DIMPOST among them has none, whatever a value looks like.
    let mut other = d.base().common.clone();
    clear_dimpost(&mut other);
    assert_eq!(dimpost(&other), None);
    assert_eq!(dimscale(&other), Some(50.0));
    // Setting it keeps DIMTAD where it was.
    set_dimpost(&mut other, "<> m");
    assert_eq!(dimpost(&other).as_deref(), Some("<> m"));
    assert_eq!(dimtxt(&other), Some(2.5));
}

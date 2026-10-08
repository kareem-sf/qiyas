//! Measurements written into the drawing itself: one entity per measured shape on a `QIYAS-*`
//! layer, carrying the measurement in xdata, so the take-off survives a trip through another CAD
//! program and can be rebuilt when the project file is missing.
//!
//! Xdata record of app `QIYAS`, in order: `String(id)`, `String(kind)` ("length", "area" or
//! "count"), `Real(value)` in drawing units, `String(description)`, `String(layer)`,
//! `String(sheet)`, `String(block_name or "")`. A count writes one point per counted reference,
//! each with the whole count's record.

use std::collections::HashSet;

use opencadcodec::xdata::{ExtendedDataRecord, XDataValue};
use opencadcodec::{
    AppId, CadDocument, Color, EntityType, Handle, Layer, LwPolyline, Point, Vector2,
};
use qiyas_geom::{flatten, Path, Xform};

use crate::session::MEASURE_TOL;
use crate::{Kind, Measurement, Units};

const APP: &str = "QIYAS";

/// The layer and its colour index for each kind.
fn layer_of(kind: Kind) -> (&'static str, u8) {
    match kind {
        Kind::Length => ("QIYAS-LENGTH", 6),
        Kind::Area => ("QIYAS-AREA", 5),
        Kind::Count => ("QIYAS-COUNT", 4),
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Length => "length",
        Kind::Area => "area",
        Kind::Count => "count",
    }
}

/// Replaces the measurement entities in `doc` with `items`: removes every entity carrying `QIYAS`
/// xdata, then adds one polyline per length or area and one point per counted reference, on the
/// measurement's sheet. A count reference that is not a block reference is skipped.
pub fn embed_measurements(doc: &mut CadDocument, items: &[Measurement]) {
    // `remove_entity` leaves the handle in its block's list; readers and writers skip such handles.
    let old: Vec<Handle> = doc
        .entities()
        .filter(|e| e.common().extended_data.get_record(APP).is_some())
        .map(|e| e.common().handle)
        .collect();
    for handle in old {
        doc.remove_entity(handle);
    }

    if !doc.app_ids.contains(APP) {
        let mut app = AppId::new(APP);
        app.handle = doc.allocate_handle();
        doc.app_ids.add(app).expect("checked it is missing");
    }
    for kind in [Kind::Length, Kind::Area, Kind::Count] {
        let (name, index) = layer_of(kind);
        if !doc.layers.contains(name) {
            let mut layer = Layer::new(name);
            layer.handle = doc.allocate_handle();
            layer.color = Color::Index(index);
            doc.layers.add(layer).expect("checked it is missing");
        }
    }

    // Shapes are worked out first: they read the document that is then added to.
    let mut shapes: Vec<(EntityType, &Measurement)> = Vec::new();
    for m in items {
        match m.kind {
            Kind::Length | Kind::Area => {
                if let Some(points) = shape_points(doc, m) {
                    let mut line = LwPolyline::from_points(
                        points.iter().map(|p| Vector2::new(p[0], p[1])).collect(),
                    );
                    line.is_closed = m.kind == Kind::Area;
                    shapes.push((EntityType::LwPolyline(line), m));
                }
            }
            Kind::Count => {
                for handle in &m.handles {
                    if let Some(EntityType::Insert(insert)) = doc.get_entity(Handle::new(*handle)) {
                        let mut point = Point::new();
                        point.location = insert.insert_point;
                        shapes.push((EntityType::Point(point), m));
                    }
                }
            }
        }
    }
    for (mut entity, m) in shapes {
        let owner = doc.block_records.get(&m.sheet).map(|b| b.handle);
        let common = entity.common_mut();
        common.layer = layer_of(m.kind).0.to_string();
        if let Some(owner) = owner {
            common.owner_handle = owner;
        }
        common.extended_data.add_record(record(m));
        doc.add_entity(entity)
            .expect("a new entity is always added");
    }
}

/// The picked points, or for a measured entity the vertices of its first path (first closed
/// path for an area). `None` when there is nothing to draw.
fn shape_points(doc: &CadDocument, m: &Measurement) -> Option<Vec<[f64; 2]>> {
    if !m.points.is_empty() {
        return Some(m.points.clone());
    }
    let entity = doc.get_entity(Handle::new(*m.handles.first()?))?;
    let mut paths = Vec::new();
    flatten(doc, entity, &Xform::identity(), MEASURE_TOL, &mut paths);
    paths
        .into_iter()
        .find(|p| m.kind == Kind::Length || p.closed)
        .map(|p| p.points)
}

fn record(m: &Measurement) -> ExtendedDataRecord {
    let mut record = ExtendedDataRecord::new(APP);
    for value in [
        XDataValue::String(m.id.clone()),
        XDataValue::String(kind_name(m.kind).to_string()),
        XDataValue::Real(m.value),
        XDataValue::String(m.description.clone()),
        XDataValue::String(m.layer.clone()),
        XDataValue::String(m.sheet.clone()),
        XDataValue::String(m.block_name.clone().unwrap_or_default()),
    ] {
        record.add_value(value);
    }
    record
}

/// Rebuilds the take-off from the entities carrying `QIYAS` xdata, in the order they were written
/// (by handle). Points come from the polyline vertices; the points of one count become one
/// measurement with the stored value. Records that don't match the layout are skipped.
pub fn read_embedded(doc: &CadDocument) -> Vec<Measurement> {
    let units = Units::from_header(doc.header.insertion_units);
    let mut entities: Vec<(&EntityType, &ExtendedDataRecord)> = doc
        .entities()
        .filter_map(|e| Some((e, e.common().extended_data.get_record(APP)?)))
        .collect();
    entities.sort_by_key(|(e, _)| e.common().handle.value());

    let mut items: Vec<Measurement> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (entity, record) in entities {
        let Some(mut m) = parse(record, units) else {
            continue;
        };
        if !seen.insert(m.id.clone()) {
            continue;
        }
        if let (Kind::Length | Kind::Area, EntityType::LwPolyline(line)) = (m.kind, entity) {
            m.points = line
                .vertices
                .iter()
                .map(|v| [v.location.x, v.location.y])
                .collect();
            m.self_intersecting = m.kind == Kind::Area
                && Path {
                    points: m.points.clone(),
                    closed: true,
                }
                .self_intersects();
        }
        items.push(m);
    }
    items
}

/// A measurement from one record, made now by the user, with no points or handles yet.
fn parse(record: &ExtendedDataRecord, units: Units) -> Option<Measurement> {
    let [XDataValue::String(id), XDataValue::String(kind), XDataValue::Real(value), XDataValue::String(description), XDataValue::String(layer), XDataValue::String(sheet), XDataValue::String(block_name)] =
        record.values.as_slice()
    else {
        return None;
    };
    let kind = match kind.as_str() {
        "length" => Kind::Length,
        "area" => Kind::Area,
        "count" => Kind::Count,
        _ => return None,
    };
    let mut m = Measurement::new(
        kind,
        *value,
        units,
        description.clone(),
        layer.clone(),
        sheet.clone(),
    );
    m.id = id.clone();
    m.block_name = (!block_name.is_empty()).then(|| block_name.clone());
    Some(m)
}

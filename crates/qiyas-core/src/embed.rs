//! Measurements written into the drawing itself: one entity per measured shape on a `QIYAS-*`
//! layer, carrying the measurement in xdata, so the take-off survives a trip through another CAD
//! program and can be rebuilt when the project file is missing.
//!
//! Xdata record of app `QIYAS`, in order: `String(id)`, `String(kind)` ("length", "area" or
//! "count"), `Real(value)` in drawing units, `String(description)`, `String(layer)`,
//! `String(sheet)`, `String(block_name or "")`. A count writes one point per counted reference,
//! each with the whole count's record.

use std::collections::HashMap;

use opencadcodec::xdata::{ExtendedDataRecord, XDataValue};
use opencadcodec::{
    AppId, CadDocument, Color, EntityType, Handle, Layer, LwPolyline, Point, Vector2, Vector3,
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
/// xdata, then adds one polyline per path of a length or area and one point per counted reference
/// (or per stored point, for a count rebuilt from the drawing), on the measurement's sheet. A count
/// reference that is not a block reference is skipped.
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
                for points in shape_paths(doc, m) {
                    let mut line = LwPolyline::from_points(
                        points.iter().map(|p| Vector2::new(p[0], p[1])).collect(),
                    );
                    line.is_closed = m.kind == Kind::Area;
                    shapes.push((EntityType::LwPolyline(line), m));
                }
            }
            Kind::Count => {
                let locations: Vec<Vector3> = if m.handles.is_empty() {
                    m.points
                        .iter()
                        .map(|p| Vector3::new(p[0], p[1], 0.0))
                        .collect()
                } else {
                    m.handles
                        .iter()
                        .filter_map(|h| match doc.get_entity(Handle::new(*h)) {
                            Some(EntityType::Insert(insert)) => Some(insert.insert_point),
                            _ => None,
                        })
                        .collect()
                };
                for location in locations {
                    let mut point = Point::new();
                    point.location = location;
                    shapes.push((EntityType::Point(point), m));
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

/// The picked points as one path, or for a measured entity the vertices of each of its paths
/// (each closed path, for an area). Empty when there is nothing to draw.
fn shape_paths(doc: &CadDocument, m: &Measurement) -> Vec<Vec<[f64; 2]>> {
    if !m.points.is_empty() {
        return vec![m.points.clone()];
    }
    let Some(entity) = m
        .handles
        .first()
        .and_then(|h| doc.get_entity(Handle::new(*h)))
    else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    flatten(doc, entity, &Xform::identity(), MEASURE_TOL, &mut paths);
    paths
        .into_iter()
        .filter(|p| m.kind == Kind::Length || p.closed)
        .map(|p| p.points)
        .collect()
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
/// (by handle). Entities with the same id merge into one measurement with the stored value: a
/// length or area takes its points from its first polyline, a count gets one point per `Point`.
/// A measured entity with several paths therefore comes back with the first path only, and is
/// written back as that one polyline on the next save. Records that don't match the layout are
/// skipped.
pub fn read_embedded(doc: &CadDocument) -> Vec<Measurement> {
    let units = Units::from_header(doc.header.insertion_units);
    let mut entities: Vec<(&EntityType, &ExtendedDataRecord)> = doc
        .entities()
        .filter_map(|e| Some((e, e.common().extended_data.get_record(APP)?)))
        .collect();
    entities.sort_by_key(|(e, _)| e.common().handle.value());

    let mut items: Vec<Measurement> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (entity, record) in entities {
        let Some(m) = parse(record, units) else {
            continue;
        };
        let i = *index.entry(m.id.clone()).or_insert_with(|| {
            items.push(m);
            items.len() - 1
        });
        let m = &mut items[i];
        match entity {
            EntityType::Point(p) if m.kind == Kind::Count => {
                m.points.push([p.location.x, p.location.y]);
            }
            EntityType::LwPolyline(line) if m.kind != Kind::Count && m.points.is_empty() => {
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
            _ => {}
        }
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

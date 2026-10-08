//! A small synthetic drawing for tests. No customer drawing is ever committed.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};

use opencadcodec::entities::Insert;
use opencadcodec::{
    Arc, BlockRecord, CadDocument, Color, DwgWriter, DxfWriter, EntityType, Layer, Line,
    LwPolyline, Text, Vector2, Vector3,
};

/// Layers A-WALL (red) and A-DOOR (green); a line, a closed rectangle and an arc on A-WALL; three DOOR
/// references on A-DOOR (plain, rotated a quarter turn, scaled 2); a title text. Units are metres.
pub fn sample_doc() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = 6;
    for (name, index) in [("A-WALL", 1), ("A-DOOR", 3)] {
        let mut layer = Layer::new(name);
        layer.handle = doc.allocate_handle();
        layer.color = Color::Index(index);
        doc.layers.add(layer).unwrap();
    }

    let mut door = BlockRecord::new("DOOR");
    door.handle = doc.allocate_handle();
    door.block_entity_handle = doc.allocate_handle();
    door.block_end_handle = doc.allocate_handle();
    let owner = door.handle;
    doc.block_records.add(door).unwrap();
    let mut leaf = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 0.9, 0.0, 0.0));
    leaf.common_mut().owner_handle = owner;
    doc.add_entity(leaf).unwrap();

    let mut rectangle = LwPolyline::new();
    for (x, y) in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)] {
        rectangle.add_point(Vector2::new(x, y));
    }
    rectangle.is_closed = true;
    let walls = [
        EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 10.0, 0.0, 0.0)),
        EntityType::LwPolyline(rectangle),
        EntityType::Arc(Arc::from_center_radius_angles(
            Vector3::new(20.0, 0.0, 0.0),
            2.0,
            0.0,
            PI,
        )),
    ];
    let doors = [
        Insert::new("DOOR", Vector3::new(1.0, 1.0, 0.0)),
        Insert::new("DOOR", Vector3::new(5.0, 1.0, 0.0)).with_rotation(PI / 2.0),
        Insert::new("DOOR", Vector3::new(9.0, 1.0, 0.0)).with_scale(2.0, 2.0, 2.0),
    ];
    for (layer, entity) in walls
        .into_iter()
        .map(|e| ("A-WALL", e))
        .chain(doors.into_iter().map(|i| ("A-DOOR", EntityType::Insert(i))))
    {
        let mut entity = entity;
        entity.common_mut().layer = layer.to_string();
        doc.add_entity(entity).unwrap();
    }
    doc.add_entity(EntityType::Text(
        Text::with_value("GROUND FLOOR PLAN", Vector3::new(0.0, -2.0, 0.0)).with_height(0.5),
    ))
    .unwrap();
    doc
}

/// Writes the sample as `sample.dxf` or `sample.dwg` in `dir`.
pub fn write_sample(dir: &Path, ext: &str) -> PathBuf {
    let path = dir.join(format!("sample.{ext}"));
    let doc = sample_doc();
    if ext.eq_ignore_ascii_case("dxf") {
        DxfWriter::new(&doc).write_to_file(&path).unwrap();
    } else {
        DwgWriter::write_to_file(&path, &doc).unwrap();
    }
    path
}

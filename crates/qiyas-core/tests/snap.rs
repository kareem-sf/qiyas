//! Object snaps on the sample drawing, and the drafting settings in the project file.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

use opencadcodec::entities::Insert;
use opencadcodec::{Arc, BlockRecord, DxfWriter, EntityType, Line, Point, Vector3};
use qiyas_core::{Command, Drafting, Session, SnapKind, SnapModes};

/// The sample (a 10 m line along y = 0, a 4 x 3 rectangle from the origin, an arc of radius 2
/// about (20, 0) from 0 to 180 degrees, three DOOR references) plus a diagonal line from (1, -1)
/// to (3, 4), crossing the rectangle's top edge at (2.6, 3), and a point entity at (30, 5).
fn session() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = qiyas_core::fixtures::sample_doc();
    let mut diagonal = EntityType::Line(Line::from_coords(1.0, -1.0, 0.0, 3.0, 4.0, 0.0));
    diagonal.common_mut().layer = "A-WALL".to_string();
    doc.add_entity(diagonal).unwrap();
    doc.add_entity(EntityType::Point(Point::from_coords(30.0, 5.0, 0.0)))
        .unwrap();
    let path = dir.path().join("snaps.dxf");
    DxfWriter::new(&doc).write_to_file(&path).unwrap();
    let session = Session::open(&path).unwrap();
    (dir, session)
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9
}

fn snap(s: &Session, p: [f64; 2], tol: f64, modes: &SnapModes) -> (SnapKind, [f64; 2]) {
    let found = s.snap(p, tol, modes, None).expect("a snap");
    (found.kind, found.point)
}

#[test]
fn the_rectangle_corner_is_an_end() {
    let (_dir, s) = session();
    let (kind, point) = snap(&s, [4.03, 2.97], 0.05, &SnapModes::default());
    assert_eq!(kind, SnapKind::End);
    assert!(near(point, [4.0, 3.0]), "{point:?}");
}

#[test]
fn the_middle_of_the_line() {
    let (_dir, s) = session();
    let (kind, point) = snap(&s, [5.02, 0.01], 0.05, &SnapModes::default());
    assert_eq!(kind, SnapKind::Mid);
    assert!(near(point, [5.0, 0.0]), "{point:?}");
}

#[test]
fn where_the_diagonal_crosses_the_rectangle() {
    let (_dir, s) = session();
    let (kind, point) = snap(&s, [2.62, 3.01], 0.05, &SnapModes::default());
    assert_eq!(kind, SnapKind::Intersection);
    assert!(near(point, [2.6, 3.0]), "{point:?}");
}

#[test]
fn on_the_arc_its_centre() {
    let (_dir, s) = session();
    // On the arc at 45 degrees, far from its ends and its middle.
    let r = 2.0 / 2f64.sqrt();
    let (kind, point) = snap(&s, [20.0 + r + 0.01, r + 0.01], 0.05, &SnapModes::default());
    assert_eq!(kind, SnapKind::Centre);
    assert!(near(point, [20.0, 0.0]), "{point:?}");
}

#[test]
fn an_end_beats_a_nearer_middle() {
    let (_dir, s) = session();
    // The arc's middle (20, 2) is 0.5 away; its end (22, 0) about 2.5.
    let (kind, point) = snap(&s, [20.5, 1.95], 2.6, &SnapModes::default());
    assert_eq!(kind, SnapKind::End);
    assert!(near(point, [22.0, 0.0]), "{point:?}");
}

#[test]
fn nearest_only_when_it_is_on() {
    let (_dir, s) = session();
    let p = [7.3, 0.01];
    assert_eq!(s.snap(p, 0.05, &SnapModes::default(), None), None);
    let modes = SnapModes {
        nearest: true,
        ..SnapModes::default()
    };
    let (kind, point) = snap(&s, p, 0.05, &modes);
    assert_eq!(kind, SnapKind::Nearest);
    assert!(near(point, [7.3, 0.0]), "{point:?}");
}

#[test]
fn perpendicular_from_the_previous_point() {
    let (_dir, s) = session();
    let only = SnapModes {
        end: false,
        mid: false,
        centre: false,
        node: false,
        intersection: false,
        nearest: false,
        perpendicular: true,
    };
    let found = s.snap([5.02, 0.01], 0.05, &only, Some([5.0, 5.0])).unwrap();
    assert_eq!(found.kind, SnapKind::Perpendicular);
    assert!(near(found.point, [5.0, 0.0]), "{:?}", found.point);
    // Without a previous point there is no perpendicular.
    assert_eq!(s.snap([5.02, 0.01], 0.05, &only, None), None);
}

#[test]
fn a_point_entity_is_a_node() {
    let (_dir, s) = session();
    let (kind, point) = snap(&s, [30.01, 5.01], 0.05, &SnapModes::default());
    assert_eq!(kind, SnapKind::Node);
    assert!(near(point, [30.0, 5.0]), "{point:?}");
}

#[test]
fn an_end_inside_a_scaled_block_names_its_chain() {
    let (_dir, s) = session();
    // The third DOOR is at (9, 1), scaled 2: its leaf runs to (10.8, 1).
    let found = s
        .snap([10.79, 1.02], 0.05, &SnapModes::default(), None)
        .unwrap();
    assert_eq!(found.kind, SnapKind::End);
    assert!(near(found.point, [10.8, 1.0]), "{:?}", found.point);
    assert_eq!(found.chain.len(), 1);
}

#[test]
fn nothing_near_is_no_snap() {
    let (_dir, s) = session();
    assert_eq!(
        s.snap([50.0, 50.0], 0.05, &SnapModes::default(), None),
        None
    );
}

#[test]
fn drafting_settings_are_kept_in_the_project_file() {
    let (dir, mut s) = session();
    let drafting = Drafting {
        ortho: true,
        grid_spacing: 0.5,
        precision: 2,
        ..Drafting::default()
    };
    s.run(Command::SetDrafting {
        drafting: drafting.clone(),
    })
    .unwrap();
    // Not an undo step.
    assert!(!s.can_undo());
    s.save_project().unwrap();
    let again = Session::open(&dir.path().join("snaps.dxf")).unwrap();
    assert_eq!(again.drafting, drafting);
}

#[test]
fn drafting_settings_out_of_range_are_refused() {
    let (_dir, mut s) = session();
    for bad in [
        Drafting {
            grid_spacing: 0.0,
            ..Drafting::default()
        },
        Drafting {
            polar_step_deg: 0.0,
            ..Drafting::default()
        },
        Drafting {
            precision: 7,
            ..Drafting::default()
        },
    ] {
        assert!(s.run(Command::SetDrafting { drafting: bad }).is_err());
    }
    assert_eq!(s.drafting, Drafting::default());
}

#[test]
fn of_two_ends_in_reach_the_nearer_wins() {
    let (_dir, s) = session();
    // The rectangle's corner (0, 0) and the first door leaf's end (1, 1) are both in reach;
    // whichever is nearer wins, from either side.
    let (kind, point) = snap(&s, [0.6, 0.6], 1.2, &SnapModes::default());
    assert_eq!(kind, SnapKind::End);
    assert!(near(point, [1.0, 1.0]), "{point:?}");
    let (kind, point) = snap(&s, [0.3, 0.3], 1.2, &SnapModes::default());
    assert_eq!(kind, SnapKind::End);
    assert!(near(point, [0.0, 0.0]), "{point:?}");
}

/// A block ARCB holding a quarter arc about (1, 0), radius 1, from 0 to 90 degrees, inserted at
/// (50, 50) turned 45 degrees and scaled 2. In the world its centre is (50 + sqrt 2, 50 + sqrt 2),
/// its radius 2, and it runs from 45 to 135 degrees.
fn rotated_block_session() -> (tempfile::TempDir, Session, [f64; 2]) {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = qiyas_core::fixtures::sample_doc();
    let mut record = BlockRecord::new("ARCB");
    record.handle = doc.allocate_handle();
    let owner = record.handle;
    doc.block_records.add(record).unwrap();
    let mut arc = EntityType::Arc(Arc::from_center_radius_angles(
        Vector3::new(1.0, 0.0, 0.0),
        1.0,
        0.0,
        FRAC_PI_2,
    ));
    arc.common_mut().owner_handle = owner;
    doc.add_entity(arc).unwrap();
    let insert = Insert::new("ARCB", Vector3::new(50.0, 50.0, 0.0))
        .with_rotation(FRAC_PI_4)
        .with_scale(2.0, 2.0, 2.0);
    doc.add_entity(EntityType::Insert(insert)).unwrap();
    let path = dir.path().join("arcb.dxf");
    DxfWriter::new(&doc).write_to_file(&path).unwrap();
    let session = Session::open(&path).unwrap();
    let c = 50.0 + 2f64.sqrt();
    (dir, session, [c, c])
}

#[test]
fn an_arc_in_a_turned_and_scaled_block_snaps_where_it_is_drawn() {
    let (_dir, s, centre) = rotated_block_session();
    let at = |deg: f64, r: f64| {
        let a = deg.to_radians();
        [centre[0] + r * a.cos(), centre[1] + r * a.sin()]
    };
    // On the arc at 70 degrees, away from its ends (45, 135) and middle (90): its centre.
    let found = s
        .snap(at(70.0, 2.01), 0.05, &SnapModes::default(), None)
        .unwrap();
    assert_eq!(found.kind, SnapKind::Centre);
    assert!(near(found.point, centre), "{:?}", found.point);
    assert_eq!(found.chain.len(), 1);
    // Near the top: its middle, at 90 degrees.
    let found = s
        .snap(at(90.5, 2.02), 0.05, &SnapModes::default(), None)
        .unwrap();
    assert_eq!(found.kind, SnapKind::Mid);
    assert!(near(found.point, at(90.0, 2.0)), "{:?}", found.point);
    // And an end, at 45 degrees.
    let found = s
        .snap(at(45.5, 2.01), 0.05, &SnapModes::default(), None)
        .unwrap();
    assert_eq!(found.kind, SnapKind::End);
    assert!(near(found.point, at(45.0, 2.0)), "{:?}", found.point);
}

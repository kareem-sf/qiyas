//! Test PDFs, written with printpdf.

use std::path::{Path, PathBuf};

use printpdf::{
    BuiltinFont, Color, Line, LinePoint, Mm, Op, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Pt, Rgb, TextItem,
};

/// The test page: 200 x 100 points.
pub const PAGE_PT: (f64, f64) = (200.0, 100.0);
/// The stroked rectangle's corners on the page, in points: 100 x 50, red, 1 pt wide.
pub const RECT_PT: [[f64; 2]; 2] = [[20.0, 20.0], [120.0, 70.0]];
/// Where "WALL" starts (its baseline's left end), in points, and its font size.
pub const WALL_AT_PT: [f64; 2] = [30.0, 80.0];
pub const WALL_SIZE_PT: f64 = 12.0;

fn page(ops: Vec<Op>) -> PdfPage {
    let mm = |pt: f64| Mm::from(Pt(pt as f32));
    PdfPage::new(mm(PAGE_PT.0), mm(PAGE_PT.1), ops)
}

fn point(x: f64, y: f64) -> LinePoint {
    LinePoint {
        p: Point {
            x: Pt(x as f32),
            y: Pt(y as f32),
        },
        bezier: false,
    }
}

fn save(doc: &mut PdfDocument, pages: Vec<PdfPage>, path: &Path) -> PathBuf {
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write(path, bytes).expect("write the test PDF");
    path.to_path_buf()
}

/// One page: the red rectangle `RECT_PT` and the text "WALL" at `WALL_AT_PT`, in `dir`.
pub fn write_wall(dir: &Path) -> PathBuf {
    let [[x0, y0], [x1, y1]] = RECT_PT;
    let ops = vec![
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::SetOutlineThickness { pt: Pt(1.0) },
        Op::DrawLine {
            line: Line {
                points: vec![point(x0, y0), point(x1, y0), point(x1, y1), point(x0, y1)],
                is_closed: true,
            },
        },
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point {
                x: Pt(WALL_AT_PT[0] as f32),
                y: Pt(WALL_AT_PT[1] as f32),
            },
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(WALL_SIZE_PT as f32),
        },
        Op::ShowText {
            items: vec![TextItem::Text("WALL".to_string())],
        },
        Op::EndTextSection,
    ];
    save(
        &mut PdfDocument::new("wall"),
        vec![page(ops)],
        &dir.join("wall.pdf"),
    )
}

/// `count` pages, each with one short line whose length is its page number in points (so pages
/// tell apart), in `dir`.
pub fn write_many(dir: &Path, count: u32) -> PathBuf {
    let pages = (1..=count)
        .map(|n| {
            page(vec![Op::DrawLine {
                line: Line {
                    points: vec![point(10.0, 10.0), point(10.0 + f64::from(n % 150), 10.0)],
                    is_closed: false,
                },
            }])
        })
        .collect();
    save(&mut PdfDocument::new("many"), pages, &dir.join("many.pdf"))
}

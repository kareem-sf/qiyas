//! Reads a plotted PDF back for the tests: the page size, every painted path with the state it
//! was painted in, and every glyph shown, all in page points.

#![allow(dead_code)]

use std::path::Path;

use lopdf::content::Content;
use lopdf::{Document, Object};

pub const PT_PER_MM: f32 = 72.0 / 25.4;

#[derive(Debug, Clone)]
pub struct Paint {
    /// The painting operator: `S`, `f`, `B`, ...
    pub op: String,
    pub stroke: [f32; 3],
    pub fill: [f32; 3],
    pub width: f32,
    /// Corners of each subpath in page points (curves keep their end points only).
    pub subpaths: Vec<Vec<[f32; 2]>>,
    /// Painted inside a clip.
    pub clipped: bool,
}

#[derive(Debug, Clone)]
pub struct Glyph {
    /// The font resource name, e.g. `F0`.
    pub font: String,
    pub code: Vec<u8>,
    pub tm: [f32; 6],
    pub fill: [f32; 3],
}

pub struct Page {
    pub doc: Document,
    pub page: lopdf::ObjectId,
    pub size: [f32; 2],
    pub paints: Vec<Paint>,
    pub glyphs: Vec<Glyph>,
    /// Images painted (`Do`).
    pub images: usize,
    /// Every operator, in order.
    pub ops: Vec<String>,
}

#[derive(Clone)]
struct State {
    ctm: [f32; 6],
    stroke: [f32; 3],
    fill: [f32; 3],
    width: f32,
    clipped: bool,
}

fn num(o: &Object) -> f32 {
    o.as_float()
        .unwrap_or_else(|_| panic!("a number, not {o:?}"))
}

fn apply(m: &[f32; 6], x: f32, y: f32) -> [f32; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

/// `m` then `n`.
fn concat(m: &[f32; 6], n: &[f32; 6]) -> [f32; 6] {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

pub fn read(path: &Path) -> Page {
    let doc = Document::load(path).expect("a PDF lopdf reads");
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 1, "one page");
    let page = *pages.values().next().unwrap();
    let dict = doc.get_dictionary(page).unwrap();
    let mb = dict.get(b"MediaBox").unwrap().as_array().unwrap();
    let size = [num(&mb[2]) - num(&mb[0]), num(&mb[3]) - num(&mb[1])];
    let content = Content::decode(&doc.get_page_content(page)).expect("a content stream");
    let mut st = State {
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        stroke: [0.0; 3],
        fill: [0.0; 3],
        width: 1.0,
        clipped: false,
    };
    let mut stack: Vec<State> = Vec::new();
    let mut path: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut clip_next = false;
    let mut tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut font = String::new();
    let mut paints = Vec::new();
    let mut glyphs = Vec::new();
    let mut images = 0;
    let mut ops = Vec::new();
    for op in &content.operations {
        let o = &op.operands;
        ops.push(op.operator.clone());
        match op.operator.as_str() {
            "q" => stack.push(st.clone()),
            "Q" => st = stack.pop().expect("q before Q"),
            "cm" => {
                let m = [0, 1, 2, 3, 4, 5].map(|i| num(&o[i]));
                st.ctm = concat(&m, &st.ctm);
            }
            "w" => st.width = num(&o[0]),
            "RG" => st.stroke = [num(&o[0]), num(&o[1]), num(&o[2])],
            "rg" => st.fill = [num(&o[0]), num(&o[1]), num(&o[2])],
            "G" => st.stroke = [num(&o[0]); 3],
            "g" => st.fill = [num(&o[0]); 3],
            "m" => path.push(vec![apply(&st.ctm, num(&o[0]), num(&o[1]))]),
            "l" => {
                path.last_mut()
                    .expect("m before l")
                    .push(apply(&st.ctm, num(&o[0]), num(&o[1])))
            }
            "c" => {
                path.last_mut()
                    .expect("m before c")
                    .push(apply(&st.ctm, num(&o[4]), num(&o[5])))
            }
            "h" => {
                if let Some(sub) = path.last_mut() {
                    let first = sub[0];
                    sub.push(first);
                }
            }
            "re" => {
                let (x, y, w, h) = (num(&o[0]), num(&o[1]), num(&o[2]), num(&o[3]));
                let c = [[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]];
                path.push(c.iter().map(|p| apply(&st.ctm, p[0], p[1])).collect());
            }
            "W" | "W*" => clip_next = true,
            "n" => {
                if clip_next {
                    st.clipped = true;
                    clip_next = false;
                }
                path.clear();
            }
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                paints.push(Paint {
                    op: op.operator.clone(),
                    stroke: st.stroke,
                    fill: st.fill,
                    width: st.width,
                    subpaths: std::mem::take(&mut path),
                    clipped: st.clipped,
                });
            }
            "BT" => tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            "Tf" => font = String::from_utf8(o[0].as_name().unwrap().to_vec()).unwrap(),
            "Tm" => tm = [0, 1, 2, 3, 4, 5].map(|i| num(&o[i])),
            "Tj" => glyphs.push(Glyph {
                font: font.clone(),
                code: o[0].as_str().unwrap().to_vec(),
                tm: concat(&tm, &st.ctm),
                fill: st.fill,
            }),
            "Do" => images += 1,
            _ => {}
        }
    }
    Page {
        doc,
        page,
        size,
        paints,
        glyphs,
        images,
        ops,
    }
}

impl Page {
    /// Every stroked segment with the paint it belongs to.
    pub fn strokes(&self) -> Vec<(&Paint, [f32; 2], [f32; 2])> {
        self.paints
            .iter()
            .filter(|p| matches!(p.op.as_str(), "S" | "s" | "B" | "B*" | "b" | "b*"))
            .flat_map(|p| {
                p.subpaths
                    .iter()
                    .flat_map(move |s| s.windows(2).map(move |w| (p, w[0], w[1])))
            })
            .collect()
    }

    /// The font dictionary of resource `name`.
    pub fn font(&self, name: &str) -> &lopdf::Dictionary {
        let (resources, _) = self.doc.get_page_resources(self.page).unwrap();
        let fonts = resources.unwrap().get(b"Font").unwrap().as_dict().unwrap();
        let id = fonts.get(name.as_bytes()).unwrap().as_reference().unwrap();
        self.doc.get_dictionary(id).unwrap()
    }

    /// The decompressed stream that `dict[key]` refers to.
    pub fn stream(&self, dict: &lopdf::Dictionary, key: &[u8]) -> Vec<u8> {
        let id = dict.get(key).unwrap().as_reference().unwrap();
        let stream = self.doc.get_object(id).unwrap().as_stream().unwrap();
        stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone())
    }

    /// The dictionary `dict[key]` refers to (or holds).
    pub fn deref<'a>(&'a self, dict: &'a lopdf::Dictionary, key: &[u8]) -> &'a lopdf::Dictionary {
        match dict.get(key).unwrap() {
            Object::Reference(id) => self.doc.get_dictionary(*id).unwrap(),
            Object::Dictionary(d) => d,
            Object::Array(a) => match &a[0] {
                Object::Reference(id) => self.doc.get_dictionary(*id).unwrap(),
                Object::Dictionary(d) => d,
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }
}

pub fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

pub fn near(p: [f32; 2], q: [f32; 2], tol: f32) -> bool {
    close(p[0], q[0], tol) && close(p[1], q[1], tol)
}

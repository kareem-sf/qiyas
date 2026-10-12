//! A PDF content stream as text, keeping the colours and line width set so each is written
//! only when it changes.

use std::fmt::Write as _;

use qiyas_geom::Xform;

/// A number with at most `decimals` decimals, without trailing zeros (`-0` as `0`).
pub fn fmt(v: f64, decimals: usize) -> String {
    let mut s = format!("{v:.decimals$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// The six numbers `a b c d e f` of `xf` as a PDF matrix.
pub fn matrix(xf: &Xform) -> [f64; 6] {
    let m = &xf.m;
    [m[0][0], m[1][0], m[0][1], m[1][1], m[0][2], m[1][2]]
}

/// `s` as a PDF text string in hex: UTF-16BE after its byte order mark, `<FEFF…>`.
pub fn utf16_hex(s: &str) -> String {
    let mut hex = String::from("<FEFF");
    for unit in s.encode_utf16() {
        let _ = write!(hex, "{unit:04X}");
    }
    hex.push('>');
    hex
}

#[derive(Clone, Copy, Default, PartialEq)]
struct State {
    stroke: Option<[u8; 3]>,
    fill: Option<[u8; 3]>,
    width: Option<f64>,
}

#[derive(Default)]
pub struct Content {
    pub buf: String,
    state: State,
    saved: Vec<State>,
}

fn rgb(c: [u8; 3]) -> String {
    c.map(|v| fmt(f64::from(v) / 255.0, 4)).join(" ")
}

impl Content {
    pub fn op(&mut self, s: &str) {
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    /// `x y` in page points.
    pub fn point(&mut self, p: [f64; 2], op: &str) {
        let _ = writeln!(self.buf, "{} {} {op}", fmt(p[0], 3), fmt(p[1], 3));
    }

    pub fn save(&mut self) {
        self.saved.push(self.state);
        self.op("q");
    }

    pub fn restore(&mut self) {
        self.state = self.saved.pop().unwrap_or_default();
        self.op("Q");
    }

    pub fn stroke(&mut self, c: [u8; 3]) {
        if self.state.stroke != Some(c) {
            self.state.stroke = Some(c);
            let _ = writeln!(self.buf, "{} RG", rgb(c));
        }
    }

    pub fn fill(&mut self, c: [u8; 3]) {
        if self.state.fill != Some(c) {
            self.state.fill = Some(c);
            let _ = writeln!(self.buf, "{} rg", rgb(c));
        }
    }

    /// Line width in points (0: the thinnest line the device draws).
    pub fn width(&mut self, w: f64) {
        if self.state.width != Some(w) {
            self.state.width = Some(w);
            let _ = writeln!(self.buf, "{} w", fmt(w, 4));
        }
    }

    pub fn matrix(&mut self, m: [f64; 6], op: &str) {
        let parts: Vec<String> = m
            .iter()
            .enumerate()
            .map(|(i, v)| fmt(*v, if i < 4 { 6 } else { 3 }))
            .collect();
        let _ = writeln!(self.buf, "{} {op}", parts.join(" "));
    }
}

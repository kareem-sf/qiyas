/// A 2D affine transform as a 3x3 matrix acting on column vectors `[x, y, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xform {
    pub m: [[f64; 3]; 3],
}

impl Xform {
    pub fn identity() -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    pub fn translate(dx: f64, dy: f64) -> Self {
        Self {
            m: [[1.0, 0.0, dx], [0.0, 1.0, dy], [0.0, 0.0, 1.0]],
        }
    }

    /// Counter-clockwise rotation about the origin.
    pub fn rotate(rad: f64) -> Self {
        let (s, c) = rad.sin_cos();
        Self {
            m: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            m: [[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// The transform that applies `self` first, then `other`.
    pub fn then(&self, other: &Xform) -> Xform {
        let (a, b) = (&other.m, &self.m);
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        Xform { m }
    }

    pub fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        let m = &self.m;
        [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2],
        ]
    }
}

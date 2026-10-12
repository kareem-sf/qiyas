//! Parametric constraints for Qiyas: a small sketch of points and curves (lines, circles, arcs),
//! geometric constraints between them (coincident, parallel, horizontal...) and dimensional ones
//! (a distance, an angle, a radius), solved by damped Gauss-Newton (`solver`); and the small
//! expression language of dimensional constraints, `d1 = 2*d2 + 1.5` (`expr`).
//!
//! The crate knows nothing of drawings: `qiyas-core` turns entity vertices and curves into a
//! `Sketch`, solves it and writes the points back.

pub mod expr;
pub mod model;
pub mod solver;

pub use expr::{evaluate_all, Expr, ExprError};
pub use model::{Constraint, Curve, CurveId, PointId, Sketch};
pub use solver::{solve, SolveError, Solved};

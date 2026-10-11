//! Paper sizes and where the plotted area lands on the page.

use qiyas_geom::Xform;
use serde::{Deserialize, Serialize};

use crate::{Orientation, PlotError, Scale};

/// PDF points per millimetre.
pub const PT_PER_MM: f64 = 72.0 / 25.4;
/// The margin a fit to paper leaves on every side, in millimetres.
pub const FIT_MARGIN_MM: f64 = 5.0;

/// A paper size, portrait (`w_mm` the shorter side for the catalog's sizes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paper {
    pub name: String,
    pub w_mm: f64,
    pub h_mm: f64,
}

/// The built-in paper sizes, portrait: ISO A0 to A4, ANSI A to E and ARCH A to E1.
pub fn paper_catalog() -> Vec<Paper> {
    [
        ("ISO A0", 841.0, 1189.0),
        ("ISO A1", 594.0, 841.0),
        ("ISO A2", 420.0, 594.0),
        ("ISO A3", 297.0, 420.0),
        ("ISO A4", 210.0, 297.0),
        ("ANSI A", 215.9, 279.4),
        ("ANSI B", 279.4, 431.8),
        ("ANSI C", 431.8, 558.8),
        ("ANSI D", 558.8, 863.6),
        ("ANSI E", 863.6, 1117.6),
        ("ARCH A", 228.6, 304.8),
        ("ARCH B", 304.8, 457.2),
        ("ARCH C", 457.2, 609.6),
        ("ARCH D", 609.6, 914.4),
        ("ARCH E1", 762.0, 1066.8),
        ("ARCH E", 914.4, 1219.2),
    ]
    .into_iter()
    .map(|(name, w_mm, h_mm)| Paper {
        name: name.to_string(),
        w_mm,
        h_mm,
    })
    .collect()
}

/// The page and the map from sheet coordinates to page points (origin at the lower left).
#[derive(Debug, Clone)]
pub struct PageMap {
    /// Page width and height in points.
    pub size: [f64; 2],
    /// Sheet coordinates to page points.
    pub xf: Xform,
}

impl PageMap {
    /// Puts `area` (`[min, max]` in sheet units) on `paper`: fitted inside the margin, or at
    /// `num` paper millimetres per `den` sheet millimetres (`mm_per_unit` sheet millimetres per
    /// unit); centred, or with its lower left at the page's (the margin's, when fitted).
    pub fn new(
        paper: &Paper,
        orientation: Orientation,
        scale: Scale,
        centre: bool,
        area: [[f64; 2]; 2],
        mm_per_unit: f64,
    ) -> Result<PageMap, PlotError> {
        let (w, h) = (paper.w_mm, paper.h_mm);
        if !(w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()) {
            return Err(PlotError::Paper(format!(
                "{} is not a paper size",
                paper.name
            )));
        }
        let (w, h) = match orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        };
        let size = [w * PT_PER_MM, h * PT_PER_MM];
        let [lo, hi] = area;
        let (aw, ah) = (hi[0] - lo[0], hi[1] - lo[1]);
        let usable = |v: f64| v > 0.0 && v.is_finite();
        if !(usable(aw) || usable(ah)) {
            return Err(PlotError::Nothing);
        }
        let (s, margin) = match scale {
            Scale::Fit => {
                let m = FIT_MARGIN_MM * PT_PER_MM;
                let fit = |page: f64, area: f64| {
                    if usable(area) {
                        (page - 2.0 * m) / area
                    } else {
                        f64::INFINITY
                    }
                };
                (fit(size[0], aw).min(fit(size[1], ah)), m)
            }
            Scale::Ratio(num, den) => {
                if !(usable(num) && usable(den)) {
                    return Err(PlotError::Paper(format!("{num}:{den} is not a plot scale")));
                }
                (num / den * mm_per_unit * PT_PER_MM, 0.0)
            }
        };
        if !usable(s) {
            return Err(PlotError::Paper("The area does not fit the paper".into()));
        }
        let (dx, dy) = if centre {
            (
                size[0] / 2.0 - (lo[0] + hi[0]) / 2.0 * s,
                size[1] / 2.0 - (lo[1] + hi[1]) / 2.0 * s,
            )
        } else {
            (margin - lo[0] * s, margin - lo[1] * s)
        };
        Ok(PageMap {
            size,
            xf: Xform::scale(s, s).then(&Xform::translate(dx, dy)),
        })
    }
}

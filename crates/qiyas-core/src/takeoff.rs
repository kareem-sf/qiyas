//! The take-off: every measurement the user (or the AI) has made in this session.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::Units;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Length,
    Area,
    Count,
}

/// One take-off line. `value` is in drawing units (units² for areas, pieces for counts);
/// `shown` is the same quantity in `unit`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    /// UUID v4.
    pub id: String,
    pub kind: Kind,
    pub value: f64,
    /// "m", "m2" or "pcs".
    pub unit: String,
    pub shown: f64,
    pub description: String,
    /// The measured entity's layer; empty for shapes the user drew.
    pub layer: String,
    /// Block name of the sheet it was measured on, e.g. `*Model_Space`.
    pub sheet: String,
    pub handles: Vec<u64>,
    /// The points the user picked, in drawing units; empty for measured entities.
    pub points: Vec<[f64; 2]>,
    pub block_name: Option<String>,
    pub self_intersecting: bool,
    /// "user" or "ai".
    pub source: String,
    pub note: String,
    /// RFC 3339.
    pub created: String,
}

impl Measurement {
    /// A new user measurement made now, converted to metres with `units`. The caller
    /// fills in handles, points, block name and the self-intersection flag.
    pub fn new(
        kind: Kind,
        value: f64,
        units: Units,
        description: String,
        layer: String,
        sheet: String,
    ) -> Measurement {
        let f = units.to_metres();
        let (unit, shown) = match kind {
            Kind::Length => ("m", value * f),
            Kind::Area => ("m2", value * f * f),
            Kind::Count => ("pcs", value),
        };
        Measurement {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            value,
            unit: unit.to_string(),
            shown,
            description,
            layer,
            sheet,
            handles: Vec::new(),
            points: Vec::new(),
            block_name: None,
            self_intersecting: false,
            source: "user".to_string(),
            note: String::new(),
            created: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .expect("UTC now formats as RFC 3339"),
        }
    }
}

/// The sum of the take-off lines of one kind and unit, in that unit.
#[derive(Debug, Clone, Serialize)]
pub struct Total {
    pub kind: Kind,
    pub unit: String,
    pub count: usize,
    pub sum: f64,
}

/// The session's measurements, in the order they were made.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Takeoff {
    pub items: Vec<Measurement>,
}

impl Takeoff {
    pub fn add(&mut self, m: Measurement) {
        self.items.push(m);
    }

    /// Removes and returns the measurement with `id`, if there is one.
    pub fn remove(&mut self, id: &str) -> Option<Measurement> {
        let i = self.items.iter().position(|m| m.id == id)?;
        Some(self.items.remove(i))
    }

    pub fn get(&self, id: &str) -> Option<&Measurement> {
        self.items.iter().find(|m| m.id == id)
    }

    /// One total per kind and unit, in order of first appearance; sums the `shown` values.
    pub fn totals(&self) -> Vec<Total> {
        let mut totals: Vec<Total> = Vec::new();
        for m in &self.items {
            match totals
                .iter_mut()
                .find(|t| t.kind == m.kind && t.unit == m.unit)
            {
                Some(t) => {
                    t.count += 1;
                    t.sum += m.shown;
                }
                None => totals.push(Total {
                    kind: m.kind,
                    unit: m.unit.clone(),
                    count: 1,
                    sum: m.shown,
                }),
            }
        }
        totals
    }
}

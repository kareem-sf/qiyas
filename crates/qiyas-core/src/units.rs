/// Drawing units, from the header's `insertion_units` (INSUNITS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Units {
    Inches,
    Feet,
    Millimetres,
    Centimetres,
    Metres,
    Unknown,
}

impl Units {
    pub fn from_header(insertion_units: i16) -> Units {
        match insertion_units {
            1 => Units::Inches,
            2 => Units::Feet,
            4 => Units::Millimetres,
            5 => Units::Centimetres,
            6 => Units::Metres,
            _ => Units::Unknown,
        }
    }

    /// Metres per drawing unit. Unknown is treated as millimetres.
    pub fn to_metres(&self) -> f64 {
        match self {
            Units::Inches => 0.0254,
            Units::Feet => 0.3048,
            Units::Millimetres | Units::Unknown => 0.001,
            Units::Centimetres => 0.01,
            Units::Metres => 1.0,
        }
    }
}

//! The properties palette's edits (`Command::SetProperty`) and matching one entity's properties
//! onto others (`Command::MatchProperties`, AutoCAD's MATCHPROP). Only the drawing's own
//! entities at the top of the sheet on screen change; a match may read its source anywhere (inside
//! a block, in an xref). One undo step each. A change of shape (a radius, a closed flag, a width,
//! a block reference's scale or turn) measures the take-off rows on the entity again
//! (`follow_edit`); a dimension or a table draws its block again when its text height or style
//! changes; a row measuring an entity whose layer changes takes the new layer.

use opencadcodec::entities::{CellStyle, Dimension, EntityCommon};
use opencadcodec::{EntityType, Handle, LineWeight};
use qiyas_geom::{dimscale, set_dimtxt, DimLook};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::draw::{parse_color, LINEWEIGHTS};
use crate::modify::measures;
use crate::session::{locate, variant_name};
use crate::{CoreError, Session, Target};

/// A property the palette edits, and its value: `layer` a layer of the drawing; `color`
/// "ByLayer", "ByBlock", "ACI n" or "#rrggbb"; `linetype` "ByLayer", "ByBlock" or a linetype of
/// the drawing; `lineweight` hundredths of a millimetre (-1 ByLayer, -2 ByBlock, -3 Default);
/// `text` a text's or an MText's text; `text_height` (drawing units, above 0) of a text, an
/// MText, a dimension or a table; `text_rotation` (degrees) of a text or an MText; `text_style`
/// a text style of the drawing; `radius` (above 0) of a circle or an arc; `closed` (true or
/// false) and `global_width` (0 or more) of a polyline; `insert_scale` (above 0, all three
/// axes) and `insert_rotation` (degrees) of a block reference; `dim_style` a dimension style of
/// the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityProp {
    Layer,
    Color,
    Linetype,
    Lineweight,
    Text,
    TextHeight,
    TextRotation,
    TextStyle,
    Radius,
    Closed,
    GlobalWidth,
    InsertScale,
    InsertRotation,
    DimStyle,
}

/// What `Command::MatchProperties` copies. Text style and height go text to text, dimension
/// style dimension to dimension, hatch pattern hatch to hatch; the rest to anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchProp {
    Layer,
    Color,
    Linetype,
    Lineweight,
    TextStyle,
    TextHeight,
    DimStyle,
    HatchPattern,
}

fn invalid(why: impl Into<String>) -> CoreError {
    CoreError::InvalidCommand(why.into())
}

fn number(value: &Value) -> Result<f64, CoreError> {
    value
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid(format!("{value} is not a number")))
}

fn positive(value: &Value) -> Result<f64, CoreError> {
    Some(number(value)?)
        .filter(|v| *v > 0.0)
        .ok_or_else(|| invalid("The value must be more than 0"))
}

fn text(value: &Value) -> Result<&str, CoreError> {
    value
        .as_str()
        .ok_or_else(|| invalid(format!("{value} is not text")))
}

/// What the entity's kind is called in a refusal.
fn no_such(entity: &EntityType, what: &str) -> CoreError {
    invalid(format!("A {} has no {what}", variant_name(entity)))
}

/// Text height of a text or an MText.
fn text_height(e: &EntityType) -> Option<f64> {
    match e {
        EntityType::Text(t) => Some(t.height),
        EntityType::MText(t) => Some(t.height),
        _ => None,
    }
}

/// Text style of a text or an MText.
fn text_style(e: &EntityType) -> Option<&str> {
    match e {
        EntityType::Text(t) => Some(&t.style),
        EntityType::MText(t) => Some(&t.style),
        _ => None,
    }
}

/// What a change to one entity needs afterwards.
#[derive(Default)]
struct After {
    /// Its shape changed: the take-off rows on it are measured again.
    reshaped: bool,
    /// A dimension's or a table's block is drawn again.
    rebuild: bool,
}

impl Session {
    /// `Command::SetProperty`.
    pub(crate) fn set_property(
        &mut self,
        target: &Target,
        prop: EntityProp,
        value: &Value,
    ) -> Result<(), CoreError> {
        let h = self.sheet_targets(std::slice::from_ref(target))?[0];
        if prop == EntityProp::Text {
            let e = self.entity(h);
            if !matches!(e, EntityType::Text(_) | EntityType::MText(_)) {
                return Err(no_such(e, "text"));
            }
            return self.edit_text(target, text(value)?, None);
        }
        let mut e = self.entity(h).clone();
        let after = self.apply_property(&mut e, prop, value)?;
        self.store(h, e, after)
    }

    /// Puts `e` back as `h` when it changed, then does what `after` asks.
    fn store(&mut self, h: Handle, e: EntityType, after: After) -> Result<(), CoreError> {
        if e == *self.entity(h) {
            return Ok(());
        }
        let layer = e.common().layer.clone();
        *self.drawing.doc.get_entity_mut(h).expect("a sheet target") = e;
        for m in &mut self.takeoff.items {
            if m.chain.is_empty() && measures(m, h.value()) {
                m.layer.clone_from(&layer);
            }
        }
        if after.rebuild {
            match self.entity(h) {
                EntityType::Dimension(_) => self.rebuild_dimension(h)?,
                EntityType::Table(_) => self.rebuild_table(h)?,
                _ => {}
            }
        }
        if after.reshaped {
            self.follow_edit(&[h], &[], &[], &[]);
        }
        Ok(())
    }

    /// `prop` of `e` takes `value`, checked against the drawing.
    fn apply_property(
        &self,
        e: &mut EntityType,
        prop: EntityProp,
        value: &Value,
    ) -> Result<After, CoreError> {
        let doc = &self.drawing.doc;
        let mut after = After::default();
        match prop {
            EntityProp::Layer => {
                let name = text(value)?.trim();
                let layer = doc
                    .layers
                    .get(name)
                    .ok_or_else(|| invalid(format!("The drawing has no layer {name}")))?;
                e.common_mut().layer.clone_from(&layer.name);
            }
            EntityProp::Color => {
                let c = text(value)?;
                let color =
                    parse_color(c).ok_or_else(|| invalid(format!("{c} is not a colour")))?;
                let common = e.common_mut();
                common.color = color;
                common.color_name = None;
                common.color_book_handle = None;
            }
            EntityProp::Linetype => {
                let lt = text(value)?.trim();
                let name = if lt.eq_ignore_ascii_case("ByLayer") {
                    String::new()
                } else if lt.eq_ignore_ascii_case("ByBlock") {
                    "ByBlock".to_string()
                } else {
                    doc.line_types
                        .get(lt)
                        .map(|t| t.name.clone())
                        .ok_or_else(|| invalid(format!("The drawing has no linetype {lt}")))?
                };
                let common = e.common_mut();
                common.linetype = name;
                common.linetype_handle = None;
            }
            EntityProp::Lineweight => {
                let w = value
                    .as_i64()
                    .and_then(|w| i16::try_from(w).ok())
                    .filter(|w| LINEWEIGHTS.contains(w) || matches!(w, -3..=-1))
                    .ok_or_else(|| invalid(format!("{value} is not a lineweight")))?;
                e.common_mut().line_weight = LineWeight::from_value(w);
            }
            EntityProp::Text => unreachable!("set_property edits text itself"),
            EntityProp::TextHeight => {
                let h = positive(value)?;
                match e {
                    EntityType::Text(t) => t.height = h,
                    EntityType::MText(t) => t.height = h,
                    EntityType::Dimension(d) => {
                        // The override is before DIMSCALE, as AutoCAD keeps DIMTXT.
                        let style = qiyas_geom::style_named(doc, &d.base().style_name);
                        let k = dimscale(&d.base().common).unwrap_or(style.dimscale);
                        let k = if k > 0.0 && k.is_finite() { k } else { 1.0 };
                        set_dimtxt(&mut d.base_mut().common, h / k);
                        after.rebuild = true;
                    }
                    EntityType::Table(t) => {
                        let base = t.base_style.clone().unwrap_or_else(CellStyle::new);
                        for cell in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                            cell.style.get_or_insert_with(|| base.clone()).text_height = h;
                        }
                        after.rebuild = true;
                    }
                    other => return Err(no_such(other, "text height")),
                }
            }
            EntityProp::TextRotation => {
                let a = number(value)?.to_radians();
                match e {
                    EntityType::Text(t) => t.rotation = a,
                    EntityType::MText(t) => t.rotation = a,
                    other => return Err(no_such(other, "text rotation")),
                }
            }
            EntityProp::TextStyle => {
                let name = text(value)?.trim();
                let style = doc
                    .text_styles
                    .get(name)
                    .map(|s| s.name.clone())
                    .ok_or_else(|| invalid(format!("The drawing has no text style {name}")))?;
                match e {
                    EntityType::Text(t) => t.style = style,
                    EntityType::MText(t) => t.style = style,
                    other => return Err(no_such(other, "text style")),
                }
            }
            EntityProp::Radius => {
                let r = positive(value)?;
                match e {
                    EntityType::Circle(c) => c.radius = r,
                    EntityType::Arc(a) => a.radius = r,
                    other => return Err(no_such(other, "radius")),
                }
                after.reshaped = true;
            }
            EntityProp::Closed => {
                let closed = value
                    .as_bool()
                    .ok_or_else(|| invalid(format!("{value} is not true or false")))?;
                match e {
                    EntityType::LwPolyline(p) => p.is_closed = closed,
                    other => return Err(no_such(other, "closed flag")),
                }
                after.reshaped = true;
            }
            EntityProp::GlobalWidth => {
                let w = number(value)?;
                if w < 0.0 {
                    return Err(invalid("A width can't be below 0"));
                }
                match e {
                    EntityType::LwPolyline(p) => {
                        p.constant_width = w;
                        for v in &mut p.vertices {
                            v.start_width = 0.0;
                            v.end_width = 0.0;
                        }
                    }
                    other => return Err(no_such(other, "global width")),
                }
                after.reshaped = true;
            }
            EntityProp::InsertScale => {
                let s = positive(value)?;
                match e {
                    EntityType::Insert(i) => {
                        i.set_x_scale(s);
                        i.set_y_scale(s);
                        i.set_z_scale(s);
                    }
                    other => return Err(no_such(other, "scale")),
                }
                after.reshaped = true;
            }
            EntityProp::InsertRotation => {
                let a = number(value)?.to_radians();
                match e {
                    EntityType::Insert(i) => i.rotation = a,
                    other => return Err(no_such(other, "rotation")),
                }
                after.reshaped = true;
            }
            EntityProp::DimStyle => {
                let name = text(value)?.trim();
                let style = doc
                    .dim_styles
                    .get(name)
                    .map(|s| s.name.clone())
                    .ok_or_else(|| invalid(format!("The drawing has no dimension style {name}")))?;
                match e {
                    EntityType::Dimension(d) => d.base_mut().style_name = style,
                    other => return Err(no_such(other, "dimension style")),
                }
                after.rebuild = true;
            }
        }
        Ok(after)
    }

    /// `Command::MatchProperties`: each of `targets` takes `props` of `source` where both have
    /// them.
    pub(crate) fn match_properties(
        &mut self,
        source: &Target,
        targets: &[Target],
        props: &[MatchProp],
    ) -> Result<(), CoreError> {
        let handles = self.sheet_targets(targets)?;
        let xrefs = self.xref_docs();
        let located = locate(&self.drawing.doc, &xrefs, source.handle, &source.chain)?;
        let from = located.entity.clone();
        // Layer "0" inside a block is drawn on its reference's: that is the layer it shows.
        let layer = self
            .drawing
            .doc
            .layers
            .get(&located.layer)
            .map(|l| l.name.clone());
        let source_doc_is_host = located.xref.is_none();
        drop(xrefs);
        let doc = &self.drawing.doc;
        let linetype = {
            let lt = &from.common().linetype;
            let ok = lt.is_empty()
                || lt.eq_ignore_ascii_case("ByBlock")
                || lt.eq_ignore_ascii_case("ByLayer")
                || doc.line_types.contains(lt);
            ok.then(|| lt.clone())
        };
        let style = text_style(&from)
            .filter(|s| doc.text_styles.contains(s))
            .map(str::to_string);
        let dim_style = match &from {
            EntityType::Dimension(d) if doc.dim_styles.contains(&d.base().style_name) => {
                Some(d.base().style_name.clone())
            }
            _ => None,
        };
        for h in handles {
            if source_doc_is_host && source.chain.is_empty() && h.value() == source.handle {
                continue;
            }
            let mut e = self.entity(h).clone();
            let mut after = After::default();
            for prop in props {
                let common: &mut EntityCommon = e.common_mut();
                let theirs = from.common();
                match prop {
                    MatchProp::Layer => {
                        if let Some(layer) = &layer {
                            common.layer.clone_from(layer);
                        }
                    }
                    MatchProp::Color => {
                        common.color = theirs.color;
                        common.color_name.clone_from(&theirs.color_name);
                        common.color_book_handle = None;
                    }
                    MatchProp::Linetype => {
                        if let Some(lt) = &linetype {
                            common.linetype.clone_from(lt);
                            common.linetype_handle = None;
                        }
                    }
                    MatchProp::Lineweight => common.line_weight = theirs.line_weight,
                    MatchProp::TextStyle => match (&mut e, &style) {
                        (EntityType::Text(t), Some(s)) => t.style.clone_from(s),
                        (EntityType::MText(t), Some(s)) => t.style.clone_from(s),
                        _ => {}
                    },
                    MatchProp::TextHeight => match (&mut e, text_height(&from)) {
                        (EntityType::Text(t), Some(height)) => t.height = height,
                        (EntityType::MText(t), Some(height)) => t.height = height,
                        _ => {}
                    },
                    MatchProp::DimStyle => {
                        if let (EntityType::Dimension(d), Some(s)) = (&mut e, &dim_style) {
                            d.base_mut().style_name.clone_from(s);
                            after.rebuild = true;
                        }
                    }
                    MatchProp::HatchPattern => {
                        if let (EntityType::Hatch(to), EntityType::Hatch(theirs)) = (&mut e, &from)
                        {
                            to.pattern = theirs.pattern.clone();
                            to.is_solid = theirs.is_solid;
                            to.pattern_type = theirs.pattern_type;
                            to.pattern_angle = theirs.pattern_angle;
                            to.pattern_scale = theirs.pattern_scale;
                            to.is_double = theirs.is_double;
                            to.gradient_color = theirs.gradient_color.clone();
                        }
                    }
                }
            }
            self.store(h, e, after)?;
        }
        Ok(())
    }
}

/// Text height a dimension draws with, for the palette.
pub(crate) fn dimension_text_height(doc: &opencadcodec::CadDocument, d: &Dimension) -> f64 {
    DimLook::of(doc, d).text_height
}

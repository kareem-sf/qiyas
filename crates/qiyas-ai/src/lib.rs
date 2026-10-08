//! The AI's tools: a catalogue of JSON-schema'd tools over the command bus and the read-only
//! queries, the executor that runs them on a `Session`, and a picture of the drawing for the
//! model to look at. The in-app assistant and the MCP server both call these.

mod execute;
mod image;
mod tools;

pub use execute::{execute, ToolError, ToolOutput};
pub use image::render_png;
pub use tools::{catalogue, ToolSpec};

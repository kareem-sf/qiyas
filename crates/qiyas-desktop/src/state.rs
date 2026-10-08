use std::sync::Mutex;

use qiyas_core::Session;

/// The open drawing, if any. Managed by Tauri and shared by every command.
#[derive(Default)]
pub struct AppState(pub Mutex<Option<Session>>);

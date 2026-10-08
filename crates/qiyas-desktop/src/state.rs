use std::path::PathBuf;
use std::sync::Mutex;

use qiyas_core::Session;

use crate::settings::{self, Settings};

/// Managed by Tauri and shared by every command.
pub struct AppState {
    /// The open drawing, if any.
    pub session: Mutex<Option<Session>>,
    /// The settings, loaded at startup and written back on every change.
    pub settings: Mutex<Settings>,
    /// Where `settings.json` lives.
    pub config_dir: PathBuf,
}

impl AppState {
    pub fn new(config_dir: PathBuf) -> Self {
        AppState {
            session: Mutex::new(None),
            settings: Mutex::new(settings::load(&config_dir)),
            config_dir,
        }
    }
}

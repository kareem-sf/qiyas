use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use qiyas_core::Session;

use crate::ai::AiRun;
use crate::settings::{self, Settings};

/// Managed by Tauri and shared by every command.
pub struct AppState {
    /// The open drawing, if any. Shared with the AI agent, which holds a clone of the `Arc`.
    pub session: Arc<Mutex<Option<Session>>>,
    /// The settings, loaded at startup and written back on every change.
    pub settings: Mutex<Settings>,
    /// Where `settings.json` lives.
    pub config_dir: PathBuf,
    /// The in-app assistant's conversation and running turn.
    pub ai: Mutex<AiRun>,
}

impl AppState {
    pub fn new(config_dir: PathBuf) -> Self {
        AppState {
            session: Arc::new(Mutex::new(None)),
            settings: Mutex::new(settings::load(&config_dir)),
            config_dir,
            ai: Mutex::new(AiRun::default()),
        }
    }
}

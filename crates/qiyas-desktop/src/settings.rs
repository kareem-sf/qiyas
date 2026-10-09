//! App settings in `<app config dir>/settings.json`. The AI provider's API key is not here: it
//! lives in the OS keychain (see `secrets`).
//!
//! Plain functions over a directory, so tests run without Tauri.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = "settings.json";

pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Who answers the in-app assistant.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    Anthropic,
    OpenaiCompatible,
}

impl Provider {
    /// The name used in the settings file and as the keychain account.
    pub fn name(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenaiCompatible => "openai_compatible",
        }
    }

    pub fn from_name(name: &str) -> Option<Provider> {
        [Provider::Anthropic, Provider::OpenaiCompatible]
            .into_iter()
            .find(|p| p.name() == name)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub provider: Provider,
    /// Used exactly as typed.
    pub model: String,
    /// The OpenAI-compatible service's address; unused for Anthropic.
    pub base_url: Option<String>,
    /// One of `EFFORTS`.
    pub effort: String,
    pub mcp_enabled: bool,
    pub mcp_port: u16,
    /// 32 hex characters, made once and kept. Empty when missing from the file, so `load` makes
    /// and writes one instead of a new random one each start.
    #[serde(default)]
    pub mcp_token: String,
    /// "en" or "ar".
    pub language: String,
    /// "system", "light" or "dark".
    pub theme: String,
    /// The user's command line aliases, alias to command (e.g. "L" to "LAYOFF"), over the
    /// built-in ones.
    pub aliases: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            provider: Provider::Anthropic,
            model: "claude-opus-5-5".into(),
            base_url: None,
            effort: "high".into(),
            mcp_enabled: false,
            mcp_port: 48123,
            mcp_token: new_token(),
            language: "en".into(),
            theme: "system".into(),
            aliases: BTreeMap::new(),
        }
    }
}

/// 16 random bytes as 32 lowercase hex characters.
pub fn new_token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the OS random number generator failed");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn valid_token(token: &str) -> bool {
    token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Checks settings coming from the window before they are kept.
pub fn validate(s: &Settings) -> Result<(), String> {
    if s.mcp_port < 1024 {
        return Err(format!(
            "The MCP port must be between 1024 and 65535, not {}",
            s.mcp_port
        ));
    }
    if !EFFORTS.contains(&s.effort.as_str()) {
        return Err(format!(
            "Unknown effort \"{}\": use one of {}",
            s.effort,
            EFFORTS.join(", ")
        ));
    }
    if s.model.trim().is_empty() {
        return Err("The model is empty".into());
    }
    if !valid_token(&s.mcp_token) {
        return Err("The MCP token must be 32 hex characters".into());
    }
    if !["en", "ar"].contains(&s.language.as_str()) {
        return Err(format!("Unknown language \"{}\"", s.language));
    }
    if !["system", "light", "dark"].contains(&s.theme.as_str()) {
        return Err(format!("Unknown theme \"{}\"", s.theme));
    }
    for (alias, command) in &s.aliases {
        if alias.is_empty() || alias.contains(char::is_whitespace) || command.trim().is_empty() {
            return Err(format!(
                "The alias \"{alias}\" needs one word and a command"
            ));
        }
    }
    Ok(())
}

/// Reads the settings in `dir`. A missing file gives the defaults; a file that can't be read as
/// settings is kept as `settings.json.bak` and the defaults are used. Whenever the defaults or a
/// new MCP token were used, the result is written back so the token stays the same next time.
pub fn load(dir: &Path) -> Settings {
    let path = dir.join(FILE);
    let (mut settings, mut write) = match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Settings>(&text) {
            Ok(s) => (s, false),
            Err(e) => {
                eprintln!("settings.json could not be read ({e}); using the defaults");
                if let Err(e) = fs::rename(&path, dir.join(format!("{FILE}.bak"))) {
                    eprintln!("settings.json could not be backed up: {e}");
                }
                (Settings::default(), true)
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => (Settings::default(), true),
        Err(e) => {
            eprintln!("settings.json could not be read ({e}); using the defaults");
            return Settings::default();
        }
    };
    if !valid_token(&settings.mcp_token) {
        settings.mcp_token = new_token();
        write = true;
    }
    if write {
        if let Err(e) = save(dir, &settings) {
            eprintln!("settings.json could not be written: {e}");
        }
    }
    // Files from before they were written 0600 hold the token too.
    for name in [FILE.to_string(), format!("{FILE}.bak")] {
        if let Err(e) = owner_only(&dir.join(&name)) {
            eprintln!("{name} could not be made private: {e}");
        }
    }
    settings
}

/// On Unix, lets only the user read and write `path`, if it exists.
fn owner_only(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(path) {
            Ok(meta) if meta.permissions().mode() & 0o777 != 0o600 => {
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            }
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Writes the settings to `dir` atomically: a temporary file, then a rename over the old one. On
/// Unix only the user can read the file, since it holds the MCP token.
pub fn save(dir: &Path, settings: &Settings) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    let text = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp)?;
    // A temporary file left by a crash keeps its old mode; set it again.
    #[cfg(unix)]
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    file.write_all(text.as_bytes())?;
    drop(file);
    fs::rename(&tmp, dir.join(FILE))
}

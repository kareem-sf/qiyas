//! Settings on disk and the API key in the keychain, without Tauri. The keychain is the
//! `keyring_core` mock store, so no test touches the real one.

use std::fs;
use std::path::Path;
use std::sync::Once;

use qiyas_desktop_lib::secrets;
use qiyas_desktop_lib::settings::{self, Provider, Settings};

fn mock_keychain() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap()));
}

fn read(dir: &Path) -> String {
    fs::read_to_string(dir.join("settings.json")).unwrap()
}

#[test]
fn defaults_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let s = settings::load(dir.path());
    assert_eq!(s.provider, Provider::Anthropic);
    assert_eq!(s.model, "claude-opus-5-5");
    assert_eq!(s.base_url, None);
    assert_eq!(s.effort, "high");
    assert!(!s.mcp_enabled);
    assert_eq!(s.mcp_port, 48123);
    assert_eq!(s.mcp_token.len(), 32);
    assert!(s.mcp_token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!((s.language.as_str(), s.theme.as_str()), ("en", "system"));
    // The token is made once: the next start reads the same one.
    assert_eq!(settings::load(dir.path()).mcp_token, s.mcp_token);
}

#[test]
fn round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let s = Settings {
        provider: Provider::OpenaiCompatible,
        model: "my-model".into(),
        base_url: Some("http://localhost:11434/v1".into()),
        effort: "max".into(),
        mcp_enabled: true,
        mcp_port: 50000,
        language: "ar".into(),
        theme: "dark".into(),
        ..Settings::default()
    };
    settings::save(dir.path(), &s).unwrap();
    assert!(read(dir.path()).contains("\"openai_compatible\""));
    assert_eq!(settings::load(dir.path()), s);
    // Saving leaves no temporary file behind.
    assert!(!dir.path().join("settings.json.tmp").exists());
}

#[test]
fn missing_fields_take_defaults() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("settings.json"), r#"{"effort":"low"}"#).unwrap();
    let s = settings::load(dir.path());
    assert_eq!(s.effort, "low");
    assert_eq!(s.model, "claude-opus-5-5");
    assert_eq!(s.mcp_token.len(), 32);
    // The new token was written back.
    assert!(read(dir.path()).contains(&s.mcp_token));
}

#[test]
fn corrupt_file_is_backed_up_and_defaults_used() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("settings.json"), "{ not json").unwrap();
    let s = settings::load(dir.path());
    assert_eq!(s.model, "claude-opus-5-5");
    assert_eq!(
        fs::read_to_string(dir.path().join("settings.json.bak")).unwrap(),
        "{ not json"
    );
    assert_eq!(settings::load(dir.path()), s);
}

#[test]
fn validation_rejects_bad_values() {
    let good = Settings::default();
    assert!(settings::validate(&good).is_ok());
    let port = Settings {
        mcp_port: 80,
        ..good.clone()
    };
    assert!(settings::validate(&port).unwrap_err().contains("1024"));
    let effort = Settings {
        effort: "ultra".into(),
        ..good.clone()
    };
    assert!(settings::validate(&effort).unwrap_err().contains("ultra"));
    let token = Settings {
        mcp_token: "short".into(),
        ..good
    };
    assert!(settings::validate(&token).is_err());
    // An unknown provider never gets past the IPC layer.
    assert!(serde_json::from_str::<Provider>("\"gemini\"").is_err());
}

#[test]
fn api_key_set_read_delete() {
    mock_keychain();
    assert_eq!(secrets::get_api_key("openai_compatible").unwrap(), None);
    secrets::set_api_key("openai_compatible", "  sk-test-123  ").unwrap();
    assert_eq!(
        secrets::get_api_key("openai_compatible")
            .unwrap()
            .as_deref(),
        Some("sk-test-123")
    );
    secrets::delete_api_key("openai_compatible").unwrap();
    assert_eq!(secrets::get_api_key("openai_compatible").unwrap(), None);
    // Deleting twice is fine.
    secrets::delete_api_key("openai_compatible").unwrap();
    assert!(secrets::set_api_key("openai_compatible", "   ").is_err());
    assert!(secrets::set_api_key("gemini", "key").is_err());
}

fn files_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn api_key_never_written_to_disk() {
    mock_keychain();
    let dir = tempfile::tempdir().unwrap();
    let key = "sk-ant-never-on-disk-5f3c9a";
    secrets::set_api_key("anthropic", key).unwrap();
    let s = settings::load(dir.path());
    settings::save(dir.path(), &s).unwrap();
    assert_eq!(
        secrets::get_api_key("anthropic").unwrap().as_deref(),
        Some(key)
    );

    let mut files = Vec::new();
    files_under(dir.path(), &mut files);
    assert!(!files.is_empty());
    for f in files {
        let bytes = fs::read(&f).unwrap();
        assert!(
            !bytes.windows(key.len()).any(|w| w == key.as_bytes()),
            "the key is in {}",
            f.display()
        );
    }
}

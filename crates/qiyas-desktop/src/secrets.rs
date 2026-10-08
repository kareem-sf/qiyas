//! The AI provider's API key, kept in the OS keychain only: service `qiyas`, account = provider
//! name. Never written to a file, never logged, never sent back to the window.
//!
//! The functions use whatever `keyring_core` default store is set: the app sets the OS one with
//! `use_native_store` at startup; tests set the mock store.

use keyring_core::{Entry, Error};

use crate::settings::Provider;

const SERVICE: &str = "qiyas";

/// Sets the OS keychain (macOS Keychain, Windows Credential Manager, Secret Service on Linux) as
/// the store. On failure the app still runs; saving a key then says there is no keychain.
pub fn use_native_store() -> Result<(), String> {
    keyring::Entry::store_status()
        .as_ref()
        .map_err(|e| format!("No keychain available: {e}"))?;
    Ok(())
}

fn entry(provider: &str) -> Result<Entry, String> {
    if Provider::from_name(provider).is_none() {
        return Err(format!("Unknown provider \"{provider}\""));
    }
    Entry::new(SERVICE, provider).map_err(describe)
}

fn describe(e: Error) -> String {
    match e {
        Error::NoDefaultStore | Error::NoStorageAccess(_) | Error::PlatformFailure(_) => {
            format!("No keychain available: {e}")
        }
        e => format!("Keychain error: {e}"),
    }
}

/// Stores `key` (trimmed) for `provider`, replacing any key already there.
pub fn set_api_key(provider: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("The API key is empty".into());
    }
    entry(provider)?.set_password(key).map_err(describe)
}

/// The key stored for `provider`, or None when there is none.
pub fn get_api_key(provider: &str) -> Result<Option<String>, String> {
    match entry(provider)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(describe(e)),
    }
}

/// Removes the key for `provider`; removing a key that isn't there is fine.
pub fn delete_api_key(provider: &str) -> Result<(), String> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(describe(e)),
    }
}

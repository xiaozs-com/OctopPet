use std::{collections::HashMap, fs, path::Path, sync::Mutex};

#[cfg(debug_assertions)]
use std::path::PathBuf;
use tauri::AppHandle;
#[cfg(all(debug_assertions, not(windows)))]
use tauri::Manager;

use crate::config_cmd;

#[cfg(not(debug_assertions))]
use keyring::{Entry, Error};

#[cfg(not(debug_assertions))]
const KEYRING_SERVICE: &str = "com.octop.pet";
#[cfg(debug_assertions)]
const DEV_SECRETS_FILE: &str = "dev-secrets.json";
static SECRETS_WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn validate_secret_key(key: &str) -> Result<(), String> {
    match key {
        "password" | "access_token" => Ok(()),
        _ => Err(format!("unsupported secret key: {key}")),
    }
}

pub fn secret_account(username: &str, key: &str) -> Result<String, String> {
    validate_secret_key(key)?;
    if username.trim().is_empty() {
        return Err("username is not configured".into());
    }
    Ok(format!("{username}:{key}"))
}

pub fn load_secrets_file(path: &Path) -> Result<HashMap<String, String>, String> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let json = fs::read_to_string(path)
        .map_err(|error| format!("failed to read secrets {}: {error}", path.display()))?;
    serde_json::from_str(&json)
        .map_err(|error| format!("failed to parse secrets {}: {error}", path.display()))
}

pub fn save_secrets_file(path: &Path, secrets: &HashMap<String, String>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create secrets directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let json = serde_json::to_string_pretty(secrets)
        .map_err(|error| format!("failed to serialize secrets: {error}"))?;
    fs::write(path, json)
        .map_err(|error| format!("failed to write secrets {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn get_secret_from_file(path: &Path, account: &str) -> Result<Option<String>, String> {
    Ok(load_secrets_file(path)?.get(account).cloned())
}

pub fn set_secret_in_file(path: &Path, account: &str, value: &str) -> Result<(), String> {
    let _guard = SECRETS_WRITE_LOCK
        .lock()
        .map_err(|_| "secrets write lock is poisoned".to_string())?;
    let mut secrets = load_secrets_file(path)?;
    secrets.insert(account.to_string(), value.to_string());
    save_secrets_file(path, &secrets)
}

pub fn delete_secret_from_file(path: &Path, account: &str) -> Result<(), String> {
    let _guard = SECRETS_WRITE_LOCK
        .lock()
        .map_err(|_| "secrets write lock is poisoned".to_string())?;
    let mut secrets = load_secrets_file(path)?;
    secrets.remove(account);
    save_secrets_file(path, &secrets)
}

#[cfg(debug_assertions)]
fn secrets_path(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let _ = app;
        Ok(crate::component_runtime::data_dir()?.join(DEV_SECRETS_FILE))
    }
    #[cfg(not(windows))]
    app.path()
        .app_config_dir()
        .map(|dir| dir.join(DEV_SECRETS_FILE))
        .map_err(|error| format!("failed to resolve app config directory: {error}"))
}

#[cfg(not(debug_assertions))]
fn entry_for_username(username: &str, key: &str) -> Result<Entry, String> {
    let account = secret_account(username, key)?;
    Entry::new(KEYRING_SERVICE, &account)
        .map_err(|error| format!("failed to access system keyring: {error}"))
}

#[cfg(not(debug_assertions))]
fn entry_for(app: &AppHandle, key: &str) -> Result<Entry, String> {
    let username = config_cmd::load_config(app.clone())?.username;
    entry_for_username(&username, key)
}

#[tauri::command]
pub fn get_secret(app: AppHandle, key: String) -> Result<Option<String>, String> {
    validate_secret_key(&key)?;
    let username = config_cmd::load_config(app.clone())?.username;
    if username.trim().is_empty() {
        return Ok(None);
    }
    let account = secret_account(&username, &key)?;
    #[cfg(not(debug_assertions))]
    let _ = &account;

    #[cfg(debug_assertions)]
    {
        get_secret_from_file(&secrets_path(&app)?, &account)
    }

    #[cfg(not(debug_assertions))]
    match entry_for_username(&username, &key)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("failed to read secret: {error}")),
    }
}

#[tauri::command]
pub fn set_secret(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let username = config_cmd::load_config(app.clone())?.username;
    let account = secret_account(&username, &key)?;
    #[cfg(not(debug_assertions))]
    let _ = &account;

    #[cfg(debug_assertions)]
    {
        set_secret_in_file(&secrets_path(&app)?, &account, &value)
    }

    #[cfg(not(debug_assertions))]
    entry_for(&app, &key)?
        .set_password(&value)
        .map_err(|error| format!("failed to store secret: {error}"))
}

#[tauri::command]
pub fn delete_secret(app: AppHandle, key: String) -> Result<(), String> {
    let username = config_cmd::load_config(app.clone())?.username;
    let account = secret_account(&username, &key)?;
    #[cfg(not(debug_assertions))]
    let _ = &account;

    #[cfg(debug_assertions)]
    {
        delete_secret_from_file(&secrets_path(&app)?, &account)
    }

    #[cfg(not(debug_assertions))]
    match entry_for(&app, &key)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("failed to delete secret: {error}")),
    }
}

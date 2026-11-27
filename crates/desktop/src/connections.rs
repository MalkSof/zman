//! Connecting AI assistants to the app.
//!
//! Claude Desktop runs local MCP servers listed in its config file; "connect"
//! here means adding one entry that launches this same app with `--mcp`.
//! Nothing else in that file is touched, and a backup is kept.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Flag that makes the app act as a stdio MCP server instead of opening.
pub const MCP_FLAG: &str = "--mcp";
/// The name the app has in Claude's list of servers.
pub const ENTRY: &str = "tuhoravale";
/// The name it had while the app was called zman.
const OLD_ENTRY: &str = "zman";

/// Where Claude Desktop keeps its config, if it is installed.
pub fn claude_config_path() -> Option<PathBuf> {
    claude_config_dir().map(|dir| dir.join("claude_desktop_config.json"))
}

fn claude_config_dir() -> Option<PathBuf> {
    let env_dir = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let mut candidates = Vec::new();
    if let Some(roaming) = env_dir("APPDATA") {
        candidates.push(roaming.join("Claude"));
    }
    // Store (MSIX) installs keep their files in a per-package folder.
    if let Some(local) = env_dir("LOCALAPPDATA") {
        if let Ok(packages) = std::fs::read_dir(local.join("Packages")) {
            for package in packages.flatten() {
                if package.file_name().to_string_lossy().starts_with("Claude_") {
                    candidates.push(package.path().join("LocalCache").join("Roaming").join("Claude"));
                }
            }
        }
    }
    if let Some(home) = env_dir("HOME") {
        candidates.push(home.join("Library").join("Application Support").join("Claude"));
        candidates.push(home.join(".config").join("Claude"));
    }
    candidates.into_iter().find(|dir| dir.is_dir())
}

fn read_config(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        // If the file is not valid JSON we refuse to overwrite it.
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("Claude's config file is not valid JSON: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

pub fn is_connected(path: &Path) -> bool {
    read_config(path).is_ok_and(|config| config["mcpServers"].get(ENTRY).is_some())
}

/// The config with the app's entry added or replaced, and without the one
/// it had under its old name.
fn with_entry(mut config: Value, command: &str) -> Result<Value, String> {
    if !config.is_object() {
        return Err("Claude's config file has an unexpected shape".into());
    }
    if config.get("mcpServers").is_none() {
        config["mcpServers"] = json!({});
    }
    let servers = config["mcpServers"].as_object_mut().ok_or("mcpServers is not an object in Claude's config")?;
    servers.remove(OLD_ENTRY);
    servers.insert(ENTRY.into(), json!({ "command": command, "args": [MCP_FLAG] }));
    Ok(config)
}

/// The config with the app's entry removed, under either name.
fn without_entry(mut config: Value) -> Value {
    if let Some(servers) = config.get_mut("mcpServers").and_then(Value::as_object_mut) {
        servers.remove(ENTRY);
        servers.remove(OLD_ENTRY);
    }
    config
}

fn write_config(path: &Path, config: &Value) -> Result<(), String> {
    // Keep the file as it was before the app first touched it.
    let backup = path.with_extension("json.before-zman");
    if path.exists() && !backup.exists() {
        std::fs::copy(path, &backup).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| e.to_string())
}

pub fn connect(path: &Path, command: &str) -> Result<(), String> {
    write_config(path, &with_entry(read_config(path)?, command)?)
}

pub fn disconnect(path: &Path) -> Result<(), String> {
    write_config(path, &without_entry(read_config(path)?))
}

/// Someone who connected Claude Desktop while the app was called zman keeps
/// it connected: the old entry, which points to where the app used to be,
/// becomes the new one. Does nothing for everyone else.
pub fn adopt_old_entry(path: &Path, command: &str) -> Result<(), String> {
    let config = read_config(path)?;
    if config["mcpServers"].get(OLD_ENTRY).is_none() {
        return Ok(());
    }
    write_config(path, &with_entry(config, command)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_is_added_without_touching_the_rest() {
        let config = json!({
            "theme": "dark",
            "mcpServers": { "other": { "command": "other.exe" } },
        });
        let updated = with_entry(config, r"C:\Apps\tuhoravale.exe").unwrap();
        assert_eq!(updated["theme"], "dark");
        assert_eq!(updated["mcpServers"]["other"]["command"], "other.exe");
        assert_eq!(updated["mcpServers"]["tuhoravale"], json!({ "command": r"C:\Apps\tuhoravale.exe", "args": ["--mcp"] }));

        let removed = without_entry(updated);
        assert!(removed["mcpServers"].get("tuhoravale").is_none());
        assert_eq!(removed["mcpServers"]["other"]["command"], "other.exe");
    }

    #[test]
    fn empty_config_gets_the_section() {
        let updated = with_entry(json!({}), "tuhoravale").unwrap();
        assert_eq!(updated["mcpServers"]["tuhoravale"]["command"], "tuhoravale");
        assert_eq!(without_entry(json!({})), json!({}));
    }

    #[test]
    fn unexpected_shapes_are_refused() {
        assert!(with_entry(json!([1, 2]), "x").is_err());
        assert!(with_entry(json!({ "mcpServers": "nope" }), "x").is_err());
    }

    #[test]
    fn round_trip_on_disk_keeps_a_backup_and_refuses_broken_files() {
        let dir = std::env::temp_dir().join(format!("zman-conn-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("claude_desktop_config.json");

        assert!(!is_connected(&path));
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"o"}}}"#).unwrap();
        connect(&path, "zman-app").unwrap();
        assert!(is_connected(&path));
        let backup = std::fs::read_to_string(path.with_extension("json.before-zman")).unwrap();
        assert!(!backup.contains("zman-app"));
        disconnect(&path).unwrap();
        assert!(!is_connected(&path));
        assert!(std::fs::read_to_string(&path).unwrap().contains("other"));

        std::fs::write(&path, "{ broken").unwrap();
        assert!(connect(&path, "zman-app").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn whoever_connected_the_app_under_its_old_name_stays_connected() {
        let dir = std::env::temp_dir().join(format!("zman-adopt-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("claude_desktop_config.json");

        // Never connected: nothing is written.
        adopt_old_entry(&path, r"C:\New\tuhoravale.exe").unwrap();
        assert!(!path.exists());
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"o"}}}"#).unwrap();
        adopt_old_entry(&path, r"C:\New\tuhoravale.exe").unwrap();
        assert!(!is_connected(&path));

        // Connected as zman, to a place the app no longer is.
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"o"},"zman":{"command":"C:\\Old\\zman-app.exe","args":["--mcp"]}}}"#).unwrap();
        assert!(!is_connected(&path));
        adopt_old_entry(&path, r"C:\New\tuhoravale.exe").unwrap();
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["tuhoravale"]["command"], r"C:\New\tuhoravale.exe");
        assert!(config["mcpServers"].get("zman").is_none());
        assert_eq!(config["mcpServers"]["other"]["command"], "o");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

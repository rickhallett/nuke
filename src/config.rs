//! `~/.config/nuke/config.toml`

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Apps never quit unless named explicitly with `--only`. Names or bundle identifiers.
    pub keep: Vec<String>,
    /// Seconds to wait for graceful quits before giving up (or force-quitting with `force_after`).
    pub timeout: Option<u64>,
    /// If set, force-quit anything still running after `timeout` seconds.
    pub force_after: Option<u64>,
    /// Also quit menu-bar / accessory apps by default.
    pub include_accessory: bool,
}

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("nuke").join("config.toml"))
}

pub fn load() -> Result<Config, String> {
    let Some(path) = path() else {
        return Ok(Config::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Key used to remember an app in `keep`: bundle id when it has one, else its name.
pub fn keep_key(app: &crate::apps::App) -> String {
    app.bundle_id.clone().unwrap_or_else(|| app.name.clone())
}

/// Rewrite one top-level key in the config file, preserving everything else
/// (including comments). Creates the file if it does not exist.
pub fn set(key: &str, value: toml_edit::Item) -> Result<(), String> {
    let Some(path) = path() else {
        return Err("cannot locate config dir".into());
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    doc[key] = value;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, doc.to_string()).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn keep_array(keep: &[String]) -> toml_edit::Item {
    toml_edit::value(toml_edit::Array::from_iter(keep.iter().map(String::as_str)))
}

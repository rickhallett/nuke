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

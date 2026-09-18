//! Deciding which apps to quit. Shared by the CLI and the menu bar app.

use std::collections::HashSet;

use crate::apps::{App, Policy};

/// Always protected, regardless of config. Finder cannot be quit this way anyway.
pub const BUILTIN_KEEP: &[&str] = &["com.apple.finder"];

pub struct Plan<'a> {
    /// Names or bundle ids to leave alone (config `keep` + `--except`).
    pub keep: Vec<&'a str>,
    /// If non-empty, quit exactly these and ignore `keep`.
    pub only: &'a [String],
    pub include_accessory: bool,
    pub ancestors: &'a HashSet<i32>,
}

impl Plan<'_> {
    /// True if this app would be quit under the plan. `self_pid` and the
    /// ancestor chain are never targets, whatever the flags say.
    pub fn targets(&self, app: &App) -> bool {
        let self_pid = std::process::id() as i32;
        if app.pid == self_pid || self.ancestors.contains(&app.pid) {
            return false;
        }
        if !self.only.is_empty() {
            return self.only.iter().any(|o| app.matches(o));
        }
        let visible = match app.policy {
            Policy::Regular => true,
            Policy::Accessory => self.include_accessory,
            Policy::Prohibited => false,
        };
        visible
            && !BUILTIN_KEEP
                .iter()
                .chain(self.keep.iter())
                .any(|k| app.matches(k))
    }

    /// True if this app is one the user could plausibly want listed in a keep list.
    pub fn candidate(&self, app: &App) -> bool {
        let self_pid = std::process::id() as i32;
        app.pid != self_pid
            && !self.ancestors.contains(&app.pid)
            && !BUILTIN_KEEP.iter().any(|k| app.matches(k))
            && match app.policy {
                Policy::Regular => true,
                Policy::Accessory => self.include_accessory,
                Policy::Prohibited => false,
            }
    }
}

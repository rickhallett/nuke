//! Thin wrapper over AppKit's `NSWorkspace` / `NSRunningApplication`.

use std::collections::HashSet;
use std::mem::MaybeUninit;
use std::thread;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};

/// One running application, as seen by the window server.
pub struct App {
    handle: Retained<NSRunningApplication>,
    pub pid: i32,
    pub name: String,
    pub bundle_id: Option<String>,
    pub policy: Policy,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Policy {
    /// Ordinary app with a Dock icon.
    Regular,
    /// Menu-bar / background agent (no Dock icon).
    Accessory,
    /// Cannot be activated at all (daemons, XPC services).
    Prohibited,
}

impl Policy {
    pub fn label(self) -> &'static str {
        match self {
            Policy::Regular => "regular",
            Policy::Accessory => "accessory",
            Policy::Prohibited => "background",
        }
    }
}

impl App {
    fn from_handle(handle: Retained<NSRunningApplication>) -> Self {
        let pid = handle.processIdentifier();
        let name = handle
            .localizedName()
            .map(|s| clean_name(&s.to_string()))
            .unwrap_or_else(|| format!("pid {pid}"));
        let bundle_id = handle.bundleIdentifier().map(|s| s.to_string());
        let policy = match handle.activationPolicy() {
            NSApplicationActivationPolicy::Regular => Policy::Regular,
            NSApplicationActivationPolicy::Accessory => Policy::Accessory,
            _ => Policy::Prohibited,
        };
        Self {
            handle,
            pid,
            name,
            bundle_id,
            policy,
        }
    }

    /// Ask the app to quit (equivalent to ⌘Q). Returns false if the request could not be sent.
    pub fn terminate(&self) -> bool {
        self.handle.terminate()
    }

    /// Kill the app without giving it a chance to save. Returns false if the request could not be sent.
    pub fn force_terminate(&self) -> bool {
        self.handle.forceTerminate()
    }

    /// Whether the process is still alive.
    ///
    /// `NSRunningApplication.terminated` is only refreshed by the main run
    /// loop, which a CLI never pumps, so ask the kernel directly instead.
    pub fn is_running(&self) -> bool {
        // SAFETY: signal 0 performs no action, only an existence/permission check.
        let rc = unsafe { libc::kill(self.pid, 0) };
        rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    /// The app's icon, if it has a bundle with one.
    pub fn icon(&self) -> Option<Retained<objc2_app_kit::NSImage>> {
        self.handle.icon()
    }

    /// True if `needle` matches this app's name (case-insensitive) or bundle identifier.
    pub fn matches(&self, needle: &str) -> bool {
        self.name.eq_ignore_ascii_case(needle)
            || self
                .bundle_id
                .as_deref()
                .is_some_and(|b| b.eq_ignore_ascii_case(needle))
    }
}

/// Strip the zero-width / bidi marks some apps (WhatsApp) put in their names,
/// so they sort and match like the visible text.
fn clean_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Every application the workspace knows about, sorted by name.
pub fn running() -> Vec<App> {
    let workspace = NSWorkspace::sharedWorkspace();
    let mut apps: Vec<App> = workspace
        .runningApplications()
        .to_vec()
        .into_iter()
        .map(App::from_handle)
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Block until every app in `apps` reports terminated, or `timeout` elapses.
/// Returns the apps that are still running.
pub fn wait_for_exit<'a>(apps: &[&'a App], timeout: Duration) -> Vec<&'a App> {
    let deadline = Instant::now() + timeout;
    loop {
        let alive: Vec<&App> = apps.iter().copied().filter(|a| a.is_running()).collect();
        if alive.is_empty() || Instant::now() >= deadline {
            return alive;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// PIDs of every ancestor of this process, nearest first.
///
/// Used to find the terminal emulator (or editor) we were launched from so we
/// never quit the app that owns our own stdout.
pub fn ancestor_pids() -> HashSet<i32> {
    let mut seen = HashSet::new();
    let mut pid = std::process::id() as i32;
    while pid > 1 {
        match parent_pid(pid) {
            Some(ppid) if seen.insert(ppid) => pid = ppid,
            _ => break,
        }
    }
    seen
}

fn parent_pid(pid: i32) -> Option<i32> {
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size = size_of::<libc::proc_bsdinfo>() as i32;
    // SAFETY: the buffer is exactly `size` bytes of proc_bsdinfo, which is
    // what PROC_PIDTBSDINFO fills; the kernel returns the bytes written.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(info.pbi_ppid as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestors_include_parent_and_stop_at_launchd() {
        let ancestors = ancestor_pids();
        let ppid = unsafe { libc::getppid() };
        assert!(ancestors.contains(&ppid));
        assert!(!ancestors.contains(&0));
        assert!(!ancestors.contains(&(std::process::id() as i32)));
    }

    #[test]
    fn clean_name_strips_bidi_marks() {
        assert_eq!(clean_name("\u{200E}WhatsApp"), "WhatsApp");
        assert_eq!(clean_name("  Mail "), "Mail");
    }
}

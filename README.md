# quitall

Quit every running macOS app from the command line — a free CLI stand-in for
"Quit All"-style menu bar apps.

It talks to AppKit's `NSWorkspace` / `NSRunningApplication` directly (no
AppleScript, no Accessibility permission), so a plain `quitall` is exactly
like pressing ⌘Q in each app: apps with unsaved changes show their own save
dialog and stay open until you answer it.

## Usage

```
quitall                  # quit every Dock app except the protected ones
quitall -n               # dry run: show what would be quit
quitall -l               # list running apps (name, kind, pid, bundle id)
quitall -x Safari,Mail   # leave these running as well
quitall -o Slack,Zoom    # quit only these
quitall -a               # also quit menu-bar / accessory apps
quitall --force-after 10 # ask nicely, force-quit whatever is left after 10s
quitall -f               # force-quit immediately (unsaved work is lost)
quitall --no-wait        # fire the quit requests and return at once
```

Apps can be named by their visible name (case-insensitive) or bundle
identifier (`com.apple.mail`).

Always protected, even without config:

- Finder
- the terminal, editor or launcher `quitall` was started from (found by
  walking up the parent-process chain), so it never pulls the rug out from
  under its own shell

Exit status is 0 when everything quit, 1 if any app refused or is still
running after the timeout, 2 on a bad config file.

## Config

`~/.config/quitall/config.toml` (or `$XDG_CONFIG_HOME/quitall/config.toml`):

```toml
# Never quit these unless named explicitly with --only.
keep = ["1Password", "Ghostty", "com.google.Chrome"]

# Seconds to wait for apps to exit before reporting them (default 5).
timeout = 5

# Force-quit anything still running after this many seconds. Off by default.
# force_after = 15

# Treat menu-bar apps like Dock apps by default.
include_accessory = false
```

## Build / install

```
cargo build --release
cp target/release/quitall ~/.local/bin/
```

Requires macOS; builds on stable Rust.

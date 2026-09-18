Universal binaries (Apple Silicon + Intel), ad-hoc signed.

| you are | download | then |
|---------|----------|------|
| a terminal person | `nuke-*-macos-universal.tar.gz` | drop `nuke` on your `PATH` (or just `brew install rickhallett/tap/nuke`) |
| a muggle | `Nuke-*-macos-universal.zip` | unzip, drag `Nuke.app` to Applications, right-click → Open the first time |

There is no Apple Developer ID behind this, so Gatekeeper will sulk once.
Right-click → Open, or `xattr -d com.apple.quarantine`, and it will never
mention it again. `SHA256SUMS` is there if you'd like to check the payload
before detonation.

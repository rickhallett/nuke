#!/bin/sh
# Wrap the nuke binary in a Nuke.app bundle for people who don't do terminals.
#
#   scripts/bundle-app.sh <path-to-nuke-binary> <version> <output-dir>
#
# The bundle's executable IS the CLI binary; with no arguments and a
# Contents/MacOS parent it starts in menu bar mode. LSUIElement keeps it
# out of the Dock. Ad-hoc signed so Apple Silicon will run it at all.
set -eu

bin=$1
version=$2
out=$3
app="$out/Nuke.app"
here=$(cd "$(dirname "$0")" && pwd)

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/nuke"
chmod 755 "$app/Contents/MacOS/nuke"

sh "$here/make-icns.sh" "$here/../assets/logo.png" "$app/Contents/Resources/nuke.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>                 <string>Nuke</string>
  <key>CFBundleDisplayName</key>          <string>Nuke</string>
  <key>CFBundleIdentifier</key>           <string>dev.rickhallett.nuke</string>
  <key>CFBundleVersion</key>              <string>$version</string>
  <key>CFBundleShortVersionString</key>   <string>$version</string>
  <key>CFBundlePackageType</key>          <string>APPL</string>
  <key>CFBundleExecutable</key>           <string>nuke</string>
  <key>CFBundleIconFile</key>             <string>nuke</string>
  <key>LSMinimumSystemVersion</key>       <string>11.0</string>
  <key>LSUIElement</key>                  <true/>
  <key>NSHumanReadableCopyright</key>     <string>MIT. Now I am become ⌘Q.</string>
</dict>
</plist>
PLIST

printf 'APPL????' > "$app/Contents/PkgInfo"
codesign --force --sign - --identifier dev.rickhallett.nuke "$app"
echo "$app"

#!/usr/bin/env bash
set -euo pipefail

profile="debug"
cargo_args=()

if [[ "${1:-}" == "--release" ]]; then
  profile="release"
  cargo_args+=(--release)
elif [[ $# -gt 0 ]]; then
  printf 'usage: %s [--release]\n' "$0" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app_name="naite"
bundle_dir="$repo_root/target/$profile/$app_name.app"
contents_dir="$bundle_dir/Contents"
macos_dir="$contents_dir/MacOS"
resources_dir="$contents_dir/Resources"
binary_path="$repo_root/target/$profile/naite"

cd "$repo_root"
package_version="$(cargo pkgid -p naite-app | sed 's/.*#//')"
bundle_short_version="${NAITE_BUNDLE_SHORT_VERSION:-$package_version}"
bundle_version="${NAITE_BUNDLE_VERSION:-$package_version}"

cargo build -p naite-app ${cargo_args+"${cargo_args[@]}"}

rm -rf "$bundle_dir"
mkdir -p "$macos_dir" "$resources_dir"

cp "$binary_path" "$macos_dir/naite"
cp "$repo_root/crates/naite-app/assets/app-icon.icns" "$resources_dir/app-icon.icns"

cat > "$contents_dir/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>naite</string>
  <key>CFBundleExecutable</key>
  <string>naite</string>
  <key>CFBundleIconFile</key>
  <string>app-icon</string>
  <key>CFBundleIdentifier</key>
  <string>dev.naite.app</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>naite</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$bundle_short_version</string>
  <key>CFBundleVersion</key>
  <string>$bundle_version</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

printf '%s\n' "$bundle_dir"

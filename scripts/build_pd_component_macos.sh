#!/bin/bash
# Local macOS x86_64 component build. No installation, keys, catalog merge or upload.
#
# Mirrors scripts/build_pd_component.ps1 (Windows). The Tauri program keeps its
# Cargo name; the packager copies it into the archive as `paldee-pet`, and the
# helper knows it only by that entry name.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
bridge_root="${1:?usage: build_pd_component_macos.sh <bridge-root> [output-directory]}"
output="${2:-}"

[[ "$(uname -s)" == "Darwin" ]] || { echo "macOS is required" >&2; exit 2; }
[[ "$(uname -m)" == "x86_64" ]] || { echo "native x86_64 runner required" >&2; exit 2; }
[[ -f "$bridge_root/native/Cargo.toml" ]] || { echo "bridge root not found: $bridge_root" >&2; exit 2; }

cd "$root"

npm run tauri build -- --no-bundle
cargo build --release --locked --target x86_64-apple-darwin --manifest-path "$bridge_root/native/Cargo.toml"

arguments=(
  scripts/package_pd_component_macos.py
  --pet src-tauri/target/release/paldee-pet
  --bridge "$bridge_root/native/target/x86_64-apple-darwin/release/pd-device-bridge"
  --bridge-root "$bridge_root"
)
if [[ -n "$output" ]]; then
  arguments+=(--output "$output")
fi
python3 "${arguments[@]}"

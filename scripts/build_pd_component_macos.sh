#!/bin/bash
# Local macOS component build (x86_64 or arm64). No installation, keys, catalog
# merge or upload.
#
# Mirrors scripts/build_pd_component.ps1 (Windows). The Tauri program keeps its
# Cargo name; the packager copies it into the archive as `paldee-pet`, and the
# helper knows it only by that entry name.
#
# Usage:
#   build_pd_component_macos.sh <bridge-root> [output-directory] [arch]
#   arch defaults to the runner's native arch (x86_64 or arm64).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
bridge_root="${1:?usage: build_pd_component_macos.sh <bridge-root> [output-directory] [arch]}"
output="${2:-}"
arch="${3:-}"

[[ "$(uname -s)" == "Darwin" ]] || { echo "macOS is required" >&2; exit 2; }
[[ -f "$bridge_root/native/Cargo.toml" ]] || { echo "bridge root not found: $bridge_root" >&2; exit 2; }

# Resolve the requested arch: explicit arg, else the runner's native arch.
native="$(uname -m)"
if [[ -z "$arch" ]]; then
  case "$native" in
    x86_64) arch="x86_64" ;;
    arm64)  arch="arm64"  ;;
    *) echo "unsupported native arch: $native" >&2; exit 2 ;;
  esac
fi
case "$arch" in
  x86_64) target="x86_64-apple-darwin" ;;
  arm64)  target="arm64-apple-darwin"  ;;
  *) echo "unsupported arch: $arch (use x86_64 or arm64)" >&2; exit 2 ;;
esac

# Cross-compiling needs the rust target installed; fail early with a clear hint.
if ! rustc --print target-list | grep -qx "$target"; then
  echo "rust target $target not available; install with: rustup target add $target" >&2
  exit 2
fi

cd "$root"

# Build the pet binary for the requested target. `tauri build` ships the frontend
# plus the Rust binary; --no-bundle skips the macOS installer packaging.
npm run tauri build -- --target "$target" --no-bundle
cargo build --release --locked --target "$target" --manifest-path "$bridge_root/native/Cargo.toml"

arguments=(
  scripts/package_pd_component_macos.py
  --platform "macos-$arch"
  --pet "src-tauri/target/$target/release/paldee-pet"
  --bridge "$bridge_root/native/target/$target/release/pd-device-bridge"
  --bridge-root "$bridge_root"
)
if [[ -n "$output" ]]; then
  arguments+=(--output "$output")
fi
python3 "${arguments[@]}"

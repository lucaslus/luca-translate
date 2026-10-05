#!/usr/bin/env bash
# Build and bundle the Omarchy variant without the desktop app or its release pipeline.
set -euo pipefail
task_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$task_root"
cargo build --release --locked --manifest-path crates/lucas-omarchy/Cargo.toml
version=$(python3 -c 'import json; print(json.load(open("omarchy/manifest.json"))["version"])')
out="$task_root/dist/omarchy"
mkdir -p "$out"
stage=$(mktemp -d "$out/build.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
bundle="$stage/lucas-translate-omarchy"
mkdir -p "$bundle/omarchy" "$bundle/bin" "$bundle/scripts"
tar --exclude=tests --exclude=__pycache__ --exclude='*.pyc' -cf - -C omarchy . | tar -xf - -C "$bundle/omarchy"
install -m755 crates/lucas-omarchy/target/release/lucas-translate-omarchy-backend "$bundle/bin/"
install -m755 scripts/install-omarchy-native.py "$bundle/scripts/"
tar -czf "$out/lucas-translate-omarchy-$version-$(uname -m).tar.gz" -C "$stage" lucas-translate-omarchy
echo "$out/lucas-translate-omarchy-$version-$(uname -m).tar.gz"

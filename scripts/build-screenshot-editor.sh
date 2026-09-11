#!/usr/bin/env bash
# Build a separate clipboard-only editor; never replace the system Tensaku.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$root/target/screenshot-editor/source"
revision=4bcd8339bc22e74bd81c9f3efdfdfcbcf461db70
patch_file="$root/packaging/omarchy/editor/clipboard-controls.patch"
if [[ ! -d "$source_dir/.git" ]]; then
  mkdir -p "$(dirname "$source_dir")"
  git clone --depth 1 --branch v0.29.0 https://github.com/jondkinney/tensaku.git "$source_dir"
fi
[[ $(git -C "$source_dir" rev-parse HEAD) == "$revision" ]] || {
  echo 'Unexpected screenshot editor source revision' >&2
  exit 1
}
if ! git -C "$source_dir" apply --reverse --check "$patch_file" 2>/dev/null; then
  git -C "$source_dir" apply --check "$patch_file"
  git -C "$source_dir" apply "$patch_file"
fi
cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"

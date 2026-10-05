#!/usr/bin/env bash
set -euo pipefail
plugin_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if [[ -n ${LUCAS_NATIVE_BACKEND:-} && -x ${LUCAS_NATIVE_BACKEND} ]]; then
  exec "$LUCAS_NATIVE_BACKEND" --stdio --plugin-dir "$plugin_dir"
fi
user_backend="$HOME/.local/bin/lucas-translate-omarchy-backend"
if [[ -x $user_backend ]]; then
  exec "$user_backend" --stdio --plugin-dir "$plugin_dir"
fi
exec lucas-translate-omarchy-backend --stdio --plugin-dir "$plugin_dir"

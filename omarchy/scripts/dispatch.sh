#!/usr/bin/env bash
# Select the native host only inside an Omarchy graphical session.
set -euo pipefail
if [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} && -n ${OMARCHY_PATH:-} && -f $OMARCHY_PATH/shell/shell.qml ]] \
    && command -v omarchy-shell >/dev/null \
    && omarchy-shell shell ping >/dev/null 2>&1; then
  exec "$HOME/.local/bin/lucas-translate-native" "$@"
fi
for desktop_binary in /usr/bin/lucas-translate /usr/local/bin/lucas-translate; do
  if [[ -x $desktop_binary ]]; then exec "$desktop_binary" "$@"; fi
done
echo 'The desktop Lucas Translate executable is unavailable.' >&2
exit 1

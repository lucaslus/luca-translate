-- Lucas Translate: Omarchy / Hyprland Lua integration.
-- Super+Ctrl+Shift: T toggle, D selection, S translation, C OCR, P screenshot annotation.
o.bind("SUPER + CTRL + SHIFT + T", "Translate: show / hide", "lucas-translate --toggle")
o.bind("SUPER + CTRL + SHIFT + D", "Translate: selection", "lucas-translate --selection")
o.bind("SUPER + CTRL + SHIFT + S", "Translate: screenshot", "lucas-translate --screenshot")
o.bind("SUPER + CTRL + SHIFT + C", "Translate: OCR to clipboard", "lucas-translate --ocr")
o.bind("SUPER + CTRL + SHIFT + P", "Translate: screenshot and annotate", function()
  for _, window in ipairs(hl.get_windows()) do
    if window.class == "lucas-translate" and window.title == "Lucas Translate" and window.mapped then
      hl.dispatch(hl.dsp.window.close({ window = "address:" .. window.address }))
    end
  end
  -- Let the hide animation finish before the system picker freezes the screen.
  -- Runs outside the compositor event loop; also works when Translate is not running.
  -- Keep the capture temporary; Enter copies and closes without saving a file.
  hl.exec_cmd([[sleep 0.2
capture_dir=$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/lucas-capture.XXXXXX") || exit 1
trap 'rm -rf -- "$capture_dir"' EXIT
capture=$(OMARCHY_SCREENSHOT_DIR="$capture_dir" omarchy capture screenshot region save) || exit 1
[ -n "$capture" ] && [ -f "$capture" ] || exit 0
lucas-screenshot-editor --title "Lucas Screenshot" --filename "$capture" --actions-on-enter save-to-clipboard --early-exit --actions-on-escape exit --copy-command wl-copy --disable-notifications]])
end)
o.window("lucas-translate", { float = true, center = true })
o.window({ class = "lucas-translate", title = "Lucas Translate" }, {
  size = { 420, 650 },
})

-- Observe presses without consuming them. Pointer motion/focus changes do nothing.
local function dismiss_outside()
  local cursor = hl.get_cursor_pos()
  for _, window in ipairs(hl.get_windows()) do
    local is_panel = window.class == "lucas-translate" and window.title == "Lucas Translate"
    local is_editor = window.class == "dev.tensaku.Tensaku" and window.title == "Lucas Screenshot"
    if (is_panel or is_editor) and window.mapped and window.visible then
      local at, size = window.at, window.size
      if cursor.x < at.x or cursor.x >= at.x + size.x
          or cursor.y < at.y or cursor.y >= at.y + size.y then
        -- CloseRequested hides the panel or discards the dedicated screenshot editor.
        hl.dispatch(hl.dsp.window.close({ window = "address:" .. window.address }))
      end
    end
  end
end
for _, button in ipairs({ 272, 273, 274 }) do
  o.bind("mouse:" .. button, "Translate: dismiss outside", dismiss_outside, {
    non_consuming = true,
    ignore_mods = true,
    transparent = true,
  })
end

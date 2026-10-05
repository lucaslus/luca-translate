-- Lucas Translate: Omarchy / Hyprland Lua integration.
-- Edit shortcuts in Lucas Translate Settings; empty values disable a binding.
-- Keep normal compositor bindings out of the focused shortcut editor. The lease
-- expires if the webview stops responding; focus loss restores the previous map.
local recording_map = "lucas-translate-recording"
local recording, recording_timer
local function recording_window(pid)
  local window = hl.get_active_window()
  return window and window.class == "lucas-translate"
    and window.title == "Lucas Translate 偏好设置" and window.pid == pid
end
local function stop_recording()
  local previous = recording
  recording = nil
  if recording_timer then recording_timer:set_enabled(false) end
  if previous and hl.get_current_submap() == recording_map then
    hl.dispatch(hl.dsp.submap(previous.submap == "" and "reset" or previous.submap))
  end
end
hl.define_submap(recording_map, function()
  hl.bind("Escape", stop_recording, {
    description = "Lucas Translate: finish shortcut recording",
    non_consuming = true, ignore_mods = true,
  })
end)
recording_timer = hl.timer(function()
  if recording and (os.time() >= recording.expires or not recording_window(recording.pid)
      or hl.get_current_submap() ~= recording_map) then stop_recording() end
end, { timeout = 250, type = "repeat" })
recording_timer:set_enabled(false)
hl.on("window.active", function()
  if recording and not recording_window(recording.pid) then stop_recording() end
end)
-- Called only by the settings IPC with a unique token per focused input.
function lucas_translate_shortcut_recording(active, token, pid)
  if not active then
    if recording and recording.token == token then stop_recording() end
    return true
  end
  if not recording_window(pid) then return false end
  if not recording then
    local previous = hl.get_current_submap()
    recording = { submap = previous == recording_map and "" or previous }
  end
  recording.token, recording.pid, recording.expires = token, pid, os.time() + 4
  hl.dispatch(hl.dsp.submap(recording_map))
  recording_timer:set_enabled(true)
  return true
end
-- A config reload must not leave an obsolete recording session active.
if hl.get_current_submap() == recording_map then hl.dispatch(hl.dsp.submap("reset")) end
-- BEGIN MANAGED SHORTCUTS
local shortcuts = {
  input = "SUPER + CTRL + SHIFT + I",
  toggle = "SUPER + CTRL + SHIFT + T",
  selection = "SUPER + CTRL + SHIFT + D",
  screenshot = "SUPER + CTRL + SHIFT + S",
  ocr = "SUPER + CTRL + SHIFT + C",
  annotate = "SUPER + CTRL + SHIFT + P",
}
-- END MANAGED SHORTCUTS
local function bind(action, description, command)
  if shortcuts[action] and shortcuts[action] ~= "" then
    o.bind(shortcuts[action], description, command)
  end
end
bind("input", "Translate: new input", "lucas-translate --input")
bind("toggle", "Translate: show / hide", "lucas-translate --toggle")
bind("selection", "Translate: selection", "lucas-translate --selection")
bind("screenshot", "Translate: screenshot", "lucas-translate --screenshot")
bind("ocr", "Translate: OCR to clipboard", "lucas-translate --ocr")
bind("annotate", "Translate: screenshot and annotate", function()
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

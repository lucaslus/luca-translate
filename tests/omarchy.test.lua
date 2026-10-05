-- Exercise the actual integration with a synthetic compositor, never real clicks.
local bindings, windows, closed = {}, {}, {}
local screenshot, launched
local current_submap, active_window, timer_callback, escape_callback = "", nil, nil, nil
local events, timer_enabled = {}, false
local cursor = { x = 100, y = 100 }
o = {
  window = function() end,
  bind = function(key, _, callback, options)
    if key == "SUPER + CTRL + SHIFT + P" then screenshot = callback end
    if key:find("mouse:", 1, true) then
      assert(options.non_consuming and options.ignore_mods and options.transparent)
      bindings[#bindings + 1] = callback
    end
  end,
}
hl = {
  get_active_window = function() return active_window end,
  get_current_submap = function() return current_submap end,
  define_submap = function(name, callback) assert(name == "lucas-translate-recording"); callback() end,
  bind = function(key, callback, opts)
    assert(key == "Escape" and opts.non_consuming and opts.ignore_mods)
    escape_callback = callback
  end,
  on = function(event, callback) events[event] = callback end,
  timer = function(callback, opts)
    assert(opts.type == "repeat" and opts.timeout == 250)
    timer_callback = callback
    return { set_enabled = function(_, enabled) timer_enabled = enabled end }
  end,
  get_cursor_pos = function() return cursor end,
  get_windows = function() return windows end,
  dispatch = function(command)
    if type(command) == "function" then return command() end
    closed[#closed + 1] = command
  end,
  exec_cmd = function(command) launched = command end,
  dsp = {
    submap = function(name) return function() current_submap = name == "reset" and "" or name end end,
    window = { close = function(args) return args.window end },
  },
}
dofile("packaging/omarchy/lucas-translate.lua")
assert(#bindings == 3)
local panel = {
  class = "lucas-translate", title = "Lucas Translate", address = "0x123",
  mapped = true, visible = true, at = { x = -100, y = 50 }, size = { x = 420, y = 650 },
}
windows = { panel }
for _, click in ipairs(bindings) do click() end
assert(#closed == 0, "inside clicks must not hide")
cursor = { x = 320, y = 100 }
assert(#closed == 0, "moving outside must not hide")
bindings[1]()
assert(#closed == 1 and closed[1] == "address:0x123", "outside press closes exact panel")
panel.visible = false
bindings[2]()
assert(#closed == 1, "hidden workspace must not trigger dismissal")
panel.visible = true
panel.mapped = false
bindings[3]()
assert(#closed == 1, "unmapped panel must not trigger dismissal")
panel.mapped = true
panel.title = "Settings"
bindings[1]()
assert(#closed == 1, "settings must not be closed")
windows = {}
bindings[1]()
assert(#closed == 1, "clicks must not launch a missing app")
print("PASS Omarchy outside-click dismissal, passthrough flags and hidden-window guards")
panel.title = "Lucas Translate"
windows = { panel }
screenshot()
assert(closed[#closed] == "address:0x123", "hide the panel before capture")
assert(launched:find('OMARCHY_SCREENSHOT_DIR="$capture_dir"', 1, true), "capture stays in temporary directory")
assert(launched:find("trap 'rm -rf --", 1, true), "temporary capture is cleaned up")
assert(launched:find('--actions-on-enter save-to-clipboard --early-exit', 1, true), "Enter copies and closes")
assert(launched:find('--disable-notifications', 1, true), "no save notification")
assert(not launched:find('--save-after-copy', 1, true), "copy must not save")
local count = #closed
windows = {}
launched = nil
screenshot()
assert(#closed == count and launched, "native capture works without starting Translate")
print("PASS Omarchy native screenshot opens annotation without OCR or translation")

-- Only this integration's editor is dismissed; ordinary Tensaku windows stay open.
local editor = {
  class = "dev.tensaku.Tensaku", title = "Lucas Screenshot", address = "0x456",
  mapped = true, visible = true, at = { x = 0, y = 0 }, size = { x = 600, y = 400 },
}
windows = { editor }
cursor = { x = 300, y = 100 }
local before = #closed
bindings[1]()
assert(#closed == before, "editing inside must not dismiss")
cursor = { x = 700, y = 100 }
bindings[1]()
assert(#closed == before + 1 and closed[#closed] == "address:0x456", "outside click discards exact editor")
editor.visible = false
bindings[1]()
assert(#closed == before + 1, "editor on a hidden workspace stays open")
editor.visible = true
editor.title = "Tensaku"
bindings[1]()
assert(#closed == before + 1, "ordinary Tensaku windows stay open")
print("PASS dedicated screenshot editor outside-click cancellation")

-- Recording protection is leased to the exact settings window and session token.
assert(not lucas_translate_shortcut_recording(true, "first", 123))
active_window = { class = "lucas-translate", title = "Lucas Translate 偏好设置", pid = 123 }
current_submap = "user-mode"
assert(lucas_translate_shortcut_recording(true, "first", 123))
assert(current_submap == "lucas-translate-recording" and timer_enabled)
assert(lucas_translate_shortcut_recording(true, "second", 123))
lucas_translate_shortcut_recording(false, "first", 123)
assert(current_submap == "lucas-translate-recording", "stale input blur cannot end a newer lease")
escape_callback()
assert(current_submap == "user-mode" and not timer_enabled, "Escape restores the previous map")
assert(lucas_translate_shortcut_recording(true, "third", 123))
active_window = nil
events["window.active"]()
assert(current_submap == "user-mode" and not timer_enabled, "focus loss restores shortcuts immediately")
active_window = { class = "lucas-translate", title = "Lucas Translate 偏好设置", pid = 123 }
assert(lucas_translate_shortcut_recording(true, "fourth", 123))
local original_time = os.time
os.time = function() return original_time() + 10 end
timer_callback()
os.time = original_time
assert(current_submap == "user-mode" and not timer_enabled, "hung webview cannot leave global keys disabled")
assert(lucas_translate_shortcut_recording(true, "fifth", 123))
current_submap = "another-mode"
timer_callback()
assert(current_submap == "another-mode", "never overwrite a subsequent desktop mode")
print("PASS shortcut recording lease, stale tokens, Escape, focus loss, timeout and other submaps")

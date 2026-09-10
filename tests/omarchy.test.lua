-- Exercise the actual integration with a synthetic compositor, never real clicks.
local bindings, windows, closed = {}, {}, {}
local screenshot, launched
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
  get_cursor_pos = function() return cursor end,
  get_windows = function() return windows end,
  dispatch = function(command) closed[#closed + 1] = command end,
  exec_cmd = function(command) launched = command end,
  dsp = { window = { close = function(args) return args.window end } },
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

-- Lucas Translate: Omarchy / Hyprland Lua integration.
-- Super+Ctrl+Shift: T toggle, D selection, S translation, C OCR, P image copy.
o.bind("SUPER + CTRL + SHIFT + T", "Translate: show / hide", "lucas-translate --toggle")
o.bind("SUPER + CTRL + SHIFT + D", "Translate: selection", "lucas-translate --selection")
o.bind("SUPER + CTRL + SHIFT + S", "Translate: screenshot", "lucas-translate --screenshot")
o.bind("SUPER + CTRL + SHIFT + C", "Translate: OCR to clipboard", "lucas-translate --ocr")
o.bind("SUPER + CTRL + SHIFT + P", "Translate: screenshot to clipboard", function()
  for _, window in ipairs(hl.get_windows()) do
    if window.class == "lucas-translate" and window.title == "Lucas Translate" and window.mapped then
      hl.dispatch(hl.dsp.window.close({ window = "address:" .. window.address }))
    end
  end
  -- Let the hide animation finish before the system picker freezes the screen.
  -- Runs outside the compositor event loop; also works when Translate is not running.
  hl.exec_cmd("sleep 0.2; exec omarchy capture screenshot region copy")
end)
o.window("lucas-translate", { float = true, center = true })
o.window({ class = "lucas-translate", title = "Lucas Translate" }, {
  size = { 420, 650 },
})

-- Observe presses without consuming them. Pointer motion/focus changes do nothing.
local function dismiss_outside()
  local cursor = hl.get_cursor_pos()
  for _, window in ipairs(hl.get_windows()) do
    if window.class == "lucas-translate" and window.title == "Lucas Translate"
        and window.mapped and window.visible then
      local at, size = window.at, window.size
      if cursor.x < at.x or cursor.x >= at.x + size.x
          or cursor.y < at.y or cursor.y >= at.y + size.y then
        -- CloseRequested hides the existing panel; never launch a process on clicks.
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

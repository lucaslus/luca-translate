-- Lucas Translate native Shell plugin. Managed by the native settings panel.
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
local actions = {
  input = "Translate: new input",
  toggle = "Translate: show / hide",
  selection = "Translate: selection",
  screenshot = "Translate: screenshot",
  ocr = "Translate: OCR to clipboard",
  annotate = "Translate: screenshot and annotate",
}
for action, description in pairs(actions) do
  if shortcuts[action] and shortcuts[action] ~= "" then
    o.bind(shortcuts[action], description, "lucas-translate-native --" .. action)
  end
end

-- Preserve the system editor's previous outside-click cancellation.
local function dismiss_screenshot_editor()
  local cursor = hl.get_cursor_pos()
  for _, window in ipairs(hl.get_windows()) do
    if window.class == "dev.tensaku.Tensaku" and window.title == "Lucas Screenshot"
        and window.mapped and window.visible then
      local at, size = window.at, window.size
      if cursor.x < at.x or cursor.x >= at.x + size.x
          or cursor.y < at.y or cursor.y >= at.y + size.y then
        hl.dispatch(hl.dsp.window.close({ window = "address:" .. window.address }))
      end
    end
  end
end
for _, button in ipairs({ 272, 273, 274 }) do
  o.bind("mouse:" .. button, "Translate: dismiss screenshot editor", dismiss_screenshot_editor, {
    non_consuming = true, ignore_mods = true, transparent = true,
  })
end

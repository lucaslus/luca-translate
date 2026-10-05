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

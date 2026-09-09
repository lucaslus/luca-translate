-- Lucas Translate: Omarchy / Hyprland Lua integration.
-- Super+Ctrl+Shift: T toggle, D selection, S screenshot, C silent OCR.
o.bind("SUPER + CTRL + SHIFT + T", "Translate: show / hide", "lucas-translate --toggle")
o.bind("SUPER + CTRL + SHIFT + D", "Translate: selection", "lucas-translate --selection")
o.bind("SUPER + CTRL + SHIFT + S", "Translate: screenshot", "lucas-translate --screenshot")
o.bind("SUPER + CTRL + SHIFT + C", "Translate: OCR to clipboard", "lucas-translate --ocr")
o.window("lucas-translate", { float = true, center = true })
o.window({ class = "lucas-translate", title = "Lucas Translate" }, {
  size = { 420, 650 },
})

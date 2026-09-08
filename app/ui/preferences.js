(() => {
  window.__TAURI__.app?.getVersion().then((version) => { window.LucasVersion = version; }).catch(() => {});
  const defaults = {
    font_size: 14,
    shortcuts: {
      input: "Alt+A",
      selection: "Alt+D",
      screenshot: "Alt+S",
      ocr: "Alt+C",
    },
  };
  let current = structuredClone(defaults);
  const shortcut = (action) => {
    const value = current.shortcuts[action] || "未设置";
    return navigator.userAgent.includes("Mac")
      ? value
          .replace(/Alt\+/g, "⌥")
          .replace(/Super\+/g, "⌘")
          .replace(/Ctrl\+/g, "⌃")
          .replace(/Shift\+/g, "⇧")
      : value;
  };
  async function refresh() {
    const data = await window.Lucas.invoke("get_preferences");
    if (!data?.preferences) throw Error("暂时无法读取偏好设置");
    current = data.preferences;
    document.documentElement.style.setProperty(
      "--reading-size",
      current.font_size + "px",
    );
    document.querySelectorAll("[data-shortcut]").forEach((node) => {
      node.textContent = shortcut(node.dataset.shortcut);
    });
    return data;
  }
  window.LucasPreferences = { refresh, shortcut, defaults };
  refresh().catch(() => {});
  window.__TAURI__.event
    .listen("lucas://preferences-changed", () => refresh().catch(() => {}))
    .catch(() => {});
})();

// A compositor lease protects shortcut inputs; never claim protection before IPC succeeds.
(() => {
  const { invoke } = window.Lucas;
  let current = null, queue = Promise.resolve();
  const lease = (active, token) => {
    const operation = queue.catch(() => {}).then(() => invoke("set_shortcut_recording", { active, token }));
    queue = operation.catch(() => {});
    return operation;
  };
  function stop() {
    const session = current;
    if (!session) return;
    current = null;
    window.clearInterval(session.timer);
    session.input.classList.remove("is-recording");
    session.note.hidden = true;
    lease(false, session.token).catch(() => {});
  }
  async function begin(input, note, enabled) {
    if (!enabled() || current?.input === input) return;
    stop();
    const session = { input, note, token: crypto.randomUUID(), armed: false, pending: false, modifiers: new Set() };
    current = session;
    note.textContent = "正在启用录入保护，请稍候…";
    note.hidden = false;
    try {
      await lease(true, session.token);
      if (current !== session || document.activeElement !== input) return;
      session.armed = true;
      input.classList.add("is-recording");
      note.textContent = "录入保护已开启 · 按下组合键，Esc 结束";
      session.timer = window.setInterval(async () => {
        if (current !== session || session.pending) return;
        session.pending = true;
        try {
          await lease(true, session.token);
        } catch (_) {
          if (current === session) {
            stop();
            note.textContent = "录入保护已结束，请重新聚焦后再按组合键";
            note.hidden = false;
          }
        } finally { session.pending = false; }
      }, 1000);
    } catch (error) {
      if (current === session) {
        stop();
        note.textContent = String(error);
        note.hidden = false;
      }
    }
  }
  function attach(input, note, enabled, changed) {
    const modifier = (event) => {
      if (["Meta", "Super", "OS", "Hyper"].includes(event.key) || /^(Meta|Super|OS)(Left|Right)$/.test(event.code)) return "Super";
      if (event.key === "Control") return "Ctrl";
      if (["Alt", "AltGraph"].includes(event.key)) return "Alt";
      if (event.key === "Shift") return "Shift";
      return null;
    };
    input.addEventListener("focus", () => begin(input, note, enabled));
    input.addEventListener("blur", () => { if (current?.input === input) stop(); });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && current?.input === input) {
        e.preventDefault();
        e.stopImmediatePropagation();
        stop();
        input.blur();
        return;
      }
      if (e.isComposing) return;
      const mod = modifier(e);
      if (mod) {
        if (current?.input === input) current.modifiers.add(mod);
        return;
      }
      const held = current?.input === input ? current.modifiers : new Set();
      const superKey = e.metaKey || held.has("Super");
      const ctrlKey = e.ctrlKey || held.has("Ctrl");
      const altKey = e.altKey || held.has("Alt");
      const shiftKey = e.shiftKey || held.has("Shift");
      if (!(ctrlKey || altKey || superKey)) return;
      // Keep ordinary text selection and clipboard editing available.
      if (ctrlKey && ["a", "c", "v", "x"].includes(e.key.toLowerCase()) && !shiftKey && !altKey && !superKey) return;
      e.preventDefault();
      e.stopPropagation();
      if (!current?.armed || current.input !== input) return;
      const key = e.code.startsWith("Key") ? e.code.slice(3)
        : e.code.startsWith("Digit") ? e.code.slice(5)
          : e.key === " " ? "Space" : e.key;
      input.value = [superKey && "Super", ctrlKey && "Ctrl", altKey && "Alt", shiftKey && "Shift", key].filter(Boolean).join("+");
      changed();
    });
    input.addEventListener("keyup", (event) => {
      if (current?.input === input) current.modifiers.delete(modifier(event));
    });
    window.addEventListener("focus", () => {
      if (document.activeElement === input) begin(input, note, enabled);
    });
  }
  window.addEventListener("blur", stop);
  window.addEventListener("pagehide", stop);
  document.addEventListener("visibilitychange", () => { if (document.hidden) stop(); });
  window.LucasShortcutRecorder = { attach, stop };
})();

// Compact settings with explicit save/install actions and preserved edits on failure.
(() => {
  const { $, el, button, invoke, confirmAction } = window.Lucas;
  let preferences,
    officialDirty = false,
    preferencesDirty = false,
    fontDirty = false,
    saving = false,
    updating = false,
    desktopManaged = false;
  window.LucasSettingsDirty = () =>
    officialDirty || preferencesDirty || fontDirty || saving || updating;
  window.LucasSettingsBusy = () => saving || updating;
  const field = (label, node) =>
    el(
      "div",
      { class: "field" },
      el("label", { class: "f-label", for: node.id, text: label }),
      node,
    );
  const status = (id) =>
    el("p", { id, class: "settings-status", role: "status" });
  const key = el("input", {
    id: "official-key",
    type: "password",
    autocomplete: "off",
    placeholder: "填写 DeepL API Key",
    maxlength: 1024,
  });
  const plan = el(
    "select",
    { id: "official-plan", class: "mini-select" },
    el("option", { value: "free", text: "DeepL API Free" }),
    el("option", { value: "pro", text: "DeepL API Pro" }),
  );
  const enabled = el("input", { id: "official-enabled", type: "checkbox" });
  let officialLoaded = false,
    officialLoad = 0,
    clearKey = false,
    officialSaving = false;
  const officialStatus = status("official-status");
  const save = button(
    "保存",
    async () => {
      if (!officialLoaded || officialSaving || saving) return;
      officialSaving = saving = true;
      ++officialLoad;
      setOfficialDisabled(true);
      officialStatus.textContent = "正在安全保存…";
      try {
        await invoke("set_official_config", {
          config: {
            enabled: enabled.checked,
            pro: plan.value === "pro",
            api_key: key.value.trim() || null,
            clear_key: clearKey,
          },
        });
        officialDirty = false;
        key.value = "";
        await loadOfficial();
        officialStatus.textContent = "已保存 · 密钥仅存于系统凭据库";
      } catch (error) {
        officialStatus.textContent = String(error);
      } finally {
        officialSaving = saving = false;
        setOfficialDisabled(!officialLoaded);
      }
    },
    true,
  );
  save.id = "official-save";
  const test = button("测试连接", async () => {
    if (saving) return;
    if (officialDirty) {
      officialStatus.textContent = "请先保存修改，再测试已保存的配置";
      return;
    }
    officialSaving = saving = true;
    setOfficialDisabled(true);
    officialStatus.textContent = "正在连接 DeepL…";
    try {
      await invoke("test_official_connection");
      officialStatus.textContent = "连接正常 · 未发送原文，不消耗翻译额度";
    } catch (error) {
      officialStatus.textContent = String(error);
    } finally {
      officialSaving = saving = false;
      setOfficialDisabled(false);
    }
  });
  test.id = "official-test";
  const remove = button("移除密钥", () => {
    clearKey = true;
    officialDirty = true;
    key.value = "";
    enabled.checked = false;
    key.placeholder = "保存后移除密钥";
    officialStatus.textContent = "尚未保存";
  });
  function setOfficialDisabled(value) {
    [key, plan, enabled, save, test, remove].forEach((n) => {
      n.disabled = value;
    });
  }
  async function loadOfficial() {
    const generation = ++officialLoad;
    try {
      const data = await invoke("get_official_config");
      if (generation !== officialLoad || officialDirty) return;
      enabled.checked = data.enabled;
      plan.value = data.pro ? "pro" : "free";
      key.value = "";
      key.placeholder = data.has_api_key
        ? "已保存 · 留空保持原密钥"
        : "填写 DeepL API Key";
      officialLoaded = true;
      clearKey = false;
      setOfficialDisabled(false);
    } catch (_) {
      officialStatus.textContent = "读取失败，重新进入此页可重试";
    }
  }
  const official = el(
    "div",
    { class: "group", id: "official-group" },
    el(
      "div",
      { class: "field" },
      el("label", { for: enabled.id }, enabled, " 启用 DeepL 官方 API"),
      el("p", {
        class: "rule-hint",
        text: "与免费网页通道独立；额度与费用由 DeepL 账户决定。测试仅查询额度，不提交翻译。",
      }),
    ),
    field("账户类型", plan),
    field("API Key · 系统凭据存储", key),
    el(
      "div",
      { class: "field ai-actions" },
      officialStatus,
      remove,
      test,
      save,
    ),
  );
  $("svc-list").after(official);
  [key, plan, enabled].forEach((node) =>
    node.addEventListener("input", () => {
      officialDirty = true;
      officialStatus.textContent = "尚未保存";
    }),
  );
  setOfficialDisabled(true);
  loadOfficial();
  document
    .querySelector('[data-view="services"]')
    .addEventListener("click", () => {
      if (!officialDirty && !officialSaving) loadOfficial();
    });

  const font = el(
    "select",
    { id: "reading-size", class: "mini-select" },
    ...[11, 12, 13, 14, 15, 16, 18, 20].map((size) =>
      el("option", { value: size, text: size + " px" }),
    ),
  );
  const fontStatus = status("font-status");
  font.addEventListener("change", () => {
    fontDirty = true;
    fontStatus.textContent = "尚未保存";
  });
  const fontSave = button("保存字号", async () => {
    if (!preferences || saving) return;
    saving = true;
    fontSave.disabled = true;
    try {
      await invoke("set_preferences", {
        preferences: { ...preferences, font_size: Number(font.value) },
        fontOnly: true,
      });
      preferences.font_size = Number(font.value);
      fontDirty = false;
      await window.LucasPreferences.refresh();
      fontStatus.textContent = "已保存";
    } catch (error) {
      fontStatus.textContent = String(error);
    } finally {
      saving = false;
      fontSave.disabled = false;
    }
  });
  $("view-general")
    .querySelector(".group")
    .after(
      el(
        "div",
        { class: "group" },
        field("原文与译文字号", font),
        el("div", { class: "field ai-actions" }, fontStatus, fontSave),
      ),
    );
  const hotkeys = el("div", { class: "group" });
  const labels = {
    input: "空白输入翻译",
    toggle: "显示 / 隐藏翻译窗口",
    selection: "划词翻译",
    screenshot: "截图翻译",
    ocr: "截图取字（仅复制）",
    annotate: "截图编辑标注",
  };
  let validationTimer, validationVersion = 0;
  const actions = () => Object.keys(labels).filter((action) =>
    desktopManaged || !["toggle", "annotate"].includes(action),
  );
  const editedShortcuts = () => Object.fromEntries(actions().map((action) => [
    action, $("hotkey-" + action).value.trim(),
  ]));
  function rowStatus(action, kind, message) {
    const indicator = $("hotkey-indicator-" + action);
    indicator.dataset.status = kind;
    indicator.textContent = { ok: "✓", conflict: "×", pending: "…", disabled: "—", error: "!" }[kind];
    indicator.title = message;
    indicator.setAttribute("aria-label", message);
    $("hotkey-" + action).setAttribute("aria-invalid", String(kind === "conflict"));
    $("hotkey-conflict-" + action).textContent = message;
    $("hotkey-conflict-" + action).hidden = kind !== "conflict" && kind !== "error";
  }
  function showConflicts(conflicts = {}) {
    for (const action of Object.keys(labels)) {
      const value = $("hotkey-" + action).value.trim();
      rowStatus(action, conflicts[action] ? "conflict" : value ? "ok" : "disabled",
        conflicts[action] || (value ? "未发现桌面快捷键冲突；其他应用内部快捷键无法自动检测" : "未设置，已禁用"));
    }
  }
  async function validateShortcuts() {
    if (!preferences || saving) return;
    const version = ++validationVersion;
    try {
      const conflicts = await invoke("check_shortcuts", {
        preferences: { ...preferences, shortcuts: editedShortcuts() },
      });
      if (version !== validationVersion || saving) return;
      showConflicts(conflicts);
      hotkeyStatus.textContent = Object.keys(conflicts).length
        ? "尚未保存；请查看各行的 × 提示"
        : desktopManaged ? "尚未保存" : "格式正确；保存时检查系统占用";
    } catch (error) {
      if (version === validationVersion) {
        for (const action of actions())
          if ($("hotkey-" + action).value.trim()) rowStatus(action, "error", "检测失败：" + error);
        hotkeyStatus.textContent = "检测暂时不可用，请稍后重试";
      }
    }
  }
  function changed() {
    preferencesDirty = true;
    ++validationVersion;
    clearTimeout(validationTimer);
    hotkeyStatus.textContent = "尚未保存";
    for (const action of actions()) rowStatus(action, "pending", "正在检测…");
    validationTimer = setTimeout(validateShortcuts, 350);
  }
  for (const [action, label] of Object.entries(labels)) {
    const input = el("input", {
      id: "hotkey-" + action,
      type: "text",
      spellcheck: "false",
      autocomplete: "off",
      disabled: true,
      placeholder: "输入组合键；留空禁用",
      maxlength: 80,
      "aria-describedby": "hotkey-conflict-" + action,
    });
    input.addEventListener("input", changed);
    const recordingNote = el("p", { class: "recording-note", id: "hotkey-recording-" + action, hidden: true, role: "status" });
    window.LucasShortcutRecorder.attach(input, recordingNote, () => desktopManaged && !!preferences, changed);
    const clear = button("清空", () => { input.value = ""; changed(); input.focus(); });
    clear.disabled = true;
    clear.setAttribute("aria-label", "清空" + label + "快捷键");
    const row = field(label, input);
    row.id = "hotkey-field-" + action;
    const edit = el("div", { class: "hotkey-edit" });
    input.replaceWith(edit);
    const indicator = el("span", {
      id: "hotkey-indicator-" + action, class: "hotkey-indicator", role: "img", tabindex: "0",
      "aria-label": "等待检测", title: "等待检测", "data-status": "pending", text: "…",
    });
    edit.append(input, indicator, clear);
    row.append(el("p", { id: "hotkey-conflict-" + action, class: "state-sr-only", hidden: true, role: "status" }));
    row.append(recordingNote);
    hotkeys.append(row);
  }
  const hotkeyStatus = status("hotkey-status");
  const hotkeySave = button("保存快捷键", async () => {
    if (!preferences || saving) return;
    window.LucasShortcutRecorder.stop();
    saving = true;
    ++validationVersion;
    clearTimeout(validationTimer);
    hotkeys.querySelectorAll("input, button").forEach((node) => { node.disabled = true; });
    hotkeyStatus.textContent = "正在检查并保存…";
    try {
      await invoke("set_preferences", {
        preferences: { ...preferences, shortcuts: editedShortcuts() },
      });
      preferencesDirty = false;
      const data = await loadPreferences();
      if (data) hotkeyStatus.textContent = Object.keys(data.conflicts || {}).length
        ? "可用快捷键已保存；冲突键位已留空，请重新设置。" : "已保存，立即生效";
    } catch (error) {
      hotkeyStatus.textContent = String(error);
    } finally {
      saving = false;
      hotkeys.querySelectorAll("input, button").forEach((node) => { node.disabled = false; });
    }
  }, true);
  hotkeySave.id = "hotkey-save";
  hotkeys.append(el("div", { class: "field ai-actions" }, hotkeyStatus, hotkeySave));
  $("view-hotkeys").querySelector(".group").before(
    el("p", { id: "hotkey-hint", class: "rule-hint" }), hotkeys,
  );
  async function loadPreferences() {
    try {
      const data = await window.LucasPreferences.refresh();
      preferences = data.preferences;
      desktopManaged = !!data.desktop_managed;
      document.body.classList.toggle("omarchy", desktopManaged);
      $("hotkey-hint").textContent = desktopManaged
        ? "点击输入框，等待录入保护开启后按下组合键；也可输入或粘贴组合键文字。✓ 表示未发现桌面冲突，× 表示冲突，悬停可查看原因；应用内部快捷键无法自动检测。留空禁用，保存后生效。"
        : "输入组合键，例如 Alt+A、Ctrl+Shift+D；macOS 的 Command 写作 Super。留空禁用。保存时检查系统占用。";
      if (!fontDirty) font.value = String(preferences.font_size);
      for (const action of Object.keys(labels)) {
        $("hotkey-field-" + action).hidden = !actions().includes(action);
        if (!preferencesDirty) $("hotkey-" + action).value = preferences.shortcuts[action] || "";
      }
      if (!saving) hotkeys.querySelectorAll("input, button").forEach((node) => { node.disabled = false; });
      showConflicts(data.conflicts);
      hotkeyStatus.textContent = Object.keys(data.conflicts || {}).length ? "请重新设置标记 × 的快捷键" : "";
      fontSave.disabled = hotkeySave.disabled = false;
      return data;
    } catch (error) {
      hotkeyStatus.textContent = "读取失败：" + error;
      fontStatus.textContent = "设置暂时无法读取";
      return null;
    }
  }
  fontSave.disabled = hotkeySave.disabled = true;
  loadPreferences();
  document.querySelector('[data-view="hotkeys"]').addEventListener("click", () => {
    if (!preferencesDirty && !saving) loadPreferences();
  });
  document.querySelector('[data-view="general"]').addEventListener("click", () => {
    if (!preferences && !saving) loadPreferences();
  });

  let available = null;
  const updateStatus = status("update-status");
  const progress = el("progress", {
    id: "update-progress",
    "aria-label": "更新下载进度",
    hidden: true,
  });
  const cancel = button("取消下载", async () => {
    try {
      await invoke("cancel_update");
    } catch (error) {
      updateStatus.textContent = String(error);
    }
  });
  cancel.hidden = true;
  const install = button(
    "下载并安装",
    async () => {
      if (!available || updating) return;
      if (window.LucasSettingsDirty() || window.LucasAiUnsaved?.()) {
        updateStatus.textContent = "请先保存设置修改，再安装更新";
        return;
      }
      if (
        !(await confirmAction(
          "安装更新？",
          "应用将在下载、校验签名后重启。请先完成当前翻译。",
          "安装并重启",
        ))
      )
        return;
      updating = true;
      document.querySelectorAll(".nav-item").forEach((node) => { node.disabled = true; });
      check.disabled = install.disabled = true;
      cancel.hidden = false;
      progress.hidden = false;
      progress.removeAttribute("value");
      updateStatus.textContent = "正在下载并校验…";
      try {
        await invoke("install_update", { version: available });
      } catch (error) {
        updateStatus.textContent = String(error);
      } finally {
        updating = false;
        document.querySelectorAll(".nav-item").forEach((node) => { node.disabled = false; });
        check.disabled = install.disabled = false;
        cancel.hidden = progress.hidden = true;
      }
    },
    true,
  );
  install.id = "update-install";
  install.hidden = true;
  const check = button("检查更新", async () => {
    if (updating) return;
    check.disabled = true;
    install.hidden = true;
    available = null;
    updateStatus.textContent = "正在检查…";
    try {
      const data = await invoke("check_update");
      $("app-version").textContent = data.current;
      available = data.version;
      updateStatus.textContent = data.version
        ? "发现新版本 " +
          data.version +
          (data.installable ? "" : " · 当前安装方式请从 Releases 手动更新")
        : "已是最新版本";
      install.hidden = !data.version || !data.installable;
    } catch (error) {
      updateStatus.textContent = String(error);
    } finally {
      check.disabled = false;
    }
  });
  check.id = "update-check";
  $("view-about")
    .querySelector(".group")
    .after(
      el(
        "div",
        { class: "group" },
        el(
          "div",
          { class: "field" },
          el("p", {
            class: "rule-hint",
            text: "手动检查，确认后安装。更新包会验证项目签名；通过包管理器安装的版本请使用原渠道更新。",
          }),
          updateStatus,
          progress,
        ),
        el("div", { class: "field ai-actions" }, cancel, check, install),
      ),
    );
  window.__TAURI__.app
    ?.getVersion()
    .then((version) => {
      $("app-version").textContent = version;
    })
    .catch(() => {});
  window.__TAURI__.event
    .listen("lucas://update-progress", ({ payload }) => {
      if (!updating) return;
      if (payload.phase === "installing") {
        cancel.hidden = true;
        updateStatus.textContent = "签名已验证，正在安装并重启…";
        progress.removeAttribute("value");
      } else if (payload.total) {
        progress.max = payload.total;
        progress.value = payload.downloaded;
        updateStatus.textContent =
          "正在下载 " +
          Math.floor((payload.downloaded / payload.total) * 100) +
          "%";
      }
    })
    .catch(() => {});
})();

const {
  $,
  invoke,
  el,
  icon,
  button,
  toast,
  confirmAction,
  names,
  catalog,
  toggle,
  theme,
} = window.Lucas;
const { listen, emit } = window.__TAURI__.event;
const settingsWindow = window.__TAURI__.window.getCurrentWindow();
const applyTheme = theme(settingsWindow);
const State = window.LucasState;
if (navigator.userAgent.includes("Mac")) document.body.classList.add("mac");
document.querySelector(".side").addEventListener("mousedown", (e) => {
  if (e.button === 0 && !e.target.closest("button") && e.clientY < 44) {
    e.preventDefault();
    settingsWindow.startDragging().catch((err) => toast(String(err), true));
  }
});
document.querySelectorAll(".nav-item[data-view]").forEach((item) =>
  item.addEventListener("click", () => {
    State.close();
    document.querySelectorAll(".nav-item").forEach((n) => {
      n.classList.toggle("active", n === item);
      n.setAttribute("aria-current", n === item ? "page" : "false");
    });
    document
      .querySelectorAll(".view")
      .forEach((v) =>
        v.classList.toggle("active", v.id === "view-" + item.dataset.view),
      );
    document.querySelector(".content").scrollTop = 0;
  }),
);
function renderTheme() {
  document.querySelectorAll("#theme-seg button").forEach((b) => {
    const active =
      b.dataset.theme === (localStorage.getItem("lucas-theme") || "system");
    b.classList.toggle("active", active);
    b.setAttribute("aria-pressed", String(active));
  });
}
document.querySelectorAll("#theme-seg button").forEach((b) =>
  b.addEventListener("click", () => {
    localStorage.setItem("lucas-theme", b.dataset.theme);
    applyTheme();
    renderTheme();
    emit("lucas://theme-changed", b.dataset.theme).catch((e) =>
      toast(String(e), true),
    );
  }),
);
renderTheme();

async function logStatus() {
  try {
    const s = await invoke("diagnostics_status");
    $("log-status").textContent =
      !s?.available || s.write_failed
        ? "日志暂时不可写，请检查磁盘空间或目录权限"
        : s.dropped_events
          ? "日志繁忙，已跳过 " + s.dropped_events + " 条记录"
          : "已启用 · 仅保存在本机";
  } catch (_) {
    $("log-status").textContent = "暂时无法读取日志状态";
  }
}
$("open-logs").addEventListener("click", async () => {
  try {
    await invoke("open_diagnostics");
  } catch (e) {
    toast(String(e), true);
  } finally {
    logStatus();
  }
});
document
  .querySelector('[data-view="about"]')
  .addEventListener("click", logStatus);
logStatus();

let permissionBusy = false;
async function checkPermissions() {
  if (permissionBusy) return;
  permissionBusy = true;
  $("refresh").disabled = true;
  for (const id of ["chip-accessibility", "chip-screen"]) {
    $(id).textContent = "检测中";
    $(id).className = "state-chip";
  }
  try {
    const s = await invoke("permission_status");
    if (s.platform && s.platform !== "macos") {
      $("chip-accessibility").textContent = "非 TCC 权限";
      $("chip-screen").textContent = "非 TCC 权限";
      for (const id of ["open-accessibility", "open-screen"]) $(id).hidden = true;
      $("chip-accessibility").closest(".row").querySelector(".d").textContent = s.platform === "linux"
        ? s.desktop_managed
          ? "Omarchy / Hyprland 使用 wl-clipboard 读取 Wayland PRIMARY 选区；不支持选区的应用请手动复制。API 密钥需要已解锁的 Secret Service。"
          : "划词需要 X11 和 xclip；Wayland 尚不完整支持。API 密钥需要已解锁的 Secret Service。"
        : "划词通过系统复制操作获取选区；受保护窗口或更高权限的应用可能无法读取。";
      $("chip-screen").closest(".row").querySelector(".d").textContent = s.platform === "linux"
        ? s.desktop_managed
          ? "Omarchy / Hyprland 使用 slurp 框选、grim 截图，Tesseract 在本地识别；需要 eng / chi_sim 语言包。"
          : "OCR 需要 Tesseract 与 eng / chi_sim 语言包。截图可用性取决于桌面会话。"
        : "OCR 使用 Windows 系统识别能力，请安装对应语言包；受保护内容可能无法截取。";
      return;
    }
    for (const [id, ok] of [
      ["chip-accessibility", s.accessibility],
      ["chip-screen", s.screen_capture],
    ]) {
      $(id).textContent = ok ? "已授权" : "未授权";
      $(id).className = "state-chip " + (ok ? "ok" : "bad");
    }
  } catch (e) {
    for (const id of ["chip-accessibility", "chip-screen"])
      $(id).textContent = "检测失败";
    toast("权限检测失败，可重新检测：" + e, true);
  } finally {
    permissionBusy = false;
    $("refresh").disabled = false;
  }
}
$("refresh").addEventListener("click", checkPermissions);
for (const [id, kind] of [
  ["open-accessibility", "accessibility"],
  ["open-screen", "screen"],
])
  $(id).addEventListener("click", () =>
    invoke("open_permission_settings", { kind }).catch((e) =>
      toast(String(e), true),
    ),
  );
checkPermissions();

const auto = $("autostart");
auto.disabled = true;
(async () => {
  try {
    const api = window.__TAURI__.autostart;
    if (!api) throw Error("自启动组件不可用");
    auto.checked = await api.isEnabled();
    auto.disabled = false;
    auto.addEventListener("change", async () => {
      const value = auto.checked;
      auto.disabled = true;
      try {
        await (value ? api.enable() : api.disable());
      } catch (e) {
        auto.checked = !value;
        toast("设置开机启动失败：" + e, true);
      } finally {
        auto.disabled = false;
      }
    });
  } catch (e) {
    toast(String(e), true);
  }
})();

let routing = null,
  ruleSaving = false,
  ruleLoading = 0;
const languages = Object.entries(names).filter(([id]) => id !== "auto");
function selectLanguage(value, label, change) {
  const select = el(
    "select",
    { class: "mini-select", "aria-label": label },
    languages.map(([id, name]) => el("option", { value: id, text: name })),
  );
  select.value = value;
  select.addEventListener("change", () => change(select.value));
  return select;
}
function renderRules() {
  if (!routing) return;
  const list = $("rule-list");
  list.replaceChildren(
    ...routing.rules.map((r, index) =>
      el(
        "div",
        { class: "rule-row" },
        selectLanguage(r.from, "规则源语言", (value) =>
          saveRules((d) => (d.rules[index].from = value)),
        ),
        el("span", { class: "arrow", text: "→" }),
        selectLanguage(r.to, "规则目标语言", (value) =>
          saveRules((d) => (d.rules[index].to = value)),
        ),
        button("删除", () => saveRules((d) => d.rules.splice(index, 1))),
      ),
    ),
  );
  if (!routing.rules.length)
    list.append(
      State.create("empty", {
        label: "暂无规则",
        description: "翻译将使用默认目标语言，可以添加规则。",
      }),
    );
  $("rule-fallback").replaceChildren(
    ...languages.map(([id, name]) => el("option", { value: id, text: name })),
  );
  $("rule-fallback").value = routing.fallback;
  $("rule-add").disabled =
    ruleSaving || routing.rules.length >= languages.length;
}
function rulesDisabled(disabled) {
  document
    .querySelectorAll(
      "#rule-list select,#rule-list > .rule-row button,#rule-fallback,#rule-add",
    )
    .forEach((n) => (n.disabled = disabled));
}
async function loadRules() {
  const generation = ++ruleLoading;
  rulesDisabled(true);
  if (!routing)
    $("rule-list").replaceChildren(
      State.create("loading", { label: "正在读取语言规则" }),
    );
  try {
    const data = await invoke("get_routing");
    if (generation !== ruleLoading || ruleSaving) return;
    routing = data;
    renderRules();
  } catch (e) {
    $("rule-list").replaceChildren(
      State.create("error", {
        label: "语言规则读取失败",
        description: "原有配置未更改，可以重新读取。",
        actions: [button("重新加载", loadRules)],
      }),
    );
  } finally {
    if (generation === ruleLoading && !ruleSaving) rulesDisabled(!routing);
  }
}
async function saveRules(change) {
  if (ruleSaving || !routing) return;
  const draft = structuredClone(routing);
  change(draft);
  ruleSaving = true;
  rulesDisabled(true);
  try {
    await invoke("set_routing", { cfg: draft });
    routing = draft;
    toast("规则已保存");
  } catch (e) {
    toast(String(e), true);
  } finally {
    ruleSaving = false;
    renderRules();
    rulesDisabled(false);
  }
}
$("rule-fallback").addEventListener("change", (e) =>
  saveRules((d) => (d.fallback = e.target.value)),
);
$("rule-add").addEventListener("click", () =>
  saveRules((d) => {
    const source = languages.find(
      ([id]) => !d.rules.some((r) => r.from === id),
    )?.[0];
    if (source)
      d.rules.push({
        from: source,
        to: source === "zh-Hans" ? "en" : "zh-Hans",
      });
  }),
);
loadRules();

let serviceLoad = 0,
  serviceSaving = false,
  serviceReloadQueued = false;
async function loadServices() {
  if (serviceSaving) {
    serviceReloadQueued = true;
    return;
  }
  const generation = ++serviceLoad;
  const list = $("svc-list");
  list.setAttribute("aria-busy", "true");
  if (!list.children.length)
    list.append(State.create("loading", { label: "正在读取服务配置" }));
  try {
    const [catalogue, values] = await Promise.all([
      catalog(),
      invoke("get_services"),
    ]);
    if (generation !== serviceLoad || serviceSaving) return;
    list.replaceChildren(
      ...catalogue.filter((m) => m.id !== "deepl_api").map((m) =>
        el(
          "div",
          { class: "row" },
          el(
            "div",
            { class: "label" },
            el("div", { class: "t", text: m.label }),
            el("div", { class: "d", text: m.description }),
          ),
          toggle(m.id, m.label, !!values[m.id], async (control) => {
            const value = control.checked;
            serviceSaving = true;
            list.querySelectorAll("input").forEach((n) => (n.disabled = true));
            try {
              await invoke("set_service", { id: m.id, enabled: value });
              toast(m.label + (value ? "已启用" : "已停用"));
            } catch (e) {
              control.checked = !value;
              toast(String(e), true);
            } finally {
              serviceSaving = false;
              list
                .querySelectorAll("input")
                .forEach((n) => (n.disabled = false));
              if (serviceReloadQueued) {
                serviceReloadQueued = false;
                await loadServices();
                list.querySelector('input[data-svc="' + m.id + '"]')?.focus();
              }
            }
          }),
        ),
      ),
    );
  } catch (e) {
    if (generation === serviceLoad)
      list.replaceChildren(
        State.create("error", {
          label: "服务配置读取失败",
          description: "原有配置未更改，可以重新读取。",
          actions: [button("重新加载", loadServices)],
        }),
      );
  } finally {
    if (generation === serviceLoad) list.removeAttribute("aria-busy");
  }
}
loadServices();

let aiLoaded = false,
  aiDirty = false,
  aiSaving = false,
  hasKey = false,
  clearKey = false,
  aiGeneration = 0;
const aiFields = ["ai-enabled", "ai-base-url", "ai-key", "ai-model"];
function aiDisabled(value) {
  for (const id of [...aiFields, "ai-save", "ai-clear-key"])
    $(id).disabled = value;
}
function dirty() {
  aiDirty = true;
  $("ai-status").textContent = "有未保存的更改";
}
aiFields.forEach((id) => $(id).addEventListener("input", dirty));
async function loadAI() {
  if (aiDirty || aiSaving) return;
  const generation = ++aiGeneration;
  aiDisabled(true);
  $("ai-status").textContent = "正在加载…";
  try {
    const c = await invoke("get_ai_config");
    if (generation !== aiGeneration) return;
    $("ai-enabled").checked = c.enabled;
    $("ai-base-url").value = c.base_url || "";
    $("ai-model").value = c.model || "";
    $("ai-key").value = "";
    hasKey = !!c.has_api_key;
    clearKey = false;
    $("ai-key").placeholder = hasKey
      ? "已安全保存；留空保持不变"
      : "本地服务可留空";
    aiLoaded = true;
    $("ai-save").textContent = "保存";
    $("ai-status").textContent = hasKey
      ? "密钥已保存在系统凭据存储"
      : "尚未配置密钥";
  } catch (e) {
    aiLoaded = false;
    $("ai-status").textContent = "加载失败：" + e;
    $("ai-save").textContent = "重新加载";
  } finally {
    aiDisabled(!aiLoaded);
    $("ai-save").disabled = false;
  }
}
$("ai-clear-key").addEventListener("click", async () => {
  if (!hasKey && !$("ai-key").value) return;
  if (
    !(await confirmAction(
      "移除 API Key？",
      "保存后生效。云端翻译可能需要重新配置密钥。",
      "移除",
    ))
  )
    return;
  clearKey = true;
  $("ai-key").value = "";
  $("ai-key").placeholder = "保存后移除密钥";
  dirty();
});
$("ai-save").addEventListener("click", async () => {
  if (!aiLoaded) {
    loadAI();
    return;
  }
  if (aiSaving) return;
  aiSaving = true;
  aiDisabled(true);
  $("ai-status").textContent = "正在保存…";
  try {
    await invoke("set_ai_config", {
      cfg: {
        enabled: $("ai-enabled").checked,
        base_url: $("ai-base-url").value.trim(),
        model: $("ai-model").value.trim(),
        api_key: $("ai-key").value.trim() || null,
        clear_key: clearKey,
      },
    });
    aiDirty = false;
    toast("已保存");
    $("ai-save").textContent = "保存";
  } catch (e) {
    $("ai-status").textContent = "保存失败，输入已保留";
    toast(String(e), true);
  } finally {
    aiSaving = false;
    aiDisabled(false);
    if (!aiDirty) loadAI();
  }
});
loadAI();
window.LucasAiUnsaved = () => aiDirty || aiSaving;
listen("lucas://config-changed", () => {
  loadServices();
  if (!ruleSaving) loadRules();
  if (!aiDirty && !aiSaving) loadAI();
}).catch((e) => toast(String(e), true));
// All settings exit paths preserve unsaved edits and wait for saves/updates.
async function approveLeave() {
  if (aiSaving || window.LucasSettingsBusy?.()) {
    toast("正在保存或安装更新，请稍候");
    return false;
  }
  if (!aiDirty && !window.LucasSettingsDirty?.()) return true;
  return confirmAction("放弃未保存的设置？", "设置修改尚未保存。", "放弃修改");
}
settingsWindow
  .onCloseRequested(async (event) => {
    if (!(await approveLeave())) event.preventDefault();
  })
  .catch((error) => toast("关闭保护初始化失败：" + error, true));
function closeSettings() {
  settingsWindow.close().catch((error) => toast(String(error), true));
}
$("settings-close").addEventListener("click", closeSettings);
$("settings-quit").addEventListener("click", async () => {
  if (!(await approveLeave())) return;
  invoke("quit_app").catch((error) => toast(String(error), true));
});
window.addEventListener("keydown", (event) => {
  if (event.key !== "Escape" || event.defaultPrevented || document.querySelector("dialog[open]")) return;
  event.preventDefault();
  closeSettings();
});

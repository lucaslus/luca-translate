// 偏好设置页逻辑
const { invoke } = window.__TAURI__.core;
const { emit } = window.__TAURI__.event;

// ---------- 主题（与主窗口共享 localStorage，支持跟随系统） ----------

// macOS Overlay：整个侧边栏顶部区域为拖拽区（导航按钮除外）
if (navigator.userAgent.includes("Mac")) document.body.classList.add("mac");
const settingsWindow = window.__TAURI__.window.getCurrentWindow();
document.querySelector(".side").addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  if (e.target.closest(".nav-item")) return;
  if (e.clientY - document.querySelector(".side").getBoundingClientRect().top > 44) return;
  e.preventDefault();
  settingsWindow
    .startDragging()
    .catch((err) => toast("拖拽失败: " + String(err)));
});
function systemTheme() {
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
function resolveTheme() {
  const pref = localStorage.getItem("lucas-theme") || "system";
  return pref === "system" ? systemTheme() : pref;
}
function applyTheme() {
  document.documentElement.dataset.theme = resolveTheme();
}
applyTheme();
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  if ((localStorage.getItem("lucas-theme") || "system") === "system") applyTheme();
});

// 主题分段控件
const $seg = document.getElementById("theme-seg");
function renderSeg() {
  const pref = localStorage.getItem("lucas-theme") || "system";
  $seg.querySelectorAll("button").forEach((b) => {
    b.classList.toggle("active", b.dataset.theme === pref);
  });
}
$seg.querySelectorAll("button").forEach((b) => {
  b.addEventListener("click", () => {
    localStorage.setItem("lucas-theme", b.dataset.theme);
    applyTheme();
    renderSeg();
    emit("lucas://theme-changed", b.dataset.theme); // 同步主窗口
  });
});
renderSeg();

// ---------- 侧边栏视图切换 ----------
document.querySelectorAll(".nav-item").forEach((item) => {
  item.addEventListener("click", () => {
    document.querySelectorAll(".nav-item").forEach((i) => i.classList.remove("active"));
    item.classList.add("active");
    document.querySelectorAll(".view").forEach((v) => v.classList.remove("active"));
    document.getElementById(`view-${item.dataset.view}`).classList.add("active");
  });
});

// ---------- 权限检测 ----------
function setChip(id, ok) {
  const chip = document.getElementById(id);
  if (!chip) return;
  chip.textContent = ok ? "已授权" : "未授权";
  chip.className = `state-chip ${ok ? "ok" : "bad"}`;
}

async function checkPermissions() {
  try {
    const s = await invoke("permission_status");
    setChip("chip-accessibility", s.accessibility);
    setChip("chip-screen", s.screen_capture);
  } catch (e) {
    console.error(e);
  }
}
document.getElementById("refresh").addEventListener("click", checkPermissions);
document.getElementById("open-accessibility").addEventListener("click", () => invoke("open_permission_settings", { kind: "accessibility" }));
document.getElementById("open-screen").addEventListener("click", () => invoke("open_permission_settings", { kind: "screen" }));
// ---------- 默认翻译规则 ----------
const RULE_FROM = [
  ["zh-Hans", "中文"],
  ["en", "英语"],
  ["ja", "日语"],
  ["ko", "韩语"],
];
const RULE_TO = [
  ["zh-Hans", "简体中文"],
  ["zh-Hant", "繁体中文"],
  ["en", "English"],
  ["ja", "日本語"],
  ["ko", "한국어"],
  ["fr", "Français"],
  ["de", "Deutsch"],
  ["ru", "Русский"],
  ["es", "Español"],
];
let routing = { rules: [], fallback: "zh-Hans" };

function ruleOptions(kind, selected) {
  const list = kind === "from" ? RULE_FROM : RULE_TO;
  return list.map(([v, n]) => `<option value="${v}" ${v === selected ? "selected" : ""}>${n}</option>`).join("");
}

function renderRules() {
  const list = document.getElementById("rule-list");
  list.innerHTML = routing.rules.map((r, i) => `
    <div class="rule-row" data-i="${i}">
      <select class="mini-select" data-i="${i}" data-kind="from">${ruleOptions("from", r.from)}</select>
      <span class="arrow">→</span>
      <select class="mini-select" data-i="${i}" data-kind="to">${ruleOptions("to", r.to)}</select>
      <button class="rule-del" data-i="${i}" title="删除规则"><span class="icon" style="font-size:14px">delete</span></button>
    </div>`).join("") || `<div class="row"><div class="label"><div class="d">暂无规则，点击下方「添加规则」</div></div></div>`;
  document.getElementById("rule-fallback").innerHTML = ruleOptions("to", routing.fallback);
}

function bindRuleEvents() {
  const list = document.getElementById("rule-list");
  list.addEventListener("change", (e) => {
    const i = +e.target.dataset.i;
    const kind = e.target.dataset.kind;
    if (kind === "from") routing.rules[i].from = e.target.value;
    else routing.rules[i].to = e.target.value;
    saveRouting();
  });
  list.addEventListener("click", (e) => {
    const del = e.target.closest(".rule-del");
    if (del) {
      routing.rules.splice(+del.dataset.i, 1);
      renderRules();
      saveRouting();
    }
  });
  document.getElementById("rule-add").addEventListener("click", () => {
    routing.rules.push({ from: "en", to: "zh-Hans" });
    renderRules();
    saveRouting();
  });
  document.getElementById("rule-fallback").addEventListener("change", (e) => {
    routing.fallback = e.target.value;
    saveRouting();
  });
}

async function saveRouting() {
  try {
    await invoke("set_routing", { cfg: routing });
    toast("规则已保存，立即生效");
  } catch (e) {
    console.error(e);
  }
}

(async () => {
  try {
    routing = await invoke("get_routing");
    if (!routing.rules) routing = { rules: [], fallback: "zh-Hans" };
    renderRules();
    bindRuleEvents();
  } catch (e) {
    console.error(e);
  }
})();

checkPermissions();

// ---------- 开机自启 ----------
const $autostart = document.getElementById("autostart");
(async () => {
  try {
    const autostart = window.__TAURI__.autostart;
    $autostart.checked = await autostart.isEnabled();
    $autostart.addEventListener("change", () => {
      $autostart.checked ? autostart.enable() : autostart.disable();
    });
  } catch (e) {
    console.error("autostart unavailable:", e);
    $autostart.disabled = true;
  }
})();

// ---------- 翻译服务开关 ----------
const SERVICE_LIST = [
  ["youdao", "有道词典", "词典卡片 · 英美音标 · 整句翻译"],
  ["bing", "Bing", "微软 Edge 免费通道 · 国内可直连"],
  ["deepl", "DeepL", "免费端点 · 译文质量极佳"],
  ["google", "Google", "免费端点 · 需要网络代理"],
  ["baidu", "百度翻译", "网页版通道 · 可能不稳定，默认关闭"],
];
let svcToggles = {};

function renderSvcList() {
  const list = document.getElementById("svc-list");
  list.innerHTML = SERVICE_LIST.map(([id, name, desc]) => `
    <div class="row">
      <div class="label"><div class="t">${name}</div><div class="d">${desc}</div></div>
      <label class="switch"><input type="checkbox" data-svc="${id}" ${svcToggles[id] ? "checked" : ""} /><span class="slider"></span></label>
    </div>`).join("");
  list.querySelectorAll("input[data-svc]").forEach((inp) => {
    inp.addEventListener("change", async () => {
      svcToggles[inp.dataset.svc] = inp.checked;
      try {
        await invoke("set_services", { services: svcToggles });
        toast(inp.checked ? `${inp.dataset.svc} 已启用` : `${inp.dataset.svc} 已停用`);
      } catch (e) {
        console.error(e);
      }
    });
  });
}

(async () => {
  try {
    svcToggles = await invoke("get_services");
    if (typeof svcToggles !== "object" || !svcToggles) svcToggles = {};
    renderSvcList();
  } catch (e) {
    console.error(e);
  }
})();

// ---------- AI 翻译服务配置 ----------
const $aiEnabled = document.getElementById("ai-enabled");
const $aiBaseUrl = document.getElementById("ai-base-url");
const $aiKey = document.getElementById("ai-key");
const $aiModel = document.getElementById("ai-model");

(async () => {
  try {
    const cfg = await invoke("get_ai_config");
    $aiEnabled.checked = !!cfg.enabled;
    $aiBaseUrl.value = cfg.base_url || "";
    $aiKey.value = cfg.api_key || "";
    $aiModel.value = cfg.model || "";
  } catch (e) {
    console.error(e);
  }
})();

document.getElementById("ai-save").addEventListener("click", async () => {
  await invoke("set_ai_config", {
    cfg: {
      enabled: $aiEnabled.checked,
      base_url: $aiBaseUrl.value.trim(),
      api_key: $aiKey.value.trim(),
      model: $aiModel.value.trim(),
    },
  });
  toast("已保存，立即生效");
});

// ---------- 轻提示 ----------
function toast(msg) {
  document.querySelectorAll(".toast").forEach((t) => t.remove());
  const t = document.createElement("div");
  t.className = "toast";
  t.innerHTML = `<span class="icon" style="font-size:14px;vertical-align:-2px;margin-right:5px">check_circle</span>${msg}`;
  document.body.appendChild(t);
  setTimeout(() => t.remove(), 1800);
}

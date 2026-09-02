// lucas-translate UI ·「L 纸墨」设计语言
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const currentWindow = window.__TAURI__.window.getCurrentWindow();

const $ = (id) => document.getElementById(id);
// macOS Overlay 标题栏：为系统红绿灯留白
if (navigator.userAgent.includes("Mac")) document.body.classList.add("mac");
const $input = $("input");
const $result = $("result");
const $from = $("from-lang");
const $to = $("to-lang");
const $serviceTag = $("service-tag");
const $errorTag = $("error-tag");

const LANG_NAMES = {
  auto: "自动", "zh-Hans": "简体中文", "zh-Hant": "繁体中文",
  en: "英语", ja: "日语", ko: "韩语", fr: "法语", de: "德语", ru: "俄语", es: "西班牙语",
};

// 服务元信息：展示名 / 品牌色 / 能力标签
// 服务快捷开关弹层
const SVC_LIST = [
  ["youdao", "有道词典", "词典卡片 · 英美音标"],
  ["bing", "Bing", "国内可直连"],
  ["deepl", "DeepL", "质量极佳"],
  ["google", "Google", "需要代理"],
];
let svcToggles = {};
let svcPopLoaded = false;

const $svcBtn = $("svc-btn");
const $svcPop = $("svc-pop");

async function toggleSvcPop() {
  const show = $svcPop.style.display === "none";
  $svcPop.style.display = show ? "" : "none";
  $svcBtn.classList.toggle("active", show);
  if (show && !svcPopLoaded) {
    try {
      svcToggles = await invoke("get_services");
      svcPopLoaded = true;
      renderSvcPop();
    } catch (e) { console.error(e); }
  }
}

function renderSvcPop() {
  $svcPop.innerHTML = SVC_LIST.map(([id, name, desc]) => `
    <div class="svc-pop-row">
      <div class="svc-pop-name">${name}<div class="svc-pop-desc">${desc}</div></div>
      <label class="switch"><input type="checkbox" data-svc="${id}" ${svcToggles[id] ? "checked" : ""} /><span class="slider"></span></label>
    </div>`).join("") + `
    <div class="svc-pop-foot"><button id="svc-pop-settings">更多设置…</button></div>`;
  $svcPop.querySelectorAll("input[data-svc]").forEach((inp) => {
    inp.addEventListener("change", async () => {
      svcToggles[inp.dataset.svc] = inp.checked;
      try { await invoke("set_services", { services: svcToggles }); }
      catch (e) { console.error(e); }
    });
  });
  document.getElementById("svc-pop-settings").addEventListener("click", () => {
    $svcPop.style.display = "none";
    $svcBtn.classList.remove("active");
    invoke("open_settings").catch(() => {});
  });
}

$svcBtn.addEventListener("click", toggleSvcPop);
// 点击弹层外部关闭
document.addEventListener("mousedown", (e) => {
  if ($svcPop.style.display !== "none" && !e.target.closest("#svc-pop") && !e.target.closest("#svc-btn")) {
    $svcPop.style.display = "none";
    $svcBtn.classList.remove("active");
  }
});

const SERVICE_META = {
  YoudaoDict: { label: "有道", cap: "词典+翻译", logo: "logos/youdao.png" },
  DeepLFree: { label: "DeepL", cap: "翻译", logo: "logos/deepl.png" },
  GoogleFree: { label: "Google", cap: "翻译", logo: "logos/google.png" },
  Bing: { label: "Bing", cap: "翻译", logo: "logos/bing.png" },
  AI: { label: "AI", color: "var(--c-ai)", cap: "LLM", logo: "✦" },
};
const svcMeta = (name) => SERVICE_META[name] || { label: name, color: "var(--text-3)", cap: "" };

// ---------- 主题（默认跟随系统，精确切换在偏好设置-通用里） ----------
function systemTheme() {
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
function resolveTheme() {
  const pref = localStorage.getItem("lucas-theme") || "system";
  return pref === "system" ? systemTheme() : pref;
}
function applyTheme() {
  const pref = localStorage.getItem("lucas-theme") || "system";
  const t = resolveTheme();
  document.documentElement.dataset.theme = t;
  // 同步原生窗口外观（否则 macOS 会画出与内容不符的深色边框）
  try {
    currentWindow.setTheme(pref === "system" ? null : t);
  } catch (_) {}
}
applyTheme();
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  if ((localStorage.getItem("lucas-theme") || "system") === "system") applyTheme();
});
// 设置页修改主题后广播到主窗口
listen("lucas://theme-changed", (e) => {
  localStorage.setItem("lucas-theme", e.payload);
  applyTheme();
});

// ---------- 工具 ----------
function escapeHtml(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}
function escAttr(s) {
  return String(s ?? "").replace(/\\/g, "\\\\").replace(/'/g, "\\'").replace(/\n/g, "\\n");
}
function showToast(msg) {
  document.querySelectorAll(".toast").forEach((t) => t.remove());
  const t = document.createElement("div");
  t.className = "toast";
  t.textContent = msg;
  document.body.appendChild(t);
  setTimeout(() => t.remove(), 1900);
}
function copyText(text) {
  navigator.clipboard.writeText(text).then(() => showToast("已复制")).catch(() => {});
}
function speak(url) {
  new Audio(url).play().catch(() => {});
}

// ---------- 翻译主流程（流式：每个服务完成即渲染） ----------
let stream = null; // { dictRendered, received, failed, done }

async function doTranslate() {
  const text = $input.value.trim();
  if (!text) return;
  stream = { dictRendered: false, received: 0, failed: 0, done: false };
  setLoading();
  try {
    await invoke("translate", { text, from: $from.value, to: $to.value });
  } catch (e) {
    renderError(String(e));
  }
}

function svcCardHtml(r) {
  const meta = svcMeta(r.service);
  const failed = !r.paragraphs.length;
  return `<section class="svc ${failed ? "failed" : ""}" data-service="${r.service}">
    <div class="svc-head">
      <img class="svc-logo-img" src="${meta.logo}" alt="" />
      <span class="svc-name">${escapeHtml(meta.label)}</span>
      ${meta.cap ? `<span class="cap">${meta.cap}</span>` : ""}
      <span class="lang-info">${LANG_NAMES[r.detected_from] || r.detected_from} → ${LANG_NAMES[r.detected_to] || r.detected_to}</span>
      ${failed ? "" : `<button class="ghost copy-btn" data-copy="${escAttr(r.paragraphs.join("\n"))}" title="复制此结果"><span class="icon">content_copy</span></button>`}
      <span class="chev"><span class="icon">expand_more</span></span>
    </div>
    <div class="svc-body">${failed
      ? `<span class="fail-hint"><span class="icon">cloud_off</span>该渠道本次未响应<span class="reason" title="${escapeHtml(r.error || "网络原因")}">${escapeHtml(r.error || "网络原因")}</span></span><button class="ghost copy-btn retry-svc" title="重试"><span class="icon">refresh</span></button>`
      : r.paragraphs.map((p) => `<p>${escapeHtml(p)}</p>`).join("")}</div>
  </section>`;
}

function onServicesStart(names) {
  if (!stream) return;
  document.querySelector(".loading-note")?.remove();
  const skel = $result.querySelector(".skel");
  if (skel) skel.remove();
  const html = names.map((n) => {
    const meta = svcMeta(n);
    return `<section class="svc pending" data-service="${n}">
      <div class="svc-head">
        <img class="svc-logo-img" src="${meta.logo}" alt="" />
        <span class="svc-name">${escapeHtml(meta.label)}</span>
        ${meta.cap ? `<span class="cap">${meta.cap}</span>` : ""}
        <span class="mini-spin"></span>
      </div>
      <div class="svc-body pending-body">等待结果…</div>
    </section>`;
  }).join("");
  $result.innerHTML = html;
  $serviceTag.textContent = "0/" + names.length;
}

function onServiceResult(r) {
  if (!stream || stream.done) return;
  document.querySelector(".loading-note")?.remove();
  stream.received++;
  const failed = !r.paragraphs.length;
  if (failed) stream.failed++;

  // 词典卡：单词查询，替换整个结果区
  if (r.dict && !stream.dictRendered) {
    stream.dictRendered = true;
    const d = r.dict;
    let html = `<section class="dict-card">
      <div class="dict-top">
        <h1 class="dict-word">${escapeHtml(d.word)}</h1>
        <button class="ghost copy-btn" data-copy="${escAttr(d.word)}" title="复制"><span class="icon">content_copy</span></button>
      </div>
      <div class="dict-sub">dictionary · ${d.meanings.length} 条词义</div>`;
    const phs = [];
    if (d.uk_phonetic) phs.push(phChip("英", d.uk_phonetic, d.uk_speech));
    if (d.us_phonetic) phs.push(phChip("美", d.us_phonetic, d.us_speech));
    if (phs.length) html += `<div class="phonetics">${phs.join("")}</div>`;
    if (d.meanings.length) {
      html += `<ol class="senses">` + d.meanings
        .map(([pos, m]) => `<li><span class="pos">${escapeHtml(pos)}</span><span>${escapeHtml(m)}</span></li>`)
        .join("") + `</ol>`;
    }
    html += `</section>`;
    // 词典卡放回它自己的占位卡位置（YoudaoDict），不影响其他服务卡
    const dictSlot = $result.querySelector('section[data-service="YoudaoDict"]');
    if (dictSlot) {
      dictSlot.outerHTML = html;
    } else {
      $result.innerHTML = html;
    }
    $result.scrollTop = 0;
    return;
  }
  if (r.dict) return; // 后续词典结果忽略

  // 找到对应占位卡替换
  const card = $result.querySelector(`section[data-service="${r.service}"]`);
  if (card) {
    card.outerHTML = svcCardHtml(r);
  } else {
    $result.insertAdjacentHTML("beforeend", svcCardHtml(r));
  }
  $serviceTag.textContent = `${stream.received - stream.failed}/${stream.total || stream.received}`;
}

function onTranslateDone() {
  if (!stream || stream.done) return;
  stream.done = true;
  document.querySelector(".loading-note")?.remove();
  const skel = $result.querySelector(".skel");
  if (skel) skel.remove();
  // 仍未返回的占位卡 → 优雅失败态
  $result.querySelectorAll(".svc.pending").forEach((card) => {
    card.classList.add("failed");
    card.classList.remove("pending");
    card.querySelector(".mini-spin")?.remove();
    const body = card.querySelector(".svc-body");
    if (body) body.outerHTML = `<div class="svc-body"><span class="fail-hint"><span class="icon">cloud_off</span>该渠道本次未响应<span class="reason">超时</span></span></div>`;
  });
  if (stream.received === 0 || stream.received === stream.failed) {
    $result.innerHTML = `<div class="action-card">
      <span class="icon-big">wifi_off</span>
      <div class="title">翻译暂时不可用</div>
      <div class="desc">所有服务均未响应，可能是网络原因（部分渠道需要网络代理）。请检查网络后重试。</div>
      <div class="actions">
        <button class="btn primary" id="all-retry"><span class="icon">refresh</span>重试</button>
      </div>
    </div>`;
    document.getElementById("all-retry").addEventListener("click", () => {
      const t = $input.value;
      doTranslate();
    });
    return;
  }
}

function setLoading() {
  $errorTag.textContent = "";
  $serviceTag.textContent = "";
  $result.innerHTML = `<div class="loading-note">正在翻译<span class="dots"><i>.</i><i>.</i><i>.</i></span></div>
  <div class="skel">
    <div class="sk h28 w45"></div>
    <div class="sk w90"></div>
    <div class="sk w70"></div>
    <div class="sk w90"></div>
    <div class="sk w45"></div>
  </div>`;
}

function renderError(msg) {
  $result.innerHTML = `<div class="empty-state"><p style="font-size:26px">◌</p>
    <p style="margin-top:10px;color:var(--text-2)">${escapeHtml(msg)}</p></div>`;
  $errorTag.textContent = "翻译失败";
}

function phChip(tag, value, speechUrl) {
  const btn = speechUrl
    ? `<button class="speak" data-speech="${escapeHtml(speechUrl)}" title="发音"><span class="icon">volume_up</span></button>`
    : "";
  return `<span class="ph"><i class="tag">${tag}</i>/${escapeHtml(value)}/${btn}</span>`;
}

// 事件委托：复制 & 发音
$result.addEventListener("click", (e) => {
  const copyBtn = e.target.closest("[data-copy]");
  if (copyBtn) { copyText(copyBtn.dataset.copy); return; }
  const speakBtn = e.target.closest(".speak");
  if (speakBtn?.dataset.speech) speak(speakBtn.dataset.speech);
  // 服务卡头部点击 → 折叠/展开
  const head = e.target.closest(".svc-head");
  if (head && !e.target.closest("[data-copy]")) {
    head.parentElement.classList.toggle("collapsed");
  }
});

// ---------- 输入交互 ----------
$input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    doTranslate();
  }
});

// 自动规则 info 图标：选中「自动规则」时显示
const $autoRuleInfo = $("auto-rule-info");
function updateAutoRuleInfo() {
  $autoRuleInfo.style.display = $to.value === "auto" ? "" : "none";
}
$autoRuleInfo.addEventListener("click", () => invoke("open_settings").catch(() => {}));
$to.addEventListener("change", updateAutoRuleInfo);
updateAutoRuleInfo();

$("swap").addEventListener("click", () => {
  const f = $from.value, t = $to.value;
  $from.value = t === "auto" ? "en" : t;
  $to.value = f === "auto" ? "zh-Hans" : f;
});

function resetResult() {
  $result.innerHTML = `<div class="empty-state">
    <p style="font-size:24px;color:var(--text-3)">⌥</p>
    <p style="margin-top:10px">输入或划词开始翻译</p>
    <div class="keys">
      <span><b>⌥A</b>输入</span>
      <span><b>⌥D</b>划词</span>
      <span><b>⌥S</b>截图</span>
      <span><b>⌥C</b>静默OCR</span>
    </div></div>`;
  $serviceTag.textContent = "";
  $errorTag.textContent = "";
}

// ---------- 后端事件 ----------
if (listen) {
  listen("lucas://services-start", (e) => onServicesStart(e.payload));
  listen("lucas://service-result", (e) => onServiceResult(e.payload));
  listen("lucas://translate-done", () => onTranslateDone());
  listen("lucas://translate-text", (e) => {
    showToast("划词翻译 · ⌥D");
    $input.value = e.payload;
    doTranslate();
  });
  listen("lucas://focus-input", () => $input.focus());
  listen("lucas://ocr-result", (e) => showOcr(e.payload));
  listen("lucas://ocr-copied", (n) => showToast(`已复制 ${n.payload} 行识别文本`));
  listen("lucas://error", (e) => { renderError(e.payload); showToast(String(e.payload)); });
  // 语言自动路由：跟随展示（不覆盖用户手动选择的语言）
  listen("lucas://lang-resolved", (e) => {
    const { from, to } = e.payload;
    $detectInfo.style.display = "";
    $detectInfo.textContent = `检测到 ${LANG_NAMES[from] || from} · 翻译至 ${LANG_NAMES[to] || to}`;
  });

  // 划词捕获失败：区分无权限 / 无选中，给出可操作的引导
  listen("lucas://capture-failed", (e) => renderCaptureFailed(e.payload));
  listen("lucas://screen-permission", () => renderScreenPermission());
}

function renderCaptureFailed(kind) {
  $errorTag.textContent = "";
  $serviceTag.textContent = "";
  const isPermission = kind === "permission";
  $result.innerHTML = `<div class="action-card">
    <span class="icon-big">${isPermission ? "lock" : "search"}</span>
    <div class="title">${isPermission ? "需要辅助功能权限" : "未检测到选中的文本"}</div>
    <div class="desc">${isPermission
      ? "划词翻译需要辅助功能权限来读取你选中的内容。已为你弹出系统授权提示，也可以点击下方按钮前往设置手动开启。"
      : "请先在任意应用中选中一段文字，再按 <b style=\"font-family:var(--font-mono)\">⌥D</b> 触发划词翻译。"}</div>
    <div class="actions">
      ${isPermission ? `<button class="btn primary" id="cap-open-settings"><span class="icon">settings</span>前往权限设置</button>` : ""}
      <button class="btn" id="cap-retry"><span class="icon">refresh</span>重试</button>
    </div>
  </div>`;
  const openBtn = document.getElementById("cap-open-settings");
  if (openBtn) openBtn.addEventListener("click", () => invoke("open_permission_settings", { kind: "accessibility" }));
  document.getElementById("cap-retry").addEventListener("click", () => {
    setLoading();
    invoke("capture_selection").catch((e) => renderError(String(e)));
  });
}

// OCR 结果视图（走服务卡样式）
function showOcr(text) {
  $serviceTag.textContent = "OCR · Apple Vision 离线";
  $errorTag.textContent = "";
  const lines = text.split("\n");
  $result.innerHTML = `
    <div class="pinyin-row" style="border-left-color:var(--ok);color:var(--text-2)">
      识别到 ${lines.length} 行 · Apple Vision 离线识别
    </div>
    <section class="svc">
      <div class="svc-head">
        <span class="dot" style="--dot:var(--ok)"></span>
        <span class="svc-name">OCR 识别结果</span>
        <span class="lang-info">${lines.length} 行</span>
        <button class="ghost copy-btn" data-copy="${escAttr(text)}" title="复制全部"><span class="icon">content_copy</span></button>
      </div>
      <div class="svc-body"><p>${escapeHtml(text)}</p></div>
    </section>`;
  if (!$panel.classList.contains("hidden")) closePanel();
}

// ---------- 标题栏按钮 ----------
// ---------- 标题栏交互 ----------
// 手动拖拽：data-tauri-drag-region 与 macOS Overlay 标题栏冲突会卡顿，
// 改为 mousedown 时显式调用 startDragging()，由系统接管移动窗口
const titlebar = document.querySelector(".titlebar");
titlebar.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  if (e.target.closest("button")) return; // 按钮不触发拖拽
  e.preventDefault();
  currentWindow.startDragging().catch(() => {});
});
titlebar.addEventListener("selectstart", (e) => e.preventDefault());

$("btn-close").addEventListener("click", () => currentWindow.hide());
$("btn-settings").addEventListener("click", () => invoke("open_settings").catch((e) => showToast(String(e))));

// ---------- 历史 / 收藏面板 ----------
const $panel = $("panel");
const $panelList = $("panel-list");
let currentTab = "history";

function fmtTime(ts) {
  const d = new Date(ts * 1000);
  const pad = (n) => String(n).padStart(2, "0");
  return `${d.getMonth() + 1}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

async function loadPanel() {
  try {
    const items = currentTab === "history"
      ? await invoke("history_list", { limit: 100 })
      : await invoke("favorites_list");
    if (!items.length) {
      $panelList.innerHTML = `<div class="empty">暂无${currentTab === "history" ? "历史" : "收藏"}记录</div>`;
      return;
    }
    $panelList.innerHTML = items.map((it) => {
      const actions = currentTab === "history"
        ? `<button onclick="favItem('${escAttr(it.text)}','${escAttr(it.result)}','${escAttr(it.service)}')">收藏</button>`
        : `<button onclick="delFav(${it.id})">删除</button>`;
      return `<div class="item" onclick="reuseItem('${escAttr(it.text)}')">
        <div class="item-text">${escapeHtml(it.text)}</div>
        <div class="item-result">${escapeHtml(it.result)}</div>
        <div class="item-meta">
          <span>${escapeHtml(it.service || "")} · ${fmtTime(it.created_at)}</span>
          <span class="item-actions" onclick="event.stopPropagation()">${actions}</span>
        </div>
      </div>`;
    }).join("");
  } catch (e) {
    $panelList.innerHTML = `<div class="empty">加载失败: ${escapeHtml(String(e))}</div>`;
  }
}

function openPanel(tab) {
  currentTab = tab;
  document.querySelectorAll(".panel-tabs .tab").forEach((t) => {
    t.classList.toggle("active", t.dataset.tab === tab);
  });
  $("panel-clear").style.display = tab === "history" ? "" : "none";
  $panel.classList.remove("hidden");
  loadPanel();
}
function closePanel() { $panel.classList.add("hidden"); }

function reuseItem(text) {
  $input.value = text.replace(/\\n/g, "\n");
  closePanel();
  doTranslate();
}
async function favItem(text, result, service) {
  await invoke("favorite_add", { text: text.replace(/\\n/g, "\n"), result: result.replace(/\\n/g, "\n"), service });
  showToast("已收藏");
}
async function delFav(id) {
  await invoke("favorite_remove", { id });
  loadPanel();
}
window.reuseItem = reuseItem;
window.favItem = favItem;
window.delFav = delFav;
window.doTranslate = doTranslate;

$("btn-history").addEventListener("click", () => openPanel("history"));
$("btn-favorites").addEventListener("click", () => openPanel("favorites"));
$("panel-close").addEventListener("click", closePanel);
$("panel-clear").addEventListener("click", async () => {
  await invoke("history_clear");
  loadPanel();
});
document.querySelectorAll(".panel-tabs .tab").forEach((t) => {
  t.addEventListener("click", () => openPanel(t.dataset.tab));
});

// 全局 Esc：先关面板，再清输入
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (!$panel.classList.contains("hidden")) { closePanel(); return; }
    if (document.activeElement === $input && $input.value) {
      $input.value = "";
      resetResult();
    }
  }
});

$input.focus();


// 屏幕录制权限引导卡
function renderScreenPermission() {
  $serviceTag.textContent = "";
  $result.innerHTML = `<div class="action-card">
    <span class="icon-big">verified_user</span>
    <div class="title">需要屏幕录制权限</div>
    <div class="desc">截图 OCR 需要截取屏幕内容进行<b>本地离线识别</b>（不上传）。前往设置开启后，<b>重启应用生效</b>。</div>
    <div class="actions">
      <button class="btn primary" id="perm-open"><span class="icon">settings</span>前往系统设置</button>
      <button class="btn" id="perm-retry"><span class="icon">refresh</span>重新检测</button>
    </div>
  </div>`;
  document.getElementById("perm-open").addEventListener("click", () => invoke("open_permission_settings", { kind: "screen" }));
  document.getElementById("perm-retry").addEventListener("click", async () => {
    const s = await invoke("permission_status");
    if (s.screen_capture) {
      invoke("start_screenshot");
    } else {
      showToast("仍未授权，请先在系统设置中开启");
    }
  });
}

// 服务失败卡的重试按钮（事件委托）
$result.addEventListener("click", (e) => {
  const retry = e.target.closest(".retry-svc");
  if (retry) doTranslate();
});

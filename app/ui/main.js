// Request-scoped rendering; all text is inserted through DOM APIs.
const {
  $,
  invoke,
  el,
  icon,
  button,
  toast,
  busy,
  confirmAction,
  names,
  catalog,
  logo,
  toggle,
  theme,
} = window.Lucas;
const { listen } = window.__TAURI__.event;
const currentWindow = window.__TAURI__.window.getCurrentWindow();
if (navigator.userAgent.includes("Mac")) document.body.classList.add("mac");
const input = $("input"),
  result = $("result"),
  from = $("from-lang"),
  to = $("to-lang");
const applyTheme = theme(currentWindow);
const State = window.LucasState;
let stream = null,
  watchdog,
  meta = [],
  captureBusy = false,
  booted = false;
const serviceMeta = (name) =>
  meta.find((m) => m.service === name) || {
    label: name === "AI" ? "AI" : name,
  };
function setBusy(active) {
  result.setAttribute("aria-busy", String(active));
  $("cancel-work").hidden = !active;
  if (!active) $("work-indicator").hidden = true;
}
function empty(title, description, actions = [], state = "idle") {
  State.close();
  $("detect-info").textContent = "";
  $("service-tag").textContent = "";
  $("error-tag").textContent = "";
  const node = State.create(state, { label: title, description, actions });
  node.classList.add("state-standalone");
  result.replaceChildren(node);
}
function resetResult() {
  setBusy(false);
  $("service-tag").textContent = "";
  $("error-tag").textContent = "";
  empty("输入或划词开始翻译", "输入文字，按 Enter 翻译");
  result.querySelector(".state-content").append(
    el(
      "div",
      { class: "shortcut-list", "aria-label": "翻译快捷键" },
      [
        ["输入翻译", "input"],
        ["划词翻译", "selection"],
        ["截图翻译", "screenshot"],
        ["截图取字", "ocr"],
      ].map(([label, key]) =>
        el(
          "div",
          { class: "shortcut-item" },
          el("span", { text: label }),
          el("kbd", { text: window.LucasPreferences.shortcut(key), "data-shortcut": key }),
        ),
      ),
    ),
    el("p", { class: "state-description", text: "截图取字仅复制识别文字" }),
  );
}
function cancelWork(reset = false) {
  clearTimeout(watchdog);
  if (stream && !stream.done)
    invoke("cancel_translation", { requestId: stream.id }).catch((e) =>
      toast(String(e), true),
    );
  const wasCapturing = captureBusy;
  if (captureBusy) invoke("cancel_ocr").catch((e) => toast(String(e), true));
  captureBusy = false;
  if (stream && !stream.done) {
    finish("已取消");
    if (!stream.results.size) empty("已取消翻译", "可以修改文字后重新翻译。");
  }
  if (wasCapturing) empty("已取消识别", "可按 ⌥S 重新截图。");
  setBusy(false);
  if (reset) {
    stream = null;
    resetResult();
  }
}
async function doTranslate(service = null) {
  if (!booted) return;
  const previous = stream;
  const text = service && previous ? previous.text : input.value.trim();
  const source = service && previous ? previous.from : from.value;
  const target = service && previous ? previous.to : to.value;
  if (!text) return;
  if (text.length > 20000) {
    toast("内容较长，请分段翻译（最多 20000 字符）", true);
    return;
  }
  if (
    !service &&
    previous &&
    !previous.done &&
    previous.text === text &&
    previous.from === source &&
    previous.to === target
  )
    return;
  cancelWork();
  State.close();
  if (!panel.classList.contains("hidden")) closePanel();
  closeServices();
  stream = {
    id: crypto.randomUUID(),
    text,
    from: source,
    to: target,
    done: false,
    pending: new Set(service ? [service] : []),
    services: [],
    results: service && previous ? new Map(previous.results) : new Map(),
    confirmedSources:
      service && previous
        ? new Map(
            [...previous.results]
              .filter(([, value]) => value.source_confirmed)
              .map(([name, value]) => [name, value.detected_from]),
          )
        : new Map(),
    error: null,
    only: service,
  };
  $("error-tag").textContent = "";
  setBusy(true);
  if (service) {
    const slot = [...result.querySelectorAll("[data-service]")].find(
      (n) => n.dataset.service === service,
    );
    slot?.replaceWith(pendingCard(service));
    stream.results.delete(service);
  } else {
    // Before the backend supplies the enabled channels, use a tiny footer
    // indicator, never a full-result loading screen or guessed provider list.
    result.replaceChildren();
    $("work-indicator").replaceChildren(
      State.create("loading", { label: "正在读取翻译渠道", compact: true }),
    );
    $("work-indicator").hidden = false;
  }
  const active = stream;
  watchdog = setTimeout(() => {
    if (stream !== active || active.done) return;
    invoke("cancel_translation", { requestId: active.id }).catch(() => {});
    active.error = "等待超时，已保留完成的结果";
    finish("超时，请重试此渠道");
  }, 70000);
  try {
    await invoke("translate", {
      requestId: active.id,
      text,
      from: source,
      to: target,
      service,
    });
  } catch (e) {
    if (stream === active) {
      active.error = String(e);
      finish(String(e));
    }
  }
}
function pendingCard(name) {
  const loading = State.create("loading", {
    label: `${serviceMeta(name).label} 正在翻译`,
    compact: true,
  });
  loading.classList.add("channel-loading");
  return el(
    "section",
    { class: "svc pending", "data-service": name },
    el(
      "div",
      { class: "svc-head" },
      logo(serviceMeta(name)),
      el("span", { class: "svc-name", text: serviceMeta(name).label }),
      loading,
    ),
  );
}
function copy(text) {
  navigator.clipboard
    .writeText(text)
    .then(() => toast("已复制"))
    .catch(() => toast("复制失败，请重试", true));
}
function card(r) {
  const failed = !r.paragraphs?.length;
  const section = el("section", {
    class: "svc" + (failed ? " failed" : ""),
    "data-service": r.service,
  });
  const bodyId = "body-" + crypto.randomUUID();
  const body = el("div", { class: "svc-body", id: bodyId });
  const displayedSource =
    stream?.from && stream.from !== "auto" ? stream.from : r.detected_from;
  const sourceLabel =
    stream?.from === "auto" && !r.source_confirmed
      ? "待确认"
      : names[displayedSource] || displayedSource;
  const collapse = el(
    "button",
    {
      class: "svc-toggle",
      type: "button",
      "aria-expanded": "true",
      "aria-controls": bodyId,
    },
    logo(serviceMeta(r.service)),
    el("span", { class: "svc-name", text: serviceMeta(r.service).label }),
    el("span", {
      class: "lang-info",
      text: sourceLabel + " → " + (names[r.detected_to] || r.detected_to),
    }),
    icon("expand_more"),
  );
  collapse.addEventListener("click", () => {
    body.hidden = !body.hidden;
    collapse.setAttribute("aria-expanded", String(!body.hidden));
  });
  const head = el("div", { class: "svc-head" }, collapse);
  if (!failed) {
    const copyBtn = button(
      "",
      () => copy(r.paragraphs.join("\n")),
      false,
      "content_copy",
    );
    copyBtn.className = "ghost copy-btn";
    copyBtn.title = "复制译文";
    copyBtn.setAttribute(
      "aria-label",
      "复制" + serviceMeta(r.service).label + "译文",
    );
    head.append(copyBtn);
  }
  if (failed) {
    body.append(
      window.LucasErrors.render(r, {
        label: serviceMeta(r.service).label,
        retry: () => doTranslate(r.service),
        copy,
        settings: () =>
          invoke("open_settings").catch((e) => toast(String(e), true)),
        logs: () =>
          invoke("open_diagnostics").catch((e) => toast(String(e), true)),
      }),
    );
  } else if (r.dict) {
    const d = r.dict;
    body.append(el("h2", { class: "dict-word", text: d.word }));
    const phonetics = el("div", { class: "phonetics" });
    for (const [label, value, url] of [
      ["英", d.uk_phonetic, d.uk_speech],
      ["美", d.us_phonetic, d.us_speech],
    ]) {
      if (!value) continue;
      const chip = el("span", { class: "ph" }, label + " /" + value + "/");
      // Only the configured HTTPS voice endpoint may be played.
      if (url) {
        try {
          const u = new URL(url);
          if (u.protocol === "https:" && u.hostname === "dict.youdao.com") {
            const play = button(
              label + "音",
              () =>
                new Audio(url)
                  .play()
                  .catch(() => toast("发音暂时不可用", true)),
              false,
              "volume_up",
            );
            play.classList.add("speak");
            chip.append(play);
          }
        } catch (_) {}
      }
      phonetics.append(chip);
    }
    body.append(
      phonetics,
      el(
        "ol",
        { class: "senses" },
        (d.meanings || []).map(([pos, text]) =>
          el("li", {}, el("span", { class: "pos", text: pos }), text),
        ),
      ),
    );
  } else {
    body.append(...r.paragraphs.map((text) => el("p", { text })));
  }
  if (!failed && r.pinyin)
    body.append(
      el(
        "details",
        { class: "pinyin-details" },
        el("summary", { text: "拼音" }),
        el("p", { text: r.pinyin }),
      ),
    );
  section.append(head, body);
  return section;
}
function replaceCard(r) {
  const old = [...result.querySelectorAll("[data-service]")].find(
    (n) => n.dataset.service === r.service,
  );
  if (old) old.replaceWith(card(r));
  else result.append(card(r));
  syncRecoveryButtons();
}
let recoveryTimer;
function syncRecoveryButtons() {
  clearTimeout(recoveryTimer);
  let waiting = false;
  document.querySelectorAll(".retry-svc").forEach((b) => {
    const seconds = Math.max(
      0,
      Math.ceil((Number(b.dataset.retryAt || 0) - Date.now()) / 1000),
    );
    b.disabled = !stream?.done || seconds > 0;
    const wait =
      seconds >= 3600
        ? Math.ceil(seconds / 3600) + " 小时"
        : seconds >= 60
          ? Math.ceil(seconds / 60) + " 分钟"
          : seconds + " 秒";
    b.querySelector(".btn-label").textContent = seconds
      ? wait + "后可重试"
      : "重试此渠道";
    b.title = seconds
      ? "冷却期间不会发起请求"
      : !stream?.done
        ? "其他渠道完成后可单独重试"
        : "仅重试这个渠道";
    waiting ||= seconds > 0;
  });
  if (waiting && !document.hidden)
    recoveryTimer = setTimeout(syncRecoveryButtons, 1000);
}
document.addEventListener("visibilitychange", syncRecoveryButtons);
document.addEventListener("lucas:statechange", syncRecoveryButtons);
function finishPending(message) {
  if (!stream) return;
  for (const name of stream.pending) {
    const r = {
      service: name,
      detected_from: stream.source || stream.from,
      detected_to: stream.target || stream.to,
      paragraphs: [],
      error: message,
    };
    stream.results.set(name, r);
    replaceCard(r);
  }
  stream.pending.clear();
}
function updateCount() {
  if (!stream) return;
  const success = [...stream.results.values()].filter(
    (r) => r.paragraphs?.length,
  ).length;
  $("service-tag").textContent = success ? success + " 个结果" : "";
}
function confirmedSource() {
  if (!stream?.confirmedSources?.size) return null;
  const counts = new Map();
  for (const language of stream.confirmedSources.values())
    counts.set(language, (counts.get(language) || 0) + 1);
  const ranked = [...counts.entries()].sort((a, b) => b[1] - a[1]);
  return !ranked[1] || ranked[0][1] > ranked[1][1] ? ranked[0][0] : null;
}
function updateLanguageStatus() {
  if (!stream?.source || !stream?.target) return;
  let prefix = "源语言";
  let source = stream.source;
  if (stream.from === "auto") {
    const confirmed = confirmedSource();
    if (confirmed) {
      prefix = "检测";
      source = confirmed;
    } else if (stream.detection?.reliable) {
      prefix = "检测";
    } else {
      $("detect-info").textContent =
        "语言待确认 · 译至 " + (names[stream.target] || stream.target);
      return;
    }
  }
  $("detect-info").textContent =
    prefix +
    "：" +
    (names[source] || source) +
    " · 译至 " +
    (names[stream.target] || stream.target);
}
function finish(message = "渠道未完成，请重试") {
  if (!stream) return;
  stream.done = true;
  clearTimeout(watchdog);
  finishPending(message);
  setBusy(false);
  updateCount();
  syncRecoveryButtons();
  if (stream.error) {
    if (!stream.results.size)
      empty(
        "暂时无法翻译",
        stream.error,
        [
          button(
            "翻译设置",
            () => invoke("open_settings").catch((e) => toast(String(e), true)),
            true,
          ),
          button("重试", () => doTranslate()),
        ],
        "error",
      );
  }
}
function translationEvent({ request_id, kind, data }) {
  if (!stream || stream.id !== request_id || stream.done) return;
  if (kind === "start") {
    State.close();
    $("work-indicator").hidden = true;
    stream.services = data.services;
    stream.pending = new Set(data.services);
    stream.source = data.from;
    stream.target = data.to;
    stream.detection = data.detection || null;
    updateLanguageStatus();
    if (!stream.only) result.replaceChildren(...data.services.map(pendingCard));
    updateCount();
  } else if (kind === "result") {
    if (!stream.pending.has(data.service)) return;
    stream.pending.delete(data.service);
    data._requestId = request_id;
    if (
      stream.from === "auto" &&
      data.source_confirmed &&
      data.paragraphs?.length &&
      data.detected_from
    ) {
      stream.confirmedSources.set(data.service, data.detected_from);
      updateLanguageStatus();
    }
    $("result-status").textContent =
      serviceMeta(data.service).label +
      (data.paragraphs?.length ? "译文已就绪" : window.LucasErrors.label(data));
    stream.results.set(data.service, data);
    replaceCard(data);
    updateCount();
  } else if (kind === "error") stream.error = data.message;
  else if (kind === "warning") toast(data.message, true);
  else if (kind === "done") finish();
}
$("cancel-work").addEventListener("click", () => cancelWork());
input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    doTranslate();
  }
});
$("swap").addEventListener("click", () => {
  const a = from.value,
    b = to.value;
  from.value = b === "auto" ? "en" : b;
  to.value = a === "auto" ? "zh-Hans" : a;
  updateRuleInfo();
});
function updateRuleInfo() {
  $("auto-rule-info").hidden = to.value !== "auto";
}
to.addEventListener("change", updateRuleInfo);
$("auto-rule-info").addEventListener("click", () =>
  invoke("open_settings").catch((e) => toast(String(e), true)),
);
$("btn-settings").addEventListener("click", () =>
  invoke("open_settings").catch((e) => toast(String(e), true)),
);
updateRuleInfo();

// Service controls use atomic single-field commands, never save a stale snapshot.
let serviceRefresh = 0,
  serviceSaving = false,
  serviceRefreshQueued = false;
async function refreshServices() {
  if (serviceSaving) {
    serviceRefreshQueued = true;
    return;
  }
  const generation = ++serviceRefresh,
    container = $("svc-pop");
  container.setAttribute("aria-busy", "true");
  try {
    const [list, values] = await Promise.all([
      catalog(),
      invoke("get_services"),
    ]);
    meta = list;
    if (generation !== serviceRefresh) return;
    container.replaceChildren(
      ...list.map((m) =>
        el(
          "div",
          { class: "svc-pop-row" },
          el(
            "div",
            { class: "svc-pop-name", text: m.label },
            el("div", { class: "svc-pop-desc", text: m.description }),
          ),
          toggle(m.id, m.label, !!values[m.id], async (control) => {
            const checked = control.checked;
            serviceSaving = true;
            container
              .querySelectorAll("input")
              .forEach((n) => (n.disabled = true));
            try {
              await invoke("set_service", { id: m.id, enabled: checked });
            } catch (e) {
              control.checked = !checked;
              toast(String(e), true);
            } finally {
              serviceSaving = false;
              container
                .querySelectorAll("input")
                .forEach((n) => (n.disabled = false));
              if (serviceRefreshQueued) {
                serviceRefreshQueued = false;
                await refreshServices();
                if (!container.hidden)
                  container
                    .querySelector('input[data-svc="' + m.id + '"]')
                    ?.focus();
              }
            }
          }),
        ),
      ),
      button("更多设置…", () =>
        invoke("open_settings").catch((e) => toast(String(e), true)),
      ),
    );
  } catch (e) {
    if (generation === serviceRefresh)
      container.replaceChildren(
        el("p", { text: String(e) }),
        button("重新加载", refreshServices),
      );
  } finally {
    if (generation === serviceRefresh) container.removeAttribute("aria-busy");
  }
}
function closeServices() {
  $("svc-pop").hidden = true;
  $("svc-btn").setAttribute("aria-expanded", "false");
}
$("svc-btn").addEventListener("click", () => {
  const open = $("svc-pop").hidden;
  $("svc-pop").hidden = !open;
  $("svc-btn").setAttribute("aria-expanded", String(open));
  if (open) refreshServices();
});
document.addEventListener("mousedown", (e) => {
  if (!e.target.closest("#svc-pop") && !e.target.closest("#svc-btn"))
    closeServices();
});

// Panel responses are tagged with the exact view/generation that requested them.
const panel = $("panel"),
  panelList = $("panel-list");
let panelTab = "history",
  panelGeneration = 0,
  panelItems = [],
  panelOffset = 0,
  panelFocus;
async function loadPanel(append = false) {
  const generation = ++panelGeneration,
    tab = panelTab,
    offset = append ? panelOffset : 0;
  panelList.setAttribute("aria-busy", "true");
  if (!append) {
    panelItems = [];
    panelList.replaceChildren(
      State.create("loading", { label: "正在读取记录" }),
    );
  } else
    panelList.querySelectorAll("button").forEach((b) => (b.disabled = true));
  try {
    const items = await invoke(
      tab === "history" ? "history_list" : "favorites_list",
      { limit: 100, offset },
    );
    if (
      generation !== panelGeneration ||
      tab !== panelTab ||
      panel.classList.contains("hidden")
    )
      return;
    panelItems = append ? panelItems.concat(items) : items;
    panelOffset = offset + items.length;
    renderPanel(items.length === 100);
  } catch (e) {
    if (generation === panelGeneration)
      panelList.replaceChildren(
        State.create("error", {
          label: "记录读取失败",
          description: "记录暂时无法读取，请稍后重试。",
          actions: [button("重试", () => loadPanel())],
        }),
      );
  } finally {
    if (generation === panelGeneration) panelList.removeAttribute("aria-busy");
  }
}
function renderPanel(more) {
  if (!panelItems.length) {
    panelList.replaceChildren(
      State.create("empty", {
        label: "暂无" + (panelTab === "history" ? "历史" : "收藏") + "记录",
        description:
          panelTab === "history"
            ? "完成一次翻译后，记录会出现在这里。"
            : "在历史记录中收藏译文后，可以在这里找到。",
      }),
    );
    return;
  }
  panelList.replaceChildren(
    ...panelItems.map((item) => {
      const reuse = el(
        "button",
        { class: "item-reuse", type: "button" },
        el("div", { class: "item-text", text: item.text }),
        el("div", { class: "item-result", text: item.result }),
      );
      reuse.addEventListener("click", () => {
        input.value = item.text;
        closePanel();
        doTranslate();
      });
      const action = button(
        panelTab === "history" ? "收藏" : "删除",
        async () => {
          if (panelTab === "history") {
            try {
              await busy(
                action,
                () =>
                  invoke("favorite_add", {
                    text: item.text,
                    result: item.result,
                    service: item.service,
                  }),
                "已收藏",
              );
            } catch (_) {}
          } else {
            const generation = panelGeneration;
            try {
              await busy(action, () =>
                invoke("favorite_remove", { id: item.id }),
              );
              if (generation === panelGeneration) loadPanel();
              toast("已删除收藏", false, {
                label: "撤销",
                run: async () => {
                  try {
                    await invoke("favorite_add", {
                      text: item.text,
                      result: item.result,
                      service: item.service,
                    });
                    if (
                      panelTab === "favorites" &&
                      !panel.classList.contains("hidden")
                    )
                      loadPanel();
                  } catch (e) {
                    toast(String(e), true);
                  }
                },
              });
            } catch (_) {}
          }
        },
      );
      return el(
        "div",
        { class: "item" },
        reuse,
        el(
          "div",
          { class: "item-meta" },
          el("span", {
            text:
              item.service +
              " · " +
              new Date(item.created_at * 1000).toLocaleString(),
          }),
          action,
        ),
      );
    }),
  );
  if (more) panelList.append(button("加载更多", () => loadPanel(true)));
}
function openPanel(tab) {
  if (panel.classList.contains("hidden")) panelFocus = document.activeElement;
  panelTab = tab;
  panel.classList.remove("hidden");
  document.querySelectorAll(".panel-tabs .tab").forEach((b) => {
    b.classList.toggle("active", b.dataset.tab === tab);
    b.setAttribute("aria-selected", String(b.dataset.tab === tab));
  });
  $("panel-clear").hidden = tab !== "history";
  loadPanel();
  $("panel-close").focus();
}
function closePanel() {
  ++panelGeneration;
  panel.classList.add("hidden");
  panelFocus?.focus();
}
$("btn-history").addEventListener("click", () => openPanel("history"));
$("btn-favorites").addEventListener("click", () => openPanel("favorites"));
$("panel-close").addEventListener("click", closePanel);
document
  .querySelectorAll(".panel-tabs .tab")
  .forEach((b) => b.addEventListener("click", () => openPanel(b.dataset.tab)));
$("panel-clear").addEventListener("click", async () => {
  if (
    !(await confirmAction(
      "清空历史记录？",
      "将清空所有历史记录，收藏不受影响。此操作无法撤销。",
      "清空历史",
    ))
  )
    return;
  try {
    await busy($("panel-clear"), () => invoke("history_clear"));
    loadPanel();
  } catch (_) {}
});
panel.addEventListener("keydown", (e) => {
  if (e.key !== "Tab") return;
  const controls = [...panel.querySelectorAll("button:not([disabled])")].filter(
    (n) => !n.hidden,
  );
  const first = controls[0],
    last = controls.at(-1);
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last?.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first?.focus();
  }
});
window.addEventListener("keydown", (e) => {
  if (e.key !== "Escape" || document.querySelector("dialog[open]")) return;
  if (!panel.classList.contains("hidden")) {
    closePanel();
    return;
  }
  if (!$("svc-pop").hidden) {
    closeServices();
    $("svc-btn").focus();
    return;
  }
  if (captureBusy || (stream && !stream.done)) {
    cancelWork();
    return;
  }
  currentWindow.hide().catch((e) => toast(String(e), true));
});
document.querySelector(".titlebar").addEventListener("mousedown", (e) => {
  if (e.button !== 0 || e.target.closest("button")) return;
  e.preventDefault();
  currentWindow.startDragging().catch((e) => toast(String(e), true));
});
const pin = $("btn-pin");
let pinned = false;
function renderPin(value) {
  pinned = value;
  pin.classList.toggle("active", value);
  pin.setAttribute("aria-pressed", String(value));
  pin.setAttribute("aria-label", value ? "取消置顶" : "置顶窗口");
  pin.title = value ? "取消置顶" : "置顶窗口";
}
pin.addEventListener("click", async () => {
  try {
    await busy(pin, () => currentWindow.setAlwaysOnTop(!pinned));
    renderPin(!pinned);
  } catch (_) {}
});
currentWindow
  .isAlwaysOnTop()
  .then(renderPin)
  .catch(() => renderPin(false))
  .finally(() => (pin.disabled = false));
function screenPermission() {
  if (!panel.classList.contains("hidden")) closePanel();
  cancelWork();
  stream = null;
  setBusy(false);
  empty(
    "需要屏幕录制权限",
    "开启屏幕录制权限后即可截图识别。若系统要求重新打开应用，点击重启。",
    [
      button(
        "打开设置",
        () =>
          invoke("open_permission_settings", { kind: "screen" }).catch((e) =>
            toast(String(e), true),
          ),
        true,
        "settings",
      ),
      button(
        "重启应用",
        () => invoke("restart_app").catch((e) => toast(String(e), true)),
        false,
        "refresh",
      ),
    ],
    "action",
  );
}
function captureFailed(kind) {
  cancelWork();
  stream = null;
  empty(
    kind === "permission" ? "需要辅助功能权限" : "未检测到选中文字",
    kind === "permission"
      ? "请允许辅助功能权限，以读取选中的文字。"
      : "请先在其他应用中选中文字，再按 ⌥D。",
    kind === "permission"
      ? [
          button(
            "打开设置",
            () =>
              invoke("open_permission_settings", {
                kind: "accessibility",
              }).catch((e) => toast(String(e), true)),
            true,
          ),
        ]
      : [],
    kind === "permission" ? "action" : "empty",
  );
}
async function init() {
  input.disabled = true;
  try {
    await Promise.all([
      listen("lucas://translation", (e) => translationEvent(e.payload)),
      listen("lucas://theme-changed", (e) => {
        localStorage.setItem("lucas-theme", e.payload);
        applyTheme();
      }),
      listen("lucas://config-changed", () => {
        if (!$("svc-pop").hidden) refreshServices();
      }),
      listen("lucas://focus-input", () => input.focus()),
      listen("lucas://translate-text", (e) => {
        input.value = e.payload;
        doTranslate();
      }),
      listen("lucas://screen-permission", screenPermission),
      listen("lucas://capture-failed", (e) => captureFailed(e.payload)),
      listen("lucas://ocr-started", () => {
        cancelWork();
        if (!panel.classList.contains("hidden")) closePanel();
        closeServices();
        stream = null;
        captureBusy = true;
        setBusy(true);
        empty("正在识别截图", "识别完成后自动翻译", [], "loading");
      }),
      listen("lucas://ocr-result", (e) => {
        captureBusy = false;
        input.value = String(e.payload || "");
        if (input.value.trim()) doTranslate();
        else {
          setBusy(false);
          empty(
            "未识别到文字",
            "选取清晰的文字区域，再试一次。",
            [
              button("重新截图", () =>
                invoke("start_screenshot", { silent: false }).catch((err) =>
                  toast(String(err), true),
                ),
              ),
            ],
            "empty",
          );
        }
      }),
      listen("lucas://ocr-copied", (e) =>
        toast("已复制 " + e.payload + " 行文字"),
      ),
      listen("lucas://error", (e) => {
        captureBusy = false;
        toast(String(e.payload), true);
        if (!stream || stream.done) {
          setBusy(false);
          empty(
            "操作未完成",
            String(e.payload),
            [
              button("重新截图", () =>
                invoke("start_screenshot", { silent: false }).catch((err) =>
                  toast(String(err), true),
                ),
              ),
            ],
            "error",
          );
        }
      }),
    ]);
    try {
      meta = await catalog();
    } catch (e) {
      toast("服务信息加载失败，可在设置中重试", true);
    }
    booted = true;
    input.disabled = false;
    input.focus();
    resetResult();
  } catch (e) {
    empty("界面初始化失败", String(e), [
      button("重新加载", () => location.reload()),
    ]);
  }
}
window.doTranslate = doTranslate;
init();

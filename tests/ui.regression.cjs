// Real UI assets, synthetic IPC/data; never contacts a translation provider.
const { chromium } = require("playwright");
const assert = require("node:assert/strict");
const { createServer } = require("node:http");
const { readFile } = require("node:fs/promises");
const { join, resolve, extname } = require("node:path");
const { tmpdir } = require("node:os");
const { mkdtemp } = require("node:fs/promises");
const root = resolve(__dirname, "../app/ui");
const config = require("../app/src-tauri/tauri.conf.json");
const catalogue = [
  {
    id: "youdao",
    service: "YoudaoDict",
    label: "有道",
    description: "中英互译",
    logo: "logos/youdao.png",
  },
  {
    id: "bing",
    service: "Bing",
    label: "Bing",
    description: "免费通道",
    logo: "logos/bing.png",
  },
  {
    id: "google",
    service: "GoogleFree",
    label: "Google",
    description: "免费通道",
    logo: "logos/google.png",
  },
];
async function main() {
  const output = await mkdtemp(join(tmpdir(), "lucas-ui-regression-"));
  const server = createServer(async (req, res) => {
    try {
      const path = resolve(
        root,
        "." + decodeURIComponent(new URL(req.url, "http://localhost").pathname),
      );
      if (!path.startsWith(root + "/")) {
        res.writeHead(403).end();
        return;
      }
      const data = await readFile(path);
      res.writeHead(200, {
        "Content-Type":
          {
            ".html": "text/html",
            ".js": "text/javascript",
            ".css": "text/css",
            ".png": "image/png",
            ".woff2": "font/woff2",
          }[extname(path)] || "application/octet-stream",
        "Content-Security-Policy": config.app.security.csp,
      });
      res.end(data);
    } catch (_) {
      res.writeHead(404).end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = "http://127.0.0.1:" + server.address().port;
  const browser = await chromium
    .launch({
      headless: true,
      ...(process.env.CHROME_CHANNEL
        ? { channel: process.env.CHROME_CHANNEL }
        : {}),
    })
    .catch((error) => {
      server.close();
      throw error;
    });
  const context = await browser.newContext({
    viewport: { width: 380, height: 650 },
    colorScheme: "dark",
  });
  const errors = [];
  const checks = [];
  await context.route("**/*", (route) =>
    route
      .request()
      .url()
      .startsWith(base + "/")
      ? route.continue()
      : route.abort(),
  );
  await context.addInitScript(
    ({ catalogue, settingsCanDestroy }) => {
      const listeners = {};
      const mock = (window.__mock = {
        calls: [],
        defer: [],
        reject: [],
        pending: {},
        copied: null,
        values: { youdao: true, bing: true, google: true, deepl: true },
        emit(name, payload) {
          for (const f of listeners["lucas://" + name] || []) f({ payload });
        },
        resolve(name, data) {
          this.pending[name].shift()(data);
        },
      });
      const win = {
        hide: async () => { mock.hidden = true; },
        setTheme: async () => {},
        isAlwaysOnTop: async () => false,
        setAlwaysOnTop: async () => {},
        startDragging: async () => {},
        onCloseRequested: async (fn) => {
          mock.closeRequest = fn;
        },
        close: async () => {
          let prevented = false;
          await mock.closeRequest?.({ preventDefault() { prevented = true; } });
          if (!prevented) {
            if (mock.closeRequest && !settingsCanDestroy) throw Error("window.destroy not allowed");
            mock.closed = true;
          }
        },
      };
      window.__TAURI__ = {
        window: { getCurrentWindow: () => win },
        autostart: {
          isEnabled: async () => false,
          enable: async () => {},
          disable: async () => {},
        },
        event: {
          listen: async (name, fn) => {
            (listeners[name] ||= []).push(fn);
            return () => {};
          },
          emit: async () => {},
        },
        core: {
          invoke: async (name, args) => {
            mock.calls.push({ name, args: structuredClone(args) });
            if (mock.reject.includes(name)) throw Error("mock save failed");
            if (mock.defer.includes(name))
              return new Promise((resolve) =>
                (mock.pending[name] ||= []).push(resolve),
              );
            if (name === "desktop_ready") return new URL(location.href).searchParams.has("omarchy");
            if (name === "service_catalog") return catalogue;
            if (name === "get_services") return structuredClone(mock.values);
            if (name === "set_service") {
              mock.values[args.id] = args.enabled;
              return structuredClone(mock.values);
            }
            if (name === "get_routing")
              return {
                rules: [{ from: "en", to: "zh-Hans" }],
                fallback: "zh-Hans",
              };
            if (name === "get_ai_config")
              return {
                enabled: false,
                base_url: "https://example.invalid/v1",
                model: "mock",
                has_api_key: true,
              };
            if (name === "get_preferences") return {preferences: mock.preferences || {font_size:14,shortcuts:{input:"Alt+A",selection:"Alt+D",screenshot:"Alt+S",ocr:"Alt+C"}}, warnings:[],desktop_managed:new URL(location.href).searchParams.has("omarchy")};
            if (name === "set_preferences") { mock.preferences = args.preferences; return; }
            if (name === "get_official_config") return {enabled:false,pro:false,has_api_key:true};
            if (name === "check_update") return {current:"0.1.0", version:"0.2.0", installable:true};
            if (name === "permission_status")
              return { accessibility: true, screen_capture: true, platform: new URL(location.href).searchParams.has("omarchy") ? "linux" : "macos", desktop_managed: new URL(location.href).searchParams.has("omarchy") };
            if (name === "diagnostics_status")
              return {
                available: true,
                write_failed: false,
                dropped_events: 0,
              };
            if (name === "history_list" || name === "favorites_list") return [];
          },
        },
      };
      Object.defineProperty(navigator, "clipboard", {
        value: { writeText: async (text) => (mock.copied = text) },
      });
    },
    { catalogue, settingsCanDestroy: require("../app/src-tauri/capabilities/settings.json").permissions.includes("core:window:allow-destroy") },
  );
  const page = await context.newPage();
  page.on("pageerror", (e) => errors.push(String(e)));
  const fresh = async () => {
    await page.goto(base + "/index.html");
    await page.waitForLoadState("networkidle");
    await page.waitForFunction(
      () => !document.querySelector("#input").disabled,
    );
  };
  const start = async (text, services = ["Bing"]) => {
    await page.locator("#input").fill(text);
    await page.locator("#input").press("Enter");
    const id = await page.evaluate(
      () =>
        __mock.calls.filter((c) => c.name === "translate").at(-1).args
          .requestId,
    );
    await event(id, "start", {
      services,
      from: "en",
      to: "zh-Hans",
      approximate: true,
      detection: {
        language: "en",
        confidence: 0,
        reliable: false,
        basis: "short_text_fallback",
      },
    });
    return id;
  };
  const event = (request_id, kind, data = {}) =>
    page.evaluate((payload) => __mock.emit("translation", payload), {
      request_id,
      kind,
      data,
    });
  const value = (text, service = "Bing", error = null) => ({
    text: "synthetic",
    service,
    detected_from: "en",
    detected_to: "zh-Hans",
    paragraphs: error ? [] : [text],
    dict: null,
    pinyin: null,
    source_confirmed: false,
    error,
  });
  const check = (name) => {
    checks.push(name);
    console.log("PASS " + name);
  };
  const assertRecoveryVisible = async (service) => {
    const card = page.locator(`.svc[data-service="${service}"]`);
    assert(await card.locator(".state-title").isVisible());
    assert(await card.locator(".state-actions > button").first().isVisible());
    assert.equal(await card.locator(".state-trigger").count(), 0);
  };
  // Inspect text glyph rectangles, not just button clickability. This catches
  // icon-only fixed sizing accidentally applied to a newly labelled button.
  const assertLayout = async (scene) => {
    await page.evaluate(() => document.fonts.ready);
    const defects = await page.evaluate(() => {
      const issues = [];
      for (const node of document.querySelectorAll(".icon")) {
        if (!/^[\uE000-\uF8FF]$/.test(node.textContent))
          issues.push(`Unmapped icon ligature: ${node.textContent}`);
      }
      for (const button of document.querySelectorAll(
        ".btn,.ghost,.nav-item,.segmented button,.panel-tabs button,.chip-btn",
      )) {
        const box = button.getBoundingClientRect();
        if (
          !box.width ||
          !box.height ||
          getComputedStyle(button).visibility === "hidden"
        )
          continue;
        const walker = document.createTreeWalker(button, NodeFilter.SHOW_TEXT);
        while (walker.nextNode()) {
          const node = walker.currentNode;
          if (
            !node.textContent.trim() ||
            node.parentElement.closest(".icon,.tooltip")
          )
            continue;
          const range = document.createRange();
          range.selectNodeContents(node);
          for (const rect of range.getClientRects()) {
            if (
              rect.width &&
              (rect.left < box.left - 1 ||
                rect.right > box.right + 1 ||
                rect.top < box.top - 1 ||
                rect.bottom > box.bottom + 1)
            )
              issues.push(
                `${button.id || button.className}: ${node.textContent.trim()} outside button`,
              );
          }
        }
      }
      for (const container of document.querySelectorAll(
        "html,#result,.content,.view.active,.state-popover,dialog[open]",
      )) {
        if (
          container.clientWidth &&
          container.scrollWidth > container.clientWidth + 1
        )
          issues.push(
            `${container.id || container.className || container.tagName}: horizontal overflow`,
          );
      }
      for (const label of document.querySelectorAll(
        ".retry-svc .btn-label,.action-card .btn-label",
      )) {
        if (
          label.getBoundingClientRect().height >
          parseFloat(getComputedStyle(label).lineHeight) + 1
        )
          issues.push(`${label.textContent}: short action label wraps`);
      }
      return issues;
    });
    if (defects.length) {
      await page.screenshot({ path: join(output, "layout-failure.png") });
      console.log(
        "Layout failure screenshot:",
        join(output, "layout-failure.png"),
      );
      console.log(
        await page.evaluate(() =>
          [...document.querySelectorAll("#result *")]
            .filter(
              (e) =>
                e.getBoundingClientRect().right >
                document.documentElement.clientWidth,
            )
            .map((e) => ({
              cls: e.className,
              width: e.getBoundingClientRect().width,
              right: e.getBoundingClientRect().right,
              scroll: e.scrollWidth,
            })),
        ),
      );
    }
    assert.deepEqual(defects, [], scene);
  };
  try {
    await fresh();
    assert(
      await page.evaluate(
        () =>
          Lucas.icon("future_unknown_glyph").textContent ===
          Lucas.icon("info").textContent,
      ),
    );
    await assertLayout("initial empty state and icon fallback");
    assert(
      (await page.locator("#result").innerText()).includes(
        "输入或划词开始翻译",
      ),
    );
    assert.equal(await page.locator(".shortcut-item kbd").count(), 4);
    assert.equal(await page.locator("#result .state-trigger").count(), 0);
    assert(
      await page
        .locator("#input")
        .evaluate((node) => node === document.activeElement),
    );
    await page.screenshot({ path: join(output, "welcome-dark.png") });
    await page.emulateMedia({ colorScheme: "light" });
    await page.waitForFunction(
      () =>
        document.documentElement.dataset.theme === "light" &&
        getComputedStyle(document.body).backgroundColor ===
          "rgb(255, 255, 255)",
    );
    await page.screenshot({ path: join(output, "welcome-light.png") });
    await page.setViewportSize({ width: 320, height: 560 });
    await assertLayout("welcome at narrow width");
    await page.setViewportSize({ width: 380, height: 650 });
    await page.emulateMedia({ colorScheme: "dark" });
    check(
      "welcome shortcuts and guidance are immediately visible without stealing focus",
    );
    check(
      "unknown icons fall back to a single bundled glyph instead of English text",
    );
    let id = await start("autonomous");
    assert.equal(
      await page.locator("#detect-info").textContent(),
      "语言待确认 · 译至 简体中文",
    );
    assert.equal(
      await page.locator(".svc .lang-info").count(),
      0,
      "pending cards should stay minimal",
    );
    await event(id, "result", {
      ...value("自主的"),
      detected_from: "en",
      source_confirmed: true,
    });
    assert.equal(
      await page.locator("#detect-info").textContent(),
      "检测：英语 · 译至 简体中文",
    );
    assert.equal(
      await page.locator(".svc .lang-info").textContent(),
      "英语 → 简体中文",
    );
    await event(id, "done");
    check("short text stays unknown until a provider confirms its language");
    await fresh();
    await page.locator("#from-lang").selectOption("en");
    id = await start("manual source");
    await event(id, "result", {
      ...value("manual result"),
      detected_from: "fr",
      source_confirmed: true,
    });
    assert.equal(
      await page.locator("#detect-info").textContent(),
      "源语言：英语 · 译至 简体中文",
    );
    assert.equal(
      await page.locator(".svc .lang-info").textContent(),
      "英语 → 简体中文",
    );
    await event(id, "done");
    check("provider detection never overrides a manual source language");
    await fresh();
    id = await start("safe rendering");
    const text =
      'He said "hello".\nIt\'s safe. "><img src=x onerror="window.pwned=true">';
    await event(id, "result", value(text));
    await event(id, "done");
    assert.equal(await page.locator(".svc-body p").textContent(), text);
    assert.equal(await page.locator(".svc-body img").count(), 0);
    await page.locator(".copy-btn").click();
    assert.equal(await page.evaluate(() => __mock.copied), text);
    check("HTML-like text and copy round-trip");
    await fresh();
    const a = await start("request A");
    const b = await start("request B");
    await event(a, "result", value("OLD"));
    await event(a, "done");
    await event(b, "result", value("NEW"));
    await event(b, "done");
    assert.equal(await page.locator(".svc-body p").textContent(), "NEW");
    check("out-of-order response isolation");
    await fresh();
    id = await start("same query");
    await page.locator("#input").press("Enter");
    assert.equal(
      await page.evaluate(
        () => __mock.calls.filter((c) => c.name === "translate").length,
      ),
      1,
    );
    check("duplicate in-flight input coalesced");
    await page.locator("#input").press("Escape");
    await event(id, "result", value("LATE"));
    assert(!(await page.locator("#result").innerText()).includes("LATE"));
    check("cancel ignores late results");
    await fresh();
    await page.locator("#input").fill("cancel before channels load");
    await page.locator("#input").press("Enter");
    await page.locator("#input").press("Escape");
    assert.equal(await page.locator(".skel,.mini-spin").count(), 0);
    assert.equal(
      await page.locator("#result .state-title").textContent(),
      "已取消翻译",
    );
    assert(await page.locator("#work-indicator").isHidden());
    check("cancel before start removes preparation loading");
    await fresh();
    id = await start("multi service", ["Bing", "GoogleFree"]);
    await event(id, "result", value("GOOD"));
    await event(id, "result", value("", "GoogleFree", "网络超时"));
    await event(id, "done");
    await assertRecoveryVisible("GoogleFree");
    await page.getByRole("button", { name: "重试此渠道" }).click();
    const retry = await page.evaluate(
      () => __mock.calls.filter((c) => c.name === "translate").at(-1).args,
    );
    assert.equal(retry.service, "GoogleFree");
    assert((await page.locator("#result").innerText()).includes("GOOD"));
    check("single-channel retry preserves successful cards");
    await event(retry.requestId, "start", {
      services: ["GoogleFree"],
      from: "en",
      to: "zh-Hans",
    });
    await event(retry.requestId, "result", value("", "GoogleFree", "again"));
    await event(retry.requestId, "done");
    await page.evaluate(() => __mock.reject.push("translate"));
    await assertRecoveryVisible("GoogleFree");
    await page.getByRole("button", { name: "重试此渠道" }).click();
    await page.waitForFunction(() => !document.querySelector(".svc.pending"));
    assert((await page.locator("#result").innerText()).includes("GOOD"));
    check("retry IPC failure cannot strand a spinner");
    await fresh();
    id = await start("translation continues");
    await page.evaluate(() => __mock.emit("error", "截图启动失败"));
    assert.equal(
      await page.locator("#result").getAttribute("aria-busy"),
      "true",
    );
    assert(await page.locator("#cancel-work").isVisible());
    await event(id, "result", value("still works"));
    await event(id, "done");
    check("capture error preserves unrelated translation state");
    await page.locator("#btn-history").click();
    await page.evaluate(() => __mock.emit("ocr-started"));
    assert(await page.locator("#panel").isHidden());
    assert.equal(
      await page.locator('#result [data-state="loading"]').count(),
      1,
    );
    assert(
      (await page.locator("#result .state-title").textContent()).length > 0,
    );
    check("OCR progress is not hidden behind history");
    await page.locator("#cancel-work").click();
    assert.equal(
      await page.locator("#result .state-title").textContent(),
      "已取消识别",
    );
    assert(await page.locator("#cancel-work").isHidden());
    check("OCR cancellation has a clear terminal state");
    await page.evaluate(() => {
      __mock.emit("ocr-started");
      __mock.emit("ocr-result", "");
    });
    assert.equal(
      await page.locator("#result .state-title").textContent(),
      "未识别到文字",
    );
    assert(
      await page
        .getByRole("button", { name: "重新截图", exact: true })
        .isVisible(),
    );
    assert(await page.locator("#cancel-work").isHidden());
    assert.equal(await page.locator("#result .state-spinner").count(), 0);
    check("empty OCR result ends loading and directly offers recapture");
    await fresh();
    id = await start("no services", []);
    await event(id, "error", {
      code: "configuration",
      message: "没有启用的翻译服务",
    });
    await event(id, "done");
    assert((await page.locator("#result").innerText()).includes("没有启用"));
    assert(!(await page.locator("#result").innerText()).includes("网络原因"));
    check("configuration error retains correct recovery");
    await fresh();
    await page.evaluate(() =>
      __mock.defer.push("history_list", "favorites_list"),
    );
    await page.locator("#btn-history").click();
    await page.locator('[data-tab="favorites"]').click();
    const item = (id, text) => ({
      id,
      text,
      result: "test result",
      service: "Bing",
      created_at: 1,
    });
    await page.evaluate(
      (i) => __mock.resolve("favorites_list", [i]),
      item(99, "FAVORITE"),
    );
    await page.evaluate(
      (i) => __mock.resolve("history_list", [i]),
      item(7, "HISTORY"),
    );
    await assertLayout("populated favorites");
    assert(
      (await page.locator("#panel-list").innerText()).includes("FAVORITE"),
    );
    assert(
      !(await page.locator("#panel-list").innerText()).includes("HISTORY"),
    );
    await page.getByRole("button", { name: "删除", exact: true }).click();
    assert.equal(
      await page.evaluate(
        () => __mock.calls.find((c) => c.name === "favorite_remove").args.id,
      ),
      99,
    );
    check("history/favorite race cannot retarget deletion");
    await fresh();
    await page.locator("#btn-history").click();
    await page.locator("#panel-clear").click();
    await assertLayout("history confirmation dialog");
    assert.equal(
      await page.evaluate(
        () => __mock.calls.filter((c) => c.name === "history_clear").length,
      ),
      0,
    );
    await page.getByRole("button", { name: "取消", exact: true }).click();
    assert.equal(
      await page.evaluate(
        () => __mock.calls.filter((c) => c.name === "history_clear").length,
      ),
      0,
    );
    check("history clear requires explicit confirmation");
    await fresh();
    await page.locator("#svc-btn").click();
    await page.waitForSelector('input[data-svc="bing"]');
    await assertLayout("service popover");
    await page.locator("#svc-btn").click();
    await page.evaluate(() => (__mock.values.google = false));
    await page.locator("#svc-btn").click();
    await page.locator('input[data-svc="bing"]').uncheck();
    assert.equal(await page.evaluate(() => __mock.values.google), false);
    check("single-field service save preserves external changes");
    await fresh();
    id = await start("long text");
    await event(id, "result", value("x".repeat(600)));
    await event(id, "done");
    assert(
      await page
        .locator("#result")
        .evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
    );
    check("long unbroken text does not overflow");
    await page.screenshot({ path: join(output, "long-dark.png") });
    await fresh();
    id = await start("pending", ["AI", "Bing"]);
    assert.equal(await page.locator(".svc.pending .state-spinner").count(), 2);
    assert.equal(await page.locator(".svc.pending .svc-body").count(), 0);
    assert(
      await page
        .locator('[data-state="loading"] .state-spinner')
        .first()
        .evaluate((e) => e.getAnimations().length > 0),
    );
    assert(
      await page.locator(".svc.pending .state-spinner").evaluateAll((nodes) =>
        nodes.every((node) => {
          const style = getComputedStyle(node);
          // A rotating square's transformed bounding box expands at 45°;
          // layout dimensions test the spinner size without animation-phase flakiness.
          return (
            node.offsetWidth >= 13 &&
            node.offsetWidth <= 18 &&
            node.offsetHeight >= 13 &&
            node.offsetHeight <= 18 &&
            style.borderTopColor !== style.borderBottomColor
          );
        }),
      ),
    );
    assert.equal(await page.locator('img[src="✦"]').count(), 0);
    check("live loading indicator and valid AI icon");
    await page.emulateMedia({ reducedMotion: "reduce" });
    assert.equal(
      await page
        .locator('[data-state="loading"] .state-spinner')
        .first()
        .evaluate((e) => e.getAnimations().length),
      0,
    );
    check("reduced-motion respected");
    await fresh();
    await page.evaluate(() => __mock.emit("screen-permission"));
    await assertLayout("permission guide");
    assert(
      (await page.locator("#result .state-title").textContent()).length > 0,
    );
    assert.equal(
      await page.locator("#result .state-actions button").count(),
      2,
    );
    assert(
      await page
        .locator("#result .state-slot")
        .evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
    );
    await page.screenshot({ path: join(output, "permission-dark.png") });
    await page.emulateMedia({ colorScheme: "light" });
    await page.screenshot({ path: join(output, "permission-light.png") });
    await page.setViewportSize({ width: 320, height: 560 });
    await assertLayout("permission guide at narrow stress width");
    await page.setViewportSize({ width: 380, height: 650 });
    check("permission guide remains compact with two buttons");
    await fresh();
    await page.emulateMedia({ colorScheme: "dark" });
    id = await start("Synthetic text stays here", ["Bing", "GoogleFree"]);
    await event(id, "result", value("A completed result stays usable."));
    const failure = {
      ...value(
        "",
        "GoogleFree",
        "https://example.invalid?q=private-input&key=private-key: status code 429",
      ),
      failure: {
        code: "rate_limited",
        http_status: 429,
        retry_after_secs: 60,
        retry_at_ms: Date.now() + 60000,
        incident_id: "12345678-1234-4234-8234-123456789012",
      },
    };
    await event(id, "result", failure);
    await event(id, "done");
    assert.equal(await page.locator(".svc.failed").count(), 1);
    assert.equal(
      await page.locator(".svc:not(.failed) .svc-body p").textContent(),
      "A completed result stays usable.",
    );
    assert(!(await page.locator("#result").innerText()).includes("private"));
    assert(
      (await page.locator(".svc.failed .state-title").textContent()).length > 0,
    );
    assert.equal(await page.locator(".retry-svc").count(), 1);
    await assertLayout("partial success and rate-limit recovery");
    await page.screenshot({ path: join(output, "recovery-dark.png") });
    await assertRecoveryVisible("GoogleFree");
    assert(await page.locator(".retry-svc").isDisabled());
    await page.locator(".error-details summary").click();
    await page.getByRole("button", { name: "复制诊断", exact: true }).click();
    const diagnostic = await page.evaluate(() => __mock.copied);
    assert(
      diagnostic.includes("429") &&
        diagnostic.includes(failure.failure.incident_id),
    );
    assert(!/private|Synthetic|https:/.test(diagnostic));
    await page.getByRole("button", { name: "查看日志", exact: true }).click();
    assert(
      await page.evaluate(() =>
        __mock.calls.some((c) => c.name === "open_diagnostics"),
      ),
    );
    await assertLayout("expanded diagnostics");
    check(
      "typed 429 preserves other results, hides raw errors, copies only safe diagnostics",
    );
    await page.emulateMedia({ colorScheme: "light" });
    await page.screenshot({ path: join(output, "recovery-light.png") });
    await page.setViewportSize({ width: 320, height: 560 });
    await assertRecoveryVisible("GoogleFree");
    await assertLayout("error actions at narrow stress width");
    await page.setViewportSize({ width: 380, height: 650 });
    await fresh();
    id = await start("no automatic retry", ["GoogleFree"]);
    await event(id, "result", {
      ...failure,
      failure: { ...failure.failure, retry_at_ms: Date.now() + 1200 },
    });
    await event(id, "done");
    await assertRecoveryVisible("GoogleFree");
    assert(await page.locator(".retry-svc").isDisabled());
    await page.waitForFunction(
      () => !document.querySelector(".retry-svc").disabled,
    );
    assert.equal(
      await page.evaluate(
        () => __mock.calls.filter((c) => c.name === "translate").length,
      ),
      1,
    );
    assert.equal(
      await page.locator("#input").inputValue(),
      "no automatic retry",
    );
    assert.equal(await page.locator(".batch-summary").count(), 0);
    await assertLayout("all channels failed and cooldown elapsed");
    check(
      "cooldown expiry enables manual retry without automatically sending text",
    );
    for (const code of [
      "network",
      "timeout",
      "unavailable",
      "unauthorized",
      "forbidden",
      "configuration",
      "unsupported",
      "invalid_request",
      "invalid_response",
      "internal",
      "cancelled",
    ]) {
      await fresh();
      id = await start("error taxonomy", ["GoogleFree"]);
      await event(id, "result", { ...failure, failure: { code } });
      await event(id, "done");
      assert.equal(
        await page.locator(".svc .state-slot").getAttribute("data-error-code"),
        code,
      );
      assert(!(await page.locator("#result").innerText()).includes("private"));
      assert(
        (await page.locator(".svc.failed .state-title").textContent()).length >
          0,
      );
      await assertRecoveryVisible("GoogleFree");
      await assertLayout(code);
      const settings = [
        "unauthorized",
        "forbidden",
        "configuration",
        "unsupported",
      ].includes(code);
      assert.equal(
        await page
          .getByRole("button", { name: "服务设置", exact: true })
          .count(),
        settings ? 1 : 0,
      );
      assert.equal(await page.locator(".retry-svc").count(), settings ? 0 : 1);
    }
    check(
      "all error categories have readable, correctly targeted recovery actions",
    );
    await fresh();
    await page.emulateMedia({
      colorScheme: "dark",
      reducedMotion: "no-preference",
    });
    id = await start("Independent channels", ["Bing", "GoogleFree", "AI"]);
    assert.equal(await page.locator(".svc.pending").count(), 3);
    assert.equal(await page.locator('.svc [data-state="loading"]').count(), 3);
    assert.equal(await page.locator(".svc.pending .svc-body").count(), 0);
    assert.equal(await page.locator(".svc.pending .state-spinner").count(), 3);
    assert(await page.locator("#work-indicator").isHidden());
    assert(
      await page
        .locator(".state-art")
        .evaluateAll((nodes) =>
          nodes.every(
            (node) =>
              node.getBoundingClientRect().width <= 32 &&
              node.getBoundingClientRect().height <= 32,
          ),
        ),
    );
    await assertLayout("three independent small loading states");
    await page.screenshot({ path: join(output, "channels-loading-dark.png") });
    await page.emulateMedia({ colorScheme: "light" });
    assert(
      await page.locator(".svc.pending .state-spinner").evaluateAll((nodes) =>
        nodes.every((node) => {
          const style = getComputedStyle(node);
          return style.borderTopColor !== style.borderBottomColor;
        }),
      ),
    );
    await page.screenshot({ path: join(output, "channels-loading-light.png") });
    await page.emulateMedia({ colorScheme: "dark" });
    await event(id, "result", value("A real result appears immediately."));
    await event(id, "result", { ...failure, failure: { code: "network" } });
    assert.equal(await page.locator(".svc.pending").count(), 1);
    assert.equal(await page.locator(".svc.failed").count(), 1);
    await assertRecoveryVisible("GoogleFree");
    assert(await page.locator(".retry-svc").isDisabled());
    await page.locator(".error-details summary").click();
    await page.keyboard.press("Escape");
    assert.equal(await page.locator(".error-details[open]").count(), 0);
    assert.equal(await page.locator(".state-popover").count(), 0);
    assert.equal(
      await page.locator("#input").inputValue(),
      "Independent channels",
    );
    assert.equal(
      await page.evaluate(
        () => document.activeElement.closest(".svc")?.dataset.service,
      ),
      "GoogleFree",
    );
    assert.equal(
      await page.locator("#result").getAttribute("aria-busy"),
      "true",
    );
    await page.screenshot({ path: join(output, "channels-mixed-dark.png") });
    await event(id, "result", value("AI completed independently.", "AI"));
    await event(id, "done");
    await assertRecoveryVisible("GoogleFree");
    assert(await page.locator(".retry-svc").isEnabled());
    await page.locator("#input").click();
    assert.equal(await page.locator(".state-popover").count(), 0);
    check(
      "independent channel loading, immediate results, minimal failure, and Escape focus recovery",
    );
    await fresh();
    id = await start("Every channel failed", ["Bing", "GoogleFree", "AI"]);
    for (const service of ["Bing", "GoogleFree", "AI"])
      await event(id, "result", {
        ...failure,
        service,
        failure: { code: "network" },
      });
    await event(id, "done");
    assert.equal(await page.locator(".svc.failed").count(), 3);
    assert.deepEqual(
      await page.locator(".svc .state-title").allTextContents(),
      ["连接异常", "连接异常", "连接异常"],
    );
    assert.equal(await page.locator(".svc .retry-svc").count(), 3);
    assert.equal(
      await page.locator(".batch-summary,.recovery-copy,.action-card").count(),
      0,
    );
    assert(!(await page.locator("#result").innerText()).includes("未完成"));
    await page.screenshot({ path: join(output, "channels-failed-dark.png") });
    await page.emulateMedia({ colorScheme: "light" });
    await page.screenshot({ path: join(output, "channels-failed-light.png") });
    await assertRecoveryVisible("Bing");
    await page
      .locator('.svc[data-service="GoogleFree"] .error-details summary')
      .focus();
    await page.keyboard.press("Enter");
    assert.equal(await page.locator(".error-details[open]").count(), 1);
    assert.equal(await page.locator(".state-popover").count(), 0);
    await page.locator('.svc[data-service="GoogleFree"] .svc-toggle').click();
    assert(
      await page
        .locator('.svc[data-service="GoogleFree"] .svc-body')
        .isHidden(),
    );
    check(
      "all failures show concise reasons and recovery; only diagnostics need expansion",
    );
    await fresh();
    await page.locator("#btn-history").click();
    assert.equal(
      await page.locator('#panel-list [data-state="empty"]').count(),
      1,
    );
    assert.equal(await page.locator("#panel-list .state-trigger").count(), 0);
    assert(
      (await page.locator("#panel-list").innerText()).includes("完成一次翻译"),
    );
    await page.keyboard.press("Escape");
    assert(await page.locator("#panel").isHidden());
    check(
      "empty history shows guidance immediately; Escape returns to translation",
    );
    await page.setViewportSize({ width: 640, height: 470 });
    await page.goto(base + "/settings.html");
    await page.waitForLoadState("networkidle");
    await page.locator("#settings-close").click();
    await page.waitForFunction(() => __mock.closed);
    await page.evaluate(() => { delete __mock.closed; });
    await page.keyboard.press("Escape");
    await page.waitForFunction(() => __mock.closed);
    await page.evaluate(() => { delete __mock.closed; });
    check("clean settings close through button and Escape with destroy permission");
    await page.locator('[data-view="services"]').click();
    await page.waitForSelector('input[data-svc="bing"]');
    assert.equal(await page.locator('input[data-svc="baidu"]').count(), 0);
    await page.evaluate(() => __mock.reject.push("set_service"));
    await page.locator('input[data-svc="bing"]').click();
    await page.waitForFunction(
      () => document.querySelector('input[data-svc="bing"]').checked,
    );
    assert(await page.locator(".toast-error").count());
    check("failed service save rolls back and reports error");
    await page.evaluate(() => {
      __mock.reject = [];
      __mock.defer.push("set_service");
    });
    await page.locator('input[data-svc="bing"]').click();
    assert(await page.locator('input[data-svc="google"]').isDisabled());
    await page.evaluate(() => __mock.resolve("set_service", {}));
    await page.waitForFunction(
      () => !document.querySelector('input[data-svc="google"]').disabled,
    );
    check("service save prevents overlapping stale UI writes");
    assert.equal(await page.locator("#ai-key").inputValue(), "");
    await page.locator("#ai-model").fill("new-model");
    await page.evaluate(() => __mock.reject.push("set_ai_config"));
    await page.locator("#ai-save").click();
    await page.waitForFunction(() =>
      document.querySelector("#ai-status").textContent.includes("失败"),
    );
    assert.equal(await page.locator("#ai-model").inputValue(), "new-model");
    check("AI save failure preserves input without exposing saved key");
    await page.evaluate(() => {
      __mock.closeRequest({
        preventDefault() {
          __mock.closePrevented = true;
        },
      });
    });
    await page.waitForSelector("dialog[open]");
    await assertLayout("unsaved settings confirmation dialog");
    assert.equal(await page.evaluate(() => __mock.closed), undefined);
    await page.getByRole("button", { name: "取消", exact: true }).click();
    assert.equal(await page.locator("#ai-model").inputValue(), "new-model");
    await page.locator("#settings-close").click();
    await page.waitForSelector("dialog[open]");
    await page.keyboard.press("Escape");
    assert.equal(await page.evaluate(() => __mock.closed), undefined);
    await page.locator("#settings-close").click();
    await page.getByRole("button", { name: "放弃修改", exact: true }).click();
    await page.waitForFunction(() => __mock.closed);
    await page.evaluate(() => { delete __mock.closed; });
    await page.locator("#settings-quit").click();
    await page.getByRole("button", { name: "取消", exact: true }).click();
    assert.equal(await page.evaluate(() => __mock.calls.some(c => c.name === "quit_app")), false);
    await page.locator("#settings-quit").click();
    await page.getByRole("button", { name: "放弃修改", exact: true }).click();
    await page.waitForFunction(() => __mock.calls.some(c => c.name === "quit_app"));
    check("dirty settings close and quit support cancel and discard; Escape cancels dialog");
    await page.screenshot({ path: join(output, "settings-light.png") });
    await page.locator('[data-view="permissions"]').click();
    await page.evaluate(() => __mock.reject.push("permission_status"));
    await page.locator("#refresh").click();
    await page.waitForFunction(
      () => document.querySelector("#chip-screen").textContent === "检测失败",
    );
    check("permission check failure has a terminal state");
    await page.evaluate(() => {
      __mock.reject = [];
      __mock.emit("config-changed");
    });
    assert.equal(await page.locator("#ai-model").inputValue(), "new-model");
    check("settings notifications preserve unsaved AI draft");
    for (const colorScheme of ["light", "dark"]) {
      await page.emulateMedia({ colorScheme });
      for (const viewport of [
        { width: 580, height: 420 },
        { width: 640, height: 470 },
        { width: 900, height: 600 },
      ]) {
        await page.setViewportSize(viewport);
        for (const view of [
          "general",
          "services",
          "permissions",
          "hotkeys",
          "about",
        ]) {
          await page.locator(`[data-view="${view}"]`).click();
          await assertLayout(
            `settings ${view} ${viewport.width} ${colorScheme}`,
          );
        }
      }
    }
    await page.waitForFunction(() =>
      document.querySelector("#log-status").textContent.includes("已启用"),
    );
    await page.locator("#open-logs").click();
    assert(
      await page.evaluate(() =>
        __mock.calls.some((c) => c.name === "open_diagnostics"),
      ),
    );
    check(
      "all settings pages, both themes and three sizes keep text inside controls",
    );
    await page.evaluate(() => {
      __mock.reject.push("get_routing", "get_services");
      __mock.emit("config-changed");
    });
    await page.locator('[data-view="general"]').click();
    await page.waitForSelector("#rule-list .state-title");
    assert(
      await page
        .locator("#rule-list")
        .getByRole("button", { name: "重新加载" })
        .isEnabled(),
    );
    await page.evaluate(() => {
      __mock.reject = [];
    });
    await page
      .locator("#rule-list")
      .getByRole("button", { name: "重新加载" })
      .click();
    await page.waitForSelector("#rule-list .rule-row");
    await page.locator('[data-view="services"]').click();
    assert(await page.locator("#svc-list .state-title").isVisible());
    await page
      .locator("#svc-list")
      .getByRole("button", { name: "重新加载" })
      .click();
    await page.waitForSelector('#svc-list input[data-svc="bing"]');
    check(
      "settings load failures expose enabled recovery buttons and recover in place",
    );
    await page.locator("#official-key").fill("synthetic-official-key");
    await page.evaluate(() => __mock.reject.push("set_official_config"));
    await page.locator("#official-save").click();
    await page.waitForFunction(() => !document.querySelector("#official-save").disabled);
    assert.equal(await page.locator("#official-key").inputValue(), "synthetic-official-key");
    await page.locator("#official-test").click();
    assert.equal(await page.evaluate(() => __mock.calls.filter(c => c.name === "test_official_connection").length), 0);
    await page.evaluate(() => { __mock.reject = []; });
    await page.locator("#official-save").click();
    await page.waitForFunction(() => !document.querySelector("#official-save").disabled);
    assert.equal(await page.locator("#official-key").inputValue(), "");
    await page.locator("#official-test").click();
    await page.waitForFunction(() => document.querySelector("#official-status").textContent.includes("连接正常"));
    check("official credentials preserve failed edits, never return saved keys, and test only saved configuration");
    await page.locator('[data-view="hotkeys"]').click();
    await page.locator("#hotkey-input").fill("Ctrl+Shift+A");
    await page.evaluate(() => __mock.reject.push("set_preferences"));
    await page.locator("#hotkey-save").click();
    await page.waitForFunction(() => !document.querySelector("#hotkey-save").disabled);
    assert.equal(await page.locator("#hotkey-input").inputValue(), "Ctrl+Shift+A");
    await page.evaluate(() => { __mock.reject = []; });
    await page.locator("#hotkey-save").click();
    await page.waitForFunction(() => document.querySelector("#hotkey-status").textContent.includes("已保存"));
    assert.equal(await page.evaluate(() => __mock.preferences.shortcuts.input), "Ctrl+Shift+A");
    await page.screenshot({path:join(output,"custom-hotkeys.png")});
    await page.locator('[data-view="general"]').click();
    await page.locator("#reading-size").selectOption("20");
    await page.getByRole("button", {name:"保存字号",exact:true}).click();
    await page.waitForFunction(() => document.documentElement.style.getPropertyValue("--reading-size") === "20px");
    assert.equal(await page.evaluate(() => __mock.preferences.shortcuts.input), "Ctrl+Shift+A");
    check("shortcut save failure preserves draft; font save preserves custom shortcuts");
    await page.locator('[data-view="about"]').click();
    await page.evaluate(() => __mock.reject.push("check_update"));
    await page.locator("#update-check").click();
    await page.waitForFunction(() => !document.querySelector("#update-check").disabled);
    assert(await page.locator("#update-install").isHidden());
    await page.evaluate(() => { __mock.reject = []; __mock.defer.push("install_update"); });
    await page.locator("#update-check").click();
    await page.waitForSelector("#update-install", {state:"visible"});
    await page.locator("#update-install").click();
    assert((await page.locator("#update-status").textContent()).includes("请先保存"));
    await page.locator('[data-view="services"]').click();
    await page.locator("#ai-save").click();
    await page.waitForFunction(() => !window.LucasAiUnsaved());
    await page.locator('[data-view="about"]').click();
    await page.locator("#update-install").click();
    await page.getByRole("button", {name:"取消",exact:true}).click();
    assert.equal(await page.evaluate(() => __mock.calls.filter(c=>c.name === "install_update").length),0);
    await page.locator("#update-install").click();
    await page.getByRole("button", {name:"安装并重启",exact:true}).click();
    await page.waitForFunction(() => __mock.pending.install_update?.length);
    await page.evaluate(() => __mock.emit("update-progress", {phase:"downloading",downloaded:20,total:100}));
    assert.equal(await page.locator("#update-progress").getAttribute("value"),"20");
    await assertLayout("signed updater progress and controls");
    await page.screenshot({path:join(output,"signed-updater.png")});
    await page.getByRole("button", {name:"取消下载",exact:true}).click();
    assert.equal(await page.evaluate(() => __mock.calls.filter(c=>c.name === "cancel_update").length),1);
    await page.evaluate(() => __mock.resolve("install_update"));
    await page.waitForFunction(() => !document.querySelector("#update-check").disabled);
    check("update check failures recover; install requires confirmation; download progress and cancel remain visible");
    await page.goto(base + "/index.html");
    await page.waitForLoadState("networkidle");
    await page.locator("#input").fill("preserve on Escape");
    await page.locator("#input").press("Escape");
    assert.equal(await page.locator("#input").inputValue(),"preserve on Escape");
    assert.equal(await page.evaluate(() => __mock.hidden),true);
    check("idle Escape hides the panel without destroying input");
    await page.goto(base + "/settings.html?omarchy=1");
    await page.waitForLoadState("networkidle");
    await page.locator('[data-view="hotkeys"]').click();
    assert(await page.locator("#hotkey-save").isHidden());
    assert((await page.locator("#hotkey-hint").textContent()).includes("hyprctl reload"));
    await page.locator('[data-view="permissions"]').click();
    assert((await page.locator("#view-permissions").textContent()).includes("Wayland PRIMARY"));
    assert(await page.locator("#open-accessibility").isHidden());
    await page.locator("#settings-quit").click();
    await page.waitForFunction(() => __mock.calls.some(c => c.name === "quit_app"));
    check("Omarchy settings show desktop shortcut management, Wayland guidance and working quit");
    await page.goto(base + "/index.html?omarchy=1");
    await page.waitForLoadState("networkidle");
    assert(await page.locator(".titlebar").isHidden());
    assert(await page.locator("#btn-pin").isHidden());
    assert(await page.locator(".statusbar #btn-settings").isVisible());
    await page.locator("#btn-settings").click();
    assert(await page.evaluate(() => __mock.calls.some(c => c.name === "open_settings")));
    assert(await page.evaluate(() => __mock.calls.some(c => c.name === "setup_desktop")));
    check("Omarchy hides window controls, keeps settings accessible, and enables integration");
    await page.locator("#input").fill("preserve Omarchy input");
    await page.locator("#btn-history").click();
    await page.keyboard.press("Escape");
    assert.equal(await page.evaluate(() => __mock.hidden), true);
    assert(await page.locator("#panel").isHidden());
    assert.equal(await page.locator("#input").inputValue(), "preserve Omarchy input");
    check("Omarchy Escape closes history and hides immediately without clearing input");
    assert.deepEqual(errors, []);
    check("no uncaught JavaScript errors");
    console.log(
      JSON.stringify({ checks: checks.length, screenshots: output }, null, 2),
    );
  } finally {
    await browser.close();
    await new Promise((resolve) => server.close(resolve));
  }
}
main().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});

const { test } = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

function overlay() {
  const handlers = new Map();
  const listeners = new Map();
  const events = [];
  const nodes = Object.fromEntries(["mask", "selection", "tip", "label"].map(id => [id, { style: {} }]));
  nodes.selection.querySelector = () => nodes.label;
  let registered;
  const win = {
    visible: false,
    async show() { this.visible = true; },
    async hide() { this.visible = false; },
    async setFocus() {},
  };
  const window = {
    addEventListener: (name, handler) => handlers.set(name, handler),
    __TAURI__: {
      event: {
        listen(name, handler) {
          listeners.set(name, handler);
          return new Promise(resolve => { registered = resolve; });
        },
        async emit(name, payload) { events.push({ name, payload }); },
      },
      window: { getCurrentWindow: () => win },
    },
  };
  vm.runInNewContext(readFileSync(join(__dirname, "../app/ui/overlay.js"), "utf8"), {
    window, document: { getElementById: id => nodes[id] },
  });
  return {
    events, nodes, win,
    async ready() { registered(); await new Promise(setImmediate); },
    async init(silent) { await listeners.get("lucas://overlay-init")({ payload: silent }); },
    async fire(name, event) { handlers.get(name)(event); await new Promise(setImmediate); },
  };
}

const mouse = (screenX, screenY) => ({ button: 0, screenX, screenY, clientX: screenX - 1000, clientY: screenY - 200 });

test("ready handshake waits for listener registration", async () => {
  const ui = overlay();
  assert.equal(ui.events.length, 0);
  await ui.ready();
  assert.equal(ui.events[0].name, "lucas://overlay-ready");
});

test("consecutive captures reset the overlay and preserve the current mode", async () => {
  const ui = overlay();
  await ui.ready();
  for (const silent of [false, true, false]) {
    await ui.init(silent);
    assert.equal(ui.win.visible, true);
    await ui.fire("mousedown", mouse(1100, 300));
    await ui.fire("mousemove", mouse(1200, 350));
    assert.equal(ui.nodes.selection.style.left, "100px");
    assert.equal(ui.nodes.selection.style.top, "100px");
    await ui.fire("mouseup", mouse(1200, 350));
    const selected = ui.events.filter(e => e.name === "lucas://region-selected").at(-1);
    assert.deepEqual(JSON.parse(JSON.stringify(selected.payload)), { x: 1100, y: 300, w: 100, h: 50, silent, capture_id: 0 });
    await ui.fire("keydown", { key: "Escape" });
    await ui.fire("mousedown", mouse(1100, 300));
    await ui.fire("mouseup", mouse(1200, 350));
  }
  assert.equal(ui.events.filter(e => e.name === "lucas://region-selected").length, 3);
  assert.equal(ui.events.filter(e => e.name === "lucas://overlay-cancelled").length, 0);
});

test("small selections can be redrawn; cancellation permits a fresh capture", async () => {
  const ui = overlay();
  await ui.ready();
  await ui.init(false);
  await ui.fire("mousedown", mouse(1100, 300));
  await ui.fire("mouseup", mouse(1102, 302));
  assert.equal(ui.win.visible, true);
  await ui.fire("keydown", { key: "Escape" });
  assert.equal(ui.win.visible, false);
  assert.equal(ui.events.filter(e => e.name === "lucas://overlay-cancelled").length, 1);
  await ui.init(false);
  await ui.fire("mousedown", mouse(1100, 300));
  await ui.fire("mouseup", mouse(1200, 350));
  assert.equal(ui.events.filter(e => e.name === "lucas://region-selected").length, 1);
});

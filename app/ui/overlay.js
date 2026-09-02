// 截图选区覆盖层逻辑
const { emit } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const mask = document.getElementById("mask");
const sel = document.getElementById("selection");
const label = sel.querySelector(".size-label");
const tip = document.getElementById("tip");

let silent = false;
let start = null;
let dragging = false;

// 后端告知工作模式
window.__TAURI__.event.listen("lucas://overlay-init", (e) => {
  silent = !!e.payload;
  tip.textContent = silent
    ? "拖拽框选，识别结果将直接复制到剪贴板（Esc 取消）"
    : "拖拽框选识别区域（Esc 取消）";
});

async function cancel() {
  start = null;
  dragging = false;
  sel.style.display = "none";
  mask.style.display = "block";
  await getCurrentWindow().hide();
}

window.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  dragging = true;
  start = { x: e.screenX, y: e.screenY };
  sel.style.display = "block";
  sel.style.left = e.screenX + "px";
  sel.style.top = e.screenY + "px";
  sel.style.width = "0px";
  sel.style.height = "0px";
  mask.style.display = "none";
  tip.style.display = "none";
});

window.addEventListener("mousemove", (e) => {
  if (!dragging || !start) return;
  const x = Math.min(start.x, e.screenX);
  const y = Math.min(start.y, e.screenY);
  const w = Math.abs(e.screenX - start.x);
  const h = Math.abs(e.screenY - start.y);
  sel.style.left = x + "px";
  sel.style.top = y + "px";
  sel.style.width = w + "px";
  sel.style.height = h + "px";
  label.textContent = `${w} × ${h}`;
});

window.addEventListener("mouseup", (e) => {
  if (!dragging || !start || e.button !== 0) return;
  dragging = false;
  const x = Math.min(start.x, e.screenX);
  const y = Math.min(start.y, e.screenY);
  const w = Math.abs(e.screenX - start.x);
  const h = Math.abs(e.screenY - start.y);
  start = null;

  // 忽略过小的选区（误点击）
  if (w < 10 || h < 10) {
    sel.style.display = "none";
    mask.style.display = "block";
    tip.style.display = "block";
    return;
  }
  sel.style.display = "none";
  emit("lucas://region-selected", { x, y, w, h, silent });
});

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") cancel();
});

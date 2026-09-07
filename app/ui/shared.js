// Shared, dependency-free UI primitives. User/provider text is never HTML.
(() => {
  const $ = (id) => document.getElementById(id);
  const invoke = (...args) => window.__TAURI__.core.invoke(...args);
  function el(tag, attrs = {}, ...children) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {
      if (value == null) continue;
      if (key === "class") node.className = value;
      else if (key === "text") node.textContent = value;
      else if (key === "onClick") node.addEventListener("click", value);
      else if (key === "disabled") node.disabled = value;
      else node.setAttribute(key, String(value));
    }
    for (const child of children.flat())
      if (child != null)
        node.append(
          child.nodeType ? child : document.createTextNode(String(child)),
        );
    return node;
  }
  // Only glyphs present in our bundled subset. Codepoints avoid rendering an
  // unknown ligature name as a long English word (and breaking flex layouts).
  const glyphs = {
    delete: 0xe872,
    search: 0xe8b6,
    check_circle: 0xe86c,
    info: 0xe88e,
    schedule: 0xe192,
    refresh: 0xe5d5,
    content_copy: 0xe14d,
    keyboard: 0xe312,
    lock: 0xe88d,
    verified_user: 0xe8e8,
    apps: 0xe5c3,
    expand_more: 0xe5cf,
    unfold_less: 0xe5d6,
    unfold_more: 0xe5d7,
    swap_horiz: 0xe8d4,
    close: 0xe14c,
    cloud_off: 0xe2c1,
    dark_mode: 0xe51c,
    light_mode: 0xe518,
    tune: 0xe429,
    volume_up: 0xe050,
    star: 0xe838,
    wifi_off: 0xe648,
    settings: 0xe8b8,
    translate: 0xe8e2,
  };
  const glyphText = (name) => String.fromCodePoint(glyphs[name] || glyphs.info);
  const icon = (name) =>
    el("span", { class: "icon", "aria-hidden": "true", text: glyphText(name) });
  for (const node of document.querySelectorAll(".icon")) {
    node.textContent = glyphText(node.textContent.trim());
    node.setAttribute("aria-hidden", "true");
  }
  const button = (label, onClick, primary = false, glyph) =>
    el(
      "button",
      { type: "button", class: `btn${primary ? " primary" : ""}`, onClick },
      glyph ? icon(glyph) : null,
      label ? el("span", { class: "btn-label", text: label }) : null,
    );
  function toast(message, error = false, action) {
    document.querySelector(".toast")?.remove();
    const node = el(
      "div",
      {
        class: `toast${error ? " toast-error" : ""}`,
        role: error ? "alert" : "status",
      },
      el("span", { text: message }),
    );
    let timer;
    if (action)
      node.append(
        button(action.label, async () => {
          clearTimeout(timer);
          node.remove();
          await action.run();
        }),
      );
    document.body.append(node);
    timer = setTimeout(() => node.remove(), action || error ? 8000 : 2600);
  }
  async function busy(control, work, success) {
    if (control.disabled) return;
    control.disabled = true;
    control.setAttribute("aria-busy", "true");
    try {
      const result = await work();
      if (success) toast(success);
      return result;
    } catch (e) {
      toast(String(e), true);
      throw e;
    } finally {
      control.disabled = false;
      control.removeAttribute("aria-busy");
    }
  }
  function confirmAction(title, description, accept = "确认") {
    // A repeated native close request must not stack modal dialogs.
    if (document.querySelector("dialog[open]")) return Promise.resolve(false);
    return new Promise((resolve) => {
      const previous = document.activeElement;
      const dialog = el("dialog", {
        class: "confirm-dialog",
        "aria-labelledby": "confirm-title",
      });
      const finish = (result) => {
        dialog.close();
        dialog.remove();
        previous?.focus();
        resolve(result);
      };
      const cancel = button("取消", () => finish(false));
      dialog.append(
        el("h2", { id: "confirm-title", text: title }),
        el("p", { text: description }),
        el(
          "div",
          { class: "actions" },
          cancel,
          button(accept, () => finish(true), true),
        ),
      );
      dialog.addEventListener("cancel", (event) => {
        event.preventDefault();
        finish(false);
      });
      document.body.append(dialog);
      dialog.showModal();
      cancel.focus();
    });
  }
  const names = {
    auto: "自动",
    "zh-Hans": "简体中文",
    "zh-Hant": "繁体中文",
    en: "英语",
    ja: "日语",
    ko: "韩语",
    fr: "法语",
    de: "德语",
    ru: "俄语",
    es: "西班牙语",
  };
  let catalogue;
  function catalog() {
    if (!catalogue)
      catalogue = invoke("service_catalog").catch((e) => {
        catalogue = null;
        throw e;
      });
    return catalogue;
  }
  function logo(meta) {
    if (/^logos\/[a-z]+\.png$/.test(meta.logo || "")) {
      const img = el("img", { class: "svc-logo-img", src: meta.logo, alt: "" });
      img.addEventListener("error", () => img.replaceWith(icon("translate")), {
        once: true,
      });
      return img;
    }
    return icon("translate");
  }
  function toggle(id, label, checked, change) {
    const input = el("input", {
      type: "checkbox",
      "data-svc": id,
      "aria-label": label,
    });
    input.checked = checked;
    input.addEventListener("change", () => change(input));
    return el(
      "label",
      { class: "switch" },
      input,
      el("span", { class: "slider", "aria-hidden": "true" }),
    );
  }
  function theme(window) {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const pref = localStorage.getItem("lucas-theme") || "system";
      const resolved =
        pref === "system" ? (media.matches ? "dark" : "light") : pref;
      document.documentElement.dataset.theme = resolved;
      window.setTheme(pref === "system" ? null : resolved).catch(() => {});
    };
    apply();
    media.addEventListener("change", apply);
    return apply;
  }
  window.Lucas = {
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
  };
})();

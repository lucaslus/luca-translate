// Essential feedback stays visible. Only supplementary details are collapsible.
(() => {
  const { el, icon } = window.Lucas;
  const glyphs = {
    idle: "translate",
    empty: "search",
    error: "cloud_off",
    action: "lock",
    success: "check_circle",
  };
  function avatar(state) {
    return el(
      "span",
      { class: "state-art", "aria-hidden": "true" },
      state === "loading"
        ? el("span", { class: "state-spinner" })
        : icon(glyphs[state] || "info"),
    );
  }
  function create(state, options = {}) {
    const label = options.label || "暂时无法完成操作";
    const node = el(
      "div",
      {
        class:
          "state-slot" + (options.compact ? " state-compact" : " state-inline"),
        "data-state": state,
        ...(options.compact
          ? { role: "img", "aria-label": label, title: label }
          : {}),
      },
      avatar(state),
    );
    if (options.compact) return node;
    const body = el(
      "div",
      { class: "state-content" },
      el("p", { class: "state-title", text: label }),
    );
    if (options.description)
      body.append(
        el("p", { class: "state-description", text: options.description }),
      );
    if (options.content) body.append(options.content());
    if (options.actions?.length)
      body.append(el("div", { class: "state-actions" }, options.actions));
    node.append(body);
    return node;
  }
  function close() {
    document.querySelectorAll(".state-slot details[open]").forEach((node) => {
      node.open = false;
    });
  }
  // Escape inside secondary details returns to their summary without clearing
  // the input or cancelling another provider's work.
  window.addEventListener(
    "keydown",
    (event) => {
      const detail = event.target.closest?.(".state-slot details[open]");
      if (event.key !== "Escape" || !detail) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      detail.open = false;
      detail.querySelector("summary")?.focus();
    },
    true,
  );
  document.addEventListener("visibilitychange", () => {
    document.documentElement.classList.toggle("state-paused", document.hidden);
  });
  window.LucasState = { create, avatar, close };
})();

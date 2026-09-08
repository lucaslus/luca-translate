// Recovery copy is allowlisted. Raw transport errors never enter the DOM.
(() => {
  const { el, button } = window.Lucas;
  const states = {
    rate_limited: {
      description: "请求暂时受限。冷却期间不会重复请求，其他渠道仍可使用。",
      tag: "暂时限流",
    },
    network: {
      description:
        "暂时连接不上这个服务。检查网络或代理后再试，也可以使用其他渠道。",
      tag: "连接异常",
    },
    timeout: {
      description: "已停止等待，不影响其他结果。稍后可以只重试这个渠道。",
      tag: "等待超时",
    },
    unavailable: {
      description: "服务端暂时无法处理请求，稍后再来试试。",
      tag: "暂不可用",
    },
    unauthorized: {
      description: "密钥可能无效或已过期，请在翻译服务设置中检查。",
      tag: "需要配置",
      settings: true,
    },
    forbidden: {
      description: "服务拒绝了请求，请检查账户权限或改用其他渠道。",
      tag: "访问受限",
      settings: true,
    },
    configuration: {
      description: "检查服务地址、模型和系统凭据存储后，就可以继续翻译。",
      tag: "需要配置",
      settings: true,
    },
    unsupported: {
      description: "该渠道暂不支持当前语言组合，可以调整语向或使用其他渠道。",
      tag: "语向不支持",
      settings: true,
    },
    invalid_request: {
      description: "服务未接受这次请求。可以缩短文本，或调整源语言与目标语言。",
      tag: "请求未接受",
    },
    invalid_response: {
      description: "返回内容暂时无法识别，可以稍后重试或使用其他渠道。",
      tag: "返回异常",
    },
    cancelled: {
      description: "已取消等待；完成的结果会继续保留。",
      tag: "已取消",
    },
    internal: {
      description: "原文已保留。可以重试这个渠道，或复制诊断信息反馈问题。",
      tag: "暂时无法翻译",
    },
  };
  function info(result) {
    const supplied = result.failure || {};
    const code = Object.hasOwn(states, supplied.code)
      ? supplied.code
      : /status code 429\b/.test(result.error || "")
        ? "rate_limited"
        : (result.error || "").includes("已取消")
          ? "cancelled"
          : (result.error || "").includes("超时")
            ? "timeout"
            : "internal";
    const finite = (v) => (Number.isFinite(v) && v >= 0 ? v : null);
    return {
      code,
      http: finite(supplied.http_status),
      retryAt: finite(supplied.retry_at_ms) || 0,
      incident: safeId(supplied.incident_id),
    };
  }
  function safeId(value) {
    return /^[a-f0-9-]{36}$/i.test(value || "") ? value : "未分配";
  }
  function diagnostic(result) {
    const e = info(result),
      service = [
        "Bing",
        "GoogleFree",
        "DeepLFree",
        "DeepLApi",
        "YoudaoDict",
        "AI",
      ].includes(result.service)
        ? result.service
        : "unknown";
    return [
      "Lucas Translate " + (window.LucasVersion || "unknown"),
      `渠道：${service}`,
      `类别：${e.code}`,
      `HTTP：${e.http || "无"}`,
      `诊断 ID：${e.incident}`,
      `请求 ID：${safeId(result._requestId)}`,
    ].join("\n");
  }
  function render(result, { retry, settings, copy, logs }) {
    const e = info(result),
      state = states[e.code];
    const content = () => {
      const action =
        e.code === "unsupported" || state.settings
          ? button("服务设置", settings, false, "tune")
          : button("重试此渠道", retry, false, "refresh");
      if (!state.settings) {
        action.classList.add("retry-svc");
        action.dataset.retryAt = String(e.retryAt);
      }
      const details = el(
        "details",
        { class: "error-details" },
        el("summary", { text: "诊断信息" }),
        el("p", { text: state.description }),
        el("p", {
          text: `${state.tag}${e.http ? " · HTTP " + e.http : ""} · 不包含原文或密钥`,
        }),
        el(
          "div",
          { class: "state-actions" },
          button("复制诊断", () => copy(diagnostic(result))),
          button("查看日志", logs),
        ),
      );
      return el(
        "div",
        { class: "error-recovery" },
        el("div", { class: "state-actions" }, action),
        details,
      );
    };
    const node = window.LucasState.create("error", {
      label: state.tag,
      content,
    });
    node.dataset.errorCode = e.code;
    return node;
  }
  window.LucasErrors = {
    render,
    info,
    diagnostic,
    label: (result) => states[info(result).code].tag,
  };
})();

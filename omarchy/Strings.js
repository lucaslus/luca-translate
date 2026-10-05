.pragma library

var labels = {
    translate: ["Translate", "翻译"], settings: ["Settings", "设置"], history: ["History", "历史"],
    favorites: ["Favorites", "收藏"], close: ["Close", "关闭"], input: ["Type or paste text…", "输入或粘贴文字…"],
    source: ["Source", "原文"], target: ["Target", "译文"], auto: ["Automatic", "自动"],
    selection: ["Selection", "划词"], screenshot: ["Screenshot", "截图翻译"], ocr: ["OCR to clipboard", "OCR 复制"],
    annotate: ["Annotate", "截图标注"], cancel: ["Cancel", "取消"], clear: ["Clear", "清空"],
    loading: ["Translating…", "翻译中…"], empty: ["Select text, capture a region, or enter a query.", "划词、框选截图，或输入文字开始翻译。"],
    copy: ["Copy", "复制"], copied: ["Copied", "已复制"], favorite: ["Favorite", "收藏"], saved: ["Saved", "已保存"],
    retry: ["Retry", "重试"], remove: ["Remove", "移除"], save: ["Save", "保存"], more: ["Load more", "加载更多"],
    language: ["Interface language", "界面语言"], system: ["System", "跟随系统"],
    theme: ["Appearance follows Omarchy’s current theme and display settings.", "外观跟随 Omarchy 当前主题与显示设置。"],
    services: ["Translation services", "翻译服务"], ai: ["AI translation", "AI 翻译"], official: ["DeepL API", "DeepL 官方 API"],
    endpoint: ["Base URL", "服务地址"], model: ["Model", "模型"], apiKey: ["API key", "API 密钥"],
    keyStored: ["A key is stored; leave blank to keep it", "已有密钥，留空保留"], keyEmpty: ["Optional for local models", "本地模型可留空"],
    deleteKey: ["Clear saved key", "清除已存密钥"], pro: ["DeepL Pro account", "DeepL Pro 账户"],
    routing: ["Automatic target language", "自动目标语言"], fallback: ["Fallback target", "默认目标语言"],
    fromEnglish: ["English →", "英文 →"], fromChinese: ["Chinese →", "中文 →"],
    addRule: ["Add rule", "添加规则"], moveUp: ["Move up", "上移"], moveDown: ["Move down", "下移"],
    routingHelp: ["Rules match in order; unmatched languages use the fallback target.", "按顺序匹配规则；未命中的语言使用默认目标。"],
    testConnection: ["Test connection", "测试连接"], connected: ["Connected; no translation text was sent.", "连接正常，未发送翻译原文。"],
    saveBeforeTest: ["Save changes before testing the stored key.", "请先保存修改，再测试已保存的密钥。"],
    about: ["About", "关于"], diagnostics: ["Local diagnostics", "本地诊断"], openLogs: ["Open log directory", "查看日志目录"],
    logsPrivate: ["Logs contain status and duration, without source text, translations or credentials.", "日志只记录状态和耗时，不记录原文、译文或密钥。"],
    logsReady: ["Logging is available", "日志可用"], logsFailed: ["Logging is unavailable or cannot write", "日志不可用或暂时无法写入"],
    logsDropped: ["Skipped events", "已跳过记录"], shellManaged: ["Startup and plugin enablement are managed by Omarchy Shell.", "启动和插件启用由 Omarchy Shell 管理。"],
    shortcuts: ["Desktop shortcuts", "桌面快捷键"], shortcutHelp: ["Enter a combination such as Super+Ctrl+Shift+I. Leave blank to disable.", "输入组合，如 Super+Ctrl+Shift+I；留空禁用。"],
    toggle: ["Show / hide", "显示 / 隐藏"], inputAction: ["New input", "新建输入"],
    noHistory: ["No records yet", "暂无记录"], clearHistory: ["Clear history", "清空历史"],
    confirmClear: ["Delete all history?", "删除全部历史记录？"],
    backend: ["Starting translation service…", "正在启动翻译服务…"], backendFailed: ["Translation service stopped. Retry to reconnect.", "翻译服务已停止，重试以重新连接。"],
    interrupted: ["Translation interrupted; retry the unfinished services.", "翻译已中断，请重试未完成渠道。"],
    unavailable: ["Service unavailable", "服务不可用"], detected: ["Detected", "检测语言"], pronunciation: ["Pronunciation", "发音"],
    timeout: ["The operation timed out. Retry when the service responds.", "操作超时，请待服务恢复后重试。"],
    overloaded: ["Too many pending operations. Wait and retry.", "待处理操作过多，请稍后重试。"],
    english: ["English", "英语"], chinese: ["Simplified Chinese", "简体中文"], traditional: ["Traditional Chinese", "繁体中文"],
    japanese: ["Japanese", "日语"], korean: ["Korean", "韩语"], french: ["French", "法语"],
    german: ["German", "德语"], russian: ["Russian", "俄语"], spanish: ["Spanish", "西班牙语"],
    pen: ["Pen", "画笔"], arrow: ["Arrow", "箭头"], rectangle: ["Box", "方框"], ellipse: ["Circle", "圈画"],
    textTool: ["Text", "文字"], undo: ["Undo", "撤销"], accent: ["Accent", "强调色"],
    urgent: ["Highlight", "醒目色"], ink: ["Text color", "文字色"], annotationText: ["Label to place on the image", "在图片上放置的文字"],
    exportFailed: ["Cannot export the annotated image", "无法导出标注图片"]
}

function text(key, language) {
    var value = labels[key]
    return value ? value[language === "zh-CN" ? 1 : 0] : key
}

function languages(language) {
    return [{value:"auto", label:text("auto",language)}, {value:"zh-Hans",label:text("chinese",language)},
        {value:"zh-Hant",label:text("traditional",language)}, {value:"en",label:text("english",language)},
        {value:"ja",label:text("japanese",language)}, {value:"ko",label:text("korean",language)},
        {value:"fr",label:text("french",language)}, {value:"de",label:text("german",language)},
        {value:"ru",label:text("russian",language)}, {value:"es",label:text("spanish",language)}]
}

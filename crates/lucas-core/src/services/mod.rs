//! 翻译/词典服务抽象与内置实现。

pub mod bing_free;
pub mod deepl_free;
pub mod openai_compat;
pub mod google_free;
pub mod youdao_dict;

use serde::{Deserialize, Serialize};

use crate::lang;
use crate::pinyin;
use crate::QueryResult;

/// 服务错误
#[derive(Debug)]
pub enum ServiceError {
    Network(String),
    Parse(String),
    Unsupported(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::Network(m) => write!(f, "网络错误: {m}"),
            ServiceError::Parse(m) => write!(f, "解析错误: {m}"),
            ServiceError::Unsupported(m) => write!(f, "不支持: {m}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// 单词词典卡片 —— 音标的家。
/// 对标 Bob 的 toDict，但我们的词典层是强制路由，英文单词查询永远有音标。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DictCard {
    pub word: String,
    pub us_phonetic: Option<String>,
    pub uk_phonetic: Option<String>,
    /// 词性词义，如 [("adj.", "极好的，卓越的")]
    pub meanings: Vec<(String, String)>,
    /// 英式发音 URL（可直接 <audio> 播放）
    pub uk_speech: Option<String>,
    /// 美式发音 URL
    pub us_speech: Option<String>,
    pub source: String,
}

/// 翻译服务（句子/文本翻译）。Send + Sync 以支持并行多开。
pub trait TranslateService: Send + Sync {
    fn name(&self) -> &'static str;
    /// `from`/`to` 使用 Bob 风格语言代码，支持 "auto"
    fn translate(&self, text: &str, from: &str, to: &str)
        -> Result<Vec<String>, ServiceError>;
    fn detect(&self, text: &str) -> String {
        lang::detect_source(text).to_string()
    }
}

/// 词典服务（单词/短语查询，返回音标）
pub trait DictService: Send + Sync {
    fn name(&self) -> &'static str;
    fn lookup(&self, text: &str) -> Result<DictCard, ServiceError>;
}

/// 默认服务集合：词典层 + 翻译层
pub fn default_dict_service() -> Box<dyn DictService> {
    Box::new(youdao_dict::YoudaoDict)
}

pub fn default_translate_services() -> Vec<Box<dyn TranslateService>> {
    vec![
        Box::new(youdao_dict::YoudaoDict),
        Box::new(deepl_free::DeepLFree::new()),
        Box::new(google_free::GoogleFree),
    ]
}

/// 核心路由（单结果便捷封装）：单词→词典，句子→第一个成功的翻译服务。
pub fn route(text: &str, from: &str, to: &str) -> Result<QueryResult, ServiceError> {
    let svcs = default_translate_services();
    let results = route_all(&svcs, text, from, to);
    results
        .into_iter()
        .next()
        .ok_or(ServiceError::Network("无可用服务".into()))
}

/// 词典路由（单词查词，永远有音标）—— 供应用层流式调用
pub fn dict_route(trimmed: &str, to: &str) -> Result<QueryResult, ServiceError> {
    let dict = default_dict_service().lookup(trimmed)?;
    let paragraphs = dict
        .meanings
        .iter()
        .map(|(pos, m)| {
            if pos.is_empty() {
                m.clone()
            } else {
                format!("{pos} {m}")
            }
        })
        .collect::<Vec<_>>();
    let detected_to = if to == "auto" { lang::auto_target("en") } else { to };
    // 拼音降噪：词典卡只标注第一条词义，避免全量拼音刷屏
    let pinyin = dict
        .meanings
        .first()
        .map(|(_, m)| m.clone())
        .and_then(|m| crate::pinyin::annotate(&m));
    Ok(QueryResult {
        text: trimmed.to_string(),
        detected_from: "en".into(),
        detected_to: detected_to.to_string(),
        paragraphs,
        dict: Some(dict),
        pinyin,
        service: "YoudaoDict".into(),
        error: None,
    })
}

/// 多服务并行多开：所有翻译服务同时查询，返回全部成功结果（保持传入顺序）。
///
/// - 英文单词/短语 → 只走词典路由（单结果，永远有音标）
/// - 其他文本 → 并行调用所有服务；`auto` 目标语言先解析成具体值再下发
/// - 每个结果自动附加拼音（若含中文）
pub fn route_all(
    services: &[Box<dyn TranslateService>],
    text: &str,
    from: &str,
    to: &str,
) -> Vec<QueryResult> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    // 单词 → 词典路由
    if lang::is_english_word_like(trimmed) {
        return match dict_route(trimmed, to) {
            Ok(mut r) => {
                r.pinyin = pinyin::annotate(&r.paragraphs.join("\n"));
                vec![r]
            }
            Err(_) => vec![],
        };
    }

    // 解析 auto 目标语言，避免透传给服务商
    let detected_hint = if from == "auto" {
        lang::detect_source(trimmed)
    } else {
        from
    };
    let resolved_to = if to == "auto" {
        lang::auto_target(detected_hint)
    } else {
        to
    };

    // 并行查询所有服务（scoped threads，保持顺序收集）
    let hits: Vec<Option<(Vec<String>, String, String)>> = std::thread::scope(|s| {
        let handles: Vec<_> = services
            .iter()
            .map(|svc| {
                s.spawn(move || {
                    svc.translate(trimmed, from, resolved_to)
                        .ok()
                        .map(|paras| (paras, svc.name().to_string(), svc.detect(trimmed)))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or(None)).collect()
    });

    let results = hits
        .into_iter()
        .flatten()
        .map(|(paras, name, detected)| {
            let detected_from = detected;
            let detected_to = if to == "auto" {
                lang::auto_target(&detected_from).to_string()
            } else {
                to.to_string()
            };
            QueryResult {
                text: trimmed.to_string(),
                detected_from,
                detected_to,
                paragraphs: paras,
                dict: None,
                pinyin: None,
                service: name,
                error: None,
            }
        })
        .collect::<Vec<_>>();

    // 差异化功能：结果含中文时自动生成拼音
    results
        .into_iter()
        .map(|mut r| {
            r.pinyin = pinyin::annotate(&r.paragraphs.join("\n"));
            r
        })
        .collect()
}

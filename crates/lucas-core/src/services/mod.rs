//! 翻译/词典服务抽象与内置实现。

pub mod bing_free;
pub mod deepl_free;
mod error;
pub mod google_free;
pub mod openai_compat;
pub mod youdao_dict;
pub use error::{ErrorCode, FailureInfo, ServiceError};

use serde::{Deserialize, Serialize};

use crate::lang;
use crate::pinyin;
use crate::QueryResult;

/// 单词词典卡片 —— 音标的家。
/// 仅在启用词典且语向匹配时查询，音标由服务返回。
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

/// 翻译渠道的原始结果。渠道支持时同时返回它实际检测到的源语言。
#[derive(Debug, Clone, PartialEq)]
pub struct TranslationOutput {
    pub paragraphs: Vec<String>,
    pub detected_from: Option<String>,
}

impl TranslationOutput {
    pub fn paragraphs(paragraphs: Vec<String>) -> Self {
        Self {
            paragraphs,
            detected_from: None,
        }
    }
}

/// 翻译服务（句子/文本翻译）。Send + Sync 以支持并行多开。
pub trait TranslateService: Send + Sync {
    fn name(&self) -> &'static str;
    /// `from`/`to` 使用 Bob 风格语言代码，支持 "auto"
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError>;
    /// 默认兼容现有渠道；能取得服务商检测语言的渠道应覆盖此方法。
    fn translate_with_detection(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslationOutput, ServiceError> {
        self.translate(text, from, to)
            .map(TranslationOutput::paragraphs)
    }
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

/// 英文查词（中文释义、可用时返回音标）—— 供应用层流式调用。
pub fn dict_route(trimmed: &str, to: &str) -> Result<QueryResult, ServiceError> {
    if !matches!(to, "auto" | "zh-Hans") {
        return Err(ServiceError::Unsupported("词典仅提供中文释义".into()));
    }
    let youdao_dict::DictionaryOutput { dict, paragraphs } =
        youdao_dict::YoudaoDict.lookup_or_translate(trimmed)?;
    let detected_to = if to == "auto" {
        lang::auto_target("en")
    } else {
        to
    };
    // 拼音降噪：词典卡只标注第一条词义，避免全量拼音刷屏
    let pinyin = if let Some(dict) = &dict {
        dict.meanings.first().and_then(|(_, m)| pinyin::annotate(m))
    } else {
        pinyin::annotate(&paragraphs.join("\n"))
    };
    Ok(QueryResult {
        text: trimmed.to_string(),
        detected_from: "en".into(),
        detected_to: detected_to.to_string(),
        // A successful dictionary lookup is useful evidence, but short words can
        // exist in multiple languages; only an auto-detecting provider confirms.
        source_confirmed: false,
        paragraphs,
        dict,
        pinyin,
        service: "YoudaoDict".into(),
        error: None,
        failure: None,
    })
}

/// 多服务并行多开：所有翻译服务同时查询，返回全部成功结果（保持传入顺序）。
///
/// - 仅调用传入的渠道；启用有道且语向匹配时，该渠道提供词典结果
/// - 各渠道并行；`auto` 目标语言先解析成具体值再下发
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

    let source = lang::source_hint(trimmed, from);
    let target = if to == "auto" {
        lang::auto_target(source)
    } else {
        to
    };
    // Every request is owned by a supplied service; there is no hidden dictionary
    // call when consumers intentionally select a local-only service list.
    std::thread::scope(|scope| {
        let handles: Vec<_> = services
            .iter()
            .map(|svc| {
                scope.spawn(move || {
                    if svc.name() == "YoudaoDict"
                        && lang::dictionary_eligible(trimmed, from, target)
                    {
                        return dict_route(trimmed, target).ok();
                    }
                    let output = svc.translate_with_detection(trimmed, from, target).ok()?;
                    let paragraphs = output.paragraphs;
                    if paragraphs.is_empty() {
                        return None;
                    }
                    let provider_source = if from == "auto" {
                        output
                            .detected_from
                            .as_deref()
                            .and_then(lang::normalize_provider_language)
                    } else {
                        None
                    };
                    Some(QueryResult {
                        text: trimmed.into(),
                        detected_from: provider_source.unwrap_or(source).into(),
                        detected_to: target.into(),
                        source_confirmed: provider_source.is_some(),
                        pinyin: pinyin::annotate(&paragraphs.join("\n")),
                        paragraphs,
                        dict: None,
                        service: svc.name().into(),
                        error: None,
                        failure: None,
                    })
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect()
    })
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    #[test]
    fn dictionary_rejects_other_targets_without_request() {
        assert!(matches!(
            dict_route("hello", "ja"),
            Err(ServiceError::Unsupported(_))
        ));
    }
    struct OnlyLocal;
    impl TranslateService for OnlyLocal {
        fn name(&self) -> &'static str {
            "OnlyLocal"
        }
        fn translate(&self, text: &str, _: &str, _: &str) -> Result<Vec<String>, ServiceError> {
            Ok(vec![text.into()])
        }
    }
    #[test]
    fn empty_services_never_invoke_dictionary() {
        assert!(route_all(&[], "hello", "en", "zh-Hans").is_empty());
    }
    #[test]
    fn local_only_and_manual_language_are_preserved() {
        let result = route_all(&[Box::new(OnlyLocal)], "bonjour", "fr", "ja");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].service, "OnlyLocal");
        assert_eq!(result[0].detected_from, "fr");
        assert_eq!(result[0].detected_to, "ja");
    }
}

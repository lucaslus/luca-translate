//! 语言代码、低成本检测与服务商语言代码归一化。
//!
//! 语言代码兼容 Bob 的语言代码子集（auto / zh-Hans / zh-Hant / en / ja / ko ...）。
//! 自动检测遵循“能确定才确定”的原则：文字系统先行，拉丁字母文本仅在样本足够且
//! Whatlang 达到可靠阈值时采用模型结论；短词与低置信结果回退为英语提示，交给
//! 支持 auto 的翻译服务做最终确认。

use serde::Serialize;
use std::sync::OnceLock;
use whatlang::{Detector, Lang};

const MIN_LATIN_LETTERS: usize = 20;
const MIN_LATIN_WORDS: usize = 3;

/// 一次本地源语言判断。`reliable = false` 时只能作为路由降级提示，不能向用户宣称
/// 已准确识别。
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct LanguageDecision {
    pub language: &'static str,
    pub confidence: f64,
    pub reliable: bool,
    pub basis: &'static str,
}

impl LanguageDecision {
    fn certain(language: &'static str, basis: &'static str) -> Self {
        Self {
            language,
            confidence: 1.0,
            reliable: true,
            basis,
        }
    }

    fn uncertain(language: &'static str, basis: &'static str) -> Self {
        Self {
            language,
            confidence: 0.0,
            reliable: false,
            basis,
        }
    }
}

/// 检测文本是否包含汉字。
pub fn contains_han(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c as u32,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF)
    })
}

/// 粗略判断输入是否是“单个英文单词或短语”（适合走词典而非整句翻译）。
/// 规则：仅由 ASCII 字母/空格/常见连字符撇号构成，且单词数 <= 3，长度 <= 40。
pub fn is_english_word_like(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > 40 {
        return false;
    }
    let ok = t
        .chars()
        .all(|c| c.is_ascii_alphabetic() || matches!(c, ' ' | '-' | '\''));
    if !ok {
        return false;
    }
    t.chars().any(|c| c.is_ascii_alphabetic()) && t.split_whitespace().count() <= 3
}

fn latin_detector() -> &'static Detector {
    static DETECTOR: OnceLock<Detector> = OnceLock::new();
    DETECTOR.get_or_init(|| {
        // 只让统计模型在产品实际支持、且使用拉丁字母的语言中选择，避免短文本被
        // 识别成丹麦语、乌兹别克语等 UI 无法表达的语言。
        Detector::with_allowlist(vec![Lang::Eng, Lang::Fra, Lang::Deu, Lang::Spa])
    })
}

fn detection_sample(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|token| {
            let lower = token.to_ascii_lowercase();
            !lower.starts_with("http://")
                && !lower.starts_with("https://")
                && !lower.starts_with("www.")
                && !token.contains('@')
        })
        .map(|token| {
            token.trim_matches(|c: char| !c.is_alphabetic() && !matches!(c, '\'' | '-' | '’'))
        })
        .filter(|token| token.chars().any(char::is_alphabetic))
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_latin_letter(c: char) -> bool {
    matches!(c as u32, 0x0041..=0x024F | 0x1E00..=0x1EFF)
}

fn mapped_language(language: Lang) -> Option<&'static str> {
    match language {
        Lang::Eng => Some("en"),
        Lang::Fra => Some("fr"),
        Lang::Deu => Some("de"),
        Lang::Spa => Some("es"),
        Lang::Rus => Some("ru"),
        _ => None,
    }
}

/// 本地源语言判断。
///
/// - 日文假名、韩文和汉字可通过文字系统快速确定，不加载额外模型。
/// - 西里尔字母在当前产品语言集合中只能推测为俄语，因此明确标记为不可靠。
/// - 拉丁字母短文本（少于 3 个词或 20 个字母）不做强判定。
/// - 足够长的拉丁文本必须通过 Whatlang 自带的可靠性门槛才采用。
pub fn detect_source_decision(input: &str) -> LanguageDecision {
    if input
        .chars()
        .any(|c| matches!(c as u32, 0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9D))
    {
        return LanguageDecision::certain("ja", "script");
    }
    if input
        .chars()
        .any(|c| matches!(c as u32, 0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F))
    {
        return LanguageDecision::certain("ko", "script");
    }
    if contains_han(input) {
        return LanguageDecision::certain("zh-Hans", "script");
    }
    if input.chars().any(|c| matches!(c as u32, 0x0400..=0x052F)) {
        return LanguageDecision::uncertain("ru", "script_hint");
    }

    let sample = detection_sample(input);
    if sample
        .chars()
        .filter(|c| c.is_alphabetic())
        .any(|c| !is_latin_letter(c))
    {
        return LanguageDecision::uncertain("en", "unsupported_script_fallback");
    }
    let letters = sample.chars().filter(|c| c.is_alphabetic()).count();
    let words = sample.split_whitespace().count();
    if letters < MIN_LATIN_LETTERS || words < MIN_LATIN_WORDS {
        return LanguageDecision::uncertain("en", "short_text_fallback");
    }

    if let Some(info) = latin_detector().detect(&sample) {
        if info.is_reliable() {
            if let Some(language) = mapped_language(info.lang()) {
                return LanguageDecision {
                    language,
                    confidence: info.confidence(),
                    reliable: true,
                    basis: "statistical",
                };
            }
        }
    }
    LanguageDecision::uncertain("en", "low_confidence_fallback")
}

/// 兼容旧调用点的源语言提示。需要区分置信状态时使用 [`detect_source_decision`]。
pub fn detect_source(s: &str) -> &'static str {
    detect_source_decision(s).language
}

pub fn source_hint<'a>(text: &str, from: &'a str) -> &'a str {
    if from == "auto" {
        detect_source(text)
    } else {
        from
    }
}

pub fn dictionary_eligible(text: &str, from: &str, to: &str) -> bool {
    is_english_word_like(text) && source_hint(text, from) == "en" && to == "zh-Hans"
}

/// auto 模式下的目标语言：中文输入 -> 译英；其他 -> 译简体中文。
pub fn auto_target(source: &str) -> &'static str {
    if source.starts_with("zh") {
        "en"
    } else {
        "zh-Hans"
    }
}

/// 将服务商返回的语言代码收敛到产品支持的代码集合。
pub fn normalize_provider_language(code: &str) -> Option<&'static str> {
    let code = code.trim().to_ascii_lowercase().replace('_', "-");
    match code.as_str() {
        "zh" | "zh-cn" | "zh-sg" | "zh-hans" => Some("zh-Hans"),
        "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant" => Some("zh-Hant"),
        "en" | "en-us" | "en-gb" => Some("en"),
        "ja" | "jp" => Some("ja"),
        "ko" | "kr" => Some("ko"),
        "fr" => Some("fr"),
        "de" => Some("de"),
        "es" => Some("es"),
        "ru" => Some("ru"),
        _ => None,
    }
}

/// 服务商语言代码映射（目前内置服务需要的部分）。
pub fn map_lang(code: &str, table: &[(&str, &str)]) -> String {
    if code == "auto" {
        return "auto".into();
    }
    for (ours, theirs) in table {
        if *ours == code {
            return theirs.to_string();
        }
    }
    code.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_detection_is_fast_and_decisive() {
        assert!(contains_han("你好世界"));
        assert!(!contains_han("hello"));
        assert_eq!(detect_source("今日はいい天気です"), "ja");
        assert_eq!(detect_source("안녕하세요"), "ko");
        assert_eq!(detect_source("你好世界"), "zh-Hans");
        assert!(detect_source_decision("你好世界").reliable);
    }

    #[test]
    fn ambiguous_short_latin_text_never_claims_a_language() {
        for text in ["autonomous", "hello", "excellent", "bonjour"] {
            let decision = detect_source_decision(text);
            assert_eq!(decision.language, "en", "{text}");
            assert!(!decision.reliable, "{text}");
            assert_eq!(decision.basis, "short_text_fallback", "{text}");
        }
    }

    #[test]
    fn long_high_confidence_text_can_be_detected() {
        let decision = detect_source_decision(
            "Bonjour, ceci est une phrase française suffisamment longue pour identifier clairement la langue.",
        );
        assert_eq!(decision.language, "fr");
        assert!(decision.reliable);
        assert_eq!(decision.basis, "statistical");
    }

    #[test]
    fn noisy_tokens_do_not_create_false_confidence() {
        let decision = detect_source_decision("https://example.com lucas@example.com autonomous");
        assert_eq!(decision.language, "en");
        assert!(!decision.reliable);
    }

    #[test]
    fn unsupported_scripts_stay_unknown_instead_of_becoming_latin_languages() {
        let decision = detect_source_decision("هذه جملة عربية طويلة بما يكفي لاختبار كشف اللغة");
        assert_eq!(decision.language, "en");
        assert!(!decision.reliable);
        assert_eq!(decision.basis, "unsupported_script_fallback");
    }

    #[test]
    fn word_routing_and_manual_override_are_preserved() {
        assert!(is_english_word_like("excellent"));
        assert!(is_english_word_like("give up"));
        assert!(!is_english_word_like(
            "The quick brown fox jumps over the lazy dog"
        ));
        assert!(!is_english_word_like("你好"));
        assert_eq!(source_hint("bonjour", "fr"), "fr");
        assert!(!is_english_word_like("---"));
        assert!(dictionary_eligible("autonomous", "auto", "zh-Hans"));
        assert!(!dictionary_eligible("bonjour", "fr", "zh-Hans"));
        assert!(!dictionary_eligible("hello", "en", "ja"));
    }

    #[test]
    fn provider_codes_are_normalized() {
        assert_eq!(normalize_provider_language("EN-US"), Some("en"));
        assert_eq!(normalize_provider_language("ZH_HANT"), Some("zh-Hant"));
        assert_eq!(normalize_provider_language("uk"), None);
    }

    #[test]
    fn auto_targets_are_stable() {
        assert_eq!(auto_target("en"), "zh-Hans");
        assert_eq!(auto_target("zh-Hans"), "en");
    }
}

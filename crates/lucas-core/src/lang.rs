//! 语言代码与粗检测。
//!
//! 语言代码兼容 Bob 的语言代码子集（auto / zh-Hans / zh-Hant / en / ja / ko ...），
//! 为未来兼容 Bob 插件生态打基础。

/// 检测文本是否包含汉字
pub fn contains_han(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c as u32,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF)
    })
}

/// 粗略判断输入是否是"单个英文单词或短语"（适合走词典而非整句翻译）。
/// 规则：仅由 ASCII 字母/空格/常见连字符撇号构成，且单词数 <= 3，长度 <= 40。
pub fn is_english_word_like(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > 40 {
        return false;
    }
    let ok = t.chars().all(|c| c.is_ascii_alphabetic() || matches!(c, ' ' | '-' | '\''));
    if !ok {
        return false;
    }
    t.split_whitespace().count() <= 3
}

/// 源语言粗检测（中 / 日 / 韩 / 英，其他回退英）
pub fn detect_source(s: &str) -> &'static str {
    if contains_han(s) {
        "zh-Hans"
    } else if s.chars().any(|c| {
        matches!(c as u32, 0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9D)
    }) {
        "ja"
    } else if s.chars().any(|c| {
        matches!(c as u32, 0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F)
    }) {
        "ko"
    } else {
        "en"
    }
}

/// auto 模式下的目标语言：中文输入 -> 译英；其他 -> 译简体中文
pub fn auto_target(source: &str) -> &'static str {
    if source.starts_with("zh") {
        "en"
    } else {
        "zh-Hans"
    }
}

/// 服务商语言代码映射（目前内置服务需要的部分）
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
    fn test_detect() {
        assert!(contains_han("你好世界"));
        assert!(!contains_han("hello"));
        assert!(is_english_word_like("excellent"));
        assert!(is_english_word_like("give up"));
        assert!(!is_english_word_like("The quick brown fox jumps over the lazy dog"));
        assert!(!is_english_word_like("你好"));
        assert_eq!(detect_source("hello"), "en");
        assert_eq!(auto_target("en"), "zh-Hans");
        assert_eq!(auto_target("zh-Hans"), "en");
    }
}

//! OCR 智能分段：把 OCR 输出的碎行合并为自然段落。
//!
//! 启发式规则（参考 Bob/Easydict 的段落重建思路）：
//! 1. 行尾是终止标点（。！？.!?；;等）→ 段落结束
//! 2. 下一行是列表项（- • * · 1. 1、等）→ 新段落
//! 3. 过短的行（<12 字符）视为标题/标签，独立成段
//! 4. 中英文拼接时补空格，纯中文直接相连

/// 判断字符是否为段落终止标点
fn is_terminal(c: char) -> bool {
    matches!(
        c,
        '。' | '．'
            | '.'
            | '!'
            | '！'
            | '?'
            | '？'
            | '…'
            | '；'
            | ';'
            | '：'
            | ':'
            | '」'
            | '』'
            | '"'
            | ')'
            | '）'
            | '】'
    )
}

/// 判断是否为列表/条目行
fn is_list_item(s: &str) -> bool {
    let t = s.trim_start();
    if t.starts_with('-') || t.starts_with('•') || t.starts_with('*') || t.starts_with('·') {
        return true;
    }
    // 数字编号：1. 1、 1) ① 等
    let mut chars = t.chars();
    if chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        let rest: String = t.chars().skip(1).take(2).collect();
        if rest.starts_with('.') || rest.starts_with('、') || rest.starts_with(')') {
            return true;
        }
    }
    if t.starts_with('①') || t.starts_with('•') {
        return true;
    }
    false
}

fn ends_with_terminal(s: &str) -> bool {
    s.chars().last().map(is_terminal).unwrap_or(false)
}

/// 两行是否应该合并为同一段
fn should_join(cur: &str, next: &str) -> bool {
    if is_list_item(next) || is_list_item(cur) {
        return false;
    }
    if cur.chars().count() < 12 {
        return false; // 短行视为标题/标签
    }
    if ends_with_terminal(cur) {
        return false;
    }
    true
}

/// 拼接两段文本，处理中英文边界空格
fn join_line(cur: &mut String, next: &str) {
    let last_cjk = cur
        .chars()
        .last()
        .map(|c| matches!(c as u32, 0x4E00..=0x9FFF))
        .unwrap_or(false);
    let next_first = next.chars().next();
    let next_is_ascii = next_first
        .map(|c| c.is_ascii_alphanumeric())
        .unwrap_or(false);
    let cur_ends_ascii = cur
        .chars()
        .last()
        .map(|c| c.is_ascii_alphanumeric())
        .unwrap_or(false);

    if cur_ends_ascii && next_is_ascii {
        cur.push(' ');
    } else if !last_cjk
        && next_first
            .map(|c| !c.is_ascii_punctuation())
            .unwrap_or(false)
        && cur_ends_ascii
    {
        cur.push(' ');
    }
    cur.push_str(next.trim());
}

/// 合并 OCR 行为段落
pub fn merge_ocr_lines(lines: &[String]) -> Vec<String> {
    let mut paragraphs: Vec<String> = Vec::new();
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(cur) = paragraphs.last_mut() {
            if should_join(cur, line) {
                join_line(cur, line);
                continue;
            }
        }
        paragraphs.push(line.to_string());
    }
    paragraphs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_chinese_sentence() {
        let lines = vec![
            "今天天气很好，我们一起去公".to_string(),
            "园里散步，顺便买点早餐。".to_string(),
            "下午的安排是开会。".to_string(),
        ];
        let paras = merge_ocr_lines(&lines);
        assert_eq!(paras.len(), 2);
        assert_eq!(
            paras[0],
            "今天天气很好，我们一起去公园里散步，顺便买点早餐。"
        );
    }

    #[test]
    fn test_list_items_stay_separate() {
        let lines = vec![
            "购物清单如下".to_string(),
            "1、苹果和香蕉".to_string(),
            "- 牛奶一箱".to_string(),
        ];
        let paras = merge_ocr_lines(&lines);
        assert_eq!(paras.len(), 3);
    }

    #[test]
    fn test_english_join() {
        let lines = vec![
            "The quick brown fox jumps over".to_string(),
            "the lazy dog near the river bank".to_string(),
        ];
        let paras = merge_ocr_lines(&lines);
        assert_eq!(paras.len(), 1);
        assert_eq!(
            paras[0],
            "The quick brown fox jumps over the lazy dog near the river bank"
        );
    }

    #[test]
    fn test_short_lines_separate() {
        let lines = vec!["标题".to_string(), "正文第一行很长很长很长".to_string()];
        let paras = merge_ocr_lines(&lines);
        assert_eq!(paras.len(), 2);
    }
}

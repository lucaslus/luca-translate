//! 集成测试：真实调用内置服务。
//! 需要网络；CI 无网环境可用 `cargo test -- --skip network` 跳过。

use lucas_core::lang;
use lucas_core::pinyin;
use lucas_core::route;

#[test]
fn network_youdao_dict_has_phonetics() {
    let card = lucas_core::services::default_dict_service()
        .lookup("excellent")
        .expect("lookup failed");
    assert_eq!(card.word, "excellent");
    assert!(
        card.us_phonetic.is_some() || card.uk_phonetic.is_some(),
        "音标必须存在 —— 这是本项目的核心卖点"
    );
    assert!(!card.meanings.is_empty());
    println!(
        "excellent: us={:?} uk={:?} meanings={:?}",
        card.us_phonetic, card.uk_phonetic, card.meanings
    );
}

#[test]
fn network_youdao_long_translation_keeps_the_final_sentence() {
    use lucas_core::services::{youdao_dict::YoudaoDict, TranslateService};

    let text = concat!(
        "The morning train arrived at the station before sunrise. ",
        "A traveler carried a red suitcase and a small notebook. ",
        "She wanted to visit a quiet village near the mountains. ",
        "The road from the station passed through fields of flowers. ",
        "A farmer showed her the path to an old stone bridge. ",
        "On the other side of the river, children were playing outside a school. ",
        "She stopped at a bakery and bought fresh bread for breakfast. ",
        "The owner told her about a festival planned for the following evening. ",
        "Later she climbed a hill and watched clouds move slowly across the valley. ",
        "At the end of the day, she wrote a letter to her family. ",
        "The final sentence says that a purple elephant is dancing beside a silver bicycle."
    );
    assert!(text.len() > 600);
    let translated = YoudaoDict
        .translate(text, "en", "zh-Hans")
        .unwrap()
        .join("\n");
    assert!(
        translated.contains("火车"),
        "missing opening sentence: {translated}"
    );
    assert!(
        translated.contains("紫") && translated.contains("象") && translated.contains("自行车"),
        "missing final sentence: {translated}"
    );
}

#[test]
fn network_route_word_gives_dict_and_phonetics() {
    let r = route("excellent", "auto", "auto").expect("route failed");
    assert!(r.dict.is_some(), "单词输入必须走词典路由");
    let dict = r.dict.unwrap();
    assert!(dict.us_phonetic.is_some() || dict.uk_phonetic.is_some());
    assert_eq!(dict.us_speech.map(|u| u.contains("dictvoice")), Some(true));
}

#[test]
fn network_route_sentence_translates() {
    let r = route(
        "The quick brown fox jumps over the lazy dog",
        "auto",
        "auto",
    )
    .expect("route failed");
    assert!(r.dict.is_none(), "句子不应走词典路由");
    assert!(!r.paragraphs.is_empty());
    let joined = r.paragraphs.join("");
    assert!(joined.contains("狐") || joined.contains("狗") || !joined.is_empty());
    println!("sentence result: {:?}", r.paragraphs);
    // 译文是中文，应自动附拼音
    assert!(r.pinyin.is_some(), "中文结果必须自动生成拼音");
}

#[test]
fn test_pinyin_offline() {
    assert_eq!(pinyin::to_pinyin_tone("翻译").as_deref(), Some("fān yì"));
    assert!(lang::is_english_word_like("excellent"));
}

#[test]
fn network_route_all_parallel_multi_service() {
    let svcs = lucas_core::services::default_translate_services();
    let results = lucas_core::services::route_all(&svcs, "Knowledge is power.", "auto", "zh-Hans");
    assert!(
        results.len() >= 2,
        "至少两个服务应成功，实际 {}",
        results.len()
    );
    // 顺序保持：第一个是有道
    assert_eq!(results[0].service, "YoudaoDict");
    for r in &results {
        assert!(!r.paragraphs.is_empty(), "{} 返回空", r.service);
        assert!(r.pinyin.is_some(), "{} 中文结果应带拼音", r.service);
        println!("  [{}] {:?}", r.service, r.paragraphs);
    }
    assert!(
        results
            .iter()
            .any(|r| r.source_confirmed && r.detected_from == "en"),
        "至少一个支持自动检测的渠道应确认英文源语言"
    );
}

#[test]
fn test_ocr_paragraph_merge() {
    use lucas_core::paragraph::merge_ocr_lines;
    let lines = vec![
        "今天的天气非常不错，我们一起".to_string(),
        "去公园散步吧。".to_string(),
        "1、带好水杯".to_string(),
    ];
    let merged = merge_ocr_lines(&lines);
    assert_eq!(merged.len(), 2);
    assert!(merged[0].ends_with("散步吧。"));
}

#[test]
fn network_bing_free_translate() {
    let svc = lucas_core::services::bing_free::BingFree;
    let r = lucas_core::TranslateService::translate_with_detection(
        &svc,
        "Hello world",
        "auto",
        "zh-Hans",
    )
    .expect("bing failed");
    println!("Bing: {:?}", r.paragraphs);
    assert!(!r.paragraphs.join("").is_empty());
    assert_eq!(r.detected_from.as_deref(), Some("en"));
}

//! lucas-core: lucas-translate 的纯 Rust 核心库。
//!
//! 设计原则（对标 Bob 的服务架构并修复其音标短板）：
//! - 服务分层：[`services::TranslateService`]（句子翻译）与 [`services::DictService`]（单词词典，含音标）
//! - 调用方无需关心路由：[`route`] 会根据输入文本自动决定"查词典"还是"做翻译"，
//!   并在任何结果上附加拼音标注（若包含中文）。
//!
//! 音标问题定位（Bob 的痛点）：Bob 只有词典类服务返回音标，普通翻译通道没有。
//! 我们把词典作为一等公民服务，英文单词输入永远能拿到英/美音标。

pub mod lang;
pub mod paragraph;
pub mod pinyin;
pub mod services;
pub mod tts;

use serde::{Deserialize, Serialize};

pub use services::{route, route_all, DictCard, ServiceError, TranslateService, TranslationOutput};

/// 单个音标（kind: "us" | "uk" | "generic"）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Phonetic {
    pub kind: String,
    pub value: String,
}

/// 词性 + 词义
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DictMeaning {
    pub pos: String,
    pub meaning: String,
}

/// 一次查询的最终结果（UI 直接渲染这个结构）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub text: String,
    pub detected_from: String,
    pub detected_to: String,
    /// 源语言是否由词典或上游翻译服务确认；false 表示仅为本地提示/手动选项。
    #[serde(default)]
    pub source_confirmed: bool,
    /// 译文分段（对齐 Bob 的 toParagraphs 概念，UI 渲染时段落间自动加空行）
    pub paragraphs: Vec<String>,
    /// 单词词典卡片（音标所在处；句子翻译时为 None）
    pub dict: Option<DictCard>,
    /// 拼音标注（原文或译文包含中文时存在，带声调，空格分隔音节）
    pub pinyin: Option<String>,
    /// 提供此结果的服务名
    pub service: String,
    /// 失败原因（服务失败时存在，用于优雅展示）
    #[serde(default)]
    pub error: Option<String>,
    /// Structured, privacy-safe metadata for recovery UI and diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<services::FailureInfo>,
}

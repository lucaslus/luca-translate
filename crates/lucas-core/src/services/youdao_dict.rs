//! 有道词典（web 端 jsonapi_s 接口）。
//!
//! 返回结构（实测）：
//! - `ec.word`: { usphone, ukphone, trs: [{pos, tran}], return-phrase, usspeech, ukspeech }
//! - `fanyi.tran`: 整句翻译时的译文段落数组
//!
//! 该接口即 Bob 第三方插件（如 Free 有道翻译）所用通道，无需密钥。

use serde_json::Value;

use super::{DictCard, DictService, ServiceError, TranslateService};

const API: &str = "https://dict.youdao.com/jsonapi_s?doctype=json&jsonversion=4";

pub struct YoudaoDict;

impl YoudaoDict {
    fn request_once(&self, text: &str) -> Result<Value, ServiceError> {
        let resp = ureq::post(API)
            .timeout(std::time::Duration::from_secs(15))
            // 浏览器 UA：降低被风控返回空结果的概率
            .set(
                "User-Agent",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .set("Referer", "https://dict.youdao.com/")
            .send_form(&[("q", text), ("keyfrom", "webdict"), ("client", "web")])
            .map_err(|e| ServiceError::Network(e.to_string()))?;
        resp.into_json::<Value>()
            .map_err(|e| ServiceError::Parse(e.to_string()))
    }

    fn request(&self, text: &str) -> Result<Value, ServiceError> {
        // 免费通道偶发限流返回空结果，自动重试一次
        let data = self.request_once(text)?;
        if data.get("fanyi").is_none() && data.get("ec").is_none() {
            std::thread::sleep(std::time::Duration::from_millis(400));
            return self.request_once(text);
        }
        Ok(data)
    }
}

impl DictService for YoudaoDict {
    fn name(&self) -> &'static str {
        "YoudaoDict"
    }

    fn lookup(&self, text: &str) -> Result<DictCard, ServiceError> {
        let data = self.request(text)?;
        let ec = data.pointer("/ec/word").cloned().unwrap_or(Value::Null);
        if ec.is_null() {
            return Err(ServiceError::Parse("ec 词典字段缺失（可能不是英文词）".into()));
        }

        let word = ec
            .get("return-phrase")
            .and_then(|v| v.get("l") )
            .and_then(|v| v.as_str())
            .or_else(|| ec.get("return-phrase").and_then(|v| v.as_str()))
            .unwrap_or(text)
            .to_string();

        let us_phonetic = ec
            .get("usphone")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                // 兜底：simple.word[0]
                data.pointer("/simple/word/0/usphone")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });
        let uk_phonetic = ec
            .get("ukphone")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                data.pointer("/simple/word/0/ukphone")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });

        let mut meanings = Vec::new();
        if let Some(trs) = ec.get("trs").and_then(|v| v.as_array()) {
            for tr in trs {
                let pos = tr.get("pos").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let tran = tr.get("tran").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if !tran.is_empty() {
                    meanings.push((pos, tran));
                }
            }
        }

        // 发音 URL（dictvoice 免费端点）
        let us_speech = us_phonetic
            .as_ref()
            .map(|_| crate::tts::youdao_voice_url(&word, false));
        let uk_speech = uk_phonetic
            .as_ref()
            .map(|_| crate::tts::youdao_voice_url(&word, true));

        Ok(DictCard {
            word,
            us_phonetic,
            uk_phonetic,
            meanings,
            uk_speech,
            us_speech,
            source: "youdao".into(),
        })
    }
}

impl TranslateService for YoudaoDict {
    fn name(&self) -> &'static str {
        "YoudaoDict"
    }

    fn translate(
        &self,
        text: &str,
        _from: &str,
        _to: &str,
    ) -> Result<Vec<String>, ServiceError> {
        let data = self.request(text)?;
        // 整句翻译结果在 fanyi.tran（实测：可能是 string，也可能是分段数组）
        match data.pointer("/fanyi/tran") {
            Some(Value::String(s)) if !s.is_empty() => return Ok(vec![s.clone()]),
            Some(Value::Array(arr)) => {
                let paras: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                if !paras.is_empty() {
                    return Ok(paras);
                }
            }
            _ => {}
        }
        // 单词兜底：拼词典释义
        let ec = data.pointer("/ec/word").cloned().unwrap_or(Value::Null);
        if !ec.is_null() {
            let mut paras = Vec::new();
            if let Some(trs) = ec.get("trs").and_then(|v| v.as_array()) {
                for tr in trs {
                    let pos = tr.get("pos").and_then(|v| v.as_str()).unwrap_or("");
                    let tran = tr.get("tran").and_then(|v| v.as_str()).unwrap_or("");
                    if !tran.is_empty() {
                        paras.push(format!("{pos} {tran}").trim().to_string());
                    }
                }
            }
            if !paras.is_empty() {
                return Ok(paras);
            }
        }
        Err(ServiceError::Parse("有道未返回译文（可能被临时限流）".into()))
    }
}

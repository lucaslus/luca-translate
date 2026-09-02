//! 应用配置：AI 服务（OpenAI 兼容）等，存为 JSON 文件。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

static CONFIG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 默认翻译路由规则（源语言 → 目标语言，自上而下匹配，未命中用 fallback）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub rules: Vec<RouteRule>,
    pub fallback: String,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            rules: vec![
                RouteRule { from: "en".into(), to: "zh-Hans".into() },
                RouteRule { from: "zh-Hans".into(), to: "en".into() },
            ],
            fallback: "zh-Hans".into(),
        }
    }
}

/// 服务开关（键名与 core 服务对应，前端展示名单独维护）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceToggles {
    #[serde(default = "default_true")]
    pub youdao: bool,
    #[serde(default = "default_true")]
    pub deepl: bool,
    #[serde(default = "default_true")]
    pub bing: bool,
    #[serde(default = "default_true")]
    pub google: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ServiceToggles {
    fn default() -> Self {
        Self {
            youdao: true,
            deepl: true,
            bing: true,
            google: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

pub fn set_config_dir(dir: PathBuf) {
    *CONFIG_DIR.lock().unwrap() = Some(dir);
}

fn config_path() -> PathBuf {
    CONFIG_DIR
        .lock()
        .unwrap()
        .clone()
        .expect("配置目录未初始化")
        .join("config.json")
}

pub fn load_ai_config() -> AiConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => AiConfig::default(),
    }
}

pub fn save_ai_config(cfg: &AiConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // 保留已有的 routing 字段
    let mut merged = match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<serde_json::Value>(&s).unwrap_or(serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    let ai_val = serde_json::to_value(cfg).map_err(|e| e.to_string())?;
    if let (Some(obj), Some(ai_obj)) = (merged.as_object_mut(), ai_val.as_object()) {
        for (k, v) in ai_obj {
            obj.insert(k.clone(), v.clone());
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ---------- 默认翻译规则 ----------

pub fn load_routing() -> RoutingConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let v: serde_json::Value = serde_json::from_str(&s).unwrap_or(serde_json::json!({}));
            let routing = v.get("routing").cloned();
            match routing {
                Some(r) => {
                    let cfg: RoutingConfig = serde_json::from_value(r).unwrap_or_default();
                    migrate_routing(cfg)
                }
                None => RoutingConfig::default(),
            }
        }
        Err(_) => RoutingConfig::default(),
    }
}

/// 迁移：旧版默认规则（单条 zh-Hans→en）升级为新的双向默认规则
fn migrate_routing(mut cfg: RoutingConfig) -> RoutingConfig {
    let is_old_default = cfg.rules.len() == 1
        && cfg.rules[0].from == "zh-Hans"
        && cfg.rules[0].to == "en"
        && cfg.fallback == "zh-Hans";
    if is_old_default {
        cfg = RoutingConfig::default();
    }
    cfg
}

pub fn load_services() -> ServiceToggles {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let v: serde_json::Value = serde_json::from_str(&s).unwrap_or(serde_json::json!({}));
            match v.get("services").cloned() {
                Some(x) => serde_json::from_value(x).unwrap_or_default(),
                None => ServiceToggles::default(),
            }
        }
        Err(_) => ServiceToggles::default(),
    }
}

pub fn save_services(cfg: &ServiceToggles) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut merged = match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<serde_json::Value>(&s).unwrap_or(serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    if let Some(obj) = merged.as_object_mut() {
        obj.insert(
            "services".into(),
            serde_json::to_value(cfg).map_err(|e| e.to_string())?,
        );
    }
    std::fs::write(&path, serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn save_routing(cfg: &RoutingConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut merged = match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<serde_json::Value>(&s).unwrap_or(serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    if let Some(obj) = merged.as_object_mut() {
        obj.insert(
            "routing".into(),
            serde_json::to_value(cfg).map_err(|e| e.to_string())?,
        );
    }
    std::fs::write(&path, serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

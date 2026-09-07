//! Atomic configuration transactions. Read IPC never returns secrets.
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
                RouteRule {
                    from: "en".into(),
                    to: "zh-Hans".into(),
                },
                RouteRule {
                    from: "zh-Hans".into(),
                    to: "en".into(),
                },
            ],
            fallback: "zh-Hans".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceToggles {
    pub youdao: bool,
    pub deepl: bool,
    pub bing: bool,
    pub google: bool,
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
#[derive(Clone, Default)]
pub struct AiConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub error: Option<String>,
}
#[derive(Serialize)]
pub struct AiView {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
}
#[derive(Deserialize)]
pub struct AiUpdate {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_key: bool,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct Document {
    enabled: bool,
    base_url: String,
    model: String,
    secret_id: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    api_key: String,
    routing: RoutingConfig,
    services: ServiceToggles,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}
trait Secrets: Send {
    fn get(&self, id: &str) -> Result<String, String>;
    fn set(&self, id: &str, key: &str) -> Result<(), String>;
    fn remove(&self, id: &str);
}
struct SystemSecrets;
impl Secrets for SystemSecrets {
    fn get(&self, id: &str) -> Result<String, String> {
        keyring::Entry::new("com.lucas.translate.ai", id)
            .and_then(|e| e.get_password())
            .map_err(|_| "无法读取系统凭据存储，请在翻译服务设置中重新保存 API Key".into())
    }
    fn set(&self, id: &str, key: &str) -> Result<(), String> {
        keyring::Entry::new("com.lucas.translate.ai", id)
            .and_then(|e| e.set_password(key))
            .map_err(|_| "无法保存到系统凭据存储；原配置未更改。请解锁钥匙串/凭据服务后重试".into())
    }
    fn remove(&self, id: &str) {
        if let Ok(e) = keyring::Entry::new("com.lucas.translate.ai", id) {
            let _ = e.delete_credential();
        }
    }
}
struct Store {
    path: PathBuf,
    vault: Box<dyn Secrets>,
    cached: Option<Document>,
    secret: Option<(String, String)>,
}
impl Store {
    fn load(&mut self) -> Result<Document, String> {
        if let Some(doc) = &self.cached {
            return Ok(doc.clone());
        }
        let mut doc: Document = match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
                "配置文件无法解析；已保留原文件，请修复 config.json 后重启应用".to_string()
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Document::default(),
            Err(_) => return Err("无法读取配置文件，请检查目录权限后重试".into()),
        };
        // A failed migration must leave the original file/key untouched.
        if !doc.api_key.is_empty() {
            let id = uuid::Uuid::new_v4().to_string();
            self.vault.set(&id, &doc.api_key)?;
            let key = std::mem::take(&mut doc.api_key);
            doc.secret_id = Some(id.clone());
            if let Err(e) = self.persist(&doc) {
                self.vault.remove(&id);
                return Err(e);
            }
            self.secret = Some((id, key));
        }
        self.cached = Some(doc.clone());
        Ok(doc)
    }
    fn persist(&mut self, doc: &Document) -> Result<(), String> {
        let parent = self.path.parent().ok_or("配置目录无效")?;
        std::fs::create_dir_all(parent).map_err(|_| "无法创建配置目录")?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| "无法写入配置目录")?;
        serde_json::to_writer_pretty(&mut file, doc).map_err(|_| "配置序列化失败")?;
        file.flush()
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "配置写入失败，原配置未更改")?;
        file.persist(&self.path)
            .map_err(|_| "配置替换失败，原配置未更改")?;
        self.cached = Some(doc.clone());
        Ok(())
    }
    fn update(
        &mut self,
        f: impl FnOnce(&mut Document) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut d = self.load()?;
        f(&mut d)?;
        self.persist(&d)
    }
    fn ai_view(&mut self) -> Result<AiView, String> {
        let d = self.load()?;
        Ok(AiView {
            enabled: d.enabled,
            base_url: d.base_url,
            model: d.model,
            has_api_key: d.secret_id.is_some(),
        })
    }
    fn ai_save(&mut self, c: AiUpdate) -> Result<(), String> {
        validate_ai(&c)?;
        let mut d = self.load()?;
        let old = d.secret_id.clone();
        let key = c.api_key.as_deref().unwrap_or("").trim();
        let new = if key.is_empty() {
            None
        } else {
            Some(uuid::Uuid::new_v4().to_string())
        };
        if let Some(id) = &new {
            self.vault.set(id, key)?;
            d.secret_id = Some(id.clone());
        } else if c.clear_key {
            d.secret_id = None;
        }
        d.enabled = c.enabled;
        d.base_url = c.base_url.trim().trim_end_matches('/').into();
        d.model = c.model.trim().into();
        if let Err(e) = self.persist(&d) {
            if let Some(id) = new {
                self.vault.remove(&id);
            }
            return Err(e);
        }
        if old != d.secret_id {
            self.secret = new.map(|id| (id, key.to_string()));
            if let Some(id) = old {
                self.vault.remove(&id);
            }
        }
        Ok(())
    }
    fn snapshot(&mut self) -> Result<(AiConfig, RoutingConfig, ServiceToggles), String> {
        let d = self.load()?;
        let mut error = None;
        let key = if d.enabled {
            if let Some(id) = &d.secret_id {
                if self.secret.as_ref().map(|(i, _)| i) != Some(id) {
                    match self.vault.get(id) {
                        Ok(key) => self.secret = Some((id.clone(), key)),
                        Err(message) => error = Some(message),
                    }
                }
                self.secret
                    .as_ref()
                    .filter(|(i, _)| i == id)
                    .map(|(_, key)| key.clone())
                    .unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        // An unavailable credential affects AI only, not unrelated providers.
        Ok((
            AiConfig {
                enabled: d.enabled,
                base_url: d.base_url,
                api_key: key,
                model: d.model,
                error,
            },
            d.routing,
            d.services,
        ))
    }
}
fn validate_ai(c: &AiUpdate) -> Result<(), String> {
    if c.enabled && (c.base_url.trim().is_empty() || c.model.trim().is_empty()) {
        return Err("启用 AI 前请填写 Base URL 和模型".into());
    }
    if !c.base_url.trim().is_empty() {
        let u = tauri::Url::parse(c.base_url.trim()).map_err(|_| "Base URL 格式不正确")?;
        let local = u.host_str().is_some_and(|h| {
            h == "localhost"
                || h.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !(u.scheme() == "https" || (u.scheme() == "http" && local))
            || !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
        {
            return Err(
                "云端地址必须使用 HTTPS，仅本机允许 HTTP；地址不能包含凭据或查询参数".into(),
            );
        }
    }
    Ok(())
}
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
pub fn set_config_dir(dir: PathBuf) {
    let _ = STORE.set(Mutex::new(Store {
        path: dir.join("config.json"),
        vault: Box::new(SystemSecrets),
        cached: None,
        secret: None,
    }));
}
fn with_store<T>(f: impl FnOnce(&mut Store) -> Result<T, String>) -> Result<T, String> {
    let mut s = STORE
        .get()
        .ok_or("配置未初始化")?
        .lock()
        .map_err(|_| "配置服务不可用，请重启应用")?;
    f(&mut s)
}
pub fn load_ai_config() -> Result<AiView, String> {
    with_store(Store::ai_view)
}
pub fn save_ai_config(c: AiUpdate) -> Result<(), String> {
    with_store(|s| s.ai_save(c))
}
pub fn load_routing() -> Result<RoutingConfig, String> {
    with_store(|s| Ok(s.load()?.routing))
}
pub fn load_services() -> Result<ServiceToggles, String> {
    with_store(|s| Ok(s.load()?.services))
}
pub fn set_service(id: &str, enabled: bool) -> Result<ServiceToggles, String> {
    with_store(|s| {
        s.update(|d| {
            match id {
                "youdao" => d.services.youdao = enabled,
                "deepl" => d.services.deepl = enabled,
                "bing" => d.services.bing = enabled,
                "google" => d.services.google = enabled,
                _ => return Err("未知服务".into()),
            };
            Ok(())
        })?;
        Ok(s.load()?.services)
    })
}
pub fn save_routing(c: RoutingConfig) -> Result<(), String> {
    const LANGS: &[&str] = &[
        "zh-Hans", "zh-Hant", "en", "ja", "ko", "fr", "de", "ru", "es",
    ];
    let mut seen = std::collections::HashSet::new();
    if !LANGS.contains(&c.fallback.as_str())
        || c.rules.iter().any(|r| {
            !LANGS.contains(&r.from.as_str())
                || !LANGS.contains(&r.to.as_str())
                || r.from == r.to
                || !seen.insert(&r.from)
        })
    {
        return Err("规则不能重复、源语言和目标语言不能相同，且必须使用支持的语言".into());
    }
    with_store(|s| {
        s.update(|d| {
            d.routing = c;
            Ok(())
        })
    })
}
pub fn snapshot() -> Result<(AiConfig, RoutingConfig, ServiceToggles), String> {
    with_store(Store::snapshot)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Memory(Mutex<std::collections::HashMap<String, String>>);
    impl Secrets for Memory {
        fn get(&self, id: &str) -> Result<String, String> {
            self.0
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .ok_or("missing".into())
        }
        fn set(&self, id: &str, key: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(id.into(), key.into());
            Ok(())
        }
        fn remove(&self, id: &str) {
            self.0.lock().unwrap().remove(id);
        }
    }
    fn store(path: PathBuf) -> Store {
        Store {
            path,
            cached: None,
            secret: None,
            vault: Box::new(Memory(Mutex::new(Default::default()))),
        }
    }
    #[test]
    fn legacy_key_migrates_without_disclosure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"api_key":"synthetic-test-secret","routing":{"rules":[],"fallback":"ja"}}"#,
        )
        .unwrap();
        let mut s = store(path.clone());
        let view = s.ai_view().unwrap();
        assert!(view.has_api_key);
        assert!(!std::fs::read_to_string(path)
            .unwrap()
            .contains("synthetic-test-secret"));
        assert!(!serde_json::to_string(&view)
            .unwrap()
            .contains("synthetic-test-secret"));
        assert!(s.load().unwrap().routing.rules.is_empty());
    }
    #[test]
    fn corrupt_config_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "broken").unwrap();
        let mut s = store(path.clone());
        assert!(s
            .update(|d| {
                d.enabled = true;
                Ok(())
            })
            .is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "broken");
    }
    #[test]
    fn reject_insecure_remote_url() {
        for url in [
            "http://api.example.com/v1",
            "http://localhost.evil/v1",
            "https://user:pass@host/v1",
        ] {
            assert!(validate_ai(&AiUpdate {
                enabled: true,
                base_url: url.into(),
                model: "test".into(),
                api_key: None,
                clear_key: false
            })
            .is_err());
        }
    }
    #[test]
    fn unavailable_credential_does_not_disable_free_services() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = store(dir.path().join("config.json"));
        s.cached = Some(Document {
            enabled: true,
            secret_id: Some("missing".into()),
            ..Document::default()
        });
        let (ai, _, services) = s.snapshot().unwrap();
        assert!(ai.error.is_some());
        assert!(ai.api_key.is_empty());
        assert!(services.bing);
    }
    #[test]
    fn failed_replacement_preserves_old_document_and_credential() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = store(dir.path().join("config.json"));
        let update = |key: &str| AiUpdate {
            enabled: true,
            base_url: "https://example.invalid/v1".into(),
            model: "test".into(),
            api_key: Some(key.into()),
            clear_key: false,
        };
        s.ai_save(update("original-test-key")).unwrap();
        let old = s.load().unwrap();
        // A directory cannot be atomically replaced by a file, on every OS.
        s.path = dir.path().join("blocked");
        std::fs::create_dir(&s.path).unwrap();
        assert!(s.ai_save(update("replacement-test-key")).is_err());
        assert_eq!(s.load().unwrap().secret_id, old.secret_id);
        assert_eq!(
            s.vault.get(old.secret_id.as_ref().unwrap()).unwrap(),
            "original-test-key"
        );
    }
}

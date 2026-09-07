//! Request-scoped events and bounded blocking network work.
use crate::{
    config, db,
    diagnostics::{self, Event, Record},
    provider_guard,
};
use lucas_core::{
    services::{FailureInfo, ServiceError, TranslateService},
    QueryResult,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

fn resolve_languages(
    text: &str,
    from: &str,
    to: &str,
    routing: &config::RoutingConfig,
) -> (String, String, Option<lucas_core::lang::LanguageDecision>) {
    let detection = (from == "auto").then(|| lucas_core::lang::detect_source_decision(text));
    let source = detection
        .as_ref()
        .map(|decision| decision.language)
        .unwrap_or(from)
        .to_string();
    let target = if to != "auto" {
        to.to_string()
    } else if detection.is_some_and(|decision| !decision.reliable) {
        // 不让低置信猜测命中用户的精确路由规则；短词交给安全的 fallback。
        routing.fallback.clone()
    } else {
        routing
            .rules
            .iter()
            .find(|rule| rule.from == source)
            .map(|rule| rule.to.clone())
            .unwrap_or_else(|| routing.fallback.clone())
    };
    (source, target, detection)
}

struct Jobs {
    current: Mutex<Option<(String, CancellationToken)>>,
    slots: Arc<Semaphore>,
}
static JOBS: OnceLock<Jobs> = OnceLock::new();
fn jobs() -> &'static Jobs {
    JOBS.get_or_init(|| Jobs {
        current: Mutex::new(None),
        slots: Arc::new(Semaphore::new(8)),
    })
}
#[derive(Clone, Serialize)]
pub struct ServiceInfo {
    pub id: &'static str,
    pub service: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub logo: &'static str,
}
pub fn catalog() -> Vec<ServiceInfo> {
    vec![
        ServiceInfo {
            id: "youdao",
            service: "YoudaoDict",
            label: "有道",
            description: "中英互译 · 英美音标",
            logo: "logos/youdao.png",
        },
        ServiceInfo {
            id: "bing",
            service: "Bing",
            label: "Bing",
            description: "免费网页通道",
            logo: "logos/bing.png",
        },
        ServiceInfo {
            id: "deepl",
            service: "DeepLFree",
            label: "DeepL",
            description: "免费通道 · 可用性受服务商影响",
            logo: "logos/deepl.png",
        },
        ServiceInfo {
            id: "google",
            service: "GoogleFree",
            label: "Google",
            description: "免费通道 · 部分网络需要代理",
            logo: "logos/google.png",
        },
    ]
}
pub fn cancel(id: &str) {
    if let Ok(current) = jobs().current.lock() {
        if let Some((active, token)) = &*current {
            if active == id {
                token.cancel();
                diagnostics::record(Record::new(Event::Cancelled, id, "", true, 0, None));
            }
        }
    }
}
fn emit(app: &AppHandle, token: &CancellationToken, id: &str, kind: &str, data: Value) {
    if !token.is_cancelled() {
        let _ = app.emit_to(
            "main",
            "lucas://translation",
            json!({"request_id":id,"kind":kind,"data":data}),
        );
    }
}
pub fn start(
    app: AppHandle,
    id: String,
    text: String,
    from: String,
    to: String,
    only: Option<String>,
) -> Result<(), String> {
    if id.is_empty() || id.len() > 128 {
        return Err("请求编号无效".into());
    }
    if text.trim().is_empty() || text.chars().count() > 20_000 {
        return Err("请输入 1–20000 个字符；较长内容请分段翻译".into());
    }
    let token = CancellationToken::new();
    {
        let mut current = jobs().current.lock().map_err(|_| "翻译队列不可用")?;
        if let Some((_, previous)) = current.take() {
            previous.cancel();
        }
        *current = Some((id.clone(), token.clone()));
    }
    tauri::async_runtime::spawn(async move {
        let outcome = tokio::select! {
            _ = token.cancelled() => return,
            r = tokio::time::timeout(std::time::Duration::from_secs(65), run(&app,&token,&id,text,from,to,only)) => r,
        };
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(message)) => emit(
                &app,
                &token,
                &id,
                "error",
                json!({"message":message,"code":"configuration"}),
            ),
            Err(_) => {
                diagnostics::record(Record::new(
                    Event::TranslationTimeout,
                    &id,
                    "",
                    false,
                    65_000,
                    None,
                ));
                emit(
                    &app,
                    &token,
                    &id,
                    "error",
                    json!({"message":"等待超时，可重试未完成的渠道","code":"timeout"}),
                )
            }
        }
        emit(&app, &token, &id, "done", json!({}));
        token.cancel(); // Discard late work, including a superseded blocking HTTP call.
    });
    Ok(())
}
async fn run(
    app: &AppHandle,
    token: &CancellationToken,
    id: &str,
    text: String,
    from: String,
    to: String,
    only: Option<String>,
) -> Result<(), String> {
    let (ai, routing, toggles) = tauri::async_runtime::spawn_blocking(config::snapshot)
        .await
        .map_err(|_| "读取设置失败")??;
    if token.is_cancelled() {
        return Ok(());
    }
    let mut services = crate::build_services(&ai, &toggles);
    if let Some(name) = &only {
        services.retain(|s| s.name() == name);
    }
    if services.is_empty() {
        return Err("没有启用的翻译服务，请在偏好设置中开启并完成配置".into());
    }
    let (source, target, detection) = resolve_languages(&text, &from, &to, &routing);
    emit(
        app,
        token,
        id,
        "start",
        json!({
            "services": services.iter().map(|s| s.name()).collect::<Vec<_>>(),
            "from": source,
            "to": target,
            "approximate": from == "auto",
            "detection": detection,
        }),
    );
    let mut tasks = tokio::task::JoinSet::new();
    for service in services {
        let request_id = id.to_string();
        let token = token.clone();
        let text = text.clone();
        let from = from.clone();
        let target = target.clone();
        let source = source.clone();
        tasks.spawn(async move {
            let name=service.name();
            if let Some(info)=provider_guard::global().blocked(name,std::time::Instant::now()) {
                return Some(skipped(&text,&source,&target,name,&request_id,info));
            }
            let provider_permit=tokio::select! { _=token.cancelled()=>return None,p=provider_guard::global().slot(name).acquire_owned()=>p.ok()? };
            let permit = tokio::select! { _ = token.cancelled() => return None, p = jobs().slots.clone().acquire_owned() => p.ok()? };
            if token.is_cancelled() { return None; }
            let work = tauri::async_runtime::spawn_blocking(move || {
                let _permit = permit; // Keep the global bound until HTTP really finishes.
                let _provider_permit=provider_permit;
                // A cancelled HTTP call can establish cooldown while we wait.
                if let Some(info)=provider_guard::global().blocked(name,std::time::Instant::now()) {
                    return skipped(&text,&source,&target,name,&request_id,info);
                }
                let started=std::time::Instant::now();
                let mut result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| query(service,&text,&from,&source,&target)))
                    .unwrap_or_else(|_| failure(&text,&source,&target,name,ServiceError::Internal));
                if let Some(info)=result.failure.as_mut(){info.incident_id=Some(uuid::Uuid::new_v4().to_string());}
                let jitter=u64::from(uuid::Uuid::new_v4().as_bytes()[0]%6);
                provider_guard::global().observe(name,result.failure.as_mut(),std::time::Instant::now(),jitter);
                diagnostics::record(Record::new(Event::ProviderResult,&request_id,name,result.failure.is_none(),started.elapsed().as_millis() as u64,result.failure.as_ref()));
                result
            });
            tokio::select! { _ = token.cancelled() => None, r = work => r.ok() }
        });
    }
    let mut recorded = only.is_some(); // A card retry must not duplicate the query history.
    let mut history_write = None;
    while let Some(result) = tasks.join_next().await {
        if token.is_cancelled() {
            return Ok(());
        }
        if let Ok(Some(result)) = result {
            emit(app, token, id, "result", json!(result)); // Storage latency must not delay the result.
            if !recorded && !result.paragraphs.is_empty() {
                recorded = true;
                let text = result.text;
                let translated = result.paragraphs.join("\n");
                let service = result.service;
                history_write = Some(tauri::async_runtime::spawn_blocking(move || {
                    db::add_history(&text, &translated, &service)
                }));
            }
        }
    }
    if let Some(write) = history_write {
        if !matches!(write.await, Ok(Ok(()))) {
            emit(
                app,
                token,
                id,
                "warning",
                json!({"message":"翻译已完成，但历史记录保存失败"}),
            );
        }
    }
    Ok(())
}
fn query(
    service: Box<dyn TranslateService>,
    text: &str,
    from: &str,
    source: &str,
    to: &str,
) -> QueryResult {
    if service.name() == "YoudaoDict" && lucas_core::lang::dictionary_eligible(text, from, to) {
        return lucas_core::services::dict_route(text, to)
            .and_then(|result| {
                if result.paragraphs.iter().any(|p| !p.trim().is_empty()) {
                    Ok(result)
                } else {
                    Err(ServiceError::Parse(String::new()))
                }
            })
            .unwrap_or_else(|e| failure(text, source, to, "YoudaoDict", e));
    }
    match service.translate_with_detection(text, from, to) {
        Ok(output) if output.paragraphs.iter().any(|p| !p.trim().is_empty()) => {
            let provider_source = if from == "auto" {
                output
                    .detected_from
                    .as_deref()
                    .and_then(lucas_core::lang::normalize_provider_language)
            } else {
                None
            };
            let paragraphs = output.paragraphs;
            QueryResult {
                text: text.into(),
                detected_from: provider_source.unwrap_or(source).into(),
                detected_to: to.into(),
                source_confirmed: provider_source.is_some(),
                pinyin: lucas_core::pinyin::annotate(&paragraphs.join("\n")),
                paragraphs,
                dict: None,
                service: service.name().into(),
                error: None,
                failure: None,
            }
        }
        Ok(_) => failure(
            text,
            source,
            to,
            service.name(),
            ServiceError::Parse(String::new()),
        ),
        Err(e) => failure(text, source, to, service.name(), e),
    }
}
fn failure(text: &str, from: &str, to: &str, service: &str, error: ServiceError) -> QueryResult {
    failed_result(text, from, to, service, error.info())
}
fn skipped(
    text: &str,
    from: &str,
    to: &str,
    service: &str,
    request_id: &str,
    info: FailureInfo,
) -> QueryResult {
    diagnostics::record(Record::new(
        Event::ProviderSkipped,
        request_id,
        service,
        false,
        0,
        Some(&info),
    ));
    failed_result(text, from, to, service, info)
}
fn failed_result(
    text: &str,
    from: &str,
    to: &str,
    service: &str,
    info: FailureInfo,
) -> QueryResult {
    QueryResult {
        text: text.into(),
        detected_from: from.into(),
        detected_to: to.into(),
        source_confirmed: false,
        paragraphs: vec![],
        dict: None,
        pinyin: None,
        service: service.into(),
        error: Some(info.code.message().into()),
        failure: Some(info),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use lucas_core::services::ServiceError;
    struct Local;
    impl TranslateService for Local {
        fn name(&self) -> &'static str {
            "AI"
        }
        fn translate(&self, text: &str, _: &str, _: &str) -> Result<Vec<String>, ServiceError> {
            Ok(vec![text.into()])
        }
    }
    struct Detecting;
    impl TranslateService for Detecting {
        fn name(&self) -> &'static str {
            "Detecting"
        }
        fn translate(&self, text: &str, _: &str, _: &str) -> Result<Vec<String>, ServiceError> {
            Ok(vec![text.into()])
        }
        fn translate_with_detection(
            &self,
            text: &str,
            _: &str,
            _: &str,
        ) -> Result<lucas_core::TranslationOutput, ServiceError> {
            Ok(lucas_core::TranslationOutput {
                paragraphs: vec![text.into()],
                detected_from: Some("FR".into()),
            })
        }
    }
    #[test]
    fn local_only_short_word_does_not_invoke_dictionary() {
        let r = query(Box::new(Local), "hello", "en", "en", "zh-Hans");
        assert_eq!(r.service, "AI");
        assert!(r.dict.is_none());
        assert_eq!(r.paragraphs, ["hello"]);
    }
    #[test]
    fn provider_detection_corrects_auto_but_never_manual_language() {
        let auto = query(Box::new(Detecting), "autonomous", "auto", "en", "zh-Hans");
        assert_eq!(auto.detected_from, "fr");
        assert!(auto.source_confirmed);

        let manual = query(Box::new(Detecting), "autonomous", "en", "en", "zh-Hans");
        assert_eq!(manual.detected_from, "en");
        assert!(!manual.source_confirmed);
    }
    #[test]
    fn uncertain_detection_uses_fallback_instead_of_a_precise_rule() {
        let routing = config::RoutingConfig {
            rules: vec![config::RouteRule {
                from: "en".into(),
                to: "ja".into(),
            }],
            fallback: "zh-Hans".into(),
        };
        let (source, target, detection) = resolve_languages("autonomous", "auto", "auto", &routing);
        assert_eq!(source, "en");
        assert_eq!(target, "zh-Hans");
        assert!(!detection.unwrap().reliable);
    }
    #[test]
    fn catalogue_matches_enabled_services() {
        let services = crate::build_services(
            &config::AiConfig::default(),
            &config::ServiceToggles::default(),
        );
        assert_eq!(services.len(), catalog().len());
        for s in services {
            assert!(catalog().iter().any(|m| m.service == s.name()));
        }
    }
    #[test]
    fn credential_error_is_an_ai_card_not_a_global_failure() {
        let ai = config::AiConfig {
            enabled: true,
            error: Some("钥匙串不可用".into()),
            ..Default::default()
        };
        let services = crate::build_services(&ai, &config::ServiceToggles::default());
        assert_eq!(services.len(), catalog().len() + 1);
        let service = services.into_iter().find(|s| s.name() == "AI").unwrap();
        let r = query(service, "test", "en", "en", "ja");
        assert_eq!(r.service, "AI");
        assert_eq!(
            r.failure.unwrap().code,
            lucas_core::services::ErrorCode::Configuration
        );
    }
    #[tokio::test]
    async fn cancellation_does_not_release_a_running_workers_slot() {
        let slots = Arc::new(Semaphore::new(1));
        let permit = slots.clone().acquire_owned().await.unwrap();
        let token = CancellationToken::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let work = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _ = started_tx.send(());
            let _ = finish_rx.recv_timeout(std::time::Duration::from_secs(2));
        });
        started_rx.await.unwrap();
        token.cancel();
        assert!(token.is_cancelled());
        assert!(slots.clone().try_acquire_owned().is_err());
        finish_tx.send(()).unwrap();
        work.await.unwrap();
        assert_eq!(slots.available_permits(), 1);
    }
}

//! Omarchy-only process: no Tauri, WebView, tray, theme engine, or HTTP listener.
mod config;
mod db;
mod desktop;
mod diagnostics;
mod ocr_text;
mod protocol;
mod provider_guard;
mod result_cache;
mod selection;
mod services;
mod storage;
mod translation;

use protocol::{EventSink, Request};
use serde::Deserialize;
use serde_json::{json, Value};
use services::build_services;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Query {
    text: String,
    #[serde(default = "automatic")]
    from: String,
    #[serde(default = "automatic")]
    to: String,
    only: Option<String>,
}
fn automatic() -> String {
    "auto".into()
}
fn string<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params[key].as_str().ok_or_else(|| format!("Missing {key}"))
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|_| "Invalid parameters".into())
}

fn settings() -> Result<Value, String> {
    Ok(
        json!({"preferences":config::preferences()?,"services":config::load_services()?,
        "routing":config::load_routing()?,"ai":config::load_ai_config()?,"official":config::official_view()?,"usage":config::load_usage()?}),
    )
}

fn test_official_connection() -> Result<Value, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err("正在检测，请稍候".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }
    let _guard = Guard;
    config::official_service()?
        .test_connection()
        .map_err(|error| error.to_string())?;
    Ok(json!({"connected":true}))
}

fn command(request: Request) -> Result<Value, String> {
    let p = request.params;
    match request.method.as_str() {
        "settings" => settings(),
        "catalog" => Ok(json!(translation::catalog())),
        "service.order" => {
            Ok(json!({"order":config::save_service_order(decode(p["order"].clone())?)?}))
        }
        "usage.save" => {
            config::save_usage(decode(p)?)?;
            let usage = config::load_usage()?;
            db::prune_history(usage.history_days, usage.history_limit)?;
            settings()
        }
        "preferences.save" => {
            config::save_preferences(decode(p)?)?;
            settings()
        }
        "routing.save" => {
            config::save_routing(decode(p)?)?;
            settings()
        }
        "ai.save" => {
            config::save_ai_config(decode(p)?)?;
            settings()
        }
        "official.save" => {
            config::save_official(decode(p)?)?;
            settings()
        }
        "official.test" => test_official_connection(),
        "service.set" => {
            config::set_service(
                string(&p, "id")?,
                p["enabled"].as_bool().ok_or("Missing enabled")?,
            )?;
            settings()
        }
        "history.list" => {
            let usage = config::load_usage()?;
            db::prune_history(usage.history_days, usage.history_limit)?;
            Ok(json!(db::search_history(
                p["limit"].as_i64().unwrap_or(50),
                p["offset"].as_i64().unwrap_or(0),
                p["search"].as_str().unwrap_or("")
            )?))
        }
        "favorites.list" => Ok(json!(db::list_favorites(
            p["limit"].as_i64().unwrap_or(50),
            p["offset"].as_i64().unwrap_or(0)
        )?)),
        "history.clear" => {
            db::clear_history()?;
            Ok(json!({}))
        }
        "favorite.add" => {
            db::add_favorite(
                string(&p, "text")?,
                string(&p, "result")?,
                string(&p, "service")?,
            )?;
            Ok(json!({}))
        }
        "favorite.remove" => {
            db::remove_favorite(p["id"].as_i64().ok_or("Missing id")?)?;
            Ok(json!({}))
        }
        "favorite.status" => Ok(
            json!({"id":db::favorite_id(string(&p,"text")?,string(&p,"result")?,string(&p,"service")?)?}),
        ),
        "favorite.toggle" => Ok(
            json!({"id":db::toggle_favorite(string(&p,"text")?,string(&p,"result")?,string(&p,"service")?)?}),
        ),
        "diagnostics" => Ok(json!(diagnostics::status())),
        "logs.directory" => Ok(json!(diagnostics::directory()?)),
        _ => Err("Unknown method".into()),
    }
}

async fn dispatch(request: Request, sink: EventSink, plugin: PathBuf) {
    let id = request.id.clone();
    let result = match request.method.as_str() {
        "translate" => decode::<Query>(request.params).and_then(|q| {
            const LANGS: &[&str] = &[
                "auto", "zh-Hans", "zh-Hant", "en", "ja", "ko", "fr", "de", "ru", "es",
            ];
            if !LANGS.contains(&q.from.as_str()) || !LANGS.contains(&q.to.as_str()) {
                return Err("Unsupported language".into());
            }
            translation::start(sink.clone(), id.clone(), q.text, q.from, q.to, q.only)?;
            Ok(json!({"started":true}))
        }),
        "cancel" => {
            translation::cancel(request.params["request_id"].as_str().unwrap_or(""));
            Ok(json!({}))
        }
        "capture" => match string(&request.params, "action") {
            Ok(action) => {
                let started = std::time::Instant::now();
                let result = desktop::capture(action, &request.params["source"]).await;
                diagnostics::record(diagnostics::Record::new(
                    if action == "ocr" {
                        diagnostics::Event::OcrResult
                    } else {
                        diagnostics::Event::CaptureResult
                    },
                    &id,
                    "",
                    result.is_ok(),
                    started.elapsed().as_millis() as u64,
                    None,
                ));
                result
            }
            Err(e) => Err(e),
        },
        "copy" => match string(&request.params, "text") {
            Ok(text) => desktop::copy(text).await,
            Err(e) => Err(e),
        },
        "speak" => match string(&request.params, "url") {
            Ok(url) => desktop::speak(url).await,
            Err(e) => Err(e),
        },
        "shortcuts.status" | "shortcuts.check" | "shortcuts.save" => {
            let template = std::fs::read_to_string(plugin.join("shortcuts.lua"));
            match template {
                Ok(template) => {
                    let mut child = tokio::process::Command::new("python3");
                    child
                        .arg(plugin.join("scripts/shortcuts.py"))
                        .args(["--template-text", &template]);
                    if request.method == "shortcuts.status" {
                        child.arg("--status");
                    } else {
                        child
                            .arg(if request.method == "shortcuts.check" {
                                "--check"
                            } else {
                                "--apply"
                            })
                            .arg(request.params.to_string());
                    }
                    child.kill_on_drop(true);
                    match tokio::time::timeout(std::time::Duration::from_secs(25), child.output())
                        .await
                    {
                        Ok(Ok(output)) if output.status.success() => {
                            serde_json::from_slice(&output.stdout)
                                .map_err(|_| "Invalid shortcut state".into())
                        }
                        Ok(Ok(output)) => {
                            Err(String::from_utf8_lossy(&output.stderr).trim().into())
                        }
                        _ => Err("Shortcut operation failed".into()),
                    }
                }
                Err(_) => Err("Native shortcut template unavailable".into()),
            }
        }
        _ => tokio::task::spawn_blocking(move || command(request))
            .await
            .unwrap_or_else(|_| Err("Command failed".into())),
    };
    sink.reply(&id, result);
}

#[tokio::main]
async fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [mode] if mode == "--selection-offer" => {
            if selection::offer().is_err() {
                std::process::exit(1)
            }
            return;
        }
        [mode] if mode == "--restore-selection-clipboard" => {
            if selection::restore().is_err() {
                std::process::exit(1)
            }
            return;
        }
        [mode, source] if mode == "--read-selection" => {
            let source: Value = serde_json::from_str(source).unwrap_or(Value::Null);
            let result = match selection::read(&source).await {
                Ok(text) => json!({"text":text.unwrap_or_default()}),
                Err(error) => json!({"error":error}),
            };
            println!("{result}");
            return;
        }
        _ => {}
    }
    let plugin = match arguments.as_slice() {
        [mode, flag, path] if mode == "--stdio" && flag == "--plugin-dir" => PathBuf::from(path),
        [mode] if mode == "--stdio" => PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(".config/omarchy/plugins/lucas.translate"),
        _ => {
            eprintln!("Usage: lucas-translate-omarchy-backend --stdio [--plugin-dir <directory>]");
            std::process::exit(2);
        }
    };
    let sink = EventSink::new();
    if let Err(error) = storage::initialize() {
        sink.event("fatal", json!({"message":error}));
        std::process::exit(1);
    }
    sink.event(
        "ready",
        json!({"protocol":1,"version":env!("CARGO_PKG_VERSION")}),
    );
    // Tokio's stdin reader uses an uncancellable blocking task that prevents
    // runtime shutdown while Shell still holds the pipe open. Use a dedicated
    // input thread and a bounded channel instead.
    let (input, mut frames) = tokio::sync::mpsc::channel(16);
    std::thread::spawn(move || {
        use std::io::{BufRead, Read};
        let mut reader = std::io::BufReader::new(std::io::stdin());
        loop {
            let mut bytes = Vec::new();
            match (&mut reader)
                .take((protocol::MAX_FRAME + 1) as u64)
                .read_until(b'\n', &mut bytes)
            {
                Ok(0) | Err(_) => break,
                _ => {}
            }
            let oversized = bytes.len() > protocol::MAX_FRAME;
            if input.blocking_send(bytes).is_err() || oversized {
                break;
            }
        }
    });
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(16));
    let mut commands = tokio::task::JoinSet::new();
    let mut terminate =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(_) => {
                sink.event(
                    "fatal",
                    json!({"message":"Cannot initialize shutdown handler"}),
                );
                return;
            }
        };
    loop {
        while commands.try_join_next().is_some() {}
        let bytes = tokio::select! {
            _ = terminate.recv() => break,
            _ = tokio::signal::ctrl_c() => break,
            bytes = frames.recv() => match bytes { Some(bytes) => bytes, None => break },
        };
        if bytes.len() > protocol::MAX_FRAME {
            sink.event("protocol_error", json!({"code":"frame_too_large"}));
            break;
        }
        let request = match protocol::parse(&bytes) {
            Ok(request) => request,
            Err(code) => {
                sink.event("protocol_error", json!({"code":code}));
                continue;
            }
        };
        let permit = match permits.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                sink.reply(&request.id, Err("Too many pending operations".into()));
                continue;
            }
        };
        let sink = sink.clone();
        let plugin = plugin.clone();
        commands.spawn(async move {
            let _permit = permit;
            dispatch(request, sink, plugin).await;
        });
    }
    translation::shutdown();
    commands.abort_all();
    while commands.join_next().await.is_some() {}
}

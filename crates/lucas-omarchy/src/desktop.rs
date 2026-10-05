//! Wayland operations live outside the shared Shell process.
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

static CAPTURING: AtomicBool = AtomicBool::new(false);
static ANNOTATION: OnceLock<Mutex<Option<(String, tempfile::TempDir)>>> = OnceLock::new();
fn annotation() -> &'static Mutex<Option<(String, tempfile::TempDir)>> {
    ANNOTATION.get_or_init(|| Mutex::new(None))
}
pub fn cleanup() {
    if let Ok(mut current) = annotation().lock() {
        current.take();
    }
}

pub fn discard_annotation(id: &str) -> Result<Value, String> {
    let mut current = annotation()
        .lock()
        .map_err(|_| "Annotation storage unavailable")?;
    if current.as_ref().is_some_and(|(active, _)| active == id) {
        current.take();
    }
    Ok(json!({}))
}

pub async fn copy_annotation(id: &str) -> Result<Value, String> {
    let path = {
        let current = annotation()
            .lock()
            .map_err(|_| "Annotation storage unavailable")?;
        let (_, directory) = current
            .as_ref()
            .filter(|(active, _)| active == id)
            .ok_or("Annotation expired")?;
        directory.path().join("annotated.png")
    };
    let bytes = std::fs::read(path).map_err(|_| "Cannot read annotated image")?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.len() > 64 * 1024 * 1024 {
        return Err("Invalid annotated image".into());
    }
    clipboard(&bytes, "image/png").await?;
    discard_annotation(id)?;
    Ok(json!({"copied":true}))
}

async fn output(command: &mut Command, seconds: u64) -> Result<std::process::Output, String> {
    command.kill_on_drop(true).stdin(Stdio::null());
    tokio::time::timeout(Duration::from_secs(seconds), command.output())
        .await
        .map_err(|_| "Desktop operation timed out".to_string())?
        .map_err(|_| "Required desktop tool is unavailable".to_string())
}

pub async fn copy(text: &str) -> Result<Value, String> {
    if text.len() > 128 * 1024 {
        return Err("Text is too large".into());
    }
    clipboard(text.as_bytes(), "text/plain;charset=utf-8").await?;
    Ok(json!({"copied":true}))
}

async fn clipboard(bytes: &[u8], mime: &str) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .args(["--type", mime])
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "wl-copy is unavailable".to_string())?;
    let mut input = child.stdin.take().ok_or("Clipboard input unavailable")?;
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        input.write_all(bytes).await?;
        drop(input);
        child.wait().await
    })
    .await
    .map_err(|_| "Clipboard operation timed out")?
    .map_err(|_| "Cannot copy text")?;
    if !outcome.success() {
        return Err("Cannot copy text".into());
    }
    Ok(())
}

pub async fn capture(action: &str) -> Result<Value, String> {
    if CAPTURING.swap(true, Ordering::AcqRel) {
        return Err("A capture is already running".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            CAPTURING.store(false, Ordering::Release);
        }
    }
    let _guard = Guard;
    if action == "selection" {
        let mut child = Command::new("wl-paste")
            .args(["--primary", "--no-newline", "--type", "text"])
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "wl-paste is unavailable".to_string())?;
        let pipe = child.stdout.take().ok_or("Cannot read selection")?;
        let mut bytes = Vec::new();
        let outcome = tokio::time::timeout(Duration::from_secs(2), async {
            pipe.take(80_001).read_to_end(&mut bytes).await?;
            if bytes.len() > 80_000 {
                let _ = child.kill().await;
            }
            child.wait().await
        })
        .await
        .map_err(|_| "Selection read timed out")?
        .map_err(|_| "Cannot read selection")?;
        if !outcome.success() {
            return Err("No PRIMARY selection; copy and paste the text instead".into());
        }
        let text = String::from_utf8(bytes).map_err(|_| "Selection is not UTF-8 text")?;
        if text.trim().is_empty() || text.chars().count() > 20_000 {
            return Err("Select 1–20000 characters".into());
        }
        return Ok(json!({"text":text,"action":action}));
    }
    if !["screenshot", "ocr", "annotate"].contains(&action) {
        return Err("Unknown capture action".into());
    }
    let temporary = tempfile::tempdir().map_err(|_| "Cannot create capture directory")?;
    let region = output(&mut Command::new("slurp"), 120).await?;
    if !region.status.success() {
        return Ok(json!({"cancelled":true}));
    }
    let geometry = String::from_utf8(region.stdout).map_err(|_| "Invalid screenshot region")?;
    let path = temporary.path().join("capture.png");
    let shot = output(
        Command::new("grim")
            .arg("-g")
            .arg(geometry.trim())
            .arg(&path),
        10,
    )
    .await?;
    if !shot.status.success() {
        return Err("Screenshot capture failed".into());
    }
    if action == "annotate" {
        let id = uuid::Uuid::new_v4().to_string();
        let output_path = temporary.path().join("annotated.png");
        let data =
            json!({"action":action,"annotation_id":id,"path":path,"output_path":output_path});
        let mut current = annotation()
            .lock()
            .map_err(|_| "Annotation storage unavailable")?;
        *current = Some((id, temporary));
        return Ok(data);
    }
    let mut ocr = output(
        Command::new("tesseract")
            .arg(&path)
            .args(["stdout", "-l", "eng+chi_sim"]),
        15,
    )
    .await?;
    if !ocr.status.success() {
        ocr = output(Command::new("tesseract").arg(&path).arg("stdout"), 15).await?;
    }
    if !ocr.status.success() {
        return Err("Local OCR failed; check installed language data".into());
    }
    let text = String::from_utf8(ocr.stdout).map_err(|_| "OCR returned invalid text")?;
    if text.trim().is_empty() {
        return Err("No text was recognized".into());
    }
    if text.chars().count() > 20_000 {
        return Err("Recognized text exceeds 20000 characters".into());
    }
    if action == "ocr" {
        copy(&text).await?;
        return Ok(json!({"action":action,"copied":true}));
    }
    Ok(json!({"action":action,"text":text}))
}

pub async fn speak(url: &str) -> Result<Value, String> {
    let parsed = url::Url::parse(url).map_err(|_| "Invalid speech URL")?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("dict.youdao.com")
        || parsed.path() != "/dictvoice"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Unsupported speech URL".into());
    }
    let result = output(
        Command::new("mpv").args(["--no-video", "--no-terminal", "--", url]),
        25,
    )
    .await?;
    if result.status.success() {
        Ok(json!({}))
    } else {
        Err("Pronunciation playback failed".into())
    }
}

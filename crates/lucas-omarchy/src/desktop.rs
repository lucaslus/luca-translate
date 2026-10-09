//! Wayland operations live outside the shared Shell process.
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

static CAPTURING: AtomicBool = AtomicBool::new(false);

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

pub async fn capture(action: &str, source: &Value) -> Result<Value, String> {
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
        // The helper completes clipboard restoration even if Shell disconnects
        // or the parent backend shuts down during selection acquisition.
        let mut child =
            Command::new(std::env::current_exe().map_err(|_| "Selection reader unavailable")?)
                .arg("--read-selection")
                .arg(source.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| "Selection reader unavailable")?;
        let pipe = child.stdout.take().ok_or("Cannot read current selection")?;
        let mut bytes = Vec::new();
        let status = tokio::time::timeout(Duration::from_secs(6), async {
            pipe.take(128_001).read_to_end(&mut bytes).await?;
            child.wait().await
        })
        .await
        .map_err(|_| "Current selection read timed out")?
        .map_err(|_| "Cannot read current selection")?;
        if !status.success() || bytes.len() > 128_000 {
            return Err("Cannot read current selection".into());
        }
        let reply: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid current selection")?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        let text = reply["text"].as_str().unwrap_or("");
        if text.chars().count() > 20_000 {
            return Err("Select 1–20000 characters".into());
        }
        return Ok(json!({"text":text,"action":action}));
    }
    if !["screenshot", "ocr", "annotate"].contains(&action) {
        return Err("Unknown capture action".into());
    }
    let temporary = tempfile::tempdir().map_err(|_| "Cannot create capture directory")?;
    if action == "annotate" {
        let shot = output(
            Command::new("omarchy")
                .args(["capture", "screenshot", "region", "save"])
                .env("OMARCHY_SCREENSHOT_DIR", temporary.path()),
            120,
        )
        .await?;
        if !shot.status.success() {
            return Err("System screenshot capture failed".into());
        }
        let selected = String::from_utf8(shot.stdout).map_err(|_| "Invalid screenshot path")?;
        let selected = selected.trim();
        if selected.is_empty() {
            return Ok(json!({"cancelled":true}));
        }
        let path = std::fs::canonicalize(selected).map_err(|_| "Screenshot file is unavailable")?;
        let directory = std::fs::canonicalize(temporary.path())
            .map_err(|_| "Capture directory is unavailable")?;
        if !path.starts_with(&directory) || !path.is_file() {
            return Err("Invalid screenshot path".into());
        }
        // Editing is user-paced. Keep the image alive until the editor closes;
        // backend shutdown drops and kills the child without leaking the file.
        let status = Command::new("lucas-screenshot-editor")
            .args(["--title", "Lucas Screenshot", "--filename"])
            .arg(&path)
            .args([
                "--actions-on-enter",
                "save-to-clipboard",
                "--early-exit",
                "--actions-on-escape",
                "exit",
                "--copy-command",
                "wl-copy",
                "--disable-notifications",
            ])
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|_| "lucas-screenshot-editor is unavailable")?;
        if !status.success() {
            return Err("Screenshot editor failed".into());
        }
        return Ok(json!({"action":action,"handled":true}));
    }
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

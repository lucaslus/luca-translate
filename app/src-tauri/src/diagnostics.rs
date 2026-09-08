//! Bounded, asynchronous, allowlisted diagnostics. No free-form messages.
use lucas_core::services::{ErrorCode, FailureInfo};
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Startup,
    ProviderResult,
    ProviderSkipped,
    Cancelled,
    TranslationTimeout,
    CapturePermission,
    CaptureResult,
    OcrResult,
}
#[derive(Serialize)]
pub struct Record {
    version: &'static str,
    timestamp_ms: u64,
    event: Event,
    request_id: Option<String>,
    service: Option<&'static str>,
    success: bool,
    duration_ms: u64,
    code: Option<ErrorCode>,
    http_status: Option<u16>,
    retry_after_secs: Option<u64>,
    incident_id: Option<String>,
}
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
impl Record {
    pub fn new(
        event: Event,
        request: &str,
        service: &str,
        success: bool,
        duration_ms: u64,
        failure: Option<&FailureInfo>,
    ) -> Self {
        let service = match service {
            "YoudaoDict" => Some("YoudaoDict"),
            "Bing" => Some("Bing"),
            "DeepLFree" => Some("DeepLFree"),
            "DeepLApi" => Some("DeepLApi"),
            "GoogleFree" => Some("GoogleFree"),
            "AI" => Some("AI"),
            _ => None,
        };
        let safe_id = |id: &str| uuid::Uuid::parse_str(id).ok().map(|id| id.to_string());
        Self {
            version: env!("CARGO_PKG_VERSION"),
            timestamp_ms: now_ms(),
            event,
            request_id: safe_id(request),
            service,
            success,
            duration_ms,
            code: failure.map(|e| e.code),
            http_status: failure.and_then(|e| e.http_status),
            retry_after_secs: failure.and_then(|e| e.retry_after_secs),
            incident_id: failure
                .and_then(|e| e.incident_id.as_deref())
                .and_then(safe_id),
        }
    }
}
struct State {
    path: PathBuf,
    tx: SyncSender<Record>,
    failed: Arc<AtomicBool>,
    dropped: AtomicU64,
}
static STATE: OnceLock<State> = OnceLock::new();
struct Writer {
    path: PathBuf,
    file: Option<File>,
    bytes: u64,
    limit: u64,
}
impl Writer {
    fn new(path: PathBuf, limit: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(&path)?;
        let mut writer = Self {
            path,
            file: None,
            bytes: 0,
            limit,
        };
        writer.open()?;
        Ok(writer)
    }
    fn open(&mut self) -> std::io::Result<()> {
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(self.path.join("diagnostics.jsonl"))?;
        self.bytes = file.metadata()?.len();
        self.file = Some(file);
        Ok(())
    }
    fn write(&mut self, record: &Record) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(record)?;
        line.push(b'\n');
        if self.bytes + line.len() as u64 > self.limit {
            self.file.take();
            // Only the logger's fixed retention files are ever removed.
            let last = self.path.join("diagnostics.3.jsonl");
            if last.exists() {
                std::fs::remove_file(last)?;
            }
            for (source, dest) in [
                ("diagnostics.2.jsonl", "diagnostics.3.jsonl"),
                ("diagnostics.1.jsonl", "diagnostics.2.jsonl"),
                ("diagnostics.jsonl", "diagnostics.1.jsonl"),
            ] {
                let source = self.path.join(source);
                if source.exists() {
                    std::fs::rename(source, self.path.join(dest))?;
                }
            }
            self.open()?;
        }
        if self.file.is_none() {
            self.open()?;
        }
        self.file
            .as_mut()
            .ok_or_else(|| std::io::Error::other("log unavailable"))?
            .write_all(&line)?;
        self.bytes += line.len() as u64;
        Ok(())
    }
}
pub fn init(path: Option<PathBuf>) {
    let Some(path) = path else { return };
    let Ok(mut writer) = Writer::new(path.clone(), 1024 * 1024) else {
        return;
    };
    let (tx, rx) = sync_channel::<Record>(256);
    let failed = Arc::new(AtomicBool::new(false));
    let flag = failed.clone();
    if std::thread::Builder::new()
        .name("diagnostics-writer".into())
        .spawn(move || {
            while let Ok(record) = rx.recv() {
                flag.store(writer.write(&record).is_err(), Ordering::Relaxed);
            }
        })
        .is_ok()
    {
        let _ = STATE.set(State {
            path,
            tx,
            failed,
            dropped: AtomicU64::new(0),
        });
        record(Record::new(Event::Startup, "", "", true, 0, None));
    }
}
pub fn record(record: Record) {
    if let Some(s) = STATE.get() {
        if s.tx.try_send(record).is_err() {
            s.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
#[derive(Serialize)]
pub struct Status {
    available: bool,
    write_failed: bool,
    dropped_events: u64,
}
pub fn status() -> Status {
    match STATE.get() {
        Some(s) => Status {
            available: true,
            write_failed: s.failed.load(Ordering::Relaxed),
            dropped_events: s.dropped.load(Ordering::Relaxed),
        },
        None => Status {
            available: false,
            write_failed: true,
            dropped_events: 0,
        },
    }
}
pub fn directory() -> Result<PathBuf, String> {
    STATE
        .get()
        .map(|s| s.path.clone())
        .ok_or("日志目录不可用，请检查磁盘空间和目录权限".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_rejects_freeform_identifiers_and_contains_no_content() {
        let mut failure = FailureInfo::new(ErrorCode::RateLimited);
        failure.http_status = Some(429);
        failure.incident_id = Some("secret-key".into());
        let r = Record::new(
            Event::ProviderResult,
            "secret-input",
            "https://host?key=secret",
            false,
            1,
            Some(&failure),
        );
        let data = serde_json::to_string(&r).unwrap();
        assert!(!data.contains("secret"));
        assert!(!data.contains("https://"));
        assert!(data.contains("rate_limited") && data.contains("429"));
    }
    #[test]
    fn rotation_is_bounded_and_json_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = Writer::new(dir.path().into(), 600).unwrap();
        for _ in 0..20 {
            writer
                .write(&Record::new(Event::Startup, "", "", true, 0, None))
                .unwrap();
        }
        let files = std::fs::read_dir(dir.path())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(files.len() <= 4);
        for f in files {
            assert!(f.metadata().unwrap().len() <= 600);
            for line in std::fs::read_to_string(f.path()).unwrap().lines() {
                serde_json::from_str::<serde_json::Value>(line).unwrap();
            }
        }
    }
}

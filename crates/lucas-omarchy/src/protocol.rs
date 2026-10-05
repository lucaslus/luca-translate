//! Private stdio transport. Text and credentials never enter argv or logs.
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex};

pub const MAX_FRAME: usize = 128 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub fn parse(bytes: &[u8]) -> Result<Request, &'static str> {
    if bytes.len() > MAX_FRAME {
        return Err("frame_too_large");
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|_| "invalid_request")?;
    if request.id.is_empty() || request.id.len() > 128 {
        return Err("invalid_request_id");
    }
    Ok(request)
}

#[derive(Clone)]
pub struct EventSink(Arc<Mutex<BufWriter<std::io::Stdout>>>);
impl EventSink {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(BufWriter::new(std::io::stdout()))))
    }
    fn send(&self, value: Value) {
        if let Ok(mut writer) = self.0.lock() {
            if serde_json::to_writer(&mut *writer, &value).is_ok() {
                let _ = writer.write_all(b"\n").and_then(|_| writer.flush());
            }
        }
    }
    pub fn event(&self, event: &str, data: Value) {
        self.send(json!({"event":event,"data":data}));
    }
    pub fn reply(&self, id: &str, outcome: Result<Value, String>) {
        match outcome {
            Ok(data) => self.send(json!({"id":id,"ok":true,"data":data})),
            Err(message) => self.send(json!({"id":id,"ok":false,"error":message})),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_frames_never_echo_input() {
        assert!(parse(b"{\"api_key\":\"synthetic-secret\"}").is_err());
        assert!(parse(&vec![b' '; MAX_FRAME + 1]).is_err());
        assert!(parse(b"{\"id\":\"\",\"method\":\"settings\"}").is_err());
    }
    #[test]
    fn newlines_and_quotes_survive_transport() {
        let data = json!({"id":"1","method":"translate","params":{"text":"line\n\"quoted\""}});
        let request = parse(&serde_json::to_vec(&data).unwrap()).unwrap();
        assert_eq!(request.params["text"], "line\n\"quoted\"");
    }
}

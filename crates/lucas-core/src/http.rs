//! Shared connection pool with cancellable I/O behind the synchronous provider API.
//! No response bodies, request URLs, or credentials enter public error messages.
use crate::services::ServiceError;
use serde::{de::DeserializeOwned, Serialize};
use std::{cell::RefCell, sync::OnceLock, time::Duration};
use tokio_util::sync::CancellationToken;

const MAX_BODY: usize = 2 * 1024 * 1024;
thread_local! {
    static TOKEN: RefCell<Option<CancellationToken>> = const { RefCell::new(None) };
}

pub fn with_cancellation<T>(token: CancellationToken, work: impl FnOnce() -> T) -> T {
    struct Restore(Option<CancellationToken>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TOKEN.with(|slot| slot.replace(self.0.take()));
        }
    }
    let _restore = Restore(TOKEN.with(|slot| slot.replace(Some(token))));
    work()
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .pool_idle_timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5
                    || (attempt.url().scheme() != "https"
                        && attempt.previous().iter().any(|url| url.scheme() == "https"))
                {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .expect("HTTP client configuration is valid")
    })
}

pub struct Request(reqwest::RequestBuilder, Duration);
pub struct Response {
    url: reqwest::Url,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
}
pub fn get(url: &str) -> Request {
    Request(client().get(url), Duration::from_secs(20))
}
pub fn post(url: &str) -> Request {
    Request(client().post(url), Duration::from_secs(20))
}
impl Request {
    pub fn timeout(self, duration: Duration) -> Self {
        Self(self.0, duration)
    }
    pub fn set(self, key: &str, value: &str) -> Self {
        Self(self.0.header(key, value), self.1)
    }
    pub fn query(self, key: &str, value: &str) -> Self {
        Self(self.0.query(&[(key, value)]), self.1)
    }
    pub fn call(self) -> Result<Response, ServiceError> {
        execute(self.0, self.1)
    }
    pub fn send_json<T: Serialize + ?Sized>(self, value: &T) -> Result<Response, ServiceError> {
        execute(self.0.json(value), self.1)
    }
    /// Consume an SSE response incrementally with the same cancellation and body bounds.
    pub fn send_stream<T: Serialize + ?Sized>(
        self,
        value: &T,
        on_data: &mut dyn FnMut(&str) -> Result<bool, ServiceError>,
    ) -> Result<(), ServiceError> {
        let runtime = runtime();
        let token = TOKEN.with(|slot| slot.borrow().clone()).unwrap_or_default();
        runtime.block_on(async {
            tokio::select! {
                biased;
                _ = token.cancelled() => Err(ServiceError::Cancelled),
                result = tokio::time::timeout(self.1, async {
                    let mut response=self.0.json(value).send().await.map_err(network_error)?;
                    if !response.status().is_success() {
                        return Err(ServiceError::Http { status:response.status().as_u16(),retry_after_secs:response.headers().get("retry-after").and_then(|v|v.to_str().ok()).and_then(|v|crate::services::retry_after(v,std::time::SystemTime::now())) });
                    }
                    let sse=response.headers().get("content-type").and_then(|v|v.to_str().ok()).is_some_and(|v|v.starts_with("text/event-stream"));
                    let mut pending=Vec::new(); let mut total=0usize; let mut event=String::new();
                    while let Some(chunk)=response.chunk().await.map_err(network_error)? {
                        total=total.saturating_add(chunk.len());
                        if total>MAX_BODY { return Err(ServiceError::Parse(String::new())); }
                        pending.extend_from_slice(&chunk);
                        if !sse { continue; }
                        while let Some(end)=pending.iter().position(|b|*b==b'\n') {
                            let line=pending.drain(..=end).collect::<Vec<_>>();
                            let line=std::str::from_utf8(&line).map_err(|_|ServiceError::Parse(String::new()))?.trim_end_matches(['\r','\n']);
                            if line.is_empty() {
                                if !event.is_empty() { if on_data(&event)? { return Ok(()); } event.clear(); }
                            } else if let Some(data)=line.strip_prefix("data:") {
                                if !event.is_empty() { event.push('\n'); }
                                event.push_str(data.strip_prefix(' ').unwrap_or(data));
                            }
                        }
                    }
                    if !sse { let body=String::from_utf8(pending).map_err(|_|ServiceError::Parse(String::new()))?; on_data(&body)?; return Ok(()); }
                    // A stream is complete only after its terminal frame, never at an unexpected EOF.
                    Err(ServiceError::Parse(String::new()))
                }) => result.unwrap_or(Err(ServiceError::Timeout)),
            }
        })
    }
    pub fn send_form(self, values: &[(&str, &str)]) -> Result<Response, ServiceError> {
        execute(self.0.form(values), self.1)
    }
}
impl Response {
    pub fn url(&self) -> &reqwest::Url {
        &self.url
    }
    pub fn all(&self, key: &str) -> Vec<&str> {
        self.headers
            .get_all(key)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect()
    }
    pub fn into_json<T: DeserializeOwned>(self) -> Result<T, ServiceError> {
        serde_json::from_slice(&self.body).map_err(|_| ServiceError::Parse(String::new()))
    }
    pub fn into_string(self) -> Result<String, ServiceError> {
        String::from_utf8(self.body).map_err(|_| ServiceError::Parse(String::new()))
    }
}
fn network_error(error: reqwest::Error) -> ServiceError {
    if error.is_timeout() {
        ServiceError::Timeout
    } else {
        ServiceError::Network(String::new())
    }
}
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("HTTP runtime")
    })
}
fn execute(request: reqwest::RequestBuilder, timeout: Duration) -> Result<Response, ServiceError> {
    let runtime = runtime();
    let token = TOKEN.with(|slot| slot.borrow().clone()).unwrap_or_default();
    runtime.block_on(async {
        tokio::select! {
            biased;
            _ = token.cancelled() => Err(ServiceError::Cancelled),
            result = tokio::time::timeout(timeout, async {
                let mut response = request.send().await.map_err(network_error)?;
                if !response.status().is_success() {
                    return Err(ServiceError::Http {
                        status: response.status().as_u16(),
                        retry_after_secs: response.headers().get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| crate::services::retry_after(v, std::time::SystemTime::now())),
                    });
                }
                if response.content_length().is_some_and(|n| n > MAX_BODY as u64) {
                    return Err(ServiceError::Parse("响应过大".into()));
                }
                let url = response.url().clone();
                let headers = response.headers().clone();
                let mut body = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(network_error)? {
                    if body.len() + chunk.len() > MAX_BODY { return Err(ServiceError::Parse("响应过大".into())); }
                    body.extend_from_slice(&chunk);
                }
                Ok(Response { url, headers, body })
            }) => result.unwrap_or(Err(ServiceError::Timeout)),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn response_retains_the_final_url_after_a_redirect() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for response in [
                "HTTP/1.1 302 Found\r\nLocation: /regional/translator\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 4\r\n\r\npage",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut buffer = [0; 4096];
                stream.read(&mut buffer).unwrap();
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        let response = get(&format!("http://{address}/translator")).call().unwrap();
        assert_eq!(
            response.url().as_str(),
            format!("http://{address}/regional/translator")
        );
        assert_eq!(response.into_string().unwrap(), "page");
        server.join().unwrap();
    }
    #[test]
    fn cancelled_request_finishes_before_provider_deadline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, started) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut buffer = [0; 4096];
            stream.read(&mut buffer).unwrap();
            ready.send(()).unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(3));
        });
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let worker = std::thread::spawn(move || {
            with_cancellation(worker_token, || get(&format!("http://{address}")).call())
        });
        started.recv_timeout(Duration::from_secs(3)).unwrap();
        let start = std::time::Instant::now();
        token.cancel();
        assert!(matches!(
            worker.join().unwrap(),
            Err(ServiceError::Cancelled)
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        release.send(()).unwrap();
        server.join().unwrap();
    }
    #[test]
    fn status_is_structured_without_response_secrets() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 4096];
            stream.read(&mut buffer).unwrap();
            stream.write_all(b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 123\r\nContent-Length: 6\r\n\r\nsecret").unwrap();
        });
        let error = get(&format!("http://{address}")).call().err().unwrap();
        assert_eq!(error.info().retry_after_secs, Some(123));
        assert!(!error.to_string().contains("secret"));
        server.join().unwrap();
    }
}

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
    pub fn send_form(self, values: &[(&str, &str)]) -> Result<Response, ServiceError> {
        execute(self.0.form(values), self.1)
    }
}
impl Response {
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
fn execute(request: reqwest::RequestBuilder, timeout: Duration) -> Result<Response, ServiceError> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("HTTP runtime")
    });
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
                let headers = response.headers().clone();
                let mut body = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(network_error)? {
                    if body.len() + chunk.len() > MAX_BODY { return Err(ServiceError::Parse("响应过大".into())); }
                    body.extend_from_slice(&chunk);
                }
                Ok(Response { headers, body })
            }) => result.unwrap_or(Err(ServiceError::Timeout)),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
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

//! Public errors contain classification, never a provider URL/body or credential.
use serde::{Deserialize, Serialize};
use std::{error::Error, io, time::SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    RateLimited,
    Timeout,
    Network,
    Unauthorized,
    Forbidden,
    Unavailable,
    InvalidRequest,
    InvalidResponse,
    Configuration,
    Unsupported,
    Cancelled,
    Internal,
}
impl ErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            Self::RateLimited => "该渠道暂时限流，请稍后再试",
            Self::Timeout => "渠道响应超时，已停止等待",
            Self::Network => "暂时无法连接该渠道",
            Self::Unauthorized => "服务凭据无效或已过期，请检查设置",
            Self::Forbidden => "服务拒绝访问，请检查账户或服务设置",
            Self::Unavailable => "渠道暂时不可用，请稍后再试",
            Self::InvalidRequest => "渠道未接受请求，可缩短文本或调整语向",
            Self::InvalidResponse => "暂时没有收到可用的译文",
            Self::Configuration => "请检查翻译服务配置",
            Self::Unsupported => "该渠道暂不支持此语向",
            Self::Cancelled => "已取消等待",
            Self::Internal => "这次请求未能完成，请重试",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureInfo {
    pub code: ErrorCode,
    pub http_status: Option<u16>,
    pub retry_after_secs: Option<u64>,
    pub retry_at_ms: Option<u64>,
    pub incident_id: Option<String>,
}
impl FailureInfo {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            http_status: None,
            retry_after_secs: None,
            retry_at_ms: None,
            incident_id: None,
        }
    }
}
#[derive(Debug)]
pub enum ServiceError {
    Configuration(String),
    Network(String),
    Parse(String),
    Unsupported(String),
    Http {
        status: u16,
        retry_after_secs: Option<u64>,
    },
    Timeout,
    Internal,
    Cancelled,
}
impl ServiceError {
    pub fn from_http(error: ureq::Error) -> Self {
        match error {
            ureq::Error::Status(status, response) => Self::Http {
                status,
                retry_after_secs: response
                    .header("Retry-After")
                    .and_then(|s| retry_after(s, SystemTime::now())),
            },
            ureq::Error::Transport(error) => {
                let mut source: Option<&(dyn Error + 'static)> = Some(&error);
                while let Some(e) = source {
                    if e.downcast_ref::<io::Error>()
                        .is_some_and(|e| e.kind() == io::ErrorKind::TimedOut)
                    {
                        return Self::Timeout;
                    }
                    source = e.source();
                }
                Self::Network(String::new())
            }
        }
    }
    pub fn from_body(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::TimedOut {
            Self::Timeout
        } else if matches!(
            error.kind(),
            io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::UnexpectedEof
        ) {
            Self::Network(String::new())
        } else {
            Self::Parse(String::new())
        }
    }
    pub fn info(&self) -> FailureInfo {
        let code = match self {
            Self::Http { status: 429, .. } => ErrorCode::RateLimited,
            Self::Http { status: 401, .. } => ErrorCode::Unauthorized,
            Self::Http {
                status: 403 | 456, ..
            } => ErrorCode::Forbidden,
            Self::Http {
                status: 408 | 504, ..
            }
            | Self::Timeout => ErrorCode::Timeout,
            Self::Http {
                status: 500..=599, ..
            } => ErrorCode::Unavailable,
            Self::Http { .. } => ErrorCode::InvalidRequest,
            Self::Network(_) => ErrorCode::Network,
            Self::Parse(_) => ErrorCode::InvalidResponse,
            Self::Configuration(_) => ErrorCode::Configuration,
            Self::Unsupported(_) => ErrorCode::Unsupported,
            Self::Internal => ErrorCode::Internal,
            Self::Cancelled => ErrorCode::Cancelled,
        };
        let mut info = FailureInfo::new(code);
        if let Self::Http {
            status,
            retry_after_secs,
        } = self
        {
            info.http_status = Some(*status);
            info.retry_after_secs = *retry_after_secs;
        }
        info
    }
}
impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.info().code.message())
    }
}
impl Error for ServiceError {}
pub(crate) fn retry_after(value: &str, now: SystemTime) -> Option<u64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds);
    }
    let date = httpdate::parse_http_date(value).ok()?;
    Some(
        date.duration_since(now)
            .map(|d| d.as_secs().saturating_add(u64::from(d.subsec_nanos() > 0)))
            .unwrap_or(0),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_and_retry_after_are_structured_without_body() {
        let response:ureq::Response="HTTP/1.1 429 Too Many Requests\r\nRetry-After: 120\r\n\r\nsecret-body https://host?q=private".parse().unwrap();
        let error = ServiceError::from_http(ureq::Error::Status(429, response));
        let info = error.info();
        assert_eq!(info.code, ErrorCode::RateLimited);
        assert_eq!(info.retry_after_secs, Some(120));
        assert!(!serde_json::to_string(&info).unwrap().contains("private"));
        assert!(!error.to_string().contains("secret"));
    }
    #[test]
    fn dates_and_invalid_retry_after() {
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        assert_eq!(
            retry_after(
                &httpdate::fmt_http_date(now + std::time::Duration::from_secs(45)),
                now
            ),
            Some(45)
        );
        assert_eq!(retry_after("-1", now), None);
        assert_eq!(retry_after("nonsense", now), None);
    }
    #[test]
    fn timeout_and_http_categories_are_distinct() {
        let timeout =
            ServiceError::from_http(io::Error::new(io::ErrorKind::TimedOut, "secret").into());
        assert_eq!(timeout.info().code, ErrorCode::Timeout);
        for (status, code) in [
            (401, ErrorCode::Unauthorized),
            (403, ErrorCode::Forbidden),
            (503, ErrorCode::Unavailable),
            (400, ErrorCode::InvalidRequest),
        ] {
            assert_eq!(
                ServiceError::Http {
                    status,
                    retry_after_secs: None
                }
                .info()
                .code,
                code
            );
        }
        for e in [
            ServiceError::Network("https://host?q=private".into()),
            ServiceError::Parse("Bearer private".into()),
        ] {
            assert!(!e.to_string().contains("private"));
        }
    }
}

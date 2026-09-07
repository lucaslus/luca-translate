//! Process-local provider cooldown and single-flight slots. No automatic retries.
use lucas_core::services::{ErrorCode, FailureInfo};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

#[derive(Default)]
pub struct Guard {
    states: Mutex<HashMap<&'static str, State>>,
    slots: Mutex<HashMap<&'static str, Arc<Semaphore>>>,
}
struct State {
    until: Instant,
    failures: u32,
    info: FailureInfo,
}
static GUARD: OnceLock<Guard> = OnceLock::new();
pub fn global() -> &'static Guard {
    GUARD.get_or_init(Guard::default)
}
impl Guard {
    pub fn slot(&self, service: &'static str) -> Arc<Semaphore> {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(service)
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    }
    pub fn blocked(&self, service: &str, now: Instant) -> Option<FailureInfo> {
        let states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let state = states.get(service)?;
        let remaining = state.until.checked_duration_since(now)?;
        if remaining.is_zero() {
            return None;
        }
        let mut info = state.info.clone();
        let secs = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
        info.retry_after_secs = Some(secs);
        info.retry_at_ms =
            Some(crate::diagnostics::now_ms().saturating_add(secs.saturating_mul(1000)));
        Some(info)
    }
    pub fn observe(
        &self,
        service: &'static str,
        info: Option<&mut FailureInfo>,
        now: Instant,
        jitter: u64,
    ) {
        let mut states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let Some(info) = info else {
            states.remove(service);
            return;
        };
        if !matches!(
            info.code,
            ErrorCode::RateLimited
                | ErrorCode::Unavailable
                | ErrorCode::Timeout
                | ErrorCode::Network
        ) {
            states.remove(service);
            return;
        }
        let failures = states
            .get(service)
            .map(|s| s.failures.saturating_add(1))
            .unwrap_or(1);
        let backoff = if info.code == ErrorCode::RateLimited {
            60u64
                .saturating_mul(1 << failures.saturating_sub(1).min(4))
                .min(900)
        } else {
            5u64.saturating_mul(1 << failures.saturating_sub(1).min(4))
                .min(60)
        };
        let mut seconds = info
            .retry_after_secs
            .unwrap_or(0)
            .max(backoff)
            .saturating_add(jitter.min(5));
        let until = now
            .checked_add(Duration::from_secs(seconds))
            .unwrap_or_else(|| {
                seconds = 86400;
                now + Duration::from_secs(seconds)
            });
        info.retry_after_secs = Some(seconds);
        info.retry_at_ms =
            Some(crate::diagnostics::now_ms().saturating_add(seconds.saturating_mul(1000)));
        states.insert(
            service,
            State {
                until,
                failures,
                info: info.clone(),
            },
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn throttling_is_per_provider_and_respects_retry_after() {
        let guard = Guard::default();
        let now = Instant::now();
        let mut info = FailureInfo::new(ErrorCode::RateLimited);
        info.retry_after_secs = Some(120);
        guard.observe("GoogleFree", Some(&mut info), now, 0);
        assert!(guard.blocked("Bing", now).is_none());
        assert_eq!(
            guard.blocked("GoogleFree", now).unwrap().retry_after_secs,
            Some(120)
        );
        assert!(guard
            .blocked("GoogleFree", now + Duration::from_secs(121))
            .is_none());
    }
    #[test]
    fn repeated_throttle_backs_off_and_success_resets() {
        let guard = Guard::default();
        let now = Instant::now();
        for expected in [60, 120, 240] {
            let mut info = FailureInfo::new(ErrorCode::RateLimited);
            guard.observe("GoogleFree", Some(&mut info), now, 0);
            assert_eq!(info.retry_after_secs, Some(expected));
        }
        guard.observe("GoogleFree", None, now, 0);
        assert!(guard.blocked("GoogleFree", now).is_none());
    }
    #[test]
    fn excessive_retry_after_keeps_reported_and_actual_delay_consistent() {
        let guard = Guard::default();
        let now = Instant::now();
        let mut info = FailureInfo::new(ErrorCode::RateLimited);
        info.retry_after_secs = Some(u64::MAX);
        guard.observe("GoogleFree", Some(&mut info), now, 5);
        assert_eq!(
            guard.blocked("GoogleFree", now).unwrap().retry_after_secs,
            info.retry_after_secs
        );
    }
    #[tokio::test]
    async fn only_one_request_per_provider_can_execute() {
        let guard = Guard::default();
        let _permit = guard.slot("Bing").acquire_owned().await.unwrap();
        assert!(guard.slot("Bing").try_acquire_owned().is_err());
        assert!(guard.slot("GoogleFree").try_acquire_owned().is_ok());
    }
    #[test]
    fn short_backoff_and_configuration_failures_do_not_permanently_lock_a_provider() {
        let guard = Guard::default();
        let now = Instant::now();
        for code in [
            ErrorCode::Network,
            ErrorCode::Timeout,
            ErrorCode::Unavailable,
        ] {
            let mut info = FailureInfo::new(code);
            guard.observe("Bing", Some(&mut info), now, 0);
            assert!(info.retry_after_secs.unwrap() <= 60);
            let mut configuration = FailureInfo::new(ErrorCode::Configuration);
            guard.observe("Bing", Some(&mut configuration), now, 0);
            assert!(guard.blocked("Bing", now).is_none());
        }
    }
}

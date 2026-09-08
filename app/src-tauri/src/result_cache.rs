//! Bounded in-memory LRU. Successful text only, no disk persistence or error caching.
use lucas_core::QueryResult;
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
#[derive(PartialEq, Eq)]
pub struct Key {
    pub identity: u64,
    pub text: String,
    pub from: String,
    pub to: String,
}
struct Entry {
    key: Key,
    result: QueryResult,
    created: Instant,
    bytes: usize,
}
#[derive(Default)]
pub struct Cache {
    entries: VecDeque<Entry>,
    bytes: usize,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
pub fn global() -> &'static Mutex<Cache> {
    CACHE.get_or_init(Default::default)
}
const MAX_BYTES: usize = 2 * 1024 * 1024;
impl Cache {
    pub fn get(&mut self, key: &Key, now: Instant) -> Option<QueryResult> {
        let position = self.entries.iter().position(|e| &e.key == key)?;
        let entry = self.entries.remove(position)?;
        if now.saturating_duration_since(entry.created) >= Duration::from_secs(300) {
            self.bytes -= entry.bytes;
            return None;
        }
        let result = entry.result.clone();
        self.entries.push_back(entry);
        Some(result)
    }
    pub fn insert(&mut self, key: Key, result: QueryResult, now: Instant) {
        if result.failure.is_some()
            || result.error.is_some()
            || !result.paragraphs.iter().any(|s| !s.trim().is_empty())
        {
            return;
        }
        let bytes = serde_json::to_vec(&result)
            .map(|v| v.len())
            .unwrap_or(MAX_BYTES)
            .saturating_add(key.text.len() + key.from.len() + key.to.len());
        if bytes > MAX_BYTES {
            return;
        }
        if let Some(i) = self.entries.iter().position(|e| e.key == key) {
            self.bytes -= self.entries.remove(i).unwrap().bytes;
        }
        while self.entries.len() >= 128 || self.bytes + bytes > MAX_BYTES {
            if let Some(old) = self.entries.pop_front() {
                self.bytes -= old.bytes;
            } else {
                break;
            }
        }
        self.bytes += bytes;
        self.entries.push_back(Entry {
            key,
            result,
            created: now,
            bytes,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(i: u64) -> Key {
        Key {
            identity: i,
            text: "hello".into(),
            from: "en".into(),
            to: "zh-Hans".into(),
        }
    }
    fn result() -> QueryResult {
        QueryResult {
            text: "hello".into(),
            detected_from: "en".into(),
            detected_to: "zh-Hans".into(),
            source_confirmed: true,
            paragraphs: vec!["你好".into()],
            dict: None,
            pinyin: None,
            service: "test".into(),
            error: None,
            failure: None,
        }
    }
    #[test]
    fn expires_bounds_and_partitions_results() {
        let mut cache = Cache::default();
        let now = Instant::now();
        cache.insert(key(1), result(), now);
        assert!(cache.get(&key(2), now).is_none());
        assert!(cache.get(&key(1), now).is_some());
        assert!(cache.get(&key(1), now + Duration::from_secs(301)).is_none());
        for i in 0..200 {
            cache.insert(key(i), result(), now);
        }
        assert_eq!(cache.entries.len(), 128);
        assert!(cache.bytes <= MAX_BYTES);
        let mut failed = result();
        failed.paragraphs.clear();
        cache.insert(key(999), failed, now);
        assert!(cache.get(&key(999), now).is_none());
        let mut failed = result();
        failed.failure = Some(lucas_core::services::FailureInfo::new(
            lucas_core::services::ErrorCode::Network,
        ));
        cache.insert(key(998), failed, now);
        assert!(cache.get(&key(998), now).is_none());
    }
}

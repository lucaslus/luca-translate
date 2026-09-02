//! SQLite 存储：历史记录与收藏夹。

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// 全局 DB 路径缓存（app_data_dir 在应用生命周期内不变）
static DB_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: i64,
    pub text: String,
    pub result: String,
    pub service: String,
    pub created_at: i64,
}

pub fn set_db_path(path: PathBuf) {
    *DB_PATH.lock().unwrap() = Some(path);
}

fn conn() -> Connection {
    let path = DB_PATH
        .lock()
        .unwrap()
        .clone()
        .expect("DB 路径未初始化");
    let c = Connection::open(path).expect("打开数据库失败");
    c.pragma_update(None, "journal_mode", "WAL").ok();
    c
}

fn init() {
    let c = conn();
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            result TEXT NOT NULL,
            service TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS favorites (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            result TEXT NOT NULL,
            service TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_history_created ON history(created_at DESC);",
    )
    .expect("建表失败");
}

pub fn add_history(text: &str, result: &str, service: &str) {
    init();
    let _ = conn().execute(
        "INSERT INTO history (text, result, service, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![text, result, service, now()],
    );
}

pub fn list_history(limit: i64) -> Vec<HistoryItem> {
    init();
    let c = conn();
    let mut stmt = c
        .prepare("SELECT id, text, result, service, created_at FROM history ORDER BY created_at DESC, id DESC LIMIT ?1")
        .expect("查询失败");
    let rows = stmt
        .query_map([limit], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                text: row.get(1)?,
                result: row.get(2)?,
                service: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .expect("查询失败");
    rows.filter_map(|r| r.ok()).collect()
}

pub fn clear_history() {
    init();
    let _ = conn().execute("DELETE FROM history", []);
}

pub fn add_favorite(text: &str, result: &str, service: &str) {
    init();
    let _ = conn().execute(
        "INSERT INTO favorites (text, result, service, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![text, result, service, now()],
    );
}

pub fn list_favorites() -> Vec<HistoryItem> {
    init();
    let c = conn();
    let mut stmt = c
        .prepare("SELECT id, text, result, service, created_at FROM favorites ORDER BY created_at DESC, id DESC")
        .expect("查询失败");
    let rows = stmt
        .query_map([], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                text: row.get(1)?,
                result: row.get(2)?,
                service: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .expect("查询失败");
    rows.filter_map(|r| r.ok()).collect()
}

pub fn remove_favorite(id: i64) {
    init();
    let _ = conn().execute("DELETE FROM favorites WHERE id = ?1", [id]);
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

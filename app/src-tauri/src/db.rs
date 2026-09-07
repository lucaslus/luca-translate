//! One cached SQLite connection; storage failures are values, never panics.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Duration,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: i64,
    pub text: String,
    pub result: String,
    pub service: String,
    pub created_at: i64,
}
struct Store {
    path: PathBuf,
    connection: Option<Connection>,
}
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
pub fn set_db_path(path: PathBuf) {
    let _ = STORE.set(Mutex::new(Store {
        path,
        connection: None,
    }));
}
fn initialize(c: &Connection) -> rusqlite::Result<()> {
    c.busy_timeout(Duration::from_secs(2))?;
    c.pragma_update(None, "journal_mode", "WAL")?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
        id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL, result TEXT NOT NULL,
        service TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS favorites (
        id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL, result TEXT NOT NULL,
        service TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL);
        CREATE INDEX IF NOT EXISTS idx_history_created ON history(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_favorites_created ON favorites(created_at DESC);",
    )
}
fn with_conn<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
    let mut s = STORE
        .get()
        .ok_or("存储尚未初始化")?
        .lock()
        .map_err(|_| "存储服务不可用")?;
    if s.connection.is_none() {
        let c = Connection::open(&s.path)
            .map_err(|_| "无法打开历史数据库，请检查目录权限或可用空间")?;
        initialize(&c).map_err(|_| "历史数据库初始化失败，原数据已保留")?;
        s.connection = Some(c);
    }
    f(s.connection.as_ref().ok_or("数据库不可用")?)
        .map_err(|_| "数据操作失败，请检查磁盘空间和数据库权限后重试".into())
}
pub fn add_history(text: &str, result: &str, service: &str) -> Result<(), String> {
    with_conn(|c| {
        c.execute(
            "INSERT INTO history(text,result,service,created_at) VALUES(?1,?2,?3,?4)",
            params![text, result, service, now()],
        )?;
        Ok(())
    })
}
fn list(
    c: &Connection,
    favorite: bool,
    limit: i64,
    offset: i64,
) -> rusqlite::Result<Vec<HistoryItem>> {
    let sql = if favorite {
        "SELECT id,text,result,service,created_at FROM favorites ORDER BY created_at DESC,id DESC LIMIT ?1 OFFSET ?2"
    } else {
        "SELECT id,text,result,service,created_at FROM history ORDER BY created_at DESC,id DESC LIMIT ?1 OFFSET ?2"
    };
    let mut stmt = c.prepare_cached(sql)?;
    let rows = stmt.query_map(params![limit.clamp(1, 100), offset.max(0)], |r| {
        Ok(HistoryItem {
            id: r.get(0)?,
            text: r.get(1)?,
            result: r.get(2)?,
            service: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    rows.collect()
}
pub fn list_history(limit: i64, offset: i64) -> Result<Vec<HistoryItem>, String> {
    with_conn(|c| list(c, false, limit, offset))
}
pub fn list_favorites(limit: i64, offset: i64) -> Result<Vec<HistoryItem>, String> {
    with_conn(|c| list(c, true, limit, offset))
}
pub fn clear_history() -> Result<(), String> {
    with_conn(|c| {
        c.execute("DELETE FROM history", [])?;
        Ok(())
    })
}
pub fn add_favorite(text: &str, result: &str, service: &str) -> Result<(), String> {
    with_conn(|c| {
        c.execute("INSERT INTO favorites(text,result,service,created_at)
        SELECT ?1,?2,?3,?4 WHERE NOT EXISTS(SELECT 1 FROM favorites WHERE text=?1 AND result=?2 AND service=?3)",
        params![text,result,service,now()])?;
        Ok(())
    })
}
pub fn remove_favorite(id: i64) -> Result<(), String> {
    with_conn(|c| {
        c.execute("DELETE FROM favorites WHERE id=?1", [id])?;
        Ok(())
    })
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paging_and_quotes_roundtrip() {
        let c = Connection::open_in_memory().unwrap();
        initialize(&c).unwrap();
        let text = "a\"b'\n<script>not HTML</script>";
        for _ in 0..3 {
            c.execute(
                "INSERT INTO history(text,result,created_at) VALUES(?1,?1,0)",
                [text],
            )
            .unwrap();
        }
        let rows = list(&c, false, 1, 1).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, text);
        assert_eq!(rows[0].id, 2);
    }
    #[test]
    fn malformed_schema_returns_error() {
        let c = Connection::open_in_memory().unwrap();
        c.execute("CREATE TABLE history(id INTEGER)", []).unwrap();
        assert!(list(&c, false, 10, 0).is_err());
    }
}

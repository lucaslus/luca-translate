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
    #[serde(default)]
    pub details: Vec<lucas_core::QueryResult>,
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
    )?;
    let mut columns = c.prepare("PRAGMA table_info(history)")?;
    let names = columns
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !names.iter().any(|name| name == "details") {
        c.execute(
            "ALTER TABLE history ADD COLUMN details TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }
    Ok(())
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
fn list(
    c: &Connection,
    favorite: bool,
    limit: i64,
    offset: i64,
) -> rusqlite::Result<Vec<HistoryItem>> {
    let sql = if favorite {
        "SELECT id,text,result,service,created_at,'[]' FROM favorites ORDER BY created_at DESC,id DESC LIMIT ?1 OFFSET ?2"
    } else {
        "SELECT id,text,result,service,created_at,details FROM history ORDER BY created_at DESC,id DESC LIMIT ?1 OFFSET ?2"
    };
    let mut stmt = c.prepare_cached(sql)?;
    let rows = stmt.query_map(params![limit.clamp(1, 100), offset.max(0)], |r| {
        Ok(HistoryItem {
            id: r.get(0)?,
            text: r.get(1)?,
            result: r.get(2)?,
            service: r.get(3)?,
            created_at: r.get(4)?,
            details: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
        })
    })?;
    rows.collect()
}
pub fn search_history(limit: i64, offset: i64, search: &str) -> Result<Vec<HistoryItem>, String> {
    if search.chars().count() > 200 {
        return Err("Search is too long".into());
    }
    with_conn(|c| {
        let mut statement = c.prepare_cached("SELECT id,text,result,service,created_at,details FROM history WHERE ?1='' OR instr(lower(text),lower(?1))>0 OR instr(lower(details),lower(?1))>0 OR instr(lower(result),lower(?1))>0 ORDER BY created_at DESC,id DESC LIMIT ?2 OFFSET ?3")?;
        let rows = statement
            .query_map(params![search, limit.clamp(1, 100), offset.max(0)], |r| {
                Ok(HistoryItem {
                    id: r.get(0)?,
                    text: r.get(1)?,
                    result: r.get(2)?,
                    service: r.get(3)?,
                    created_at: r.get(4)?,
                    details: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
                })
            })?
            .collect();
        rows
    })
}
pub fn add_query(text: &str, results: &[lucas_core::QueryResult]) -> Result<(), String> {
    let Some(first) = results
        .iter()
        .find(|r| r.error.is_none() && !r.paragraphs.is_empty())
    else {
        return Ok(());
    };
    let details = serde_json::to_string(results).map_err(|_| "Cannot save query")?;
    with_conn(|c| {
        c.execute(
            "INSERT INTO history(text,result,service,created_at,details) VALUES(?1,?2,?3,?4,?5)",
            params![
                text,
                first.paragraphs.join("\n"),
                first.service,
                now(),
                details
            ],
        )?;
        Ok(())
    })
}
pub fn prune_history(days: u32, limit: u32) -> Result<(), String> {
    with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        if days > 0 {
            tx.execute(
                "DELETE FROM history WHERE created_at < ?1",
                [now().saturating_sub(i64::from(days) * 86400)],
            )?;
        }
        if limit > 0 {
            tx.execute("DELETE FROM history WHERE id IN (SELECT id FROM history ORDER BY created_at DESC,id DESC LIMIT -1 OFFSET ?1)",[limit])?;
        }
        tx.commit()
    })
}
fn find_favorite(
    c: &Connection,
    text: &str,
    result: &str,
    service: &str,
) -> rusqlite::Result<Option<i64>> {
    use rusqlite::OptionalExtension;
    c.query_row(
        "SELECT id FROM favorites WHERE text=?1 AND result=?2 AND service=?3 LIMIT 1",
        params![text, result, service],
        |r| r.get(0),
    )
    .optional()
}
pub fn favorite_id(text: &str, result: &str, service: &str) -> Result<Option<i64>, String> {
    with_conn(|c| find_favorite(c, text, result, service))
}
pub fn toggle_favorite(text: &str, result: &str, service: &str) -> Result<Option<i64>, String> {
    with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        let id = if let Some(id) = find_favorite(&tx, text, result, service)? {
            tx.execute("DELETE FROM favorites WHERE id=?1", [id])?;
            None
        } else {
            tx.execute(
                "INSERT INTO favorites(text,result,service,created_at) VALUES(?1,?2,?3,?4)",
                params![text, result, service, now()],
            )?;
            Some(tx.last_insert_rowid())
        };
        tx.commit()?;
        Ok(id)
    })
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
    fn migration_preserves_legacy_records_and_reads_full_query_results() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE history(id INTEGER PRIMARY KEY, text TEXT NOT NULL, result TEXT NOT NULL, service TEXT NOT NULL, created_at INTEGER NOT NULL); INSERT INTO history VALUES(1,'legacy','kept','Bing',1)").unwrap();
        initialize(&c).unwrap();
        initialize(&c).unwrap();
        let old = list(&c, false, 10, 0).unwrap();
        assert_eq!(old[0].result, "kept");
        assert!(old[0].details.is_empty());
        let details = serde_json::json!([
            {"service":"AI","text":"hello","paragraphs":["你好"],"detected_from":"en","detected_to":"zh-Hans","dict":null,"pinyin":null},
            {"service":"Bing","text":"hello","paragraphs":["您好"],"detected_from":"en","detected_to":"zh-Hans","dict":null,"pinyin":null}
        ]).to_string();
        c.execute(
            "INSERT INTO history VALUES(2,'hello','你好','AI',2,?1)",
            [details],
        )
        .unwrap();
        let new = list(&c, false, 10, 0).unwrap();
        assert_eq!(new[0].details.len(), 2);
        assert_eq!(new[0].details[1].paragraphs, ["您好"]);
        assert_eq!(new[1].text, "legacy");
    }
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

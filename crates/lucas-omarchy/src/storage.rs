use rusqlite::{Connection, DatabaseName, OpenFlags};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn xdg(name: &str, fallback: &str) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Ok(path);
    }
    let home = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
    Ok(PathBuf::from(home).join(fallback))
}

pub fn initialize() -> Result<(), String> {
    let data = xdg("XDG_DATA_HOME", ".local/share")?;
    let native = data.join("lucas-translate-omarchy");
    migrate(&data.join("com.lucas.translate"), &native)?;
    crate::config::set_config_dir(native.clone());
    crate::db::set_db_path(native.join("lucas.db"));
    crate::diagnostics::init(Some(
        xdg("XDG_STATE_HOME", ".local/state")?.join("lucas-translate-omarchy/logs"),
    ));
    Ok(())
}

/// Copy once; a SQLite snapshot includes committed WAL data from a running app.
pub fn migrate(legacy: &Path, native: &Path) -> Result<(), String> {
    std::fs::create_dir_all(native).map_err(|_| "Cannot create native data directory")?;
    let config = native.join("config.json");
    if !config.exists() && legacy.join("config.json").is_file() {
        let bytes = std::fs::read(legacy.join("config.json"))
            .map_err(|_| "Cannot read existing settings")?;
        let mut value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "Existing settings are invalid; original preserved")?;
        let object = value
            .as_object_mut()
            .ok_or("Existing settings must be an object")?;
        object.insert("preferences".into(), serde_json::json!({"language":"auto"}));
        for key in ["theme", "dark_mode", "light_mode"] {
            object.remove(key);
        }
        let mut file =
            tempfile::NamedTempFile::new_in(native).map_err(|_| "Cannot migrate settings")?;
        serde_json::to_writer_pretty(&mut file, &value).map_err(|_| "Cannot migrate settings")?;
        file.persist_noclobber(config)
            .map_err(|_| "Cannot migrate settings")?;
    }
    let database = native.join("lucas.db");
    if !database.exists() && legacy.join("lucas.db").is_file() {
        let source =
            Connection::open_with_flags(legacy.join("lucas.db"), OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|_| "Cannot read existing history")?;
        let temporary =
            tempfile::NamedTempFile::new_in(native).map_err(|_| "Cannot migrate history")?;
        source
            .backup(DatabaseName::Main, temporary.path(), None)
            .map_err(|_| "Cannot snapshot existing history")?;
        temporary
            .persist_noclobber(database)
            .map_err(|_| "Cannot migrate history")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_legacy_and_includes_wal_without_theme() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let new = dir.path().join("new");
        std::fs::create_dir(&old).unwrap();
        let original =
            r#"{"preferences":{"font_size":16,"shortcuts":{}},"theme":"dark","model":"local"}"#;
        std::fs::write(old.join("config.json"), original).unwrap();
        let c = Connection::open(old.join("lucas.db")).unwrap();
        c.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE sample(value TEXT); INSERT INTO sample VALUES('retained');").unwrap();
        migrate(&old, &new).unwrap();
        let native: Value =
            serde_json::from_slice(&std::fs::read(new.join("config.json")).unwrap()).unwrap();
        assert!(native.get("theme").is_none());
        assert_eq!(native["model"], "local");
        assert_eq!(
            native["preferences"],
            serde_json::json!({"language":"auto"})
        );
        assert_eq!(
            std::fs::read_to_string(old.join("config.json")).unwrap(),
            original
        );
        let d = Connection::open(new.join("lucas.db")).unwrap();
        assert_eq!(
            d.query_row("SELECT value FROM sample", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "retained"
        );
        std::fs::write(new.join("config.json"), "native-user-settings").unwrap();
        migrate(&old, &new).unwrap();
        assert_eq!(
            std::fs::read_to_string(new.join("config.json")).unwrap(),
            "native-user-settings"
        );
    }
}

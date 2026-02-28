use anyhow::Result;
use rusqlite::{Connection, params};
use std::sync::Mutex;

pub struct AuditLog {
    pub(crate) conn: Mutex<Connection>,
}

impl AuditLog {
    pub fn open(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL DEFAULT (datetime('now')),
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL
            );"
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn log(&self, event_type: &str, payload: &str) -> Result<()> {
        let conn = self.conn.lock().expect("audit mutex poisoned");
        conn.execute(
            "INSERT INTO events (event_type, payload) VALUES (?1, ?2)",
            params![event_type, payload],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_creates_and_inserts() {
        let log = AuditLog::open(":memory:").unwrap();
        log.log("analyze_diff", r#"{"files":["foo.ts"]}"#).unwrap();
        let count: i64 = log.conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

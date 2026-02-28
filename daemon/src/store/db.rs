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

    fn count_rows(log: &AuditLog) -> i64 {
        log.conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn test_audit_log_creates_and_inserts() {
        let log = AuditLog::open(":memory:").unwrap();
        log.log("analyze_diff", r#"{"files":["foo.ts"]}"#).unwrap();
        assert_eq!(count_rows(&log), 1);
    }

    #[test]
    fn test_audit_log_multiple_entries_accumulate() {
        let log = AuditLog::open(":memory:").unwrap();
        log.log("analyze_diff", "payload1").unwrap();
        log.log("analyze_diff", "payload2").unwrap();
        log.log("ping", "{}").unwrap();
        assert_eq!(count_rows(&log), 3);
    }

    #[test]
    fn test_audit_log_stores_correct_event_type_and_payload() {
        let log = AuditLog::open(":memory:").unwrap();
        log.log("my_event", "my_payload").unwrap();
        let (event_type, payload): (String, String) = log.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT event_type, payload FROM events LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(event_type, "my_event");
        assert_eq!(payload, "my_payload");
    }

    #[test]
    fn test_audit_log_entries_have_autoincrement_ids() {
        let log = AuditLog::open(":memory:").unwrap();
        log.log("e1", "p1").unwrap();
        log.log("e2", "p2").unwrap();
        let ids: Vec<i64> = {
            let conn = log.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT id FROM events ORDER BY id").unwrap();
            stmt.query_map([], |r| r.get(0)).unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn test_audit_log_two_in_memory_dbs_are_isolated() {
        // Two separate :memory: connections should not share data.
        let log1 = AuditLog::open(":memory:").unwrap();
        let log2 = AuditLog::open(":memory:").unwrap();
        log1.log("event", "payload").unwrap();
        assert_eq!(count_rows(&log1), 1);
        assert_eq!(count_rows(&log2), 0);
    }
}

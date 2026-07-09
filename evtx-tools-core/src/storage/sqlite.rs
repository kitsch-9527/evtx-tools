use rusqlite::Connection;

use crate::{ParsedEvent, Result};

// ---------------------------------------------------------------------------
// DDL constants
// ---------------------------------------------------------------------------

const CREATE_EVENTS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS events (
    event_record_id     INTEGER NOT NULL,
    event_time          TEXT,
    provider_name       TEXT,
    provider_guid       TEXT,
    event_id            INTEGER,
    level               INTEGER,
    severity            TEXT,
    task_category       TEXT,
    opcode              TEXT,
    keywords            TEXT,
    channel             TEXT,
    computer            TEXT,
    process_id          INTEGER,
    thread_id           INTEGER,
    user_name           TEXT,
    user_sid            TEXT,
    activity_id         TEXT,
    related_activity_id TEXT,
    message             TEXT,
    level_name          TEXT,
    keywords_name       TEXT,
    event_data          TEXT,
    raw_xml             TEXT,
    file_name           TEXT,
    sequence_number     INTEGER
);
";

/// Bulk-insert PRAGMAs — applied once on open, not reverted.
///
/// - `page_size = 65536`: 64 KB pages reduce B‑tree depth and I/O for large DBs.
///   Must be set before creating the table; empty-DB-only.
/// - `synchronous = OFF`: no fsync on commit — safe because this DB is disposable.
/// - `journal_mode = MEMORY`: no WAL/index writing during bulk insert.
/// - `cache_size = -64000`: 64 MB page cache (negative = KB units).
/// - `temp_store = MEMORY`: temp tables (for sorting) stay in RAM.
/// - `mmap_size = 268435456`: 256 MB memory-map for reads.
const BULK_INSERT_PRAGMAS: &[&str] = &[
    "PRAGMA page_size = 65536",
    "PRAGMA synchronous = OFF",
    "PRAGMA journal_mode = OFF",
    "PRAGMA cache_size = -64000",
    "PRAGMA temp_store = MEMORY",
    "PRAGMA mmap_size = 268435456",
    "PRAGMA threads = 12",
];

/// Indexes — created AFTER all data is inserted to avoid per-row index maintenance.
const CREATE_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_events_event_time ON events(event_time);",
    "CREATE INDEX IF NOT EXISTS idx_events_level ON events(level);",
    "CREATE INDEX IF NOT EXISTS idx_events_channel ON events(channel);",
    "CREATE INDEX IF NOT EXISTS idx_events_provider ON events(provider_name);",
    "CREATE INDEX IF NOT EXISTS idx_events_event_id ON events(event_id);",
    "CREATE INDEX IF NOT EXISTS idx_events_seq ON events(sequence_number);",
];

/// Batch size for INSERT statements.
///
/// Callers can use this constant when splitting a large `Vec<ParsedEvent>`
/// into chunks for successive `insert_events` calls.
pub const BATCH_SIZE: usize = 20_000;

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Create the `events` table if it does not already exist.
///
/// Indexes are deliberately NOT created here — they are built in
/// [`finalize`](EventsDb::finalize) after all data has been inserted,
/// avoiding per-row index maintenance overhead during bulk load.
pub fn create_events_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(CREATE_EVENTS_TABLE)
        .map_err(|e| format!("failed to create events table: {e}"))
}

/// Insert a batch of `ParsedEvent` rows into the `events` table.
///
/// Uses multi-row `INSERT INTO ... VALUES (...), (...), ...` with inline SQL
/// (not parameterised), packed in sub-batches of 500 rows to keep SQL text
/// manageable. This is significantly faster than per-row prepared statements
/// because SQLite executes the entire set as a single VDBE program.
pub fn insert_events(conn: &Connection, batch: &[ParsedEvent]) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    conn.execute_batch("BEGIN")
        .map_err(|e| format!("failed to begin transaction: {e}"))?;

    // Multi-row INSERT with 2000-row chunks (~350 KB per SQL statement).
    // Larger chunks = fewer SQL compilations + less string building overhead.
    for chunk in batch.chunks(2000) {
        let mut sql = String::with_capacity(chunk.len() * 180);
        sql.push_str("INSERT INTO events VALUES ");

        for (i, event) in chunk.iter().enumerate() {
            if i > 0 {
                sql.push(',');
            }
            sql.push('(');
            append_int_opt(&mut sql, &event.event_record_id);
            sql.push(',');
            append_text_opt(&mut sql, &event.event_time);
            sql.push(',');
            append_text_opt(&mut sql, &event.provider_name);
            sql.push(',');
            append_text_opt(&mut sql, &event.provider_guid);
            sql.push(',');
            append_int_opt(&mut sql, &event.event_id);
            sql.push(',');
            append_int_opt(&mut sql, &event.level);
            sql.push(',');
            append_text_opt(&mut sql, &event.severity);
            sql.push(',');
            append_text_opt(&mut sql, &event.task_category);
            sql.push(',');
            append_text_opt(&mut sql, &event.opcode);
            sql.push(',');
            append_text_opt(&mut sql, &event.keywords);
            sql.push(',');
            append_text_opt(&mut sql, &event.channel);
            sql.push(',');
            append_text_opt(&mut sql, &event.computer);
            sql.push(',');
            append_int_opt(&mut sql, &event.process_id);
            sql.push(',');
            append_int_opt(&mut sql, &event.thread_id);
            sql.push(',');
            append_text_opt(&mut sql, &event.user_name);
            sql.push(',');
            append_text_opt(&mut sql, &event.user_sid);
            sql.push(',');
            append_text_opt(&mut sql, &event.activity_id);
            sql.push(',');
            append_text_opt(&mut sql, &event.related_activity_id);
            sql.push(',');
            append_text_opt(&mut sql, &event.message);
            sql.push(',');
            append_text_opt(&mut sql, &event.level_name);
            sql.push(',');
            append_text_opt(&mut sql, &event.keywords_name);
            sql.push(',');
            append_text_opt(&mut sql, &event.event_data);
            sql.push(',');
            append_text_opt(&mut sql, &event.raw_xml);
            sql.push(',');
            append_text_opt(&mut sql, &event.file_name);
            sql.push(',');
            append_int_opt(&mut sql, &event.sequence_number);
            sql.push(')');
        }

        conn.execute_batch(&sql)
            .map_err(|e| format!("failed to insert chunk: {e}"))?;
    }

    conn.execute_batch("COMMIT")
        .map_err(|e| format!("failed to commit transaction: {e}"))?;

    Ok(())
}

/// Append an optional integer as a SQL literal.
fn append_int_opt<T: std::fmt::Display>(sql: &mut String, v: &Option<T>) {
    match v {
        Some(val) => {
            use std::fmt::Write;
            write!(sql, "{val}").unwrap();
        }
        None => sql.push_str("NULL"),
    }
}

/// Append an optional text value as a SQL string literal.
///
/// Single quotes inside the text are escaped by doubling (`''`).
/// Uses `str::replace` for the (rare) quoting case; the common (no-quotes)
/// path is a single `push_str`.
fn append_text_opt(sql: &mut String, v: &Option<String>) {
    match v {
        Some(s) => {
            sql.push('\'');
            if s.contains('\'') {
                sql.push_str(&s.replace('\'', "''"));
            } else {
                sql.push_str(s);
            }
            sql.push('\'');
        }
        None => sql.push_str("NULL"),
    }
}

/// Run `ANALYZE` to update SQLite query planner statistics.
pub fn analyze(conn: &Connection) -> Result<()> {
    conn.execute_batch("ANALYZE")
        .map_err(|e| format!("failed to analyze: {e}"))
}

// ---------------------------------------------------------------------------
// EventsDb wrapper
// ---------------------------------------------------------------------------

/// A convenience wrapper around a SQLite connection for the `events` table.
///
/// Optimised for bulk inserts: indexes are created *after* data loading via
/// [`finalize`](EventsDb::finalize).
pub struct EventsDb {
    conn: Connection,
}

impl EventsDb {
    /// Open (or create) a SQLite database at the given path.
    ///
    /// Applies bulk-insert PRAGMAs that trade crash-safety for write speed.
    /// This is acceptable because `EventsDb` is write-once — data is restored
    /// by re-running the parse if needed.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| format!("failed to open db: {e}"))?;
        for pragma in BULK_INSERT_PRAGMAS {
            conn.execute_batch(pragma)
                .map_err(|e| format!("failed to set pragma '{pragma}': {e}"))?;
        }
        Ok(EventsDb { conn })
    }

    /// Create the `events` table (but *not* indexes — see [`finalize`]).
    pub fn create_table(&self) -> Result<()> {
        create_events_table(&self.conn)
    }

    /// Insert a batch of events into the `events` table.
    pub fn insert(&self, events: &[ParsedEvent]) -> Result<()> {
        insert_events(&self.conn, events)
    }

    /// Create indexes and run `ANALYZE`. Must be called once after all data is
    /// inserted. After this, the database is ready for querying.
    pub fn finalize(&self) -> Result<()> {
        for stmt in CREATE_INDEXES {
            self.conn
                .execute_batch(stmt)
                .map_err(|e| format!("failed to create index: {e}"))?;
        }
        self.conn
            .execute_batch("ANALYZE")
            .map_err(|e| format!("failed to analyze: {e}"))
    }

    /// Return a reference to the inner `Connection`.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

// ---------------------------------------------------------------------------
// Scenario table operations
// ---------------------------------------------------------------------------

/// Ensure a scenario table exists, creating it lazily if needed.
///
/// Uses the same schema as `events` but with the rule's table name.
/// Idempotent — safe to call multiple times.
pub fn ensure_scenario_table(conn: &Connection, rule: &crate::routing::Rule) -> Result<()> {
    let ddl = format!(
        "CREATE TABLE IF NOT EXISTS {} AS SELECT * FROM events WHERE 1=0",
        rule.table
    );
    conn.execute_batch(&ddl)
        .map_err(|e| format!("failed to create scenario table '{}': {e}", rule.table))
}

/// Insert a single event into a scenario table.
///
/// Always inserts the full event row (all columns). Column filtering
/// (via `rule.columns`) is applied at query time, not storage time.
pub fn insert_scenario_event(conn: &Connection, rule: &crate::routing::Rule, event: &ParsedEvent) -> Result<()> {
    // Build an INSERT with the same column order as the events table
    let mut sql = String::new();
    sql.push_str(&format!("INSERT INTO {} VALUES (", rule.table));
    append_int_opt(&mut sql, &event.event_record_id);
    sql.push(',');
    append_text_opt(&mut sql, &event.event_time);
    sql.push(',');
    append_text_opt(&mut sql, &event.provider_name);
    sql.push(',');
    append_text_opt(&mut sql, &event.provider_guid);
    sql.push(',');
    append_int_opt(&mut sql, &event.event_id);
    sql.push(',');
    append_int_opt(&mut sql, &event.level);
    sql.push(',');
    append_text_opt(&mut sql, &event.severity);
    sql.push(',');
    append_text_opt(&mut sql, &event.task_category);
    sql.push(',');
    append_text_opt(&mut sql, &event.opcode);
    sql.push(',');
    append_text_opt(&mut sql, &event.keywords);
    sql.push(',');
    append_text_opt(&mut sql, &event.channel);
    sql.push(',');
    append_text_opt(&mut sql, &event.computer);
    sql.push(',');
    append_int_opt(&mut sql, &event.process_id);
    sql.push(',');
    append_int_opt(&mut sql, &event.thread_id);
    sql.push(',');
    append_text_opt(&mut sql, &event.user_name);
    sql.push(',');
    append_text_opt(&mut sql, &event.user_sid);
    sql.push(',');
    append_text_opt(&mut sql, &event.activity_id);
    sql.push(',');
    append_text_opt(&mut sql, &event.related_activity_id);
    sql.push(',');
    append_text_opt(&mut sql, &event.message);
    sql.push(',');
    append_text_opt(&mut sql, &event.level_name);
    sql.push(',');
    append_text_opt(&mut sql, &event.keywords_name);
    sql.push(',');
    append_text_opt(&mut sql, &event.event_data);
    sql.push(',');
    append_text_opt(&mut sql, &event.raw_xml);
    sql.push(',');
    append_text_opt(&mut sql, &event.file_name);
    sql.push(',');
    append_int_opt(&mut sql, &event.sequence_number);
    sql.push(')');

    conn.execute_batch(&sql)
        .map_err(|e| format!("failed to insert into '{}': {e}", rule.table))
}

#[cfg(test)]
mod scenario_tests {
    use super::*;
    use crate::routing::{Rule, RuleFilter};

    #[test]
    fn test_ensure_scenario_table_creates() {
        let conn = Connection::open_in_memory().unwrap();
        super::create_events_table(&conn).unwrap();

        let rule = Rule {
            name: "test".into(),
            table: "my_scenario".into(),
            columns: None,
            filter: RuleFilter {
                event_ids: Some(vec![4624]),
                channels: None,
                provider_like: None,
                message_like: None,
            },
        };

        ensure_scenario_table(&conn, &rule).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='my_scenario'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_ensure_scenario_table_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        super::create_events_table(&conn).unwrap();

        let rule = Rule {
            name: "test".into(),
            table: "my_scenario".into(),
            columns: None,
            filter: RuleFilter {
                event_ids: Some(vec![4624]),
                channels: None,
                provider_like: None,
                message_like: None,
            },
        };

        ensure_scenario_table(&conn, &rule).unwrap();
        ensure_scenario_table(&conn, &rule).unwrap(); // Should not error

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='my_scenario'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_insert_scenario_event() {
        let conn = Connection::open_in_memory().unwrap();
        super::create_events_table(&conn).unwrap();

        let rule = Rule {
            name: "test".into(),
            table: "my_scenario".into(),
            columns: None,
            filter: RuleFilter {
                event_ids: Some(vec![4624]),
                channels: None,
                provider_like: None,
                message_like: None,
            },
        };

        ensure_scenario_table(&conn, &rule).unwrap();

        let event = ParsedEvent {
            event_record_id: Some(42),
            event_id: Some(4624),
            event_time: Some("2024-01-15T10:30:00Z".into()),
            provider_name: Some("Test".into()),
            channel: Some("Security".into()),
            computer: Some("PC-1".into()),
            message: Some("登录成功".into()),
            file_name: Some("test.evtx".into()),
            sequence_number: Some(0),
            // Everything else None
            provider_guid: None,
            qualifiers: None,
            level: None,
            severity: None,
            task_category: None,
            opcode: None,
            keywords: None,
            process_id: None,
            thread_id: None,
            user_name: None,
            user_sid: None,
            activity_id: None,
            related_activity_id: None,
            level_name: None,
            keywords_name: None,
            event_data: None,
            raw_xml: None,
        };

        insert_scenario_event(&conn, &rule, &event).unwrap();

        let (eid, msg): (i64, String) = conn
            .query_row(
                "SELECT event_id, message FROM my_scenario WHERE event_record_id = 42",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(eid, 4624);
        assert_eq!(msg, "登录成功");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_event(event_record_id: u64, provider: &str) -> ParsedEvent {
        ParsedEvent {
            event_record_id: Some(event_record_id),
            event_time: Some("2024-01-01T00:00:00Z".into()),
            provider_name: Some(provider.into()),
            provider_guid: None,
            event_id: Some(4624),
            qualifiers: None,
            level: Some(0),
            severity: Some("信息".into()),
            task_category: None,
            opcode: None,
            keywords: None,
            channel: Some("Security".into()),
            computer: Some("PC-1".into()),
            process_id: Some(4),
            thread_id: Some(123),
            user_name: None,
            user_sid: None,
            activity_id: None,
            related_activity_id: None,
            message: None,
            level_name: None,
            keywords_name: None,
            event_data: None,
            raw_xml: None,
            sequence_number: None,
            file_name: Some("test.evtx".into()),
        }
    }

    #[test]
    fn test_create_events_table_creates_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        create_events_table(&conn).unwrap();

        // Verify table exists by querying it
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Creating again should be idempotent (no error)
        create_events_table(&conn).unwrap();
    }

    #[test]
    fn test_insert_and_query_single_event() {
        let conn = Connection::open_in_memory().unwrap();
        create_events_table(&conn).unwrap();

        let event = make_test_event(1, "Test-Provider");
        insert_events(&conn, &[event]).unwrap();

        let (event_id, provider): (i64, String) = conn
            .query_row(
                "SELECT event_id, provider_name FROM events WHERE event_record_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(event_id, 4624);
        assert_eq!(provider, "Test-Provider");
    }

    #[test]
    fn test_insert_multiple_events() {
        let conn = Connection::open_in_memory().unwrap();
        create_events_table(&conn).unwrap();

        let events: Vec<ParsedEvent> = (1..=5)
            .map(|i| make_test_event(i, &format!("Provider-{i}")))
            .collect();

        insert_events(&conn, &events).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_insert_empty_batch() {
        let conn = Connection::open_in_memory().unwrap();
        create_events_table(&conn).unwrap();

        insert_events(&conn, &[]).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_null_fields_stored_as_null() {
        let conn = Connection::open_in_memory().unwrap();
        create_events_table(&conn).unwrap();

        let event = ParsedEvent {
            event_record_id: Some(1),
            event_time: None,
            provider_name: None,
            provider_guid: None,
            event_id: None,
            qualifiers: None,
            level: None,
            severity: None,
            task_category: None,
            opcode: None,
            keywords: None,
            channel: None,
            computer: None,
            process_id: None,
            thread_id: None,
            user_name: None,
            user_sid: None,
            activity_id: None,
            related_activity_id: None,
            message: None,
            level_name: None,
            keywords_name: None,
            event_data: None,
            raw_xml: None,
            sequence_number: None,
            file_name: None,
        };

        insert_events(&conn, &[event]).unwrap();

        // All columns should be NULL
        let (event_time, provider, message): (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT event_time, provider_name, message FROM events WHERE event_record_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert!(event_time.is_none());
        assert!(provider.is_none());
        assert!(message.is_none());
    }

    #[test]
    fn test_events_db_wrapper() {
        let tmp = std::env::temp_dir().join("test_events_db_wrapper.db");
        let _ = std::fs::remove_file(&tmp);

        let db = EventsDb::open(&tmp).unwrap();
        db.create_table().unwrap();

        let event = make_test_event(42, "Wrapper-Test");
        db.insert(&[event]).unwrap();

        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        db.finalize().unwrap();

        // Verify indexes exist after finalize
        let index_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND tbl_name='events'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(index_count, 6, "expected 6 indexes after finalize");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_batch_size_constant_available() {
        // BATCH_SIZE is a public const, verify it's positive
        const { assert!(BATCH_SIZE > 0); }
    }

    #[test]
    fn test_multiple_batches_dont_interfere() {
        let conn = Connection::open_in_memory().unwrap();
        create_events_table(&conn).unwrap();

        // Insert first batch
        let batch1: Vec<ParsedEvent> = (1..=3)
            .map(|i| make_test_event(i, "Batch1"))
            .collect();
        insert_events(&conn, &batch1).unwrap();

        // Insert second batch
        let batch2: Vec<ParsedEvent> = (4..=6)
            .map(|i| make_test_event(i, "Batch2"))
            .collect();
        insert_events(&conn, &batch2).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 6);

        // Verify all provider names are correct
        let mut stmt = conn
            .prepare("SELECT provider_name FROM events ORDER BY event_record_id")
            .unwrap();
        let providers: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(providers, vec!["Batch1", "Batch1", "Batch1", "Batch2", "Batch2", "Batch2"]);
    }
}

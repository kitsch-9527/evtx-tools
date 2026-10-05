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
    sequence_number     INTEGER,
    -- EventData 白名单宽表列（parser 源头抽取，EVENT_DATA_INDEXED_FIELDS 同步）
    logon_type          TEXT
);
";

/// EventData 白名单（单一事实源）：收编进索引化字段筛选的 EventData 键。
/// 元组 = (JSON 键名, 生成列名)。新增字段 = 加一行——建表 DDL、finalize
/// 索引、GUI 查询翻译（evtx_analysis 引用本表）全部由此驱动。
/// 新库：列由 parser 源头抽取落真实列（建表 DDL 落列）；存量库：GUI 侧
/// ensure 以 VIRTUAL 生成列（json_extract）自动 ALTER 迁移同名列，查询谓词透明。
pub const EVENT_DATA_INDEXED_FIELDS: &[(&str, &str)] = &[
    // 安全日志账户登录审计（4624/4625 预设依赖）
    ("LogonType", "logon_type"),
];

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
///
/// R44（2026-10-05）：`event_record_id` 是查看器默认排序（ORDER BY … DESC）与详情
/// 点查（WHERE event_record_id = ?）的唯一检索列——此前无索引迫使每次翻页/点行
/// 全表扫描+临时 B 树排序（203 万行/6.4 GB 实测单次 12.8 分钟）。user_name /
/// task_category / keywords_name 服务筛选对话框 GROUP BY 下拉与等值筛选，
/// 有索引时 GROUP BY 走索引扫描，不再读整表。
const CREATE_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_events_record_id ON events(event_record_id);",
    "CREATE INDEX IF NOT EXISTS idx_events_event_time ON events(event_time);",
    "CREATE INDEX IF NOT EXISTS idx_events_level ON events(level);",
    "CREATE INDEX IF NOT EXISTS idx_events_channel ON events(channel);",
    "CREATE INDEX IF NOT EXISTS idx_events_provider ON events(provider_name);",
    "CREATE INDEX IF NOT EXISTS idx_events_event_id ON events(event_id);",
    "CREATE INDEX IF NOT EXISTS idx_events_seq ON events(sequence_number);",
    "CREATE INDEX IF NOT EXISTS idx_events_user_name ON events(user_name);",
    "CREATE INDEX IF NOT EXISTS idx_events_task_category ON events(task_category);",
    "CREATE INDEX IF NOT EXISTS idx_events_keywords_name ON events(keywords_name);",
    "CREATE INDEX IF NOT EXISTS idx_events_logon_type ON events(logon_type);",
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
    // R35（2026-09-30）：快照语义——`-o` 目标文件是本次运行的输出而非追加日志。
    // 此前 CREATE TABLE IF NOT EXISTS 使重复 parse 向同一文件累积（实测 System.db
    // 33 次运行 204147 行，旧版本渲染的化石行与现行行并存且被查看工具命中）。
    // DROP 会连带删除本表索引，finalize 阶段按 IF NOT EXISTS 重建。
    conn.execute_batch("DROP TABLE IF EXISTS events;")
        .map_err(|e| format!("failed to drop stale events table: {e}"))?;
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
            sql.push(',');
            append_text_opt(&mut sql, &event.logon_type);
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
// Column helpers for scenario table operations
// ---------------------------------------------------------------------------

/// Return the SQL column type for a ParsedEvent field name.
fn column_sql_type(col: &str) -> &'static str {
    match col {
        "event_record_id" | "event_id" | "level" | "process_id"
        | "thread_id" | "sequence_number" => "INTEGER",
        _ => "TEXT",
    }
}

/// Append a single field value from `event` for the column named `col`.
fn append_field_value(sql: &mut String, col: &str, event: &ParsedEvent) {
    match col {
        "event_record_id" => append_int_opt(sql, &event.event_record_id),
        "event_time" => append_text_opt(sql, &event.event_time),
        "provider_name" => append_text_opt(sql, &event.provider_name),
        "provider_guid" => append_text_opt(sql, &event.provider_guid),
        "event_id" => append_int_opt(sql, &event.event_id),
        "level" => append_int_opt(sql, &event.level),
        "severity" => append_text_opt(sql, &event.severity),
        "task_category" => append_text_opt(sql, &event.task_category),
        "opcode" => append_text_opt(sql, &event.opcode),
        "keywords" => append_text_opt(sql, &event.keywords),
        "channel" => append_text_opt(sql, &event.channel),
        "computer" => append_text_opt(sql, &event.computer),
        "process_id" => append_int_opt(sql, &event.process_id),
        "thread_id" => append_int_opt(sql, &event.thread_id),
        "user_name" => append_text_opt(sql, &event.user_name),
        "user_sid" => append_text_opt(sql, &event.user_sid),
        "activity_id" => append_text_opt(sql, &event.activity_id),
        "related_activity_id" => append_text_opt(sql, &event.related_activity_id),
        "message" => append_text_opt(sql, &event.message),
        "level_name" => append_text_opt(sql, &event.level_name),
        "keywords_name" => append_text_opt(sql, &event.keywords_name),
        "event_data" => append_text_opt(sql, &event.event_data),
        "raw_xml" => append_text_opt(sql, &event.raw_xml),
        "file_name" => append_text_opt(sql, &event.file_name),
        "logon_type" => append_text_opt(sql, &event.logon_type),
        "sequence_number" => append_int_opt(sql, &event.sequence_number),
        _ => sql.push_str("NULL"),
    }
}

// ---------------------------------------------------------------------------
// Scenario table operations
// ---------------------------------------------------------------------------

/// Ensure a scenario table exists, creating it lazily with only the columns
/// declared in `rule.columns`.
///
/// The DDL uses the column names and types derived from the `ParsedEvent`
/// schema. Idempotent — safe to call multiple times.
pub fn ensure_scenario_table(conn: &Connection, rule: &crate::routing::Rule) -> Result<()> {
    let cols = rule.columns.as_ref()
        .ok_or_else(|| format!("rule '{}' has no 'columns' field", rule.name))?;
    if cols.is_empty() {
        return Err(format!("rule '{}' has empty columns list", rule.name));
    }

    let col_defs: Vec<String> = cols.iter().map(|c| {
        format!("{} {}", c, column_sql_type(c))
    }).collect();

    let ddl = format!(
        "CREATE TABLE IF NOT EXISTS {} ({})",
        rule.table,
        col_defs.join(", ")
    );
    conn.execute_batch(&ddl)
        .map_err(|e| format!("failed to create scenario table '{}': {e}", rule.table))
}

/// Insert a single event into a scenario table, writing only the columns
/// declared in `rule.columns` (in their declared order).
pub fn insert_scenario_event(conn: &Connection, rule: &crate::routing::Rule, event: &ParsedEvent) -> Result<()> {
    let cols = rule.columns.as_ref()
        .ok_or_else(|| format!("rule '{}' has no 'columns' field", rule.name))?;
    if cols.is_empty() {
        return Err(format!("rule '{}' has empty columns list", rule.name));
    }

    let mut sql = String::new();
    sql.push_str(&format!("INSERT INTO {} VALUES (", rule.table));
    for (i, col) in cols.iter().enumerate() {
        if i > 0 { sql.push(','); }
        append_field_value(&mut sql, col, event);
    }
    sql.push(')');

    conn.execute_batch(&sql)
        .map_err(|e| format!("failed to insert into '{}': {e}", rule.table))
}

#[cfg(test)]
mod scenario_tests {
    use super::*;
    use crate::routing::{Rule, RuleFilter};

    fn make_test_rule(table: &str, columns: Vec<&str>) -> Rule {
        Rule {
            name: "test".into(),
            table: table.into(),
            columns: Some(columns.into_iter().map(|s| s.into()).collect()),
            filter: RuleFilter {
                event_ids: Some(vec![4624]),
                channels: None,
                provider_like: None,
                message_like: None,
                levels: None,
                provider_name_re: None,
                message_re: None,
            },
        }
    }

    fn make_test_event() -> ParsedEvent {
        ParsedEvent {
            event_record_id: Some(42),
            event_id: Some(4624),
            event_time: Some("2024-01-15T10:30:00Z".into()),
            provider_name: Some("Test".into()),
            channel: Some("Security".into()),
            computer: Some("PC-1".into()),
            message: Some("登录成功".into()),
            file_name: Some("test.evtx".into()),
            sequence_number: Some(0),
            logon_type: None,
            // Everything else None
            provider_guid: None,
            qualifiers: None,
            version: None,
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
        }
    }

    #[test]
    fn test_ensure_scenario_table_creates() {
        let conn = Connection::open_in_memory().unwrap();
        super::create_events_table(&conn).unwrap();

        let rule = make_test_rule("my_scenario", vec!["event_record_id", "event_id", "message"]);
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

        let rule = make_test_rule("my_scenario", vec!["event_record_id", "message"]);
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
    fn test_scenario_table_has_only_declared_columns() {
        let conn = Connection::open_in_memory().unwrap();
        super::create_events_table(&conn).unwrap();

        let rule = make_test_rule("my_scenario", vec!["event_id", "message"]);
        ensure_scenario_table(&conn, &rule).unwrap();

        // Check column count = 2
        let col_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('my_scenario')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(col_count, 2, "table should have exactly 2 columns");

        // Check column names
        let col_names: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM pragma_table_info('my_scenario') ORDER BY cid")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert_eq!(col_names, vec!["event_id", "message"]);
    }

    #[test]
    fn test_insert_scenario_event_only_writes_declared_columns() {
        let conn = Connection::open_in_memory().unwrap();
        super::create_events_table(&conn).unwrap();

        // Only event_record_id, event_id, message
        let rule = make_test_rule("my_scenario", vec!["event_record_id", "event_id", "message"]);
        ensure_scenario_table(&conn, &rule).unwrap();

        let event = make_test_event();
        insert_scenario_event(&conn, &rule, &event).unwrap();

        // Query only the declared columns
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

    #[test]
    fn test_insert_scenario_event_skips_undeclared_columns() {
        let conn = Connection::open_in_memory().unwrap();
        super::create_events_table(&conn).unwrap();

        // Only event_id + message — no event_record_id column
        let rule = make_test_rule("my_scenario", vec!["event_id", "message"]);
        ensure_scenario_table(&conn, &rule).unwrap();

        let event = make_test_event(); // has event_record_id = 42
        insert_scenario_event(&conn, &rule, &event).unwrap();

        // Table has no event_record_id; verify by querying what exists
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM my_scenario WHERE event_id = 4624 AND message = '登录成功'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "event should exist in column-filtered table");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_events_table_snapshot_semantics() {
        // R35（2026-09-30）：`-o` 目标是快照不是追加日志。此前 CREATE TABLE IF
        // NOT EXISTS + INSERT 使重复 parse 向同一文件累积（实测 System.db 33 次
        // 运行 204147 行 / 真实 6225，旧版本渲染的化石行与现行行并存——用户看到的
        // 6009 `;;` 即化石行）。写两遍必须只剩第二遍的行。
        let tmp = std::env::temp_dir().join("evtx_sqlite_snapshot_test.db");
        let _ = std::fs::remove_file(&tmp);
        let path = tmp.to_str().unwrap();

        for (run, n) in [(1, 2usize), (2, 1usize)] {
            let db = EventsDb::open(path).unwrap();
            db.create_table().unwrap();
            let events: Vec<ParsedEvent> = (1..=n)
                .map(|i| make_test_event(i as u64, &format!("P{run}-{i}")))
                .collect();
            db.insert(&events).unwrap();
            db.finalize().unwrap();
        }

        let conn = Connection::open(path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "second run must replace, not accumulate");
        let provider: String = conn
            .query_row("SELECT provider_name FROM events LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(provider, "P2-1");
        let _ = std::fs::remove_file(&tmp);
    }


    fn make_test_event(event_record_id: u64, provider: &str) -> ParsedEvent {
        ParsedEvent {
            event_record_id: Some(event_record_id),
            event_time: Some("2024-01-01T00:00:00Z".into()),
            provider_name: Some(provider.into()),
            provider_guid: None,
            event_id: Some(4624),
            qualifiers: None,
            version: None,
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
            logon_type: None,
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
            version: None,
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
            logon_type: None,
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
        assert_eq!(index_count, 11, "expected 11 indexes after finalize");

        // R44：record_id 索引必须存在——查看器默认排序与详情点查的唯一检索路径
        let record_idx: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_events_record_id'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(record_idx, 1, "idx_events_record_id must exist after finalize");

        let _ = std::fs::remove_file(&tmp);
    }

    /// 白名单契约（GUI 引用化后此表为跨仓单一事实源）：每个白名单字段的列
    /// 必须真实存在于建表 DDL（parser 源头抽取），且 finalize 必须建索引。
    /// 新增白名单字段时本测试强制三处同步（DDL/索引/白名单表）。
    #[test]
    fn test_event_data_indexed_fields_contract() {
        assert!(
            !EVENT_DATA_INDEXED_FIELDS.is_empty(),
            "白名单不可为空（GUI 查询翻译依赖非空契约）"
        );
        for (key, col) in EVENT_DATA_INDEXED_FIELDS {
            assert!(
                CREATE_EVENTS_TABLE.contains(col),
                "EventData 键 {key} 的宽表列 {col} 未落入建表 DDL"
            );
            assert!(
                CREATE_INDEXES.iter().any(|s| s.contains(col)),
                "宽表列 {col} 缺 finalize 索引"
            );
        }
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

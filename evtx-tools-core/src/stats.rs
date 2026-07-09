//! In-memory event statistics aggregation.
//!
//! Collects group-by statistics during parsing and writes them to the database
//! during finalization. All aggregation is done in-memory using HashMaps to
//! avoid extra full-table scans.
//!
//! # Tables
//!
//! | Table | Columns | Source |
//! |-------|---------|--------|
//! | `stat_event_id` | event_id, provider_name, count | `ParsedEvent.event_id` + `.provider_name` |
//! | `stat_severity` | level, severity, count | `ParsedEvent.level` + `.severity` |
//! | `stat_hourly`  | hour_bucket, count | `ParsedEvent.timestamp` (truncated to hour) |
//! | `stat_channel` | channel, count | `ParsedEvent.channel` |
//! | `stat_computer` | computer, count | `ParsedEvent.computer` |
//! | `stat_event_id_hourly` | event_id, provider_name, hour_bucket, count | 前三者的笛卡尔积 — 检测时间跳变 |

use std::collections::HashMap;

use crate::{ParsedEvent, Result};

// ---------------------------------------------------------------------------
// SqlValue — helper for building inline SQL
// ---------------------------------------------------------------------------

/// A value that can appear in an inline SQL INSERT statement.
enum SqlValue {
    /// Rendered as `NULL`.
    Null,
    /// Rendered as a raw integer literal.
    Int(i64),
    /// Rendered as a single-quoted, escaped string literal.
    Text(String),
}

impl From<Option<u8>> for SqlValue {
    fn from(v: Option<u8>) -> Self {
        v.map(|n| SqlValue::Int(n as i64)).unwrap_or(SqlValue::Null)
    }
}

impl From<Option<String>> for SqlValue {
    fn from(v: Option<String>) -> Self {
        v.map(SqlValue::Text).unwrap_or(SqlValue::Null)
    }
}

fn append_sql_value(sql: &mut String, val: &SqlValue) {
    match val {
        SqlValue::Null => sql.push_str("NULL"),
        SqlValue::Int(n) => {
            use std::fmt::Write;
            let _ = write!(sql, "{n}");
        }
        SqlValue::Text(s) => {
            sql.push('\'');
            if s.contains('\'') {
                sql.push_str(&s.replace('\'', "''"));
            } else {
                sql.push_str(s);
            }
            sql.push('\'');
        }
    }
}

// ---------------------------------------------------------------------------
// StatsCollector
// ---------------------------------------------------------------------------

/// Tracks count and record-ID range for a single (event_id, hour_bucket) pair.
///
/// The `min_record_id` / `max_record_id` range enables time-jump detection:
/// if an earlier hour bucket has a higher max RID than a later hour's min RID,
/// the clock was rolled back.
#[derive(Debug, Clone, Default)]
pub struct HourlyRange {
    /// Number of events in this bucket.
    pub count: u64,
    /// Earliest `sequence_number` in this bucket.
    pub min_record_id: u64,
    /// Latest `sequence_number` in this bucket.
    pub max_record_id: u64,
}

/// Aggregate statistics collector.
///
/// Thread-compatible — not `Sync`. Use from a single thread (the writer thread)
/// or guard with a Mutex for concurrent access.
#[derive(Default)]
pub struct StatsCollector {
    /// (event_id, provider_name) → count
    event_id: HashMap<(u32, String), u64>,
    /// (level, severity_string) → count
    severity: HashMap<(Option<u8>, String), u64>,
    /// hour_bucket → count  ("YYYY-MM-DDTHH")
    hourly: HashMap<String, u64>,
    /// channel → count
    channel: HashMap<String, u64>,
    /// computer → count
    computer: HashMap<String, u64>,
    /// (event_id, provider_name, hour_bucket) → HourlyRange
    event_id_hourly: HashMap<(u32, String, String), HourlyRange>,
}

impl StatsCollector {
    /// Create a new, empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a parsed event into the collector, updating all counters.
    pub fn feed(&mut self, event: &ParsedEvent) {
        // stat_event_id: (event_id, provider_name)
        if let Some(eid) = event.event_id {
            let provider = event.provider_name.as_deref().unwrap_or("");
            *self.event_id.entry((eid, provider.to_string())).or_insert(0) += 1;

            // stat_event_id_hourly: (event_id, provider_name, hour_bucket)
            if let Some(ref ts) = event.timestamp {
                if ts.len() >= 13 {
                    let bucket = ts[..13].to_string();
                    let seq = event.sequence_number
                        .or(event.event_record_id)
                        .unwrap_or(0);
                    let range = self.event_id_hourly
                        .entry((eid, provider.to_string(), bucket))
                        .or_insert_with(|| HourlyRange {
                            count: 0,
                            min_record_id: seq,
                            max_record_id: seq,
                        });
                    range.count += 1;
                    if seq < range.min_record_id {
                        range.min_record_id = seq;
                    }
                    if seq > range.max_record_id {
                        range.max_record_id = seq;
                    }
                }
            }
        }

        // stat_severity: (level, severity)
        let sev = event.severity.clone().unwrap_or_default();
        *self.severity.entry((event.level, sev)).or_insert(0) += 1;

        // stat_hourly: YYYY-MM-DDTHH from timestamp
        if let Some(ref ts) = event.timestamp {
            if ts.len() >= 13 {
                let bucket = ts[..13].to_string();
                *self.hourly.entry(bucket).or_insert(0) += 1;
            }
        }

        // stat_channel
        let channel = event.channel.as_deref().unwrap_or("");
        if !channel.is_empty() {
            *self.channel.entry(channel.to_string()).or_insert(0) += 1;
        }

        // stat_computer
        if let Some(ref comp) = event.computer {
            if !comp.is_empty() {
                *self.computer.entry(comp.clone()).or_insert(0) += 1;
            }
        }
    }

    /// Merge another `StatsCollector` into this one (for multi-threaded accumulation).
    pub fn merge(&mut self, other: StatsCollector) {
        for (k, v) in other.event_id {
            *self.event_id.entry(k).or_insert(0) += v;
        }
        for (k, v) in other.severity {
            *self.severity.entry(k).or_insert(0) += v;
        }
        for (k, v) in other.hourly {
            *self.hourly.entry(k).or_insert(0) += v;
        }
        for (k, v) in other.channel {
            *self.channel.entry(k).or_insert(0) += v;
        }
        for (k, v) in other.computer {
            *self.computer.entry(k).or_insert(0) += v;
        }
        for (k, v) in other.event_id_hourly {
            let entry = self.event_id_hourly.entry(k).or_insert_with(|| HourlyRange {
                count: 0, min_record_id: v.min_record_id, max_record_id: v.max_record_id,
            });
            entry.count += v.count;
            if v.min_record_id < entry.min_record_id {
                entry.min_record_id = v.min_record_id;
            }
            if v.max_record_id > entry.max_record_id {
                entry.max_record_id = v.max_record_id;
            }
        }
    }

    /// Write all statistics tables to the database.
    ///
    /// Creates tables if they don't exist, inserts aggregated rows, then creates
    /// indexes. Call once after all events have been fed.
    pub fn write(&self, conn: &rusqlite::Connection) -> Result<()> {
        // 1. Collect all table data upfront (avoids lifetime issues with iterators)
        let mut tables: Vec<(&str, Vec<&str>, Vec<Vec<SqlValue>>)> = Vec::with_capacity(5);

        // stat_event_id: event_id INTEGER, provider_name TEXT, count INTEGER
        {
            let cols = vec!["event_id", "provider_name", "count"];
            let rows: Vec<Vec<SqlValue>> = {
                let mut pairs: Vec<_> = self.event_id.iter().collect();
                pairs.sort_by(|a, b| b.1.cmp(a.1)); // DESC by count
                pairs.into_iter().map(|((eid, prov), cnt)| {
                    vec![
                        SqlValue::Int(*eid as i64),
                        SqlValue::Text(prov.clone()),
                        SqlValue::Int(*cnt as i64),
                    ]
                }).collect()
            };
            tables.push(("stat_event_id", cols, rows));
        }

        // stat_severity: level INTEGER, severity TEXT, count INTEGER
        {
            let cols = vec!["level", "severity", "count"];
            let rows: Vec<Vec<SqlValue>> = {
                let mut pairs: Vec<_> = self.severity.iter().collect();
                pairs.sort_by(|a, b| b.1.cmp(a.1));
                pairs.into_iter().map(|((lvl, sev), cnt)| {
                    vec![
                        SqlValue::from(*lvl),
                        SqlValue::Text(sev.clone()),
                        SqlValue::Int(*cnt as i64),
                    ]
                }).collect()
            };
            tables.push(("stat_severity", cols, rows));
        }

        // stat_hourly: hour_bucket TEXT, count INTEGER
        {
            let cols = vec!["hour_bucket", "count"];
            let rows: Vec<Vec<SqlValue>> = {
                let mut pairs: Vec<_> = self.hourly.iter().collect();
                pairs.sort_by(|a, b| a.0.cmp(b.0)); // ASC by bucket
                pairs.into_iter().map(|(bucket, cnt)| {
                    vec![
                        SqlValue::Text(bucket.clone()),
                        SqlValue::Int(*cnt as i64),
                    ]
                }).collect()
            };
            tables.push(("stat_hourly", cols, rows));
        }

        // stat_channel: channel TEXT, count INTEGER
        {
            let cols = vec!["channel", "count"];
            let rows: Vec<Vec<SqlValue>> = {
                let mut pairs: Vec<_> = self.channel.iter().collect();
                pairs.sort_by(|a, b| b.1.cmp(a.1));
                pairs.into_iter().map(|(ch, cnt)| {
                    vec![
                        SqlValue::Text(ch.clone()),
                        SqlValue::Int(*cnt as i64),
                    ]
                }).collect()
            };
            tables.push(("stat_channel", cols, rows));
        }

        // stat_computer: computer TEXT, count INTEGER
        {
            let cols = vec!["computer", "count"];
            let rows: Vec<Vec<SqlValue>> = {
                let mut pairs: Vec<_> = self.computer.iter().collect();
                pairs.sort_by(|a, b| b.1.cmp(a.1));
                pairs.into_iter().map(|(comp, cnt)| {
                    vec![
                        SqlValue::Text(comp.clone()),
                        SqlValue::Int(*cnt as i64),
                    ]
                }).collect()
            };
            tables.push(("stat_computer", cols, rows));
        }

        // stat_event_id_hourly: event_id INTEGER, provider_name TEXT, hour_bucket TEXT, count INTEGER
        {
            let cols = vec!["event_id", "provider_name", "hour_bucket", "count",
                            "min_sequence_number", "max_sequence_number"];
            let rows: Vec<Vec<SqlValue>> = {
                let mut entries: Vec<_> = self.event_id_hourly.iter().collect();
                entries.sort_by(|a, b| {
                    let cmp = a.0.0.cmp(&b.0.0); // event_id first
                    if cmp == std::cmp::Ordering::Equal {
                        a.0.2.cmp(&b.0.2) // then hour_bucket
                    } else {
                        cmp
                    }
                });
                entries.into_iter().map(|((eid, prov, bucket), range)| {
                    vec![
                        SqlValue::Int(*eid as i64),
                        SqlValue::Text(prov.clone()),
                        SqlValue::Text(bucket.clone()),
                        SqlValue::Int(range.count as i64),
                        SqlValue::Int(range.min_record_id as i64),
                        SqlValue::Int(range.max_record_id as i64),
                    ]
                }).collect()
            };
            tables.push(("stat_event_id_hourly", cols, rows));
        }

        // 2. Write all tables within a single transaction
        conn.execute_batch("BEGIN")
            .map_err(|e| format!("failed to begin stats transaction: {e}"))?;

        for (name, col_names, rows) in &tables {
            let schema: String = col_names.iter().map(|c| match *c {
                "level" | "event_id" | "count" | "min_sequence_number" | "max_sequence_number" => format!("{c} INTEGER"),
                _ => format!("{c} TEXT"),
            }).collect::<Vec<_>>().join(", ");

            conn.execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS {name} ({schema});"
            )).map_err(|e| format!("failed to create {name}: {e}"))?;

            if rows.is_empty() {
                continue;
            }

            let cols = col_names.join(", ");
            let mut sql = format!("INSERT INTO {name} ({cols}) VALUES ");

            for (i, row) in rows.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('(');
                for (j, val) in row.iter().enumerate() {
                    if j > 0 {
                        sql.push(',');
                    }
                    append_sql_value(&mut sql, val);
                }
                sql.push(')');
            }

            conn.execute_batch(&sql)
                .map_err(|e| format!("failed to insert into {name}: {e}"))?;

            // Create index on count column for top-N queries
            conn.execute_batch(&format!(
                "CREATE INDEX IF NOT EXISTS idx_{name}_count ON {name}(count DESC);"
            )).map_err(|e| format!("failed to create index on {name}: {e}"))?;
        }

        conn.execute_batch("COMMIT")
            .map_err(|e| format!("failed to commit stats: {e}"))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParsedEvent;

    fn make_event(
        event_id: Option<u32>,
        provider: Option<&str>,
        level: Option<u8>,
        severity: Option<&str>,
        timestamp: Option<&str>,
        channel: Option<&str>,
        computer: Option<&str>,
    ) -> ParsedEvent {
        ParsedEvent {
            event_record_id: Some(1),
            event_id,
            provider_name: provider.map(|s| s.to_string()),
            level,
            severity: severity.map(|s| s.to_string()),
            timestamp: timestamp.map(|s| s.to_string()),
            channel: channel.map(|s| s.to_string()),
            computer: computer.map(|s| s.to_string()),
            // Everything else None
            provider_guid: None,
            qualifiers: None,
            task_category: None,
            opcode: None,
            keywords: None,
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
        }
    }

    #[test]
    fn test_collector_empty() {
        let collector = StatsCollector::new();
        assert!(collector.event_id.is_empty());
        assert!(collector.severity.is_empty());
        assert!(collector.hourly.is_empty());
        assert!(collector.channel.is_empty());
        assert!(collector.computer.is_empty());
    }

    #[test]
    fn test_collector_single_event() {
        let mut collector = StatsCollector::new();
        collector.feed(&make_event(
            Some(4624), Some("Test"), Some(0), Some("信息"),
            Some("2024-01-15T10:30:00Z"), Some("Security"), Some("PC-1"),
        ));

        assert_eq!(collector.event_id.get(&(4624, "Test".into())), Some(&1));
        assert_eq!(collector.severity.get(&(Some(0), "信息".into())), Some(&1));
        assert_eq!(collector.hourly.get("2024-01-15T10"), Some(&1));
        assert_eq!(collector.channel.get("Security"), Some(&1));
        assert_eq!(collector.computer.get("PC-1"), Some(&1));
    }

    #[test]
    fn test_collector_multiple_same_event() {
        let mut collector = StatsCollector::new();
        for _ in 0..5 {
            collector.feed(&make_event(
                Some(4624), Some("Test"), Some(0), Some("信息"),
                Some("2024-01-15T10:30:00Z"), Some("Security"), Some("PC-1"),
            ));
        }

        assert_eq!(collector.event_id.get(&(4624, "Test".into())), Some(&5));
        assert_eq!(collector.hourly.get("2024-01-15T10"), Some(&5));
    }

    #[test]
    fn test_collector_mixed_events() {
        let mut collector = StatsCollector::new();

        collector.feed(&make_event(
            Some(4624), Some("Sec"), Some(0), Some("信息"),
            Some("2024-01-15T10:30:00Z"), Some("Security"), Some("PC-1"),
        ));
        collector.feed(&make_event(
            Some(4625), Some("Sec"), Some(3), Some("警告"),
            Some("2024-01-15T11:00:00Z"), Some("Security"), Some("PC-1"),
        ));
        collector.feed(&make_event(
            Some(7045), Some("SCM"), Some(4), Some("信息"),
            Some("2024-01-15T10:45:00Z"), Some("System"), Some("PC-2"),
        ));

        assert_eq!(collector.event_id.len(), 3);
        assert_eq!(collector.event_id.get(&(4624, "Sec".into())), Some(&1));
        assert_eq!(collector.event_id.get(&(4625, "Sec".into())), Some(&1));
        assert_eq!(collector.event_id.get(&(7045, "SCM".into())), Some(&1));

        assert_eq!(collector.severity.len(), 3);  // (0,信息), (3,警告), (4,信息)
        assert_eq!(collector.channel.len(), 2);
        assert_eq!(collector.computer.len(), 2);
        assert_eq!(collector.hourly.len(), 2);   // T10 + T11
    }

    #[test]
    fn test_collector_nulls_handled() {
        let mut collector = StatsCollector::new();

        // Event with no event_id, no channel, no computer
        collector.feed(&make_event(None, Some("X"), None, None, None, None, None));

        // Should not crash, should not add to event_id/channel/computer maps
        assert!(collector.event_id.is_empty());
        assert!(collector.channel.is_empty());
        assert!(collector.computer.is_empty());

        // severity should still get the entry (with empty string)
        assert_eq!(collector.severity.get(&(None, String::new())), Some(&1));
    }

    #[test]
    fn test_collector_merge() {
        let mut a = StatsCollector::new();
        a.feed(&make_event(Some(4624), Some("Sec"), Some(0), Some("信息"),
            Some("2024-01-15T10:00:00Z"), Some("Security"), Some("PC-1")));

        let mut b = StatsCollector::new();
        b.feed(&make_event(Some(4625), Some("Sec"), Some(3), Some("警告"),
            Some("2024-01-15T11:00:00Z"), Some("Security"), Some("PC-2")));
        b.feed(&make_event(Some(4624), Some("Sec"), Some(0), Some("信息"),
            Some("2024-01-15T10:00:00Z"), Some("Security"), Some("PC-1")));

        a.merge(b);

        assert_eq!(a.event_id.get(&(4624, "Sec".into())), Some(&2));
        assert_eq!(a.event_id.get(&(4625, "Sec".into())), Some(&1));
        assert_eq!(a.computer.len(), 2);
    }

    #[test]
    fn test_write_stats_creates_tables() {
        let mut collector = StatsCollector::new();
        collector.feed(&make_event(
            Some(4624), Some("Test"), Some(0), Some("信息"),
            Some("2024-01-15T10:30:00Z"), Some("Security"), Some("PC-1"),
        ));
        collector.feed(&make_event(
            Some(4624), Some("Test"), Some(0), Some("信息"),
            Some("2024-01-15T10:30:00Z"), Some("Security"), Some("PC-1"),
        ));

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        collector.write(&conn).unwrap();

        // Verify tables exist
        let table_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'stat_%'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(table_count, 6, "expected 6 stat tables");

        // Verify data
        let count: i64 = conn
            .query_row("SELECT count FROM stat_event_id WHERE event_id = 4624", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let count: i64 = conn
            .query_row("SELECT count FROM stat_severity", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_write_stats_empty() {
        let collector = StatsCollector::new();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Should not error with empty collector
        collector.write(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'stat_%'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 6, "tables should exist even empty");
    }

    #[test]
    fn test_write_stats_idempotent() {
        let mut collector = StatsCollector::new();
        collector.feed(&make_event(
            Some(4624), Some("Test"), Some(0), Some("信息"),
            Some("2024-01-15T10:30:00Z"), Some("Security"), Some("PC-1"),
        ));

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        collector.write(&conn).unwrap();

        // Second write should be idempotent (CREATE TABLE IF NOT EXISTS)
        collector.write(&conn).unwrap();

        // Data should have been inserted a second time (the API is call-write,
        // not dedup — idempotent means no crash, not no-duplicates)
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM stat_event_id", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "write is append, not overwrite");
    }

    // ── Cycle: stat_event_id_hourly ──

    #[test]
    fn test_event_id_hourly_single_event() {
        let mut collector = StatsCollector::new();
        let mut ev = make_event(
            Some(4624), Some("Sec"), Some(0), Some("信息"),
            Some("2024-01-15T10:30:00Z"), Some("Security"), Some("PC-1"),
        );
        ev.event_record_id = Some(100);
        collector.feed(&ev);

        let got = collector.event_id_hourly.get(&(4624, "Sec".into(), "2024-01-15T10".into()));
        assert!(got.is_some());
        let r = got.unwrap();
        assert_eq!(r.count, 1);
        assert_eq!(r.min_record_id, 100);
        assert_eq!(r.max_record_id, 100);
    }

    #[test]
    fn test_event_id_hourly_multiple_events_same_bucket_tracks_range() {
        let mut collector = StatsCollector::new();
        let mut ev1 = make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:30:00Z"), None, None,
        );
        ev1.event_record_id = Some(5);
        collector.feed(&ev1);

        let mut ev2 = make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:45:00Z"), None, None,
        );
        ev2.event_record_id = Some(10);
        collector.feed(&ev2);

        let mut ev3 = make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:55:00Z"), None, None,
        );
        ev3.event_record_id = Some(1);
        collector.feed(&ev3);

        let got = collector.event_id_hourly.get(&(4624, "Sec".into(), "2024-01-15T10".into()));
        assert!(got.is_some());
        let r = got.unwrap();
        assert_eq!(r.count, 3);
        assert_eq!(r.min_record_id, 1);
        assert_eq!(r.max_record_id, 10);
    }

    #[test]
    fn test_event_id_hourly_multiple_hours() {
        let mut collector = StatsCollector::new();
        collector.feed(&make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:30:00Z"), None, None,
        ));
        collector.feed(&make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T11:00:00Z"), None, None,
        ));
        collector.feed(&make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:45:00Z"), None, None,
        ));

        assert_eq!(collector.event_id_hourly.len(), 2);

        let t10 = &collector.event_id_hourly[&(4624, "Sec".into(), "2024-01-15T10".into())];
        assert_eq!(t10.count, 2);
        let t11 = &collector.event_id_hourly[&(4624, "Sec".into(), "2024-01-15T11".into())];
        assert_eq!(t11.count, 1);
    }

    #[test]
    fn test_event_id_hourly_different_event_ids() {
        let mut collector = StatsCollector::new();
        collector.feed(&make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:00:00Z"), None, None,
        ));
        collector.feed(&make_event(
            Some(7045), Some("SCM"), None, None,
            Some("2024-01-15T10:00:00Z"), None, None,
        ));

        assert_eq!(collector.event_id_hourly.len(), 2);
    }

    #[test]
    fn test_event_id_hourly_written_to_db() {
        let mut collector = StatsCollector::new();
        let mut ev1 = make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:30:00Z"), None, None,
        );
        ev1.event_record_id = Some(1);
        collector.feed(&ev1);

        let mut ev2 = make_event(
            Some(4624), Some("Sec"), None, None,
            Some("2024-01-15T10:45:00Z"), None, None,
        );
        ev2.event_record_id = Some(5);
        collector.feed(&ev2);

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        collector.write(&conn).unwrap();

        let (count, min_rid, max_rid): (i64, i64, i64) = conn
            .query_row(
                "SELECT count, min_sequence_number, max_sequence_number FROM stat_event_id_hourly WHERE event_id = 4624",
                [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(min_rid, 1);
        assert_eq!(max_rid, 5);
    }

    /// Simulate time-jump scenario:
    ///   rid 1-100   @ T10  (normal)
    ///   rid 101-180 @ T11  (normal)
    ///   rid 181-230 @ T10  (clock jumped BACK to T10)
    ///   rid 231-290 @ T12  (clock jumped FORWARD to real time)
    #[test]
    fn test_event_id_hourly_detects_time_jump() {
        let mut collector = StatsCollector::new();

        // Normal T10 events
        for rid in 1..=100 {
            let mut ev = make_event(Some(4624), Some("Sec"), None, None,
                Some("2026-07-08T10:00:00Z"), None, None);
            ev.event_record_id = Some(rid);
            collector.feed(&ev);
        }

        // Normal T11 events
        for rid in 101..=180 {
            let mut ev = make_event(Some(4624), Some("Sec"), None, None,
                Some("2026-07-08T11:00:00Z"), None, None);
            ev.event_record_id = Some(rid);
            collector.feed(&ev);
        }

        // Clock jumped BACK to T10 — these have larger RID but older timestamp
        for rid in 181..=230 {
            let mut ev = make_event(Some(4624), Some("Sec"), None, None,
                Some("2026-07-08T10:00:00Z"), None, None);
            ev.event_record_id = Some(rid);
            collector.feed(&ev);
        }

        // Normal T12 events after clock jump forward
        for rid in 231..=290 {
            let mut ev = make_event(Some(4624), Some("Sec"), None, None,
                Some("2026-07-08T12:00:00Z"), None, None);
            ev.event_record_id = Some(rid);
            collector.feed(&ev);
        }

        // Verify: T10 has max_record_id (230) > T11's max_record_id (180)
        let t10 = &collector.event_id_hourly[&(4624, "Sec".into(), "2026-07-08T10".into())];
        let t11 = &collector.event_id_hourly[&(4624, "Sec".into(), "2026-07-08T11".into())];

        assert_eq!(t10.count, 150);  // 100 normal + 50 tampered
        assert_eq!(t10.max_record_id, 230);
        assert_eq!(t10.min_record_id, 1);
        assert_eq!(t11.max_record_id, 180);

        // The telltale: T10 (earlier time) has max_rid > T11 (later time) min_rid
        assert!(t10.max_record_id > t11.min_record_id,
            "T10 events have RID {} > T11 events starting RID {} → time jump!",
            t10.max_record_id, t11.min_record_id);

        // Verify T12 is clean (jump forward doesn't create anomalies)
        let t12 = &collector.event_id_hourly[&(4624, "Sec".into(), "2026-07-08T12".into())];
        assert_eq!(t12.min_record_id, 231);
        assert_eq!(t12.max_record_id, 290);
    }
}

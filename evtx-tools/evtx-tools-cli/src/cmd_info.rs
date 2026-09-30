//! `info` subcommand — query single-file messages.db metadata (schema_version 9).
//!
//! Shows provider count, variant count, OS version info, and
//! optional per-provider details.

use rusqlite::Connection;

use crate::InfoArgs;

type Result<T> = std::result::Result<T, String>;

/// R30（2026-09-30）：info 的版本校验跟进 schema v9——此前停在 v7，对现行 v9 库
/// 拒开（`schema_version=9, want 7`），与 parse（resolver 硬校验 v9）不一致。
/// 语义与 resolver.rs `open()` 对齐：只认当前版本，旧库一律 re-run msgdb-dump。
fn require_current_schema(conn: &Connection, db_path: &std::path::Path) -> Result<()> {
    let version: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("database '{}' has no schema_version (want v9 single-file DB): {e}", db_path.display()))?;
    if version != "9" {
        return Err(format!(
            "database '{}' schema_version={version}, want 9: re-run msgdb-dump (single-file, ADR 0003)",
            db_path.display()
        ));
    }
    Ok(())
}

pub fn run_info(args: &InfoArgs) -> Result<()> {
    let db_path = &args.db;

    if !db_path.exists() {
        return Err(format!("database not found: {}", db_path.display()));
    }

    let conn = Connection::open(db_path)
        .map_err(|e| format!("failed to open database '{}': {e}", db_path.display()))?;
    require_current_schema(&conn, db_path)?;

    // --- Schema version ---
    let schema_version: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "unknown".into());

    println!("=== hotech-evtx messages.db info (single-file) ===");
    println!("  Database: {}", db_path.display());
    println!("  Schema version: {schema_version}");

    // --- OS metadata ---
    let os_keys = ["product_name", "display_version", "build_number", "architecture", "created_at"];
    for key in &os_keys {
        let value: String = conn
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "-".into());
        println!("  {key}: {value}");
    }

    // --- Provider count ---
    let provider_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))
        .unwrap_or(0);
    println!("  Provider count: {provider_count}");

    // --- Variant count (the only message table) ---
    let variant_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM provider_message_variants", [], |row| row.get(0))
        .unwrap_or(0);
    println!("  Variant count: {variant_count}");

    // --- Per-provider details (optional filter) ---
    if let Some(ref filter) = args.provider {
        let pattern = format!("%{filter}%");
        println!("\n  Providers matching '{filter}':");

        let mut stmt = conn
            .prepare(
                "SELECT p.provider_name, p.guid, p.channel, COUNT(m.message_identifier) as variant_count
                 FROM providers p
                 LEFT JOIN provider_message_variants m ON m.provider_name = p.provider_name
                 WHERE p.provider_name LIKE ?1
                 GROUP BY p.provider_name
                 ORDER BY p.provider_name",
            )
            .map_err(|e| format!("failed to query providers: {e}"))?;

        let rows = stmt
            .query_map(rusqlite::params![pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| format!("failed to execute provider query: {e}"))?;

        let mut found = false;
        for row in rows {
            let (name, guid, channel, variant_count) =
                row.map_err(|e| format!("failed to read provider row: {e}"))?;
            println!(
                "    {name} (guid: {guid}, channel: {channel}, variants: {variant_count})",
                guid = guid.as_deref().unwrap_or("-"),
                channel = channel.as_deref().unwrap_or("-"),
            );
            found = true;
        }

        if !found {
            println!("    (none found)");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_info_fails_on_nonexistent_db() {
        let args = InfoArgs {
            db: PathBuf::from("/nonexistent/path/messages.db"),
            provider: None,
        };
        assert!(run_info(&args).is_err());
    }

    #[test]
    fn test_require_current_schema_accepts_v9_rejects_legacy() {
        // R30：info 版本校验与 parse（resolver 硬校验 v9）对齐——v9 通过，
        // v7/v8/无 metadata 表一律拒开（re-run msgdb-dump）。
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO metadata VALUES ('schema_version', '9');",
        )
        .unwrap();
        assert!(require_current_schema(&conn, std::path::Path::new(":memory:")).is_ok());

        for legacy in ["7", "8"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch(&format!(
                "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO metadata VALUES ('schema_version', '{legacy}');"
            ))
            .unwrap();
            assert!(
                require_current_schema(&conn, std::path::Path::new(":memory:")).is_err(),
                "legacy schema {legacy} must be rejected"
            );
        }
    }
}

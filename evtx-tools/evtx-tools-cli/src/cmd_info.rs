//! `info` subcommand — query single-file messages.db metadata (schema_version 7).
//!
//! Shows provider count, variant count, OS version info, and
//! optional per-provider details.

use rusqlite::Connection;

use crate::InfoArgs;

type Result<T> = std::result::Result<T, String>;

fn require_v7(conn: &Connection, db_path: &std::path::Path) -> Result<()> {
    let version: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("database '{}' has no schema_version (want v7 single-file DB): {e}", db_path.display()))?;
    if version != "7" {
        return Err(format!(
            "database '{}' schema_version={version}, want 7: re-run msgdb-dump (single-file, ADR 0003)",
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
    require_v7(&conn, db_path)?;

    // --- Schema version ---
    let schema_version: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "unknown".into());

    println!("=== hotech-evtx messages.db info (single-file v7) ===");
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
}

//! `merge` subcommand — merge multiple messages.db files into one.
//!
//! Reads all input databases and combines their messages and providers
//! into a single output database using `INSERT OR IGNORE`.

use rusqlite::Connection;

use crate::MergeArgs;

type Result<T> = std::result::Result<T, String>;

pub fn run_merge(args: &MergeArgs) -> Result<()> {
    if args.inputs.len() < 2 {
        return Err("merge requires at least two input databases".into());
    }

    let output_path = &args.output;

    // Check that all inputs exist
    for input in &args.inputs {
        if !input.exists() {
            return Err(format!("input db not found: {}", input.display()));
        }
    }

    // Create output database by copying the first input
    eprintln!(
        "[info] merging {} databases into {}",
        args.inputs.len(),
        output_path.display()
    );

    // Copy first input to output
    std::fs::copy(&args.inputs[0], output_path)
        .map_err(|e| format!("failed to copy '{}' to '{}': {e}", args.inputs[0].display(), output_path.display()))?;

    let out_conn = Connection::open(output_path)
        .map_err(|e| format!("failed to open output db: {e}"))?;

    // Merge remaining inputs
    for input in &args.inputs[1..] {
        let input_path = input.to_string_lossy().to_string();

        out_conn
            .execute("ATTACH DATABASE ?1 AS src", rusqlite::params![input_path])
            .map_err(|e| format!("failed to attach '{}': {e}", input.display()))?;

        // Merge providers
        let provider_count = out_conn
            .execute(
                "INSERT OR IGNORE INTO providers (name, guid, dll_path, channel)
                 SELECT name, guid, dll_path, channel FROM src.providers",
                [],
            )
            .map_err(|e| format!("failed to merge providers from '{}': {e}", input.display()))?;

        // Merge messages
        let msg_count = out_conn
            .execute(
                "INSERT OR IGNORE INTO messages (provider_name, event_id, message, level, task, opcode, keywords, parameters)
                 SELECT provider_name, event_id, message, level, task, opcode, keywords, parameters FROM src.messages",
                [],
            )
            .map_err(|e| format!("failed to merge messages from '{}': {e}", input.display()))?;

        out_conn
            .execute("DETACH DATABASE src", [])
            .map_err(|e| format!("failed to detach '{}': {e}", input.display()))?;

        eprintln!(
            "[info]   merged from '{}': {provider_count} new providers, {msg_count} new messages",
            input.display()
        );
    }

    // Update metadata: set updated_at timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "unknown".into());
    out_conn
        .execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('updated_at', ?1)",
            rusqlite::params![now],
        )
        .map_err(|e| format!("failed to update metadata: {e}"))?;

    // Analyze for query performance
    out_conn
        .execute_batch("ANALYZE;")
        .map_err(|e| format!("failed to analyze: {e}"))?;

    eprintln!("[done] merged database: {}", output_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_merge_requires_at_least_two_inputs() {
        let args = MergeArgs {
            inputs: vec![PathBuf::from("a.db")],
            output: PathBuf::from("out.db"),
        };
        assert!(run_merge(&args).is_err());
    }

    #[test]
    fn test_merge_checks_input_exists() {
        let args = MergeArgs {
            inputs: vec![
                PathBuf::from("nonexistent_a.db"),
                PathBuf::from("nonexistent_b.db"),
            ],
            output: PathBuf::from("out.db"),
        };
        assert!(run_merge(&args).is_err());
    }
}

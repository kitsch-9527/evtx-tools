//! `parse` subcommand — CLI entry for the parse pipeline.
//!
//! The full orchestration (writer thread, batching, message rendering,
//! scenario routing, statistics) lives in [`evtx_tools_core::pipeline`] so
//! library hosts share byte-identical semantics.  This module keeps only the
//! CLI-facing concerns: EVTX build detection, message-db path resolution
//! (`--msg-db` / `--msg-dir`), and rule loading (built-in profiles +
//! `--rules-dir`).

use evtx_tools_core::parser::detect_evtx_build;
use evtx_tools_core::pipeline::{self, PipelineConfig};
use evtx_tools_core::routing::{rule_from_yaml, SceneRouter};

use crate::ParseArgs;

type Result<T> = std::result::Result<T, String>;

pub fn run_parse(args: &ParseArgs) -> Result<()> {
    let input_path = args.input.to_string_lossy().to_string();

    // --- Detect build ---
    let build = detect_evtx_build(&input_path);
    if let Some(b) = build {
        eprintln!("[info] detected EVTX build: {b}");
    } else {
        eprintln!("[warn] could not detect EVTX build number");
    }

    // --- Resolve messages.db path (single-file v9 only, ADR 0003; no-DB is a hard error) ---
    let msg_db_path = resolve_msg_db(&args.msg_db, &args.msg_dir, build)?.ok_or_else(|| {
        "no messages db: single-file MessageDB (schema_version 9) is required; pass --msg-db or --msg-dir".to_string()
    })?;

    // Build SceneRouter from --profile and --rules-dir
    let router = build_router(&args.profile, args.rules_dir.as_deref())?;

    let config = PipelineConfig {
        input_path,
        target_lang: args.lang.clone(),
        threads: args.threads,
        with_xml: args.with_xml,
        shard: args.shard,
        batch_size: 0, // core default (20_000)
        msg_db_path,
        output_db_path: (args.format == "db" || args.format == "both")
            .then(|| args.output.to_string_lossy().to_string()),
        output_csv_path: (args.format == "csv" || args.format == "both")
            .then(|| args.output.to_string_lossy().to_string()),
        write_stats: !args.no_stats,
        router,
        on_progress: None,
        cancel: None,
    };
    pipeline::run(config)?;
    Ok(())
}

/// Resolve the path to a single-file messages.db (schema_version 7, ADR 0003).
///
/// Priority: explicit `--msg-db` > single `.db` in `--msg-dir` > None.
/// Build matching is deleted: the single file is the local-machine DB.
fn resolve_msg_db(
    explicit: &Option<std::path::PathBuf>,
    msg_dir: &Option<std::path::PathBuf>,
    _build: Option<u32>,
) -> Result<Option<String>> {
    // 1. Explicit path
    if let Some(path) = explicit {
        if path.exists() {
            return Ok(Some(path.to_string_lossy().to_string()));
        }
        return Err(format!("messages db not found: {}", path.display()));
    }

    // 2. Auto-detect from msg-dir
    if let Some(dir) = msg_dir {
        if !dir.is_dir() {
            return Err(format!("msg-dir is not a directory: {}", dir.display()));
        }

        // Collect all hotech-msg-*.db files
        let mut found: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.ends_with(".db") && name.starts_with("hotech-msg-") {
                        found.push(path);
                    }
                }
            }
        }

        match found.len() {
            0 => {
                eprintln!("[warn] no hotech-msg-*.db found in '{}'", dir.display());
            }
            1 => {
                let path = found[0].to_string_lossy().to_string();
                eprintln!("[info] using messages db: {path}");
                return Ok(Some(path));
            }
            _ => {
                return Err(format!(
                    "multiple messages db in '{}': pass --msg-db explicitly (single-file, ADR 0003): {:?}",
                    dir.display(),
                    found
                ));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn touch_db(dir: &std::path::Path, filename: &str) -> PathBuf {
        let db_path = dir.join(filename);
        rusqlite::Connection::open(&db_path).expect("failed to create test db");
        db_path
    }

    #[test]
    fn test_resolve_msg_db_single_file() {
        let dir = std::env::temp_dir().join("hotech-test-single");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _db = touch_db(&dir, "hotech-msg-test.db");
        let result = resolve_msg_db(&None, &Some(dir.clone()), None).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("hotech-msg-test.db"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_msg_db_multiple_is_error() {
        let dir = std::env::temp_dir().join("hotech-test-multi");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _a = touch_db(&dir, "hotech-msg-a.db");
        let _b = touch_db(&dir, "hotech-msg-b.db");
        let result = resolve_msg_db(&None, &Some(dir.clone()), None);
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_msg_db_empty_is_none() {
        let dir = std::env::temp_dir().join("hotech-test-empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let result = resolve_msg_db(&None, &Some(dir.clone()), None).unwrap();
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

}

// ---------------------------------------------------------------------------
// Routing helper — build a SceneRouter from CLI args
// ---------------------------------------------------------------------------

/// Build a `SceneRouter` from `--profile` names and/or `--rules-dir`.
///
/// 1. Loads built-in profiles by name from `BUILTIN_PROFILES`.
/// 2. Scans `rules_dir` for custom `.yaml` / `.yml` rule files.
/// 3. Returns a combined router (built-in + custom).
pub fn build_router(
    profile_names: &[String],
    rules_dir: Option<&std::path::Path>,
) -> evtx_tools_core::Result<SceneRouter> {
    let mut rules = Vec::new();

    // 1. Load built-in profiles
    for name in profile_names {
        let found = crate::BUILTIN_PROFILES.iter().find(|(key, _)| *key == name);
        match found {
            Some((_, yaml)) => match rule_from_yaml(yaml) {
                Ok(rule) => rules.push(rule),
                Err(e) => eprintln!("[warn] failed to parse built-in profile '{name}': {e}"),
            },
            None => eprintln!("[warn] unknown built-in profile '{name}', skipping"),
        }
    }

    // 2. Load custom rules from directory
    if let Some(dir) = rules_dir {
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "yaml" || ext == "yml" {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => match rule_from_yaml(&content) {
                                Ok(rule) => rules.push(rule),
                                Err(e) => eprintln!(
                                    "[warn] failed to parse rule '{}': {e}",
                                    path.display()
                                ),
                            },
                            Err(e) => eprintln!(
                                "[warn] failed to read rule file '{}': {e}",
                                path.display()
                            ),
                        }
                    }
                }
            }
        } else {
            eprintln!("[warn] rules-dir '{}' is not a directory", dir.display());
        }
    }

    Ok(SceneRouter::new(rules))
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    use evtx_tools_core::ParsedEvent;

    #[test]
    fn test_build_router_empty_args() {
        let router = build_router(&[], None).unwrap();
        assert_eq!(router.route(&ParsedEvent {
            event_record_id: Some(1),
            event_id: Some(4624),
            channel: Some("Security".into()),
            event_time: None,
            provider_name: None,
            provider_guid: None,
            qualifiers: None,
            version: None,
            level: None,
            severity: None,
            task_category: None,
            opcode: None,
            keywords: None,
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
            file_name: None,
            sequence_number: None,
        }).len(), 0);
    }
}

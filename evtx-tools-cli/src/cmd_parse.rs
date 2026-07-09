//! `parse` subcommand — parse .evtx files into SQLite or CSV.
//!
//! Flow:
//! 1. Open EVTX file, detect build number
//! 2. Auto-match messages.db from `--msg-dir` or use explicit `--msg-db`
//! 3. First pass: collect all unique (provider, event_id) pairs
//! 4. Warmup MessageDb cache with those pairs
//! 5. Second pass: iterate events, resolve messages via substitute, write output

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use rayon::prelude::*;

use serde_json::Value;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use evtx_tools_core::parser::{detect_evtx_build, iter_events, open_evtx};
use evtx_tools_core::resolver::MessageDb;
use evtx_tools_core::storage::csv::write_events;
use evtx_tools_core::storage::sqlite::EventsDb;
use evtx_tools_core::storage::sqlite::{ensure_scenario_table, insert_scenario_event};
use evtx_tools_core::routing::{rule_from_yaml, SceneRouter};
use evtx_tools_core::stats::StatsCollector;
use evtx_tools_core::substitute::{substitute_format_message, apply_param_enums};
use evtx_tools_core::{compute_message_id, MessageRow, ParsedEvent};

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

    // --- Resolve messages.db path ---
    let msg_db_path = resolve_msg_db(&args.msg_db, &args.msg_dir, build)?;

    // --- Open message DB (no warmup yet — lazy on first encounter) ---
    let target_lang = args.lang.clone();
    eprintln!("[info] message language: {target_lang}");
    let mut msg_db: Option<MessageDb> = None;
    if let Some(ref db_path) = msg_db_path {
        eprintln!("[info] loading messages db: {db_path}");
        match MessageDb::open(db_path) {
            Ok(mut db) => {
                if let Err(e) = db.load_param_refs() {
                    eprintln!("[warn] failed to load param refs: {e}");
                }
                if let Err(e) = db.load_value_maps() {
                    eprintln!("[warn] failed to load value maps: {e}");
                }
                msg_db = Some(db);
            }
            Err(e) => eprintln!("[warn] failed to open messages db: {e}"),
        }
    } else {
        eprintln!("[info] no messages db available — messages will be empty");
    }

    // --- Prepare output ---
    let fmt = args.format.as_str();
    let output_path = &args.output;

    // SQLite output
    let events_db = if fmt == "db" || fmt == "both" {
        let db = EventsDb::open(output_path)?;
        db.create_table()?;
        Some(db)
    } else {
        None
    };

    // CSV output — write header upfront
    let csv_writer: Option<BufWriter<File>> = if fmt == "csv" || fmt == "both" {
        let file = File::create(output_path)
            .map_err(|e| format!("failed to create csv '{}': {e}", output_path.display()))?;
        let mut writer = BufWriter::new(file);
        // Write an empty batch to emit header + BOM; actual data appended per-batch
        write_events(&mut writer, &[])?;
        Some(writer)
    } else {
        None
    };

    // --- Consumer-producer pipeline ---
    //
    // Producer thread (this thread): iterates EVTX records, extracts fields,
    //   resolves messages, sends resolved batches to the writer via a channel.
    // Writer thread (background): receives ready-to-flush batches and writes
    //   to SQLite/CSV, creates indexes at the end.
    //
    // The channel uses small buffers (4 batches) to keep memory bounded while
    // ensuring the writer always has work queued.  Sending whole batches instead
    // of individual events eliminates ~2.5M channel operations for large files.
    let (tx, rx) = mpsc::sync_channel::<Vec<ParsedEvent>>(4);

    // Build SceneRouter from --profile and --rules-dir
    let router = build_router(&args.profile, args.rules_dir.as_deref())?;

    // Move owned copies into the writer thread (can't borrow from args in a 'static closure).
    let output_display = output_path.display().to_string();
    let no_stats = args.no_stats;

    // Writer thread
    let writer = thread::Builder::new()
        .name("evtx-writer".into())
        .spawn(move || -> Result<()> {
            let mut csv_writer = csv_writer;
            let mut total = 0u64;
            let mut stats = StatsCollector::new();
            let has_rules = router.has_rules();
            let mut tables_created: std::collections::HashSet<String> = std::collections::HashSet::new();

            for events in rx {
                flush_batch(&events, &events_db, &mut csv_writer, &mut stats)?;

                // Route matching events to scenario tables
                if has_rules {
                    if let Some(ref db) = events_db {
                        for event in &events {
                            let matched = router.route(event);
                            for rule in &matched {
                                // Lazy table creation (CREATE TABLE IF NOT EXISTS)
                                if tables_created.insert(rule.table.clone()) {
                                    ensure_scenario_table(db.conn(), rule)?;
                                }
                                insert_scenario_event(db.conn(), rule, event)?;
                            }
                        }
                    }
                }

                total += events.len() as u64;
                eprintln!("[progress] {total} events processed");
            }

            // Write statistics tables (if enabled) before finalize
            if let Some(ref db) = events_db {
                if !no_stats {
                    eprintln!("[info] writing statistics tables...");
                    stats.write(db.conn())?;
                }
            }

            // Finalize — create indexes + ANALYZE after all data is inserted
            if let Some(ref db) = events_db {
                db.finalize()?;
            }

            eprintln!("[done] {total} events written to {output_display}");
            Ok(())
        })
        .map_err(|e| format!("failed to spawn writer thread: {e}"))?;

    // Producer: iterate evtx events in batches, parallel resolve, send to writer
    let mut parser = open_evtx(&input_path, args.threads)?;
    let file_name = file_stem(&args.input);

    // Wrap msg_db in Arc<Mutex> for thread-safe parallel access.
    // Mutex is used because rusqlite::Connection is not Sync (contains RefCell),
    // but the lock is only held briefly per event (HashMap lookup + Clone).
    let msg_db: Option<Arc<Mutex<MessageDb>>> = msg_db.map(|db| Arc::new(Mutex::new(db)));

    const PARALLEL_BATCH: usize = 20_000;

    let mut batch: Vec<(ParsedEvent, Option<Value>)> = Vec::with_capacity(PARALLEL_BATCH);
    let mut new_pairs: Vec<(String, u32)> = Vec::new();
    let mut warmed_up: HashSet<(String, u32)> = HashSet::new();
    // Global monotonically increasing sequence number — preserves parse order
    let mut sequence_number: u64 = 0;

    for result in iter_events(&mut parser, file_name) {
        let (mut event, event_data_value) = match result {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[warn] skipping event: {e}");
                continue;
            }
        };

        // Assign global sequence number in parse order
        event.sequence_number = Some(sequence_number);
        sequence_number += 1;

        // Collect unique (provider, eid) pairs for warmup (amortized per batch)
        if msg_db.is_some() {
            if let (Some(ref prov), Some(eid)) = (event.provider_name.clone(), event.event_id) {
                let key = (prov.clone(), eid);
                if warmed_up.insert(key) {
                    let mid = compute_message_id(eid, event.qualifiers);
                    new_pairs.push((prov.clone(), eid));
                    if mid != eid {
                        warmed_up.insert((prov.clone(), mid));
                        new_pairs.push((prov.clone(), mid));
                    }
                }
            }
        }

        batch.push((event, event_data_value));

        if batch.len() >= PARALLEL_BATCH {
            process_parallel_batch(
                &msg_db, &mut new_pairs, &target_lang, &mut batch,
            );
            let event_vec: Vec<ParsedEvent> = batch.drain(..).map(|(e, _)| e).collect();
            if tx.send(event_vec).is_err() {
                eprintln!("[error] writer thread terminated unexpectedly");
                break;
            }
        }
    }

    // Flush remaining partial batch
    if !batch.is_empty() {
        process_parallel_batch(
            &msg_db, &mut new_pairs, &target_lang, &mut batch,
        );
        let event_vec: Vec<ParsedEvent> = batch.drain(..).map(|(e, _)| e).collect();
        if tx.send(event_vec).is_err() {
            eprintln!("[error] writer thread terminated unexpectedly");
        }
    }

    // Signal writer to finish by dropping the sender, then wait
    drop(tx);
    writer.join().map_err(|_| "writer thread panicked".to_string())??;

    Ok(())
}

/// Quick 3-field JSON traversal to extract (provider_name, event_id) for warmup.
///
/// Mirrors `get_attr` / `get_str` from `extract_fields` — the evtx crate's
/// default format nests XML attributes under `#attributes` and text content
/// under `#text`:
///
/// ```json
/// {"Provider": {"#attributes": {"Name": "...", "Guid": "..."}},
///  "EventID": {"#text": 4673, "#attributes": {"Qualifiers": 0}}}
/// ```
/// Resolve messages for a batch of events in parallel using rayon.
///
/// 1. **Warmup** (lock, single-threaded): batch-loads new (provider, eid) pairs
///    from SQLite into the in-memory `MessageDb` cache.
/// 2. **Parallel resolve** (concurrent, lock briefly per event): each event
///    acquires the lock only to clone its `MessageRow` from the cache (HashMap
///    lookup + Clone), then does the heavy work (JSON traversal, `substitute`,
///    metadata copy) **without the lock** — so all cores run simultaneously.
///    A second brief lock is taken only when `%%N` expansion or value_maps
///    lookup is needed.
fn process_parallel_batch(
    msg_db: &Option<Arc<Mutex<MessageDb>>>,
    new_pairs: &mut Vec<(String, u32)>,
    target_lang: &str,
    batch: &mut [(ParsedEvent, Option<Value>)],
) {
    if batch.is_empty() {
        return;
    }

    // --- Handle no messages.db: set fallback for all events ---
    let db = match msg_db {
        Some(ref db) => db,
        None => {
            for (event, ev_data) in batch.iter_mut() {
                if event.message.is_some() {
                    continue;
                }
                let raw_values = extract_subst_params(
                    &event.event_data,
                    ev_data.as_ref(),
                    &None,
                );
                event.message = Some(
                    evtx_tools_core::substitute::generate_fallback_message(
                        event.provider_name.as_deref(),
                        event.event_id,
                        &raw_values,
                        target_lang,
                        false, // has_msg_db = false
                    ),
                );
            }
            return;
        }
    };

    // --- 1. Warmup (lock held, single-threaded) ---
    if !new_pairs.is_empty() {
        let refs: Vec<(&str, u32)> = new_pairs
            .iter()
            .map(|(p, e)| (p.as_str(), *e))
            .collect();
        if let Ok(mut guard) = db.lock() {
            if target_lang != "en-US" {
                let _ = guard.warmup_lang(&refs, "en-US");
            }
            let _ = guard.warmup_lang(&refs, target_lang);
        } else {
            eprintln!("[warn] msg_db lock poisoned during warmup");
        }
        new_pairs.clear();
    }

    // --- 2. Parallel message resolution ---
    // Lock held only briefly to clone MessageRow; substitution + metadata copy
    // run without the lock so all rayon threads execute simultaneously.
    batch.par_iter_mut().for_each(|(event, event_data_value)| {
        // ---- Brief lock: fetch cloned MessageRow and value_maps ----
        // Use an inner closure so that any lookup failure falls through
        // to fallback message generation rather than silently leaving
        // the message as None.
        struct ResolveResult {
            row: MessageRow,
            value_maps: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
            bitmap_names: std::collections::HashSet<String>,
        }

        let resolved = (|| -> Option<ResolveResult> {
            let guard = db.lock().ok()?;
            let provider = event.provider_name.as_ref()?;
            let provider_str = provider.as_str();
            let eid = event.event_id?;
            let mid = compute_message_id(eid, event.qualifiers);
            let row = guard
                .get(provider_str, mid)
                .or_else(|| guard.get(provider_str, eid))
                .cloned()?;
            let vms = row.parameters.as_ref()
                .map(|pm| guard.get_param_value_maps(pm))
                .unwrap_or_default();
            Some(ResolveResult {
                row,
                value_maps: vms,
                bitmap_names: guard.get_bitmap_names(),
            })
        })();

        let row = match resolved {
            Some(r) => r,
            None => {
                // Cache miss or missing provider/eid — set fallback message
                let raw_values = extract_subst_params(
                    &event.event_data,
                    event_data_value.as_ref(),
                    &None,
                );
                event.message = Some(
                    evtx_tools_core::substitute::generate_fallback_message(
                        event.provider_name.as_deref(),
                        event.event_id,
                        &raw_values,
                        target_lang,
                        true, // has_msg_db = true
                    ),
                );
                return;
            }
        };

        // ---- Heavy work (no lock): substitute + copy metadata ----
        // Step 1: Expand %%N parameter references BEFORE substitute, because
        // substitute() consumes %% → % (literal percent), which destroys the
        // %%NNNN marker that expand_param_refs needs to look up (e.g. %%1843).
        let template = if row.row.template.contains("%%") {
            if let Ok(guard) = db.lock() {
                guard.expand_message(&row.row.template)
            } else {
                row.row.template.clone()
            }
        } else {
            row.row.template.clone()
        };

        // Step 2: Substitute %1–%99 positional parameters.
        if template.contains('%') {
            let params = extract_subst_params(
                &event.event_data,
                event_data_value.as_ref(),
                &row.row.parameters,
            );
            // Apply Layer 1 enum resolution (standard type conventions)
            let params = apply_param_enums(&params, &row.row.parameters);
            // Apply Layer 2: CRIM VMAP/BMAP value map resolution
            let params = evtx_tools_core::substitute::apply_value_maps(&params, &row.row.parameters, &row.value_maps, &row.bitmap_names);
            // Expand %%N parameter references in parameter VALUES
            // (the template was already expanded above; param values may also
            // contain %%N refs, e.g. %%1843 → "虚拟帐户", which must be resolved
            // before substitute_format_message consumes %% → %)
            let params = expand_params_par_refs(params, db);
            let param_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            event.message = Some(substitute_format_message(&template, &param_refs));
        } else {
            event.message = Some(template);
        }

        if let Some(ref t) = row.row.task {
            if !t.is_empty() {
                event.task_category = Some(t.clone());
            }
        }
        if let Some(ref o) = row.row.opcode {
            if !o.is_empty() {
                event.opcode = Some(o.clone());
            }
        }
        if let Some(ref k) = row.row.keywords {
            if !k.is_empty() {
                event.keywords = Some(k.clone());
            }
        }
        if let Some(ref ln) = row.row.level_name {
            if !ln.is_empty() {
                event.level_name = Some(ln.clone());
            }
        }
        if let Some(ref kn) = row.row.keywords_name {
            if !kn.is_empty() {
                event.keywords_name = Some(kn.clone());
            }
        }

        // Note: %%N expansion already handled above (Step 1), before substitute.
        // No post-substitution %%N check needed — all %% are consumed by substitute.

        // ---- Final formatting: match Windows Event Viewer conventions ----
        if let Some(msg) = event.message.take() {
            event.message = Some(evtx_tools_core::substitute::format_for_display(&msg));
        }
    });
}

#[allow(dead_code)]
/// Resolve the message for a single parsed event using the message DB cache.
///
/// For classic EventLog providers (with Qualifiers), computes the full
/// message_id via `(qualifiers << 16) | event_id` and tries that first,
/// then falls back to event_id alone.
///
/// When `event_data_value` is provided (the raw `Value` from the parser), it
/// is passed directly to [`extract_subst_params`] to avoid a Value→String→Value
/// round trip.
fn resolve_event_message(
    event: &mut ParsedEvent,
    msg_db: &MessageDb,
    event_data_value: Option<&Value>,
) {
    let provider = match event.provider_name.as_ref() {
        Some(p) => p,
        None => return,
    };
    let eid = match event.event_id {
        Some(e) => e,
        None => return,
    };

    // Try computed message_id first (classic providers), fall back to event_id
    let mid = compute_message_id(eid, event.qualifiers);
    let row = match msg_db.get(provider, mid).or_else(|| msg_db.get(provider, eid)) {
        Some(r) => r,
        None => return,
    };

    // Extract substitution parameters from EventData
    let params = extract_subst_params(&event.event_data, event_data_value, &row.parameters);

    // Apply Layer 1 enum resolution (standard type conventions)
    let params = apply_param_enums(&params, &row.parameters);

    // Convert Vec<String> to Vec<&str> for substitute
    let param_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();

    // Substitute (two-pass to match Windows FormatMessageW behavior)
    event.message = Some(substitute_format_message(&row.template, &param_refs));

    // Copy metadata from message row — prefer DB values when available
    // (the EVTX raw values are often numeric IDs; DB has human-readable names)
    if let Some(ref t) = row.task {
        if !t.is_empty() { event.task_category = Some(t.clone()); }
    }
    if let Some(ref o) = row.opcode {
        if !o.is_empty() { event.opcode = Some(o.clone()); }
    }
    if let Some(ref k) = row.keywords {
        if !k.is_empty() { event.keywords = Some(k.clone()); }
    }

    // Copy level_name and keywords_name (resolved from CRIM LEVL/KEYW blocks)
    if let Some(ref ln) = row.level_name {
        if !ln.is_empty() { event.level_name = Some(ln.clone()); }
    }
    if let Some(ref kn) = row.keywords_name {
        if !kn.is_empty() { event.keywords_name = Some(kn.clone()); }
    }
}

/// Extract substitution values from event_data JSON using the parameter map.
///
/// When `event_data_value` is `Some`, it is used directly to avoid parsing the
/// JSON string (`event_data_str`) a second time — this eliminates the
/// Value→String→Value round trip that previously occurred between
/// [`extract_fields`] and this function.
///
/// Three strategies, tried in order:
///
/// 1. **Flat object** (evtx 0.12+): EventData is a flat key-value object.
///    ```json
///    {"SubjectUserSid":"S-1-5-...","SubjectUserName":"jipeng.song",...}
///    ```
///    param_map maps `"1" → "SubjectUserSid"`, `"2" → "SubjectUserName"`, etc.
///    Values are looked up by param name directly.
///
/// 2. **Named Data array** (evtx 0.8): event_data is a `Data` array with `Name`
///    attributes, and param_map provides the index→name mapping.
///    ```json
///    [{"#attributes":{"Name":"SubjectUserSid"},"$":"S-1-5-..."}, ...]
///    ```
///
/// 3. **Positional `#text`** (classic providers): event_data is an object with
///    a `#text` array whose elements correspond to `%1`, `%2`, ... positionally.
///
/// 4. **Fallback**: returns empty Vec (no substitution).
fn extract_subst_params(
    event_data_str: &Option<String>,
    event_data_value: Option<&Value>,
    param_map: &Option<HashMap<String, String>>,
) -> Vec<String> {
    // Use the pre-parsed Value if available (avoids round-trip), otherwise
    // fall back to parsing the serialised JSON string.
    let owned_value;
    let parsed: &Value = match event_data_value {
        Some(v) => v,
        None => {
            let json_str = match event_data_str.as_ref() {
                Some(j) => j.as_str(),
                None => return Vec::new(),
            };
            owned_value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(_) => return Vec::new(),
            };
            &owned_value
        }
    };

    // Strategy 1: Flat object (evtx 0.12) WITH param_map
    //   {"SubjectUserSid":"S-1-5-...","SubjectUserName":"jipeng.song",...}
    //   param_map: "1" → "SubjectUserSid", "2" → "SubjectUserName"
    if let (Some(params), Some(obj)) = (param_map.as_ref(), parsed.as_object()) {
        // Skip classic format (has "Data" key wrapping the actual data) —
        // Strategy 1.75 handles that case below.
        if !params.is_empty() && !obj.contains_key("#text") && !obj.contains_key("Data") {
            let max_idx: usize = params.keys()
                .filter_map(|k| k.parse::<usize>().ok())
                .max().unwrap_or(0);
            if max_idx > 0 {
                let mut result: Vec<String> = Vec::with_capacity(max_idx);
                for i in 1..=max_idx {
                    let idx_str = i.to_string();
                    let value = match params.get(&idx_str) {
                        Some(param_name) => obj.get(param_name.as_str())
                            .and_then(|v| match v {
                                Value::String(s) => Some(s.clone()),
                                Value::Number(n) => Some(n.to_string()),
                                _ => None,
                            })
                            .unwrap_or_default(),
                        None => String::new(),
                    };
                    result.push(value);
                }
                return result;
            }
        }
    }

    // Strategy 1.5: Classic EventLog #text array nested under "Data".
    // Format: {"Data": {"#text": [val1, val2, ...]}, "Binary": null}
    // Classic providers (MsiInstaller, MSSQLSERVER, etc.) store substitution
    // values positionally in a #text array within "Data". This is detected
    // BEFORE Strategy 2's flat-object fallback which would otherwise return
    // the raw {"Data": {...}, "Binary": null} object as-is.
    if let Some(obj) = parsed.as_object() {
        if let Some(data_obj) = obj.get("Data").and_then(|v| v.as_object()) {
            if let Some(text_val) = data_obj.get("#text") {
                let values: Vec<String> = match text_val {
                    Value::Array(arr) => arr.iter().map(|v| match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    }).collect(),
                    Value::String(s) => vec![s.clone()],
                    _ => Vec::new(),
                };
                if !values.is_empty() {
                    return values;
                }
            }
        }
    }

    // Strategy 1.75: Classic "Data" array (evtx 0.12 classic format).
    // Format: {"Data": [{"#attributes": {"Name": "Param1"}, "$": "val1"}, ...], "Binary": null}
    // This handles the case where EventData wraps the actual data under a "Data" key as an array.
    // With param_map: lookup by Name attribute (named).
    // Without param_map: extract positionally from array order.
    // Must run BEFORE Strategy 2 (flat-object fallback) which would otherwise
    // treat "Data"/"Binary" as parameter keys and return empty strings.
    if let Some(obj) = parsed.as_object() {
        if let Some(data_arr) = obj.get("Data").and_then(|v| v.as_array()) {
            if let Some(params) = param_map.as_ref() {
                // With param_map: build name→value map, then extract by param name
                let data_map: std::collections::HashMap<&str, &str> = data_arr.iter().filter_map(|item| {
                    let attrs = item.get("#attributes")?;
                    let name = attrs.get("Name")?.as_str()?;
                    let value = item.get("$").or_else(|| item.get("#text"))
                        .and_then(|v| v.as_str()).unwrap_or("");
                    Some((name, value))
                }).collect();

                if !data_map.is_empty() {
                    let max_idx: usize = params.keys()
                        .filter_map(|k| k.parse::<usize>().ok())
                        .max().unwrap_or(0);
                    if max_idx > 0 {
                        let mut result: Vec<String> = Vec::with_capacity(max_idx);
                        for i in 1..=max_idx {
                            let idx_str = i.to_string();
                            let value = match params.get(&idx_str) {
                                Some(param_name) => data_map.get(param_name.as_str()).unwrap_or(&"").to_string(),
                                None => String::new(),
                            };
                            result.push(value);
                        }
                        return result;
                    }
                }
            } else {
                // Without param_map: extract positionally (by array index)
                let values: Vec<String> = data_arr.iter().map(|item| {
                    // Each item: {"$": "value"} or {"#text": "value"}
                    if let Some(val) = item.get("$").or_else(|| item.get("#text")) {
                        match val {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    }
                }).collect();
                if !values.is_empty() {
                    return values;
                }
            }
        }
    }
    // Strategy 2: Flat object positional fallback (NO param_map).
    // When DB lacks parameter mapping, use JSON key order (which preserves
    // BinXML template order in serde_json and evtx 0.12).
    // Excludes classic format objects (those with a "Data" key) — they are
    // handled by Strategy 1.75 above.
    if let Some(obj) = parsed.as_object() {
        if !obj.contains_key("#text") && !obj.contains_key("Data") {
            let values: Vec<String> = obj.iter().map(|(_k, v)| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            }).collect();
            if !values.is_empty() {
                return values;
            }
        }
    }

    // Strategy 2: Named Data array (evtx 0.8 fallback)
    if let (Some(params), Some(data_array)) = (param_map.as_ref(), parsed.as_array()) {
        // Build map: DataName → value from "$" or "#text"
        let data_map: HashMap<&str, &str> = data_array.iter().filter_map(|item| {
            let attrs = item.get("#attributes")?;
            let name = attrs.get("Name")?.as_str()?;
            let value = item.get("$").or_else(|| item.get("#text"))
                .and_then(|v| v.as_str()).unwrap_or("");
            Some((name, value))
        }).collect();

        if !data_map.is_empty() {
            let max_idx: usize = params.keys().filter_map(|k| k.parse::<usize>().ok()).max().unwrap_or(0);
            if max_idx > 0 {
                let mut result: Vec<String> = Vec::with_capacity(max_idx);
                for i in 1..=max_idx {
                    let idx_str = i.to_string();
                    let value = match params.get(&idx_str) {
                        Some(param_name) => data_map.get(param_name.as_str()).unwrap_or(&"").to_string(),
                        None => String::new(),
                    };
                    result.push(value);
                }
                return result;
            }
        }
    }

    // Strategy 3: Positional #text array (classic providers)
    if let Some(obj) = parsed.as_object() {
        if let Some(text_arr) = obj.get("#text").and_then(|v| v.as_array()) {
            let result: Vec<String> = text_arr.iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    _ => v.to_string().trim_matches('"').to_string(),
                })
                .collect();
            if !result.is_empty() {
                return result;
            }
        }
    }

    Vec::new()
}

/// Expand `%%N` parameter references in a vector of parameter values.
///
/// After enum resolution and value map application, some parameter values may
/// still contain `%%N` references (e.g. `%%1843` → "虚拟帐户"). These need to
/// be expanded BEFORE `substitute_format_message` because substitute() consumes
/// `%%` → `%`, which would corrupt the `%%N` marker into `%N` (a positional
/// reference), leaving the remaining digits as literal text.
fn expand_params_par_refs(
    params: Vec<String>,
    db: &std::sync::Arc<std::sync::Mutex<MessageDb>>,
) -> Vec<String> {
    let has_any_ref = params.iter().any(|p| p.contains("%%"));
    if !has_any_ref {
        return params;
    }
    let guard = match db.lock() {
        Ok(g) => g,
        Err(_) => return params,
    };
    params.into_iter().map(|p| {
        if p.contains("%%") {
            guard.expand_message(&p)
        } else {
            p
        }
    }).collect()
}

/// Write a batch of events to SQLite and/or CSV output.
fn flush_batch(
    batch: &[ParsedEvent],
    events_db: &Option<EventsDb>,
    csv_writer: &mut Option<BufWriter<File>>,
    stats: &mut StatsCollector,
) -> Result<()> {
    // Feed events into statistics collector
    for event in batch {
        stats.feed(event);
    }

    if let Some(ref db) = events_db {
        db.insert(batch)?;
    }
    if let Some(ref mut writer) = csv_writer {
        write_events(writer, batch)?;
    }
    Ok(())
}

/// Resolve the path to a messages.db file.
///
/// Priority: explicit `--msg-db` > auto-detect from `--msg-dir` > None.
fn resolve_msg_db(
    explicit: &Option<std::path::PathBuf>,
    msg_dir: &Option<std::path::PathBuf>,
    build: Option<u32>,
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

        if found.is_empty() {
            eprintln!("[warn] no hotech-msg-*.db found in '{}'", dir.display());
        } else if let Some(build_num) = build {
            if build_num == 0 {
                // Build 0 is unreliable — fall back to DB metadata
                eprintln!("[info] EVTX build reported 0 (unreliable), trying DB metadata fallback");
                if let Some(path) = resolve_by_db_metadata(&found) {
                    eprintln!("[info] resolved messages db via metadata: {path}");
                    return Ok(Some(path));
                }
                eprintln!("[warn] DB metadata fallback failed, using first available messages db");
                let path = found[0].to_string_lossy().to_string();
                return Ok(Some(path));
            }
            // Filename format: hotech-msg-{product}-{displayver}-{build}.db
            // Build number is the last component before .db
            let mut candidates: Vec<std::path::PathBuf> = Vec::new();
            for path in &found {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let stem = name.strip_suffix(".db").unwrap_or(name);
                if let Some(last_part) = stem.rsplit('-').next() {
                    if last_part.starts_with(&build_num.to_string()) {
                        candidates.push(path.clone());
                    }
                }
            }

            match candidates.len() {
                0 => {
                    eprintln!("[warn] no matching messages.db found in '{}' for build {build_num}", dir.display());
                }
                1 => {
                    let path = candidates[0].to_string_lossy().to_string();
                    eprintln!("[info] auto-matched messages db: {path}");
                    return Ok(Some(path));
                }
                _ => {
                    eprintln!("[warn] multiple candidates, using first: {:?}", candidates);
                    let path = candidates[0].to_string_lossy().to_string();
                    return Ok(Some(path));
                }
            }
        } else {
            eprintln!("[info] EVTX build unknown, trying DB metadata fallback");
            if let Some(path) = resolve_by_db_metadata(&found) {
                eprintln!("[info] resolved messages db via metadata: {path}");
                return Ok(Some(path));
            }
            eprintln!("[warn] cannot auto-detect messages.db: unknown EVTX build");
        }
    }

    Ok(None)
}

fn file_stem(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown.evtx")
        .to_string()
}

/// Read `build_number` from a hotech-msg-*.db metadata table.
///
/// Opens a SQLite connection, queries `SELECT value FROM metadata
/// WHERE key = 'build_number'`, and returns the value as a `String`.
/// Returns `None` if the file cannot be opened, the table is missing,
/// or the key is not present.
fn read_db_build_number(db_path: &str) -> Option<String> {
    let conn = rusqlite::Connection::open(db_path).ok()?;
    conn.query_row(
        "SELECT value FROM metadata WHERE key = 'build_number'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

/// Fallback: when EVTX build is 0 or unknown, determine the best-matching
/// messages DB by reading `build_number` from each candidate's metadata table.
///
/// Opens each `hotech-msg-*.db` in `found`, reads its `build_number`,
/// parses the major version (integer before the first dot), and picks
/// the DB with the highest major build number.
fn resolve_by_db_metadata(found: &[std::path::PathBuf]) -> Option<String> {
    let mut best: Option<(u64, String)> = None;
    for path in found {
        let path_str = path.to_string_lossy().to_string();
        if let Some(build_str) = read_db_build_number(&path_str) {
            let major = build_str
                .split('.')
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let is_better = match best {
                Some((best_major, _)) => major > best_major,
                None => true,
            };
            if is_better {
                best = Some((major, path_str));
            }
        }
    }
    best.map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_db(dir: &std::path::Path, filename: &str, build_number: &str) -> PathBuf {
        let db_path = dir.join(filename);
        let conn = rusqlite::Connection::open(&db_path).expect("failed to create test db");
        conn.execute(
            "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .expect("failed to create metadata table");
        conn.execute(
            "INSERT INTO metadata (key, value) VALUES ('build_number', ?1)",
            [build_number],
        )
        .expect("failed to insert build_number");
        db_path
    }

    #[test]
    fn test_read_db_build_number_found() {
        let dir = std::env::temp_dir().join("hotech-test-read-build-found");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let db_path = create_test_db(&dir, "hotech-msg-test.db", "22621");
        let result = read_db_build_number(db_path.to_str().unwrap());
        assert_eq!(result, Some("22621".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_db_build_number_missing_file() {
        assert_eq!(
            read_db_build_number(r"C:\nonexistent\hotech-msg-nonexistent.db"),
            None
        );
    }

    #[test]
    fn test_read_db_build_number_missing_table() {
        let dir = std::env::temp_dir().join("hotech-test-read-build-notable");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let db_path = dir.join("hotech-msg-notable.db");
        let conn = rusqlite::Connection::open(&db_path).expect("failed to create test db");
        // Create a table but not metadata
        conn.execute("CREATE TABLE events (id INTEGER)", [])
            .expect("failed to create events table");
        drop(conn);

        let result = read_db_build_number(db_path.to_str().unwrap());
        assert_eq!(result, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_by_db_metadata_picks_highest() {
        let dir = std::env::temp_dir().join("hotech-test-meta-pick-highest");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create two DBs with different build numbers
        let db_a = create_test_db(&dir, "hotech-msg-old.db", "17763");
        let db_b = create_test_db(&dir, "hotech-msg-new.db", "22621");

        let found = vec![db_a, db_b];
        let result = resolve_by_db_metadata(&found);

        assert!(result.is_some());
        let result_path = result.unwrap();
        assert!(
            result_path.contains("hotech-msg-new.db"),
            "expected hotech-msg-new.db, got {result_path}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_by_db_metadata_empty_list() {
        let found: Vec<PathBuf> = vec![];
        assert_eq!(resolve_by_db_metadata(&found), None);
    }

    #[test]
    fn test_resolve_msg_db_uses_metadata_fallback_when_build_zero() {
        let dir = std::env::temp_dir().join("hotech-test-fallback-zero");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let _db_path = create_test_db(&dir, "hotech-msg-10-0-22621.db", "22621");
        let msg_dir = Some(dir.clone());

        let result = resolve_msg_db(&None, &msg_dir, Some(0)).unwrap();
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(
            path.contains("hotech-msg-10-0-22621.db"),
            "expected fallback match, got {path}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_msg_db_uses_metadata_fallback_when_build_none() {
        let dir = std::env::temp_dir().join("hotech-test-fallback-none");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let _db_path = create_test_db(&dir, "hotech-msg-10-0-22621.db", "22621");
        let msg_dir = Some(dir.clone());

        let result = resolve_msg_db(&None, &msg_dir, None).unwrap();
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(
            path.contains("hotech-msg-10-0-22621.db"),
            "expected fallback match, got {path}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_msg_db_no_fallback_when_build_matches_filename() {
        let dir = std::env::temp_dir().join("hotech-test-no-fallback");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a file whose name matches build 22621
        let _db_path = create_test_db(&dir, "hotech-msg-10-0-22621.db", "22621");
        // Also create a file with higher build in metadata but wrong filename
        let _db_path2 = create_test_db(&dir, "hotech-msg-10-0-17763.db", "99999");
        let msg_dir = Some(dir.clone());

        // With explicit build=22621, should match by filename (22621), not by highest metadata
        let result = resolve_msg_db(&None, &msg_dir, Some(22621)).unwrap();
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(
            path.contains("22621"),
            "expected filename match for build 22621, got {path}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extract_subst_params_classic_data_hash_text() {
        // Classic EventLog format: {"Data": {"#text": [val1, val2, ...]}, "Binary": null}
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": ["C:\\path.msi", "2128", "(NULL)", ""]},
            "Binary": null
        }));
        let param_map: Option<HashMap<String, String>> = None;

        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result.len(), 4, "should extract #text values");
        assert_eq!(result[0], "C:\\path.msi");
        assert_eq!(result[1], "2128");
    }

    #[test]
    fn test_extract_subst_params_named_flat_still_works() {
        // Manifest-based named format still works
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "SubjectUserSid": "S-1-5-21-...",
            "SubjectUserName": "admin"
        }));
        let param_map: Option<HashMap<String, String>> = Some(HashMap::from([
            ("1".into(), "SubjectUserSid".into()),
            ("2".into(), "SubjectUserName".into()),
        ]));
        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result, vec!["S-1-5-21-...", "admin"]);
    }

    #[test]
    fn test_extract_subst_params_single_text_value() {
        // Single positional Data element
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": "single_value"},
            "Binary": null
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result, vec!["single_value"]);
    }

    #[test]
    fn test_extract_subst_params_empty_event_data() {
        let result = extract_subst_params(&None, None, &None);
        assert!(result.is_empty());
    }

    // ── Strategy 1.75: Classic Data array with param_map (named lookup) ──

    #[test]
    fn test_extract_subst_params_classic_data_array_with_param_map() {
        // Classic format: {"Data": [{"#attributes": {"Name": "Param1"}, "$": "val1"}, ...], "Binary": null}
        // With param_map → extract by name
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"#attributes": {"Name": "SubjectUserSid"}, "$": "S-1-5-21-1000"},
                {"#attributes": {"Name": "SubjectUserName"}, "$": "admin"},
                {"#attributes": {"Name": "LogonType"}, "$": "2"}
            ],
            "Binary": null
        }));
        let param_map: Option<HashMap<String, String>> = Some(HashMap::from([
            ("1".into(), "SubjectUserSid".into()),
            ("2".into(), "SubjectUserName".into()),
            ("3".into(), "LogonType".into()),
        ]));
        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result, vec!["S-1-5-21-1000", "admin", "2"]);
    }

    #[test]
    fn test_extract_subst_params_classic_data_array_with_param_map_uses_text_fallback() {
        // Some items use #text instead of $
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"#attributes": {"Name": "Param1"}, "#text": "value_from_text"},
                {"#attributes": {"Name": "Param2"}, "$": "value_from_dollar"}
            ]
        }));
        let param_map: Option<HashMap<String, String>> = Some(HashMap::from([
            ("1".into(), "Param1".into()),
            ("2".into(), "Param2".into()),
        ]));
        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result, vec!["value_from_text", "value_from_dollar"]);
    }

    #[test]
    fn test_extract_subst_params_classic_data_array_with_param_map_unmatched_params_empty() {
        // param_map has an entry (e.g. %3) that doesn't match any Data item → empty string
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"#attributes": {"Name": "Param1"}, "$": "value1"}
            ]
        }));
        let param_map: Option<HashMap<String, String>> = Some(HashMap::from([
            ("1".into(), "Param1".into()),
            ("2".into(), "Param2".into()),
        ]));
        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result, vec!["value1", ""]);
    }

    // ── Strategy 1.75: Classic Data array WITHOUT param_map (positional) ──

    #[test]
    fn test_extract_subst_params_classic_data_array_no_param_map() {
        // Without param_map, extract values positionally from Data array
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"$": "first_value"},
                {"$": "second_value"},
                {"$": "third_value"}
            ],
            "Binary": null
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result, vec!["first_value", "second_value", "third_value"]);
    }

    #[test]
    fn test_extract_subst_params_classic_data_array_no_param_map_uses_text_fallback() {
        // Without param_map, use #text if $ is not present
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"#text": "text_val1"},
                {"$": "dollar_val2"}
            ]
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result, vec!["text_val1", "dollar_val2"]);
    }

    // ── Regression: flat object format still works correctly ──

    #[test]
    fn test_extract_subst_params_flat_object_with_param_map_still_works() {
        // Manifest-based flat format must not be affected by the fix
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "SubjectUserSid": "S-1-5-21-1000",
            "SubjectUserName": "admin",
            "LogonType": "2"
        }));
        let param_map: Option<HashMap<String, String>> = Some(HashMap::from([
            ("1".into(), "SubjectUserSid".into()),
            ("2".into(), "SubjectUserName".into()),
            ("3".into(), "LogonType".into()),
        ]));
        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result, vec!["S-1-5-21-1000", "admin", "2"]);
    }

    #[test]
    fn test_extract_subst_params_flat_object_no_param_map_still_works() {
        // Flat object positional fallback must not regress
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Sid": "S-1-5-18",
            "Name": "SYSTEM"
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result, vec!["S-1-5-18", "SYSTEM"]);
    }

    // ── Regression: Strategy 1.5 (#text inside "Data") still works ──

    #[test]
    fn test_extract_subst_params_data_hash_text_still_works() {
        // The existing #text format must not be broken
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": ["C:\\path.msi", "2128", "(NULL)", ""]},
            "Binary": null
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "C:\\path.msi");
        assert_eq!(result[1], "2128");
    }

    // ── Strategy 2: flat object with "Data" key must NOT be picked up ──

    #[test]
    fn test_extract_subst_params_classic_data_array_not_leaked_to_strategy2() {
        // Without Strategy 1.75, Strategy 2 would pick up "Data" and "Binary" keys
        // as parameters, returning ["", ""]. With the fix, it correctly returns
        // positional values from the Data array.
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"$": "correct_val1"},
                {"$": "correct_val2"}
            ],
            "Binary": null
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result, vec!["correct_val1", "correct_val2"],
            "Strategy 2 must NOT extract 'Data'/'Binary' keys; Strategy 1.75 must handle this");
    }

    #[test]
    fn test_extract_subst_params_empty_data_array_returns_empty() {
        // Empty Data array → fall through to Vec::new()
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [],
            "Binary": null
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert!(result.is_empty(), "empty Data array should produce empty result");
    }

    #[test]
    fn test_extract_subst_params_data_array_items_no_value_fallback() {
        // Data items that lack both $ and #text → empty string for that position
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"#attributes": {"Name": "Param1"}},
                {"$": "val2"}
            ]
        }));
        let result = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result, vec!["", "val2"]);
    }
} // mod tests

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

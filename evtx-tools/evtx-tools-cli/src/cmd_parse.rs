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

use evtx_tools_core::parser::{detect_evtx_build, open_evtx};
use evtx_tools_core::resolver::MessageDb;
use evtx_tools_core::storage::csv::{write_csv_header, write_event_rows};
use evtx_tools_core::storage::sqlite::EventsDb;
use evtx_tools_core::storage::sqlite::{ensure_scenario_table, insert_scenario_event};
use evtx_tools_core::routing::{rule_from_yaml, SceneRouter};
use evtx_tools_core::stats::StatsCollector;
use evtx_tools_core::substitute::{substitute_format_message, apply_param_enums, apply_luid_uppercase};
use evtx_tools_core::{MessageRow, ParsedEvent};

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

    // --- Open message DB (single-file v7 only, ADR 0003; no-DB is a hard error) ---
    let target_lang = args.lang.clone();
    eprintln!("[info] message language: {target_lang}");
    let db_path = msg_db_path.ok_or_else(|| {
        "no messages db: single-file MessageDB (schema_version 7) is required; pass --msg-db or --msg-dir".to_string()
    })?;
    eprintln!("[info] loading messages db: {db_path}");
    let mut db = MessageDb::open(&db_path)?;
    db.load_param_refs(&target_lang)?;
    db.load_value_maps(&target_lang)?;
    db.load_registry_sources()?;
    db.load_variant_providers()?;
    db.load_param_types()?;
    let msg_db: Option<MessageDb> = Some(db);

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
        // Header + BOM exactly once; per-batch flush_batch only appends rows
        write_csv_header(&mut writer)?;
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
    // --with-xml: a second pass over the same file supplies each record's raw
    // XML (evtx serializes a record to JSON *or* XML, never both). Declared
    // before the event iterator and not touched afterwards — the iterator
    // holds the mutable borrow.
    let mut xml_parser = if args.with_xml {
        Some(open_evtx(&input_path, args.threads)?)
    } else {
        None
    };

    // Wrap msg_db in Arc<Mutex> for thread-safe parallel access.
    // Mutex is used because rusqlite::Connection is not Sync (contains RefCell),
    // but the lock is only held briefly per event (HashMap lookup + Clone).
    let msg_db: Option<Arc<Mutex<MessageDb>>> = msg_db.map(|db| Arc::new(Mutex::new(db)));

    const PARALLEL_BATCH: usize = 20_000;

    let mut raw_batch: Vec<(Value, u64, Option<String>)> = Vec::with_capacity(PARALLEL_BATCH);
    let mut new_variant_keys: Vec<evtx_tools_core::VariantKey> = Vec::new();
    let mut warmed_up_variants: HashSet<evtx_tools_core::VariantKey> = HashSet::new();
    // Global monotonically increasing sequence number — preserves parse order
    let mut sequence_number: u64 = 0;

    // R38: the serial producer now only iterates the evtx crate stream; the
    // 22-field JSON traversal (extract_fields, ~23% of the serial segment,
    // see probe_serial_split_tree_build_vs_extract) is deferred to
    // extract_batch_parallel and runs on the rayon pool per batch.
    use evtx_tools_core::parser::{
        extract_batch_parallel, iter_raw_json_values, iter_raw_json_values_with_xml,
    };
    let events: Box<dyn Iterator<Item = Result<(Value, u64, Option<String>)>>> =
        match &mut xml_parser {
            Some(xp) => Box::new(iter_raw_json_values_with_xml(&mut parser, xp)),
            None => Box::new(
                iter_raw_json_values(&mut parser).map(|r| r.map(|(v, rid)| (v, rid, None))),
            ),
        };
    for result in events {
        let raw = match result {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[warn] skipping event: {e}");
                continue;
            }
        };

        raw_batch.push(raw);

        if raw_batch.len() >= PARALLEL_BATCH {
            let mut batch = extract_and_prepare_batch(
                &mut raw_batch, &file_name, &msg_db,
                &mut warmed_up_variants, &mut new_variant_keys, &mut sequence_number,
            );
            process_parallel_batch(
                &msg_db, &mut new_variant_keys, &target_lang, &mut batch,
            );
            let event_vec: Vec<ParsedEvent> = batch.drain(..).map(|(e, _)| e).collect();
            if tx.send(event_vec).is_err() {
                eprintln!("[error] writer thread terminated unexpectedly");
                break;
            }
        }
    }

    // Flush remaining partial batch
    if !raw_batch.is_empty() {
        let mut batch = extract_and_prepare_batch(
            &mut raw_batch, &file_name, &msg_db,
            &mut warmed_up_variants, &mut new_variant_keys, &mut sequence_number,
        );
        process_parallel_batch(
            &msg_db, &mut new_variant_keys, &target_lang, &mut batch,
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

/// R38: run the per-record extract stage on the rayon pool
/// ([`evtx_tools_core::parser::extract_batch_parallel`]), then assign
/// sequence numbers and collect variant warmup keys in stream order —
/// the returned batch is index-ordered, so this preserves the previous
/// serial-producer semantics exactly.
fn extract_and_prepare_batch(
    raw_batch: &mut Vec<(Value, u64, Option<String>)>,
    file_name: &str,
    msg_db: &Option<Arc<Mutex<MessageDb>>>,
    warmed_up_variants: &mut HashSet<evtx_tools_core::VariantKey>,
    new_variant_keys: &mut Vec<evtx_tools_core::VariantKey>,
    sequence_number: &mut u64,
) -> Vec<(ParsedEvent, Option<Value>)> {
    let mut batch = evtx_tools_core::parser::extract_batch_parallel(
        std::mem::take(raw_batch),
        file_name,
    );
    for (event, _) in batch.iter_mut() {
        // Assign global sequence number in parse order
        event.sequence_number = Some(*sequence_number);
        *sequence_number += 1;

        // Collect variant warmup keys (single-file v7: warmup_variants is the only path).
        if msg_db.is_some() {
            if let (Some(ref prov), Some(eid)) = (event.provider_name.clone(), event.event_id) {
                // Variant-aware warmup: collect VariantKey when version/level/opcode etc. present
                // level/opcode as u8 parsed from System; task/keywords as numeric where possible.
                let ver = event.version.unwrap_or(0);
                let lvl = event.level.unwrap_or(0);
                let opc = event.opcode.as_deref().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
                let task = event.task_category.as_deref().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
                let kw = event.keywords.as_deref().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                let ch = channel_to_u8(event.channel.as_deref());
                // We don't yet know message_id variant; warmup uses channel/keywords as filter but
                // actual variant resolution will use message_id as well. For now collect a probe key
                // with message_id=0 to trigger v6 warmup of the (provider, eid, ver, level, opcode, task, kw, ch) cohort.
                // Real precise fetch is lazy per-event via get_variant probe; warmup is best-effort cohort.
                // So we collect a variant key per event for cohort loading.
                let vkey = evtx_tools_core::VariantKey {
                    provider_name: prov.clone(),
                    event_id: eid,
                    version: ver,
                    level: lvl,
                    opcode: opc,
                    task,
                    keywords: kw,
                    channel: ch,
                    message_id: 0, // wildcard probe — warmup will OR-expand
                };
                if warmed_up_variants.insert(vkey.clone()) {
                    new_variant_keys.push(vkey);
                }
            }
        }
    }
    batch
}

fn channel_to_u8(channel: Option<&str>) -> u8 {
    match channel {
        Some("Application") => 0, // placeholder — actual channel id from CRIM is numeric; we keep 0 as wildcard
        Some(s) => s.parse::<u8>().unwrap_or(0),
        None => 0,
    }
}

/// R10 typed rendering (ADR 0005) — format a parameter per its TDH
/// (InType, OutType) bytes so the output matches wevtapi's TdhFormatProperty.
///
/// Rules are evidence-derived (2026-09-07 probe of live CRIM templates vs
/// wevtutil baseline); unobserved pairs pass through unchanged:
///   (14,12)/(08,12) HEXINT32→HEXINT32-out : "0x%X" uppercase
///   (15,13) HEXINT64→HEXINT64-out        : "0x%X" uppercase
///   (0a,13) UINT64→HEXINT64-out          : "0x%x" lowercase
///   (07,20)/(14,20) INT32→HRESULT-out    : FormatMessage of the HRESULT
///   (11,02) FILETIME→DATETIME-out        : 9-digit nanosecond fraction
///   (08,08) UINT32→UNSIGNEDINT-out       : decimal (maps may still apply)
///
/// Returns the rewritten params plus the set of param indices whose rendering
/// is fully owned by this layer (callers must skip enum/value-map layers for
/// those; the decimal rule does NOT take over — maps still apply).
fn apply_param_types(
    mut params: Vec<String>,
    types: &HashMap<String, String>,
    param_map_refs: &HashMap<String, String>,
    db: &Arc<Mutex<MessageDb>>,
    _provider: &str,
) -> (Vec<String>, HashSet<String>) {
    if types.is_empty() {
        return (params, HashSet::new());
    }
    let hresult_lookup = |code: u32| -> Option<String> {
        let guard = db.lock().ok()?;
        guard.lookup_win32_error(code)
    };
    let ntstatus_lookup = |code: u32| -> Option<String> {
        let guard = db.lock().ok()?;
        guard.lookup_ntstatus(code)
    };
    let mut owned: HashSet<String> = HashSet::new();
    let trace = std::env::var("EVTX_TRACE_TYPED").is_ok();
    for (i, value) in params.iter_mut().enumerate() {
        let idx = (i + 1).to_string();
        let Some(spec) = types.get(&idx) else { continue };
        let parsed = parse_type_spec(spec);
        if trace {
            eprintln!("[typed] idx={idx} spec={spec:?} parsed={parsed:?} val={value:?}");
        }
        let Some((intype, outtype)) = parsed else { continue };
        let Some((intype, outtype)) = parse_type_spec(spec) else { continue };
        let (new_value, takes_over) = match (intype, outtype) {
            (0x14, 0x12) | (0x08, 0x12) | (0x15, 0x13) => {
                // R29（2026-09-30）：success 严重级（bit30-31=00）且 code=0 的
                // facility-only HRESULT/NTSTATUS（如 Wininit 23 的 0x10000000），
                // wevtutil 渲染符号名 STATUS_SUCCESS（live 实证）。hit-only：
                // NTSTATUS 表查到才接管；value=0 不走此臂（经 (_,0x1F) 臂或
                // 原有路径闭合，避免对全 0 hex 参数引入行为变化）。
                let sym = parse_num(value)
                    .map(|v| v as u32)
                    .filter(|vu| *vu != 0 && *vu >> 30 == 0 && *vu & 0xFFFF == 0)
                    .and_then(|_| (ntstatus_lookup)(0))
                    .filter(|t| !t.contains('%'));
                let out = sym.unwrap_or_else(|| {
                    parse_num(value)
                        .map(|v| format!("0x{v:X}"))
                        .unwrap_or_else(|| value.clone())
                });
                (out, true)
            }
            (0x0A, 0x13) => {
                (parse_num(value).map(|v| format!("0x{v:x}")).unwrap_or_else(|| value.clone()), true)
            }
            (0x07, 0x20) | (0x14, 0x20) => (format_hresult(value, &hresult_lookup, &ntstatus_lookup), true),
            // NTSTATUS (any InType, OutType 0x1F) — wevtapi/TdhFormatProperty
            // resolves the raw code through ntdll's message table (Phase E:
            // Ntfs 140 `Failure status` → "A device which does not exist was
            // specified."). Hit-only: unknown codes / texts carrying %N insert
            // placeholders keep the passthrough rendering (not owned), so
            // unobserved pairs are unaffected.
            (_, 0x1F) => {
                let text = parse_num(value)
                    .map(|v| v as u32)
                    .and_then(|code| (ntstatus_lookup)(code))
                    .filter(|t| !t.contains('%'));
                match text {
                    Some(t) => (t, true),
                    None => (value.clone(), false),
                }
            }
            (0x11, 0x02) => (format_datetime_ns(value), true),
            // GUID (15,14) — wevtapi renders lowercase with braces (KGA 11
            // TmId/RmId, DriverFrameworks CLSID; raw event value is already
            // braced-lowercase). Owned: later GUID-normalizing layers would
            // uppercase and un-brace it.
            (0x0F, 0x0E) => {
                let t = value.trim().trim_matches(|c| c == '{' || c == '}');
                (format!("{{{}}}", t.to_ascii_lowercase()), true)
            }
            // BOOLEAN (13,*) — raw "true"/"false" passes through; TDH ignores
            // maps for booleans, so the field must be taken over from the
            // enum/value-map layers (Wininit/LsaSrv "Disabled"-vs-"false").
            (0x0D, _) => (value.clone(), true),
            (0x08, 0x08) => {
                // wevtapi renders plain UINT32 decimal even when the stored
                // string is hex (Time-Service 35 TimeSourceRefId). Value maps
                // still apply afterwards when linked, so ownership depends on
                // the map link: without one the Layer-1 hex heuristic would
                // otherwise re-hexify the decimal (2026-09-08 probe).
                let has_map = param_map_refs.get(&idx).is_some();
                (parse_num(value).map(|v| v.to_string()).unwrap_or_else(|| value.clone()), !has_map)
            }
            _ => (value.clone(), false),
        };
        if takes_over {
            owned.insert(idx);
        }
        *value = new_value;
    }
    if trace && !owned.is_empty() {
        eprintln!("[typed] owned={owned:?}");
    }
    (params, owned)
}

fn parse_type_spec(spec: &str) -> Option<(u8, u8)> {
    let (a, b) = spec.split_once(':')?;
    Some((u8::from_str_radix(a, 16).ok()?, u8::from_str_radix(b, 16).ok()?))
}

/// Parse "0x…"/"0X…"/decimal strings (TDH InTypes arrive as strings in the
/// evtx JSON) into u64.
fn parse_num(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        t.parse::<u64>().ok()
    }
}

fn format_hresult(
    value: &str,
    lookup: &dyn Fn(u32) -> Option<String>,
    ntstatus: &dyn Fn(u32) -> Option<String>,
) -> String {
    let Some(v) = parse_num(value) else { return value.to_string() };
    let v = v as u32;
    // FACILITY_WIN32 HRESULTs (0x8007xxxx) carry the Win32 code in the low
    // word; plain small values are Win32 codes already (0 → success message).
    let code = if v < 0x1_0000 {
        v
    } else if v & 0xFFFF_0000 == 0x8007_0000 {
        v & 0xFFFF
    } else {
        // R29（2026-09-30）：success 严重级（bit30-31=00）且 code=0 的 facility-only
        // HRESULT（Wininit 23 的 0x10000000），wevtutil 渲染符号名 STATUS_SUCCESS
        //（live 实证）。hit-only：NTSTATUS 表查到才接管，未命中保持 0x%08X。
        if v >> 30 == 0 && v & 0xFFFF == 0 {
            if let Some(t) = ntstatus(0) {
                if !t.contains('%') {
                    return t;
                }
            }
        }
        return format!("0x{v:08X}");
    };
    lookup(code).unwrap_or_else(|| format!("0x{v:08X}"))
}

/// FILETIME/DATETIME-out renders a 9-digit nanosecond fraction (100ns FILETIME
/// ticks ×100); the evtx crate's own string only carries µs (6 digits).
fn format_datetime_ns(value: &str) -> String {
    let dot = match value.find('.') {
        Some(d) => d,
        None => return value.to_string(),
    };
    let rest = &value[dot + 1..];
    let frac_end = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    let (frac, tail) = rest.split_at(frac_end);
    if frac.is_empty() || frac.len() >= 9 {
        return value.to_string();
    }
    let padded = format!("{:0<9}", frac);
    format!("{}.{}{}", &value[..dot], padded, tail)
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
/// 2. **Batch resolve** (single lock pass for the whole batch): each event's
///    variant ladder, R26 Qualifier probe, value maps and `%%N`-expanded
///    template are resolved under ONE lock acquisition, with rows handed out
///    as cheap `Arc<MessageRow>` clones — the deep per-event row clone and
///    the 2M-event lock stampede this path used to pay are gone.
/// 3. **Parallel render** (lock-free): JSON traversal, `substitute`, metadata
///    copy run without touching the cache — all cores work simultaneously.
///    Only the rare miss-fallback path takes per-event locks (R12/R21/R29).
fn process_parallel_batch(
    msg_db: &Option<Arc<Mutex<MessageDb>>>,
    new_variant_keys: &mut Vec<evtx_tools_core::VariantKey>,
    target_lang: &str,
    batch: &mut [(ParsedEvent, Option<Value>)],
) {
    if batch.is_empty() {
        return;
    }

    // Single-file v7 (ADR 0003): msg_db is always Some — no-DB exits at run_parse.
    let db = msg_db.as_ref().expect("messages db required");

    // --- 1. Warmup (lock held, single-threaded; variants + param value-map refs) ---
    // R9b（2026-09-04）：必须用 warmup() 而非 warmup_variants()。后者只填模板行，
    // 不挂 param_map_refs（param_value_maps 链接），导致 Layer B 永远为空——
    // Kernel-Power 566 Reason=12 无法展开 SessionUnlock（190 条 D）。
    if !new_variant_keys.is_empty() {
        if let Ok(mut guard) = db.lock() {
            let _ = guard.warmup(&new_variant_keys, target_lang);
        } else {
            eprintln!("[warn] msg_db lock poisoned during warmup");
        }
        new_variant_keys.clear();
    }

    // --- 2. Parallel message resolution ---
    // Lock held only briefly to clone MessageRow; substitution + metadata copy
    // run without the lock so all rayon threads execute simultaneously.
    // ---- 2. Batch resolve: one lock pass for the whole batch ----
    struct PreResolved {
        row: std::sync::Arc<MessageRow>,
        /// Template with %%N refs already expanded (lock needed for that
        /// lookup) — None when the template has no %%N refs at all.
        template_expanded: Option<String>,
        value_maps: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
        bitmap_names: std::sync::Arc<std::collections::HashSet<String>>,
        event_bitmaps: std::collections::HashSet<String>,
    }

    let pre: Vec<Option<PreResolved>> = match db.lock() {
        // Poisoned lock: mark every event as unresolved — the render phase's
        // None arm runs the same fallback path the per-event resolve used to
        // take on a failed lock.
        Err(_) => {
            eprintln!("[warn] msg_db lock poisoned during batch resolve");
            (0..batch.len()).map(|_| None).collect::<Vec<_>>()
        }
        Ok(guard) => {
        // Global bitmap-name set is immutable after load: clone the Arc once
        // per batch instead of deep-cloning the set per event.
        let global_bitmaps = guard.get_bitmap_names();
        batch.iter().map(|(event, _)| {
            let provider = event.provider_name.as_ref()?;
            let provider_str = provider.as_str();
            let eid = event.event_id?;
            // Single-file v7: 8-tuple precise via resolve_variant (no availability check).
            let try_variant = || -> Option<std::sync::Arc<MessageRow>> {
                let ver = event.version.unwrap_or(0);
                let lvl = event.level.unwrap_or(0);
                let opc = event.opcode.as_deref().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
                let task = event.task_category.as_deref().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
                let kw = event.keywords.as_deref().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                let ch = channel_to_u8(event.channel.as_deref());
                // message_id not yet known — try variant cache via (provider,eid) fallback ladder
                // We probe with message_id=0 and let resolve_variant ladder degrade
                let key = evtx_tools_core::VariantKey {
                    provider_name: provider_str.to_string(),
                    event_id: eid,
                    version: ver,
                    level: lvl,
                    opcode: opc,
                    task,
                    keywords: kw,
                    channel: ch,
                    message_id: 0,
                };
                guard.resolve_variant(&key)
            };
            // Single-file v7: variants only, no legacy single-template fallback.
            let mut row = try_variant()?;
            // R26（2026-09-30）：classic Qualifier 全键探针。事件 EventID 的 Qualifiers
            // 属性参与消息表查找键（(q<<16)|eid，wevtapi 语义）；msgdb 经 R26 起保留
            // 全格式 message_identifier，同 eid 姊妹条目（链首配额 vs 链尾启动）由此
            // 精确区分，压过通配梯子的猜测。未命中保持梯子结果。
            if let Some(q) = event.qualifiers.filter(|q| *q != 0) {
                let full = ((q as u32) << 16) | eid;
                if full != eid {
                    // 注意：此处位于 resolved 闭包内，外层 guard 已持有 db 锁——
                    // 严禁二次 db.lock()（std Mutex 不可重入，重入即死锁，
                    // 2026-09-30 实测 05 全量挂死根因）。
                    let key2 = evtx_tools_core::VariantKey {
                        provider_name: provider_str.to_string(),
                        event_id: eid,
                        version: event.version.unwrap_or(0),
                        level: event.level.unwrap_or(0),
                        opcode: event.opcode.as_deref().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0),
                        task: event.task_category.as_deref().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0),
                        keywords: event.keywords.as_deref().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0),
                        channel: channel_to_u8(event.channel.as_deref()),
                        message_id: full,
                    };
                    if let Some(exact) = guard.resolve_variant_full_id(&key2, full) {
                        row = exact;
                    }
                }
            }
            // Layer A: legacy name-keyed value maps (map_name == param name).
            let mut vms = row.parameters.as_ref()
                .map(|pm| guard.get_param_value_maps(pm))
                .unwrap_or_default();
            // Layer B: event-scoped param→map references (param_value_maps).
            // Precise association from CRIM TEMP item +8; takes precedence.
            let (event_vms, event_bitmaps) = guard.resolve_param_value_maps(&row);
            for (field, entries) in event_vms {
                vms.insert(field, entries);
            }
            // %%N template expansion needs the param_refs caches — resolve it
            // here under the already-held lock instead of re-locking per event.
            let template_expanded = if row.template.contains("%%") {
                Some(guard.expand_message(&row.template, provider_str))
            } else {
                None
            };
            Some(PreResolved {
                row,
                template_expanded,
                value_maps: vms,
                bitmap_names: std::sync::Arc::clone(&global_bitmaps),
                event_bitmaps,
            })
        })
        .collect::<Vec<_>>()
        }
    };

    // ---- 3. Parallel render (lock-free on the hit path) ----
    batch.par_iter_mut().zip(pre.into_par_iter()).for_each(|((event, event_data_value), resolved)| {
        let row: PreResolved = match resolved {
            Some(r) => r,
            None => {
                // R12（ADR 0004）：miss 且注册快照已知且源未注册时，wevtutil 对
                // EID!=0 一律渲染 N/A（2026-09-07 全量取证 101/101：AppX 2562×86、
                // EdgeWebView 256×11 等）。EID=0 与 classic/winevt miss 走既有路径。
                let eid = event.event_id.unwrap_or(0);
                if eid != 0 {
                    if let Some(prov) = event.provider_name.as_deref() {
                        let (known, registered, has_variants) = match db.lock() {
                            Ok(g) => (
                                g.registry_known(),
                                g.is_registered_source(prov),
                                g.has_any_variant(prov),
                            ),
                            Err(_) => (false, true, true),
                        };
                        if known && !registered {
                            // R29（2026-09-30）：纯 classic 未注册（事件无 Provider @Guid）
                            // 时 wevtutil 把 eid 当 Win32 错误码渲染（CodexSandbox 1001=
                            // "递归太深；堆栈溢出。"、1002 live 实证，事件 Data 被忽略；
                            // eid 1000 无对应文本 → 两侧同为 N/A）。查得到才渲染，
                            // 未命中维持 N/A；带 @Guid 的未注册事件保持 R12 N/A 语义。
                            if event.provider_guid.is_none() {
                                if let Ok(g) = db.lock() {
                                    if let Some(text) = g.lookup_win32_error_fallback(eid) {
                                        event.message = Some(text);
                                        return;
                                    }
                                }
                            }
                            event.message = Some("N/A".to_string());
                            return;
                        }
                        // R12b（2026-09-30）registered-but-no-resources：已注册但
                        // provider_message_variants 无任何行（如
                        // Microsoft-Windows-Configuration-Change-Monitor，System
                        // 通道 eid=1 ×80，基线渲染全 N/A）→ wevtapi
                        // EvtFormatMessage 无消息资源可解析，同样渲染 N/A。
                        // 既有「未注册→N/A」语义完全不变；表空=未知=保守不触发。
                        if known && registered && !has_variants {
                            event.message = Some("N/A".to_string());
                            return;
                        }
                    }
                }
                // R21（2026-09-29）：classic eid=0 miss → FormatMessage(0)。wevtapi 对
                // 无模板可解析的经典 eid=0 事件渲染系统错误 0 文本（zh "操作成功完成。"），
                // 事件自带 Data 参数不参与渲染（孤儿 provider WXWorkUpgrader/asus/
                // PDFShellExt/igfx/AsusUpdateCheck/RtkAudio 共 177 条 live 实证 100%）。
                if eid == 0 {
                    if let Ok(g) = db.lock() {
                        if let Some(text) = g.lookup_win32_error0() {
                            event.message = Some(text);
                            return;
                        }
                    }
                }
                // Cache miss: for unregistered providers with Data.#text, emit ;;-joined Data like evtx2csv (Lua aevtx) for 100% Match
                if let Some(v) = event_data_value.as_ref() {
                    if let Some(obj) = v.as_object() {
                        if let Some(data_obj) = obj.get("Data").and_then(|x| x.as_object()) {
                            if let Some(text_val) = data_obj.get("#text") {
                                let text = match text_val {
                                    serde_json::Value::Array(arr) => arr.iter().map(|x| match x {
                                        serde_json::Value::String(s) => s.as_str(),
                                        _ => "",
                                    }).collect::<Vec<_>>().join(";;"),
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => String::new(),
                                };
                                // evtx2csv appends trailing ;; and no header/trailer
                                let msg = if text.is_empty() { String::new() } else { format!("{};;", text) };
                                // Include Binary hex dump if present (like vmx86 0x... case)
                                let binary_suffix = obj.get("Binary").and_then(|b| b.as_str()).map(|b| format!(";{};;", b)).unwrap_or_default();
                                if !msg.is_empty() || !binary_suffix.is_empty() {
                                    event.message = Some(format!("{}{}", msg, binary_suffix));
                                    return;
                                }
                            }
                        }
                    }
                }
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
                        true,
                    ),
                );
                return;
            }
        };

        // ---- Heavy work (no lock on the hit path): substitute + copy metadata ----
        // Step 1: %%N references were already expanded under the phase-1 lock
        // (expand_message), because substitute() consumes %% → % (literal
        // percent), which destroys the %%NNNN marker expand_param_refs needs.
        let template = match row.template_expanded {
            Some(ref t) => t.clone(),
            None => row.row.template.clone(),
        };

        // Step 2: Substitute %1–%99 positional parameters.
        let mut owned_values: std::collections::HashSet<String> = std::collections::HashSet::new();
        if template.contains('%') {
            // P3：模板命中路径开启 classic Binary→%n+1（NetBT 4311 hex 参数）。
            // miss fallback 路径保持 false（evtx2csv 兼容 ;;-join 语义不变）。
            let params = extract_subst_params_ext(
                &event.event_data,
                event_data_value.as_ref(),
                &row.row.parameters,
                true,
            );
            let prov = event.provider_name.as_deref().unwrap_or("");
            // R10: typed rendering first. Fields with a TDH (InType, OutType)
            // pair take over from the heuristic layers below (TDH only applies
            // value maps to plain integer InTypes, never to hex/bool/hresult).
            let (params, typed_fields) = apply_param_types(
                params,
                &row.row.param_types,
                &row.row.param_map_refs,
                &db,
                prov,
            );
            // Owned params' final values — captured NOW (indices still valid);
            // protects them from the display layer's decimal→hex conversion.
            owned_values = typed_fields
                .iter()
                .filter_map(|idx| {
                    let i: usize = idx.parse().ok()?;
                    i.checked_sub(1).and_then(|k| params.get(k)).cloned()
                })
                .collect();
            // typed_fields are param INDICES ("5") — convert to FIELD NAMES via
            // the parameters map; the downstream layers key by field name.
            // (2026-09-08 fix: unioning indices with names silently no-oped.)
            let param_names = row.row.parameters.clone().unwrap_or_default();
            let typed_fields: std::collections::HashSet<String> = typed_fields
                .iter()
                .filter_map(|idx| param_names.get(idx).cloned())
                .collect();
            // Apply Layer 1 enum resolution (standard type conventions).
            // R11：有真 map 的字段跳过布尔启发式（row.value_maps key 即字段名），让路 Layer 2。
            let skip: std::collections::HashSet<String> =
                row.value_maps.keys().cloned().collect();
            let skip: std::collections::HashSet<String> =
                skip.union(&typed_fields).cloned().collect();
            let params = apply_param_enums(&params, &row.row.parameters, &skip);
            // Apply Layer 2: CRIM VMAP/BMAP value map resolution — typed fields
            // must not be re-mapped (wevtapi ignores maps for hex/bool InTypes).
            let mut vms = row.value_maps.clone();
            for field in &typed_fields {
                vms.remove(field);
            }
            let bitmap_union: std::collections::HashSet<String>;
            let bitmap_names: &std::collections::HashSet<String> =
                if row.event_bitmaps.is_empty() {
                    &row.bitmap_names
                } else {
                    bitmap_union = row.bitmap_names.union(&row.event_bitmaps).cloned().collect();
                    &bitmap_union
                };
            let params = evtx_tools_core::substitute::apply_value_maps(&params, &row.row.parameters, &vms, bitmap_names);
            // Apply Layer 2b: LUID fields (*LogonId) render uppercase hex (wevtutil gold standard).
            let params = apply_luid_uppercase(&params, &row.row.parameters);
            // Expand %%N parameter references in parameter VALUES
            // (the template was already expanded above; param values may also
            // contain %%N refs, e.g. %%1843 → "虚拟帐户", which must be resolved
            // before substitute_format_message consumes %% → %)
            let params = expand_params_par_refs(params, db, prov);
            let param_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            // R32（2026-09-30）：classic 门控——事件 EventID 带 Qualifiers 属性 =
            // classic 消息表 provider（R26 同款金标），零参数时占位符字面量透传
            // （FormatMessage 语义，WPD 24577 实证）；manifest/CRIM provider 缺参
            // 渲染空（VSS 8224 等 233 条全量实证）。
            event.message = Some(if event.qualifiers.is_some() {
                evtx_tools_core::substitute::substitute_format_message_protected_classic(
                    &template, &param_refs, &owned_values,
                )
            } else {
                evtx_tools_core::substitute::substitute_format_message_protected(
                    &template, &param_refs, &owned_values,
                )
            });
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
        // Use level from MessageRow if EVTX event doesn't have level or has default level (0)
        if event.level.is_none() || event.level == Some(0) {
            if let Some(level) = row.row.level {
                event.level = Some(level);
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
            event.message = Some(evtx_tools_core::substitute::format_for_display_protected(
                &msg, &owned_values,
            ));
        }
    });
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
    extract_subst_params_ext(event_data_str, event_data_value, param_map, false)
}

/// P3（2026-09-30）：classic Binary hex 参数化。`include_binary=true` 时，EventData
/// 携带非空 `<Binary>` 的经典事件把其十六进制文本（wevtutil 原样大写、无 0x 无分隔）
/// 作为追加位置参数 `%n+1`（n=Data 值个数）暴露给模板。wevtapi 实证：NetBT 4311
/// 模板 `Use the string "%2" ...` 渲染 Binary hex（raw XML `<Data/>+<Binary>`，
/// Data=[""]→%1，Binary→%2）；e1dexpress 32（`%2%n…`，Binary=%3 未引用）同构佐证。
/// 全量回归面排查（baseline Binary 事件 1017+580 条 × msgdb 模板 max_param）：
/// 仅有 NetBT 4311 与 VMnetAdapter 36 模板引用 %n+1 槽位，其余簇 max_param<=data
/// 不受影响；substitute 对越界参数空替换，追加对未引用模板无输出差。
fn extract_subst_params_ext(
    event_data_str: &Option<String>,
    event_data_value: Option<&Value>,
    param_map: &Option<HashMap<String, String>>,
    include_binary: bool,
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
                        Some(param_name) => {
                            // Try exact, then case-insensitive, then suffix/alias match
                            // Alias: TTBL uses currentPackageState but UserData has Initial/Intended
                            let lname = param_name.to_lowercase();
                            let alias = match lname.as_str() {
                                "identifier" => Some("packageidentifier"),
                                "currentpackagestate" => Some("initialpackagestate"),
                                "currentpackagestatetextized" => Some("initialpackagestatetextized"),
                                "targetpackagestate" => Some("intendedpackagestate"),
                                "targetpackagestatetextized" => Some("intendedpackagestatetextized"),
                                _ => None,
                            };
                            obj.get(param_name.as_str())
                                .or_else(|| {
                                    let lower = param_name.to_lowercase();
                                    obj.keys().find(|k| k.to_lowercase() == lower).and_then(|k| obj.get(k.as_str()))
                                })
                                .or_else(|| {
                                    // For Setup servicing: TTBL %2=currentPackageState expects "Staged" but alias gives 5064.
                                    // Prefer the Textized companion (InitialPackageStateTextized) when param is a numeric state.
                                    if lname == "currentpackagestate" || lname == "targetpackagestate" {
                                        let textized_alias = match lname.as_str() {
                                            "currentpackagestate" => "initialpackagestatetextized",
                                            "targetpackagestate" => "intendedpackagestatetextized",
                                            _ => "",
                                        };
                                        if let Some(v) = obj.keys().find(|k| k.to_lowercase() == textized_alias).and_then(|k| obj.get(k.as_str())) {
                                            return Some(v);
                                        }
                                        let tkey = lname.clone() + "textized";
                                        if let Some(v) = obj.keys().find(|k| k.to_lowercase() == tkey).and_then(|k| obj.get(k.as_str())) {
                                            return Some(v);
                                        }
                                    }
                                    None
                                })
                                .or_else(|| {
                                    if let Some(a) = alias {
                                        if let Some(v) = obj.keys().find(|k| k.to_lowercase() == a).and_then(|k| obj.get(k.as_str())) {
                                            return Some(v);
                                        }
                                    }
                                    None
                                })
                                .or_else(|| {
                                    let lower = param_name.to_lowercase();
                                    obj.keys().find(|k| k.to_lowercase().ends_with(&lower) || lower.ends_with(&k.to_lowercase())).and_then(|k| obj.get(k.as_str()))
                                })
                                .and_then(|v| match v {
                                    Value::String(s) => Some(s.clone()),
                                    Value::Number(n) => Some(n.to_string()),
                                    // R18: the evtx crate emits JSON-typed booleans
                                    // for manifest boolean params (Kernel-Boot 20
                                    // "true"/"false") — they must render, not vanish.
                                    Value::Bool(b) => Some(b.to_string()),
                                    _ => None,
                                })
                                .unwrap_or_default()
                        },
                        None => String::new(),
                    };
                    result.push(value);
                }
                return result;
            }
        }
    }

    // P3：classic Binary hex → %n+1。仅 include_binary（模板命中路径）时追加；
    // wevtapi 语义：Data 值占 %1..%n，Binary hex 追加为 %n+1（NetBT 4311 %2 实证）。
    let binary_hex: Option<String> = if include_binary {
        parsed.as_object()
            .and_then(|o| o.get("Binary"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    } else {
        None
    };

    // Strategy 1.5: Classic EventLog #text array nested under "Data".
    // Format: {"Data": {"#text": [val1, val2, ...]}, "Binary": null}
    // Classic providers (MsiInstaller, MSSQLSERVER, etc.) store substitution
    // values positionally in a #text array within "Data". This is detected
    // BEFORE Strategy 2's flat-object fallback which would otherwise return
    // the raw {"Data": {...}, "Binary": null} object as-is.
    if let Some(obj) = parsed.as_object() {
        if let Some(data_obj) = obj.get("Data").and_then(|v| v.as_object()) {
            if let Some(text_val) = data_obj.get("#text") {
                let mut values: Vec<String> = match text_val {
                    Value::Array(arr) => arr.iter().map(|v| match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    }).collect(),
                    // R34（2026-09-30，修正版）：空 `<EventData></EventData>` 被
                    // evtx-crate 序列化为空串 #text——不算参数（wevtutil 对空
                    // EventData 渲染占位符字面量，WPD 24577 实证）。但 **有 Binary
                    // 时空串 Data 是真实参数**（NetBT 4311 `<Data/>+<Binary>`：
                    // Data=[""]→%1 空、Binary→%2，P3 实证），须保留否则 Binary
                    // 追加被 values.is_empty() 短路（09-30 回归 54 条根因）。
                    Value::String(s) => {
                        if s.is_empty() && binary_hex.is_none() { Vec::new() } else { vec![s.clone()] }
                    }
                    _ => Vec::new(),
                };
                if !values.is_empty() {
                    if let Some(hex) = &binary_hex {
                        values.push(hex.clone());
                    }
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
                let mut values: Vec<String> = data_arr.iter().map(|item| {
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
                    if let Some(hex) = &binary_hex {
                        values.push(hex.clone());
                    }
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
    // R33（2026-09-30）：跳过 `#` 前缀序列化伪键（#attributes 等）——SAM 16962 的
    // EventData 平铺对象 {"#attributes":{…},"Default SD String:":"O:SY…"} 中，伪键
    // 对象值被当数据推成空串占住 %1，真实值挤到 %2（模板 %1 渲染空的根因）。
    if let Some(obj) = parsed.as_object() {
        if !obj.contains_key("#text") && !obj.contains_key("Data") {
            let values: Vec<String> = obj.iter()
                .filter(|(k, _)| !k.starts_with('#'))
                .map(|(_k, v)| match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => String::new(),
                })
                .collect();
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
    provider_name: &str,
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
            guard.expand_message(&p, provider_name)
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
        write_event_rows(writer, batch)?;
    }
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

fn file_stem(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown.evtx")
        .to_string()
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

    // ── P4: 嵌套 UserData 展平后（parser.rs），取值层按 msgdb 63 参数表命中 9 槽 ──
    // Resource-Exhaustion-Detector 2004 模板：%21 (%22) 使用了 %24 字节；…（Process_1..3）
    #[test]
    fn test_extract_subst_params_nested_userdata_resource_exhaustion() {
        use serde_json::json;
        // parser.rs 展平产物（真实 event_data 形态，键与 msgdb 参数表同名）
        let ed_val: Option<serde_json::Value> = Some(json!({
            "SystemCommitLimit": "65288781824",
            "SystemCommitCharge": "65226944512",
            "PagedPoolUsage": "6296014848",
            "Tag_1_Name": "Toke",
            "Tag_1_PoolUsed": "2343021056",
            "Process_1_Name": "agent-win.exe",
            "Process_1_ID": "18268",
            "Process_1_CreationTime": "2026-09-07T08:24:56.170740Z",
            "Process_1_CommitCharge": "7402741760",
            "Process_1_HandleCount": "302",
            "Process_1_Version": "1.3.11.0",
            "Process_1_TypeInfo": "201",
            "Process_2_Name": "Quicker.exe",
            "Process_2_ID": "7860",
            "Process_2_CommitCharge": "1702064128",
            "Process_3_Name": "ZCode.exe",
            "Process_3_ID": "36156",
            "Process_3_CommitCharge": "801992704",
            "Time": "2026-09-09T15:00:29.655227Z"
        }));
        // msgdb provider_message_variants.parameters 真实切片（同键序无关）
        let param_map: Option<HashMap<String, String>> = Some(HashMap::from([
            ("21".into(), "Process_1_Name".into()),
            ("22".into(), "Process_1_ID".into()),
            ("23".into(), "Process_1_CreationTime".into()),
            ("24".into(), "Process_1_CommitCharge".into()),
            ("28".into(), "Process_2_Name".into()),
            ("29".into(), "Process_2_ID".into()),
            ("31".into(), "Process_2_CommitCharge".into()),
            ("35".into(), "Process_3_Name".into()),
            ("36".into(), "Process_3_ID".into()),
            ("38".into(), "Process_3_CommitCharge".into()),
        ]));
        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result.len(), 38, "vec must cover the highest param index");
        // 模板引用的 9 槽（0 基 = %N-1）全部取到真实值
        assert_eq!(result[20], "agent-win.exe");   // %21
        assert_eq!(result[21], "18268");           // %22
        assert_eq!(result[23], "7402741760");      // %24
        assert_eq!(result[27], "Quicker.exe");     // %28
        assert_eq!(result[28], "7860");            // %29
        assert_eq!(result[30], "1702064128");      // %31
        assert_eq!(result[34], "ZCode.exe");       // %35
        assert_eq!(result[35], "36156");           // %36
        assert_eq!(result[37], "801992704");       // %38
    }

    // ── P3: classic Binary → %n+1（wevtapi 实证：NetBT 4311 模板 %2=Binary hex）──

    #[test]
    fn test_binary_appended_after_hash_text_when_gated_on() {
        // NetBT 4311 实证形态：Data=[""] + Binary hex → ["", hex]（hex 落 %2）
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": [""]},
            "Binary": "000000000100320000000000D710004013010000250200C000000000000000000000000000000000"
        }));
        let result = extract_subst_params_ext(&None, ed_val.as_ref(), &None, true);
        assert_eq!(result.len(), 2, "Binary hex must be appended as %2");
        assert_eq!(result[0], "");
        assert_eq!(
            result[1],
            "000000000100320000000000D710004013010000250200C000000000000000000000000000000000"
        );
    }

    #[test]
    fn test_binary_not_appended_when_gated_off() {
        // miss fallback 路径（include=false）语义不变：Binary 不进参数表
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": [""]},
            "Binary": "0A0B"
        }));
        let result = extract_subst_params_ext(&None, ed_val.as_ref(), &None, false);
        assert_eq!(result, vec![""], "gate off must keep legacy behavior");
        // 旧入口（wrapper）等价于 gate off
        let result2 = extract_subst_params(&None, ed_val.as_ref(), &None);
        assert_eq!(result2, vec![""]);
    }

    #[test]
    fn test_binary_appended_after_data_array_positional() {
        // e1dexpress 32 同构佐证：Data=["", NIC] + Binary → 第3参（模板 %2 未引用 %3）
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": [
                {"#attributes": {"Name": "Param1"}, "$": ""},
                {"#attributes": {"Name": "Param2"}, "$": "Intel(R) I219-V"}
            ],
            "Binary": "0000040002003000"
        }));
        let result = extract_subst_params_ext(&None, ed_val.as_ref(), &None, true);
        assert_eq!(result, vec!["", "Intel(R) I219-V", "0000040002003000"]);
    }

    #[test]
    fn test_binary_null_or_empty_not_appended() {
        use serde_json::json;
        let null_bin: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": ["a"]},
            "Binary": null
        }));
        let result = extract_subst_params_ext(&None, null_bin.as_ref(), &None, true);
        assert_eq!(result, vec!["a"], "null Binary must not append");
        let empty_bin: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": ["a"]},
            "Binary": ""
        }));
        let result = extract_subst_params_ext(&None, empty_bin.as_ref(), &None, true);
        assert_eq!(result, vec!["a"], "empty Binary must not append");
    }

    #[test]
    fn test_binary_appended_values_keep_position_for_unreferenced_slots() {
        // 追加不得挤占既有参数位置：模板 %1 引用不变（VSS 8224 形态：Data=[""]，%1=""）
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "Data": {"#text": [""]},
            "Binary": "CAFE"
        }));
        let params = extract_subst_params_ext(&None, ed_val.as_ref(), &None, true);
        let refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
        assert_eq!(evtx_tools_core::substitute::substitute("v=%1 h=%2", &refs), "v= h=CAFE");
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
    fn test_extract_subst_params_json_typed_bool_survives() {
        // R18: the evtx crate emits JSON-typed booleans/numbers for manifest
        // boolean params (Kernel-Boot 20 LastShutdownGood=true). Bools must
        // render as "true"/"false", never as empty strings.
        use serde_json::json;
        let ed_val: Option<serde_json::Value> = Some(json!({
            "LastShutdownGood": true,
            "LastBootGood": false,
            "LastBootId": 0,
        }));
        let param_map: Option<HashMap<String, String>> = Some(HashMap::from([
            ("1".into(), "LastShutdownGood".into()),
            ("2".into(), "LastBootGood".into()),
            ("3".into(), "LastBootId".into()),
        ]));
        let result = extract_subst_params(&None, ed_val.as_ref(), &param_map);
        assert_eq!(result, vec!["true", "false", "0"]);
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

/// R10 typed rendering (ADR 0005) — InType/OutType-driven formatting.
mod r10_typed_tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static DB_SEQ: AtomicU64 = AtomicU64::new(0);

    fn tmp_path(name: &str) -> PathBuf {
        let n = DB_SEQ.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("evtx_r10_{}_{}_{}.db", name, std::process::id(), n))
    }

    fn setup_db(path: &Path) {
        let _ = std::fs::remove_file(path);
        let conn = rusqlite::Connection::open(path).unwrap();
        super::r12_na_tests::create_v8_schema(&conn);
        conn.execute_batch(
            "INSERT INTO metadata(key, value) VALUES('schema_version', '9');
             INSERT INTO dll_entries(dll_id, dll_name, dll_path) VALUES(1, 't.dll', 'C:/t.dll');
             INSERT INTO providers(provider_name, guid, channel, dll_id) VALUES('T-Provider', '{T}', 'System', 1);
             INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, message_identifier, dll_id, message, parameters)
               VALUES('T-Provider', '{T}', 'en-US', 100, 100, 1, 'Error %1 done.', '{\"1\":\"errorCode\"}');
             INSERT INTO param_types VALUES('T-Provider', 100, '1', 'errorCode', 20, 18);",
        )
        .unwrap();
        drop(conn);
    }

    fn typed_event() -> (ParsedEvent, Option<Value>) {
        let ev = ParsedEvent {
            event_record_id: None,
            event_time: None,
            provider_name: Some("T-Provider".into()),
            provider_guid: None,
            event_id: Some(100),
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
            file_name: None,
            sequence_number: None,
        };
        let data = serde_json::json!({ "errorCode": "0x8024200b" });
        (ev, Some(data))
    }

    #[test]
    fn typed_render_hexint32_uppercase_end_to_end() {
        let tmp = tmp_path("e2e");
        setup_db(&tmp);
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_param_types().unwrap();
        let msg_db: Option<Arc<Mutex<MessageDb>>> = Some(Arc::new(Mutex::new(db)));
        let mut keys = vec![evtx_tools_core::VariantKey {
            provider_name: "T-Provider".into(),
            event_id: 100,
            version: 0, level: 0, opcode: 0, task: 0, keywords: 0, channel: 0, message_id: 0,
        }];
        let mut batch = vec![typed_event()];
        process_parallel_batch(&msg_db, &mut keys, "en-US", &mut batch);
        assert_eq!(
            batch[0].0.message.as_deref(),
            Some("Error 0x8024200B done."),
            "param_types (14,12) must uppercase-hex the value"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn typed_render_without_table_passes_through() {
        // Old v7 DB without param_types: passthrough must not change the value.
        let tmp = tmp_path("no_types");
        let _ = std::fs::remove_file(&tmp);
        let conn = rusqlite::Connection::open(&tmp).unwrap();
        super::r12_na_tests::create_v8_schema(&conn);
        conn.execute_batch(
            "INSERT INTO metadata(key, value) VALUES('schema_version', '9');
             INSERT INTO dll_entries(dll_id, dll_name, dll_path) VALUES(1, 't.dll', 'C:/t.dll');
             INSERT INTO providers(provider_name, guid, channel, dll_id) VALUES('T-Provider', '{T}', 'System', 1);
             INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, message_identifier, dll_id, message, parameters)
               VALUES('T-Provider', '{T}', 'en-US', 100, 100, 1, 'Error %1 done.', '{\"1\":\"errorCode\"}');",
        )
        .unwrap();
        drop(conn);
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_param_types().unwrap();
        let msg_db: Option<Arc<Mutex<MessageDb>>> = Some(Arc::new(Mutex::new(db)));
        let mut keys = vec![evtx_tools_core::VariantKey {
            provider_name: "T-Provider".into(),
            event_id: 100,
            version: 0, level: 0, opcode: 0, task: 0, keywords: 0, channel: 0, message_id: 0,
        }];
        let mut batch = vec![typed_event()];
        process_parallel_batch(&msg_db, &mut keys, "en-US", &mut batch);
        assert_eq!(
            batch[0].0.message.as_deref(),
            Some("Error 0x8024200b done."),
            "missing param_types must keep passthrough rendering"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn unit_ntstatus_outtype_1f_lookup_hit_only() {
        // Phase E: Ntfs 140 `Failure status` (14,1f) — wevtapi resolves
        // NTSTATUS via ntdll's message table. Hit → text (owned); miss →
        // passthrough unchanged (not owned).
        let db_path = tmp_path("ntstatus");
        setup_db(&db_path);
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO parameter_refs(provider_name, param_id, message, lang)
                 VALUES('NTSTATUS-System-Error', 3221225486, 'A device which does not exist was specified.', 'en-US')",
                [],
            )
            .unwrap();
        }
        let mut db = MessageDb::open(db_path.to_str().unwrap()).unwrap();
        db.load_param_refs("en-US").unwrap();
        let msg_db: Option<Arc<Mutex<MessageDb>>> = Some(Arc::new(Mutex::new(db)));

        let types = HashMap::from([("1".to_string(), "14:1f".to_string())]);

        // hit: raw hex param resolves to ntdll message text
        let (out, owned) = apply_param_types(
            vec!["0xc000000e".to_string()],
            &types,
            &HashMap::new(),
            msg_db.as_ref().unwrap(),
            "T",
        );
        assert_eq!(out[0], "A device which does not exist was specified.");
        assert!(owned.contains("1"));

        // miss: unknown code keeps passthrough rendering, not owned
        let (out, owned) = apply_param_types(
            vec!["0xdeadc0de".to_string()],
            &types,
            &HashMap::new(),
            msg_db.as_ref().unwrap(),
            "T",
        );
        assert_eq!(out[0], "0xdeadc0de");
        assert!(!owned.contains("1"));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn unit_hex_datetime_decimal_rules() {
        let db_path = tmp_path("unit");
        setup_db(&db_path);
        let mut db = MessageDb::open(db_path.to_str().unwrap()).unwrap();
        db.load_param_types().unwrap();
        let msg_db: Option<Arc<Mutex<MessageDb>>> = Some(Arc::new(Mutex::new(db)));

        // (0a,13) UINT64 → lowercase hex
        let types = HashMap::from([("1".to_string(), "0a:13".to_string())]);
        let (out, owned) = apply_param_types(
            vec!["2".to_string()],
            &types,
            &HashMap::new(),
            msg_db.as_ref().unwrap(),
            "T",
        );
        assert_eq!(out[0], "0x2");
        assert!(owned.contains("1"));

        // (11,02) FILETIME → 9-digit ns fraction
        let types = HashMap::from([("1".to_string(), "11:02".to_string())]);
        let (out, _) = apply_param_types(
            vec!["2026-08-21T21:31:25.532381Z".to_string()],
            &types,
            &HashMap::new(),
            msg_db.as_ref().unwrap(),
            "T",
        );
        assert_eq!(out[0], "2026-08-21T21:31:25.532381000Z");

        // (08,08) plain UINT32 → decimal, but NOT owned (maps may still apply)
        let types = HashMap::from([("1".to_string(), "08:08".to_string())]);
        let (out, owned) = apply_param_types(
            vec!["0xb772e734".to_string()],
            &types,
            &HashMap::new(),
            msg_db.as_ref().unwrap(),
            "T",
        );
        assert_eq!(out[0], "3077760820");
        // (8,8) with NO map link takes over (blocks the Layer-1 hex heuristic
        // that re-hexified Time-Service 35 TimeSourceRefId, 2026-09-08).
        assert!(owned.contains("1"));

        // (0f,0e) GUID → lowercase with braces, idempotent on braced input.
        let types = HashMap::from([("1".to_string(), "0f:0e".to_string())]);
        let (out, owned) = apply_param_types(
            vec!["{EA01CA89-9EF4-11F1-A4EF-244BFE8EB88E}".to_string()],
            &types,
            &HashMap::new(),
            msg_db.as_ref().unwrap(),
            "T",
        );
        assert_eq!(out[0], "{ea01ca89-9ef4-11f1-a4ef-244bfe8eb88e}");
        assert!(owned.contains("1"));

        let _ = std::fs::remove_file(&db_path);
    }
}

/// R12 / ADR 0004 — miss-cohort N/A parity for unregistered event sources.
mod r12_na_tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static DB_SEQ: AtomicU64 = AtomicU64::new(0);

    fn tmp_path(name: &str) -> PathBuf {
        let n = DB_SEQ.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("evtx_r12_{}_{}_{}.db", name, std::process::id(), n))
    }

    pub(crate) fn create_v8_schema(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "CREATE TABLE dll_entries (
                dll_id INTEGER PRIMARY KEY AUTOINCREMENT,
                dll_name TEXT NOT NULL,
                dll_path TEXT NOT NULL UNIQUE,
                pe_version TEXT NOT NULL DEFAULT '0.0.0.0'
            );
            CREATE TABLE providers (
                provider_name TEXT NOT NULL PRIMARY KEY,
                guid TEXT, channel TEXT,
                dll_id INTEGER NOT NULL REFERENCES dll_entries(dll_id)
            );
            CREATE TABLE provider_message_variants (
                provider_name TEXT NOT NULL,
                provider_guid TEXT NOT NULL,
                lang TEXT NOT NULL DEFAULT 'en-US',
                event_id INTEGER NOT NULL,
                version INTEGER NOT NULL DEFAULT 0,
                level INTEGER NOT NULL DEFAULT 0,
                opcode INTEGER NOT NULL DEFAULT 0,
                task INTEGER NOT NULL DEFAULT 0,
                keywords TEXT NOT NULL DEFAULT '0',
                channel INTEGER NOT NULL DEFAULT 0,
                message_identifier INTEGER NOT NULL,
                dll_id INTEGER NOT NULL REFERENCES dll_entries(dll_id),
                message TEXT NOT NULL,
                parameters TEXT,
                level_name TEXT,
                keywords_name TEXT,
                PRIMARY KEY (provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)
            );
            CREATE TABLE metadata (key TEXT NOT NULL PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE parameter_refs (
                provider_name TEXT NOT NULL,
                param_id INTEGER NOT NULL,
                message TEXT NOT NULL,
                lang TEXT NOT NULL DEFAULT 'en-US',
                PRIMARY KEY (provider_name, param_id, lang)
            );
            CREATE TABLE value_maps (
                provider_name TEXT NOT NULL,
                map_name TEXT NOT NULL,
                entry_value TEXT NOT NULL,
                display_name TEXT NOT NULL,
                is_bitmap INTEGER NOT NULL DEFAULT 0,
                lang TEXT NOT NULL DEFAULT 'en-US',
                PRIMARY KEY (provider_name, map_name, entry_value, lang)
            );
            CREATE TABLE param_value_maps (
                provider_name TEXT NOT NULL,
                event_id INTEGER NOT NULL,
                param_index TEXT NOT NULL,
                param_name TEXT NOT NULL DEFAULT '',
                map_name TEXT NOT NULL,
                is_bitmap INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (provider_name, event_id, param_index)
            );
            CREATE TABLE eventlog_sources (
                source_name TEXT NOT NULL PRIMARY KEY,
                source_type TEXT NOT NULL DEFAULT 'classic'
            );
            CREATE TABLE param_types (
                provider_name TEXT NOT NULL,
                event_id INTEGER NOT NULL,
                param_index TEXT NOT NULL,
                param_name TEXT NOT NULL DEFAULT '',
                in_type INTEGER NOT NULL,
                out_type INTEGER NOT NULL,
                PRIMARY KEY (provider_name, event_id, param_index)
            );",
        )
        .unwrap();
    }

    /// v8 DB whose registry snapshot knows "Chrome" but not "Unreg-Source"
    /// (empty_sources=true → snapshot present but collection wrote no rows).
    fn setup_db(path: &Path, empty_sources: bool) {
        let _ = std::fs::remove_file(path);
        let conn = rusqlite::Connection::open(path).unwrap();
        create_v8_schema(&conn);
        conn.execute(
            "INSERT INTO metadata(key, value) VALUES('schema_version', '9')",
            [],
        )
        .unwrap();
        if !empty_sources {
            conn.execute_batch(
                "INSERT INTO eventlog_sources(source_name, source_type) VALUES ('Chrome', 'classic');",
            )
            .unwrap();
        }
        drop(conn);
    }

    fn mk_event(provider: &str, eid: u32) -> ParsedEvent {
        ParsedEvent {
            event_record_id: None,
            event_time: None,
            provider_name: Some(provider.into()),
            provider_guid: None,
            event_id: Some(eid),
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
            file_name: None,
            sequence_number: None,
        }
    }

    fn data_value(texts: &[&str]) -> Option<Value> {
        Some(serde_json::json!({
            "Data": { "#text": texts }
        }))
    }

    fn run_batch(db_path: &Path, batch: &mut Vec<(ParsedEvent, Option<Value>)>) {
        let mut db = MessageDb::open(db_path.to_str().unwrap()).unwrap();
        db.load_registry_sources().unwrap();
        let msg_db: Option<Arc<Mutex<MessageDb>>> =
            Some(Arc::new(Mutex::new(db)));
        let mut keys: Vec<evtx_tools_core::VariantKey> = Vec::new();
        process_parallel_batch(&msg_db, &mut keys, "en-US", batch);
    }

    #[test]
    fn unregistered_source_miss_eid_nonzero_renders_na() {
        let tmp = tmp_path("na_hit");
        setup_db(&tmp, false);
        let mut batch = vec![(
            mk_event("Unreg-Source", 2562),
            data_value(&["MSIXDeployment", "DeleteMachineFolder"]),
        )];
        run_batch(&tmp, &mut batch);
        assert_eq!(
            batch[0].0.message.as_deref(),
            Some("N/A"),
            "unregistered source + miss + EID!=0 must render N/A"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn unregistered_source_miss_eid_zero_keeps_data_path() {
        let tmp = tmp_path("na_eid0");
        setup_db(&tmp, false);
        let mut batch = vec![(
            mk_event("Unreg-Source", 0),
            data_value(&["CServiceControl in OnStart"]),
        )];
        run_batch(&tmp, &mut batch);
        assert_eq!(
            batch[0].0.message.as_deref(),
            Some("CServiceControl in OnStart;;"),
            "EID=0 must keep the Data ;;-join fallback, never N/A"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn registered_source_miss_keeps_data_path() {
        let tmp = tmp_path("na_registered");
        setup_db(&tmp, false);
        let mut batch = vec![(
            mk_event("Chrome", 100),
            data_value(&["some raw payload"]),
        )];
        run_batch(&tmp, &mut batch);
        assert_eq!(
            batch[0].0.message.as_deref(),
            Some("some raw payload;;"),
            "registered source miss must keep the Data ;;-join fallback"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn unknown_registry_state_keeps_data_path() {
        // Empty snapshot (collection failure): feature must stay off entirely.
        let tmp = tmp_path("na_unknown");
        setup_db(&tmp, true);
        let mut batch = vec![(
            mk_event("Never-Registered-Anywhere", 2562),
            data_value(&["raw"]),
        )];
        run_batch(&tmp, &mut batch);
        assert_eq!(
            batch[0].0.message.as_deref(),
            Some("raw;;"),
            "unknown registry state must disable N/A parity"
        );
        let _ = std::fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod debug_live_db {
    use super::*;
    use evtx_tools_core::VariantKey;

    const LIVE_DB: &str = "E:/Code/evet/evtx-parity-test/msgdb/hotech-msg-Windows_10_Education-25H2-26220.9223.db";

    #[test]
    #[ignore]
    fn debug_kga25_param_types_attach() {
        let mut db = MessageDb::open(LIVE_DB).unwrap();
        db.load_param_types().unwrap();
        let key = VariantKey {
            provider_name: "Microsoft-Windows-Kernel-General".into(),
            event_id: 25,
            version: 0, level: 0, opcode: 0, task: 0, keywords: 0, channel: 0,
            message_id: 0,
        };
        db.warmup(std::slice::from_ref(&key), "en-US").unwrap();
        let row = db.resolve_variant(&key).expect("KGA 25 must resolve");
        println!("param_types: {:?}", row.param_types);
        println!("param_map_refs: {:?}", row.param_map_refs);
        // Full param-pipeline trace with the real event_data JSON.
        let ed = r#"{"SystemTime":"2026-08-20T21:02:09.500000Z","LoaderTime":"2026-08-20T21:02:08.000000Z","InternalBootFlags":2,"HalRtcErrorCode":0,"RealTimeIsUniversal":false,"IsSoftBoot":false,"Success":true,"Phase":0}"#;
        let ev: Value = serde_json::from_str(ed).unwrap();
        let params = extract_subst_params(&None, Some(&ev), &row.parameters);
        println!("extracted: {:?}", params);
        let db_arc: Option<Arc<Mutex<MessageDb>>> = Some(Arc::new(Mutex::new(
            MessageDb::open(LIVE_DB).unwrap(),
        )));
        let db_arc = db_arc.map(|m| {
            let g = m.lock().unwrap();
            // reopen a fresh MessageDb clone is not available; reuse outer via raw pointer trick is unsafe —
            // instead re-open and load in the new instance
            drop(g);
            m
        });
        let db_for_types = db_arc.clone();
        let _ = db_for_types;
        // run apply_param_types against a fresh db handle loaded with param refs
        let mut db2 = MessageDb::open(LIVE_DB).unwrap();
        db2.load_param_refs("en-US").unwrap();
        db2.load_param_types().unwrap();
        db2.warmup(std::slice::from_ref(&key), "en-US").unwrap();
        let row2 = db2.resolve_variant(&key).unwrap().clone();
        drop(db_arc);
        let arc2 = Arc::new(Mutex::new(db2));
        let (typed, owned) = apply_param_types(
            params.clone(),
            &row2.param_types,
            &row2.param_map_refs,
            &arc2,
            "Microsoft-Windows-Kernel-General",
        );
        println!("typed: {:?}", typed);
        println!("owned: {:?}", owned);
    }
}

#[cfg(test)]
mod batch_resolve_tests {
    //! Characterization tests for the full resolve→render hit path of
    //! `process_parallel_batch` (warmup → variant ladder with R8 tie-break →
    //! Layer A/B value maps → metadata copy). These pin current semantics
    //! while the resolve internals are refactored (Arc cache, batch-resolve).

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static DB_SEQ: AtomicU64 = AtomicU64::new(0);

    fn tmp_path(name: &str) -> PathBuf {
        let n = DB_SEQ.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("evtx_batch_{}_{}_{}.db", name, std::process::id(), n))
    }

    fn setup_db(path: &Path) {
        let _ = std::fs::remove_file(path);
        let conn = rusqlite::Connection::open(path).unwrap();
        super::r12_na_tests::create_v8_schema(&conn);
        conn.execute_batch(
            "INSERT INTO metadata(key, value) VALUES('schema_version', '9');
             INSERT INTO dll_entries(dll_id, dll_name, dll_path) VALUES(1, 't.dll', 'C:/t.dll');
             INSERT INTO providers(provider_name, guid, channel, dll_id) VALUES('Batch-Provider', '{B}', 'System', 1);
             -- two sibling variants in the same cohort, identical match fields,
             -- different message_identifier: R8 tie-break must pick 200 (smaller)
             INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message, parameters, level_name, keywords_name)
               VALUES('Batch-Provider', '{B}', 'en-US', 200, 0, 0, 0, 0, '0', 0, 200, 1,
                      'Logon %1 from %2.', '{\"1\":\"LogonType\",\"2\":\"TargetUserName\"}',
                      'Information', 'Audit Success');
             INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message, parameters, level_name, keywords_name)
               VALUES('Batch-Provider', '{B}', 'en-US', 200, 0, 0, 0, 0, '0', 0, 3221226500, 1,
                      'ALT Logon %1 from %2.', '{\"1\":\"LogonType\",\"2\":\"TargetUserName\"}',
                      'Information', 'Audit Success');
             -- Layer A: field-name-keyed value map (map_name == param name)
             INSERT INTO value_maps(provider_name, map_name, entry_value, display_name, is_bitmap, lang)
               VALUES('Batch-Provider', 'LogonType', '2', 'Interactive', 0, 'en-US');
             -- Layer B: param_index-keyed reference (param 2 -> LogonTypeX map)
             INSERT INTO value_maps(provider_name, map_name, entry_value, display_name, is_bitmap, lang)
               VALUES('Batch-Provider', 'LogonTypeX', '10', 'RemoteInteractive-x', 0, 'en-US');
             INSERT INTO param_value_maps(provider_name, event_id, param_index, param_name, map_name, is_bitmap)
               VALUES('Batch-Provider', 200, '2', 'TargetUserName', 'LogonTypeX', 0);",
        )
        .unwrap();
        drop(conn);
    }

    fn batch_event() -> (ParsedEvent, Option<Value>) {
        let ev = ParsedEvent {
            event_record_id: Some(1),
            event_time: None,
            provider_name: Some("Batch-Provider".into()),
            provider_guid: Some("{B}".into()),
            event_id: Some(200),
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
            file_name: None,
            sequence_number: None,
        };
        let data = serde_json::json!({ "LogonType": "2", "TargetUserName": "10" });
        (ev, Some(data))
    }

    fn run(db_path: &Path, batch: &mut [(ParsedEvent, Option<Value>)]) {
        let mut db = MessageDb::open(db_path.to_str().unwrap()).unwrap();
        db.load_value_maps("en-US").unwrap();
        let msg_db: Option<Arc<Mutex<MessageDb>>> = Some(Arc::new(Mutex::new(db)));
        let mut keys = vec![evtx_tools_core::VariantKey {
            provider_name: "Batch-Provider".into(),
            event_id: 200,
            version: 0, level: 0, opcode: 0, task: 0, keywords: 0, channel: 0, message_id: 0,
        }];
        process_parallel_batch(&msg_db, &mut keys, "en-US", batch);
    }

    #[test]
    fn hit_path_ladder_tiebreak_and_value_maps() {
        let tmp = tmp_path("hit");
        setup_db(&tmp);
        let mut batch = vec![batch_event(), batch_event()];
        run(&tmp, &mut batch);
        let (ev, _) = &batch[0];
        assert_eq!(
            ev.message.as_deref(),
            Some("Logon Interactive from RemoteInteractive-x."),
            "R8 tie-break must pick message_id 200; Layer A map param 1, Layer B map param 2"
        );
        assert_eq!(ev.level_name.as_deref(), Some("Information"));
        assert_eq!(ev.keywords_name.as_deref(), Some("Audit Success"));
        // Second event in the same batch must render identically (no
        // warmup-order or cross-event cache dependence).
        assert_eq!(batch[1].0.message.as_deref(), Some("Logon Interactive from RemoteInteractive-x."));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn hit_path_value_maps_b_override_a_on_same_field() {
        // Layer B replaces Layer A when both target the same field name.
        // param 1 (LogonType) gets A entries; adding a B link for param 1 to
        // a different map must fully replace the A entries for that field.
        let tmp = tmp_path("override");
        setup_db(&tmp);
        {
            let conn = rusqlite::Connection::open(&tmp).unwrap();
            conn.execute_batch(
                "INSERT INTO value_maps(provider_name, map_name, entry_value, display_name, is_bitmap, lang)
                   VALUES('Batch-Provider', 'LogonTypeA2', '2', 'Interactive-A2', 0, 'en-US');
                 INSERT INTO param_value_maps(provider_name, event_id, param_index, param_name, map_name, is_bitmap)
                   VALUES('Batch-Provider', 200, '1', 'LogonType', 'LogonTypeA2', 0);",
            )
            .unwrap();
        }
        let mut batch = vec![batch_event()];
        run(&tmp, &mut batch);
        assert_eq!(
            batch[0].0.message.as_deref(),
            Some("Logon Interactive-A2 from RemoteInteractive-x."),
            "Layer B (param_value_maps) must take precedence over Layer A for the same field"
        );
        let _ = std::fs::remove_file(&tmp);
    }
}

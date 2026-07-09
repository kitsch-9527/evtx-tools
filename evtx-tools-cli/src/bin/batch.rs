//! evtx-batch — 批量 EVTX 解析工具（对标 Lua `evtx_2_Sqlite.lua`）
//!
//! 遍历目录下所有 .evtx 文件，按规则逐个解析，汇总报告。
//!
//! # 用法
//!
//! ```bash
//! evtx-batch <DIR> --msg-db <DB> --rules-dir <DIR> -o <OUTPUT_DIR>
//! ```
//!
//! # 与 Lua 脚本对照
//!
//! | Lua 脚本 | evtx-batch |
//! |----------|------------|
//! | lfs.dir() 遍历 *.evtx | std::fs::read_dir 遍历 *.evtx |
//! | evtx2csv.exe → CSV | evtx_tools_core 直接解析 → SQLite |
//! | sqlite3.exe .import | rusqlite 批量写入 |
//! | CREATE TABLE aevtx_... AS SELECT | 规则引擎 → 场景表 |
//! | PAUSE | 无（静默运行） |

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;

use evtx_tools_core::parser::{iter_events, open_evtx};
use evtx_tools_core::resolver::MessageDb;
use evtx_tools_core::routing::{rule_from_yaml, SceneRouter};
use evtx_tools_core::stats::StatsCollector;
use evtx_tools_core::storage::sqlite::{ensure_scenario_table, insert_scenario_event, EventsDb};
use evtx_tools_core::substitute::{substitute_format_message, apply_param_enums};
use evtx_tools_core::{compute_message_id, ParsedEvent};

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use serde_json::Value;

type Result<T> = std::result::Result<T, String>;

// ---------------------------------------------------------------------------
// CLI args — 与 Lua 脚本参数对应
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "evtx-batch", version, about = "Batch EVTX parser — traverse, parse & route")]
struct Args {
    /// 目录路径（包含 .evtx 文件）
    input: PathBuf,

    /// 消息库文件路径
    #[arg(long)]
    msg_db: PathBuf,

    /// 规则 YAML 文件目录
    #[arg(long)]
    rules_dir: Option<PathBuf>,

    /// 输出目录
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// 消息语言
    #[arg(long, default_value = "zh-CN")]
    lang: String,

    /// 解析线程数（0 = 自动）
    #[arg(long, default_value_t = 0)]
    threads: usize,

    /// 跳过统计表
    #[arg(long)]
    no_stats: bool,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    // 收集所有 .evtx 文件
    let evtx_files = collect_evtx_files(&args.input);
    if evtx_files.is_empty() {
        eprintln!("[error] no .evtx files found in '{}'", args.input.display());
        std::process::exit(1);
    }
    eprintln!("[info] found {} .evtx file(s) in '{}'", evtx_files.len(), args.input.display());

    // 加载消息库
    let msg_db_path = args.msg_db.to_string_lossy().to_string();
    eprintln!("[info] loading messages db: {msg_db_path}");
    let mut msg_db_opt: Option<MessageDb> = None;
    match MessageDb::open(&msg_db_path) {
        Ok(mut db) => {
            let _ = db.load_param_refs();
            let _ = db.load_value_maps();
            msg_db_opt = Some(db);
        }
        Err(e) => eprintln!("[warn] failed to open messages db: {e}"),
    }

    // 加载规则
    let mut rules = Vec::new();
    if let Some(ref rules_dir) = args.rules_dir {
        if rules_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(rules_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "yaml" || ext == "yml" {
                        match fs::read_to_string(&path) {
                            Ok(content) => match rule_from_yaml(&content) {
                                Ok(rule) => {
                                    eprintln!("[info] loaded rule: {} (→ {})", rule.name, rule.table);
                                    rules.push(rule);
                                }
                                Err(e) => eprintln!("[warn] failed to parse '{}': {e}", path.display()),
                            },
                            Err(e) => eprintln!("[warn] failed to read '{}': {e}", path.display()),
                        }
                    }
                }
            }
        }
    }

    let router = SceneRouter::new(rules);
    let has_rules = router.has_rules();
    if has_rules {
        eprintln!("[info] router has {} rule(s)", router.rules().len());
    }

    // 逐个解析文件
    let start = Instant::now();
    let mut total_events: u64 = 0;

    // Ensure output directory exists
    fs::create_dir_all(&args.output)
        .unwrap_or_else(|e| { eprintln!("[error] failed to create output dir: {e}"); std::process::exit(1); });

    for evtx_path in &evtx_files {
        let stem = evtx_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
        let output_path = args.output.join(format!("{stem}_batch"));

        eprintln!("\n[process] parsing: {} (→ {})", evtx_path.display(), output_path.display());

        // Take ownership of msg_db for this file (clone by re-opening)
        let file_msg_db = msg_db_opt.as_ref().map(|_| {
            let mut db = MessageDb::open(&msg_db_path).expect("failed to reopen msg db");
            let _ = db.load_param_refs();
            let _ = db.load_value_maps();
            db
        });

        match process_single(
            evtx_path,
            &output_path,
            file_msg_db,
            router.clone(),
            &args.lang,
            args.threads,
            args.no_stats,
        ) {
            Ok(count) => {
                total_events += count;
                eprintln!("[done] {count} events from {}", evtx_path.display());
            }
            Err(e) => {
                eprintln!("[error] failed to parse '{}': {e}", evtx_path.display());
            }
        }
    }

    let elapsed = start.elapsed();
    eprintln!("\n========================================");
    eprintln!("  BATCH COMPLETE");
    eprintln!("  Files:     {}", evtx_files.len());
    eprintln!("  Events:    {total_events}");
    eprintln!("  Duration:  {:.1}s", elapsed.as_secs_f64());
    eprintln!("  Output:    {}", args.output.display());
    eprintln!("========================================");
}

// ---------------------------------------------------------------------------
// File collection (mirrors Lua's lfs.dir + string.match)
// ---------------------------------------------------------------------------

fn collect_evtx_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext.eq_ignore_ascii_case("evtx") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

// ---------------------------------------------------------------------------
// Single-file parse (adapted from cmd_parse.rs producer-writer pipeline)
// ---------------------------------------------------------------------------

fn process_single(
    input_path: &Path,
    output_path: &Path,
    msg_db_opt: Option<MessageDb>,
    router: SceneRouter,
    lang: &str,
    threads: usize,
    no_stats: bool,
) -> Result<u64> {
    let input_str = input_path.to_string_lossy().to_string();
    let output_display = output_path.to_string_lossy().to_string();

    // Open EVTX
    let mut parser = open_evtx(&input_str, threads)?;
    let file_name = input_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Prepare output DB
    let events_db = Some(EventsDb::open(output_path)?);
    if let Some(ref db) = events_db {
        db.create_table()?;
    }

    // Producer-writer channel
    let (tx, rx) = mpsc::sync_channel::<Vec<ParsedEvent>>(4);

    // Message DB for parallel resolve
    let msg_db: Option<Arc<Mutex<MessageDb>>> =
        msg_db_opt.map(|db| Arc::new(Mutex::new(db)));

    let lang_owned = lang.to_string();
    let has_rules = router.has_rules();

    // Writer thread
    let writer = thread::Builder::new()
        .name("evtx-batch-writer".into())
        .spawn(move || -> Result<u64> {
            let mut total = 0u64;
            let mut stats = StatsCollector::new();
            let mut tables_created: HashSet<String> = HashSet::new();

            for events in rx {
                // Feed stats
                if !no_stats {
                    for event in &events {
                        stats.feed(event);
                    }
                }

                // Write to events table
                if let Some(ref db) = events_db {
                    db.insert(&events)?;
                }

                // Route to scenario tables
                if has_rules {
                    if let Some(ref db) = events_db {
                        for event in &events {
                            let matched = router.route(event);
                            for rule in &matched {
                                if tables_created.insert(rule.table.clone()) {
                                    ensure_scenario_table(db.conn(), rule)?;
                                }
                                insert_scenario_event(db.conn(), rule, event)?;
                            }
                        }
                    }
                }

                total += events.len() as u64;
            }

            // Write stats
            if let Some(ref db) = events_db {
                if !no_stats {
                    stats.write(db.conn())?;
                }
                db.finalize()?;
            }

            Ok(total)
        })
        .map_err(|e| format!("failed to spawn writer thread: {e}"))?;

    // Producer
    const PARALLEL_BATCH: usize = 20_000;
    let mut batch: Vec<(ParsedEvent, Option<Value>)> = Vec::with_capacity(PARALLEL_BATCH);
    let mut new_pairs: Vec<(String, u32)> = Vec::new();
    let mut warmed_up: HashSet<(String, u32)> = HashSet::new();
    let mut seq: u64 = 0;

    for result in iter_events(&mut parser, file_name) {
        let (mut event, event_data_value) = match result {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[warn] skipping event: {e}");
                continue;
            }
        };

        // Assign sequence number
        event.sequence_number = Some(seq);
        seq += 1;

        // Warmup pairs
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
            process_parallel_batch(&msg_db, &mut new_pairs, &lang_owned, &mut batch);
            let event_vec: Vec<ParsedEvent> = batch.drain(..).map(|(e, _)| e).collect();
            if tx.send(event_vec).is_err() {
                break;
            }
        }
    }

    // Flush remaining
    if !batch.is_empty() {
        process_parallel_batch(&msg_db, &mut new_pairs, &lang_owned, &mut batch);
        let event_vec: Vec<ParsedEvent> = batch.drain(..).map(|(e, _)| e).collect();
        let _ = tx.send(event_vec);
    }

    drop(tx);
    writer.join().map_err(|_| "writer thread panicked".to_string())?
}

// ---------------------------------------------------------------------------
// Parallel batch processing (adapted from cmd_parse.rs)
// ---------------------------------------------------------------------------

fn process_parallel_batch(
    msg_db: &Option<Arc<Mutex<MessageDb>>>,
    new_pairs: &mut Vec<(String, u32)>,
    target_lang: &str,
    batch: &mut [(ParsedEvent, Option<Value>)],
) {
    let Some(ref db) = msg_db else { return };
    if batch.is_empty() {
        return;
    }

    // Warmup
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
        }
        new_pairs.clear();
    }

    // Parallel resolve
    use rayon::prelude::*;
    batch.par_iter_mut().for_each(|(event, event_data_value)| {
        let (maybe_row, local_value_maps, local_bitmap_names) = {
            let guard = match db.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let provider = match event.provider_name.as_ref() {
                Some(p) => p.as_str(),
                None => return,
            };
            let eid = match event.event_id {
                Some(e) => e,
                None => return,
            };
            let mid = compute_message_id(eid, event.qualifiers);
            let row = guard
                .get(provider, mid)
                .or_else(|| guard.get(provider, eid))
                .cloned();
            let vms = match &row {
                Some(r) => r.parameters.as_ref()
                    .map(|pm| guard.get_param_value_maps(pm))
                    .unwrap_or_default(),
                None => HashMap::new(),
            };
            (row, vms, guard.get_bitmap_names())
        };

        let row = match maybe_row {
            Some(r) => r,
            None => return,
        };

        // Step 1: Expand %%N parameter references BEFORE substitute.
        let template = if row.template.contains("%%") {
            if let Ok(guard) = db.lock() {
                guard.expand_message(&row.template)
            } else {
                row.template.clone()
            }
        } else {
            row.template.clone()
        };

        // Step 2: Substitute %1–%99 positional parameters.
        if template.contains('%') {
            let params = extract_subst_params(&event.event_data, event_data_value.as_ref(), &row.parameters);
            let params = apply_param_enums(&params, &row.parameters);
            let params = evtx_tools_core::substitute::apply_value_maps(&params, &row.parameters, &local_value_maps, &local_bitmap_names);
            let param_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            event.message = Some(substitute_format_message(&template, &param_refs));
        } else {
            event.message = Some(template);
        }

        // Copy metadata
        if let Some(ref t) = row.task { if !t.is_empty() { event.task_category = Some(t.clone()); } }
        if let Some(ref o) = row.opcode { if !o.is_empty() { event.opcode = Some(o.clone()); } }
        if let Some(ref k) = row.keywords { if !k.is_empty() { event.keywords = Some(k.clone()); } }
        if let Some(ref ln) = row.level_name { if !ln.is_empty() { event.level_name = Some(ln.clone()); } }
        if let Some(ref kn) = row.keywords_name { if !kn.is_empty() { event.keywords_name = Some(kn.clone()); } }

        // Note: %%N expansion already handled above (Step 1), before substitute.

        if let Some(msg) = event.message.take() {
            event.message = Some(evtx_tools_core::substitute::format_for_display(&msg));
        }
    });
}

// ---------------------------------------------------------------------------
// Substitution parameter extraction (from cmd_parse.rs)
// ---------------------------------------------------------------------------

fn extract_subst_params(
    event_data_str: &Option<String>,
    event_data_value: Option<&Value>,
    param_map: &Option<HashMap<String, String>>,
) -> Vec<String> {
    use serde_json::Value;

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

    // Strategy 1: Flat object with param map
    if let (Some(params), Some(obj)) = (param_map.as_ref(), parsed.as_object()) {
        if !params.is_empty() && !obj.contains_key("#text") {
            let max_idx: usize = params.keys()
                .filter_map(|k| k.parse::<usize>().ok())
                .max().unwrap_or(0);
            if max_idx > 0 {
                let mut result = Vec::with_capacity(max_idx);
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

    // Strategy 2: #text under Data
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

        // Strategy 3: Flat positional fallback
        if !obj.contains_key("#text") {
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

    Vec::new()
}

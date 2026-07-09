# MUI+CRIM 消息解析修复方案（历史记录）

> **⚠️ 注意：** 本文档描述的是旧的 Rust dumper 实现。消息提取（dump）功能已用 Go 重写。
> 本文档保留供历史参考。消息库生成由 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（二进制 `msgdb-dump`）提供。

**Goal (historical):** 让 System.evtx 的事件描述也能被 Rust `dump` 命令提取

**根因:** system-events.dll 和 netevent.dll 使用 EventManifest 系统，消息文本在 MUI 伴生文件（`en-US/xxx.dll.mui`）的 `RT_MESSAGETABLE` 中，而 event_id→message_identifier 映射在 DLL 本身的 WEVT_TEMPLATE（CRIM 格式）中。当前代码只查主 DLL 的 RT_MESSAGETABLE，没有查 MUI 伴生文件，也没有解析 CRIM 映射。

**Tech Stack:** Rust, pelite, evtx (crate), rusqlite

## 全局约束

- 不增加新的外部 crate 依赖（evtx 已在依赖树中，pelite 已有）
- dump 命令保持 Windows-only
- messages.db schema 不改变（兼容现有 parse 流程）
- 原有测试全部通过

---

### Task 1: extractor.rs — 添加 `extract_raw_crim_blobs()`

**Files:**
- Modify: `hotech-evtx-dumper/src/extractor.rs`

**Interfaces:**
- Produces: `pub fn extract_raw_crim_blobs(dll_path: &str) -> Result<Vec<Vec<u8>>>` — returns raw CRIM blob bytes from PE WEVT_TEMPLATE resources

- [ ] **Step 1: Verify existing tests pass**

```bash
cd /e/Code/rust/hotech-analyzer-gui/crates/hotech-evtx && cargo test -p hotech-evtx-dumper 2>&1 | tail -5
```
Expected: "test result: ok. X passed"

- [ ] **Step 2: Add the new function + test**

Add after `extract_wevt_templates_from_resources()`:

```rust
/// Extract raw WEVT_TEMPLATE CRIM blob bytes from a PE file.
///
/// Returns a Vec of raw byte blobs (one per CRIM resource found).
/// The caller can then parse them with `evtx::wevt_templates::manifest::CrimManifest`.
pub fn extract_raw_crim_blobs(dll_path: &str) -> Result<Vec<Vec<u8>>> {
    use std::collections::HashMap;

    let file_map = pelite::FileMap::open(dll_path)
        .map_err(|e| format!("failed to open PE file '{dll_path}': {e}"))?;
    let pe = pelite::PeFile::from_bytes(&file_map)
        .map_err(|e| format!("failed to parse PE file '{dll_path}': {e}"))?;
    let resources = pe.resources()
        .map_err(|e| format!("failed to read resources from '{dll_path}': {e}"))?;
    let root = resources.root()
        .map_err(|e| format!("failed to read resource root: {e}"))?;

    let mut blobs = Vec::new();

    for type_entry in root.entries() {
        let is_wevt = match type_entry.name() {
            Ok(pelite::resources::Name::Wide(words)) => {
                String::from_utf16_lossy(words) == "WEVT_TEMPLATE"
            }
            _ => false,
        };
        if !is_wevt {
            continue;
        }
        let name_dir = match type_entry.entry() {
            Ok(pelite::resources::Entry::Directory(d)) => d,
            _ => continue,
        };
        for name_entry in name_dir.entries() {
            let entry = match name_entry.entry() {
                Ok(e) => e,
                _ => continue,
            };
            let lang_dir = match entry {
                pelite::resources::Entry::Directory(d) => d,
                pelite::resources::Entry::DataEntry(d) => {
                    if let Ok(bytes) = d.bytes() {
                        blobs.push(bytes.to_vec());
                    }
                    continue;
                }
            };
            for lang_entry in lang_dir.entries() {
                if let Ok(pelite::resources::Entry::DataEntry(d)) = lang_entry.entry() {
                    if let Ok(bytes) = d.bytes() {
                        blobs.push(bytes.to_vec());
                    }
                }
            }
        }
    }

    if blobs.is_empty() {
        Err(format!("no WEVT_TEMPLATE resource found in '{dll_path}'"))
    } else {
        Ok(blobs)
    }
}
```

Add test:

```rust
#[test]
fn test_extract_raw_crim_from_system_events() {
    #[cfg(windows)]
    {
        let path = r"C:\Windows\System32\Microsoft-Windows-System-Events.dll";
        if !std::path::Path::new(path).exists() {
            eprintln!("SKIP: {path} not found");
            return;
        }
        let blobs = extract_raw_crim_blobs(path).expect("should extract CRIM blobs");
        assert!(!blobs.is_empty(), "should have at least one CRIM blob");
        assert!(blobs[0].len() > 100, "CRIM blob should be substantial");
        assert_eq!(&blobs[0][..4], b"CRIM", "first blob should start with CRIM signature");

        // Verify it can be parsed by evtx's CrimManifest
        let manifest = evtx::wevt_templates::manifest::CrimManifest::parse(&blobs[0]);
        assert!(manifest.is_ok(), "CrimManifest should parse the blob: {:?}", manifest.err());
        let manifest = manifest.unwrap();
        assert!(!manifest.providers.is_empty(), "should have providers");
        eprintln!("  CRIM: {} providers, {} events total",
            manifest.providers.len(),
            manifest.providers.iter()
                .filter_map(|p| p.wevt.elements.events.as_ref())
                .map(|e| e.events.len())
                .sum::<usize>()
        );
    }
}
```

- [ ] **Step 3: Run the test**

```bash
cargo test -p hotech-evtx-dumper -- extract_raw_crim -- --nocapture
```

Expected: PASS with CRIM provider/event count printed

- [ ] **Step 4: Commit**

```bash
git add hotech-evtx-dumper/src/extractor.rs
git commit -m "feat(dumper): add extract_raw_crim_blobs() for raw WEVT_TEMPLATE bytes"
```

---

### Task 2: CLI crate — add evtx dependency

**Files:**
- Modify: `hotech-evtx/Cargo.toml`

- [ ] **Step 1: Add evtx dependency**

```toml
evtx = "0.12"
```

- [ ] **Step 2: Verify compile**

```bash
cargo check -p hotech-evtx 2>&1 | tail -3
```

Expected: "Checking hotech-evtx v0.1.0" + "Finished"

- [ ] **Step 3: Commit**

```bash
git add hotech-evtx/Cargo.toml
git commit -m "build(cli): add evtx dependency for CRIM parsing"
```

---

### Task 3: cmd_dump.rs — MUI 伴生文件查找 + CRIM 映射

**Files:**
- Modify: `hotech-evtx/src/cmd_dump.rs`
- Create: `hotech-evtx/src/mui_helper.rs`

**Interfaces:**
- Consumes: `extract_raw_crim_blobs()`, `extract_messages()`, `evtx::wevt_templates::manifest::CrimManifest`
- Produces: MUI+CRIM 消息正确写入 messages.db

- [ ] **Step 1: Create mui_helper.rs**

```rust
//! MUI companion file resolution helpers.
//!
//! Windows stores localized message text in MUI companion files
//! located at `{dll_dir}\{lang}\{dll_name}.dll.mui`.
//! These files may contain RT_MESSAGETABLE resources with the
//! actual message text, while the main DLL only has WEVT_TEMPLATE.

use std::path::{Path, PathBuf};

/// Language code to LCID mapping for MUI lookup
const LANG_MAP: &[(&str, &str)] = &[
    ("zh-CN", "2052"),
    ("zh-TW", "1028"),
    ("en-US", "1033"),
    ("ja-JP", "1041"),
    ("ko-KR", "1042"),
];

/// Find MUI companion file path for a given DLL and language code.
///
/// Returns `Some(PathBuf)` if the .mui file exists, `None` otherwise.
///
/// Example:
///   DLL:  C:\Windows\System32\Microsoft-Windows-System-Events.dll
///   Lang: en-US
///   MUI:  C:\Windows\System32\en-US\Microsoft-Windows-System-Events.dll.mui
pub fn find_mui_companion(dll_path: &str, lang_code: &str) -> Option<PathBuf> {
    let dll = Path::new(dll_path);
    let parent = dll.parent()?;
    let file_stem = dll.file_name()?;
    let mui_name = format!("{}.mui", file_stem.to_string_lossy());
    let mui_path = parent.join(lang_code).join(&mui_name);

    if mui_path.exists() {
        Some(mui_path)
    } else {
        None
    }
}

/// Get all available MUI companion paths for a DLL across all languages.
pub fn find_all_mui_companions(dll_path: &str) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    for &(code, _lcid) in LANG_MAP {
        if let Some(path) = find_mui_companion(dll_path, code) {
            result.push((code.to_string(), path));
        }
    }
    result
}

/// Convert a language code to an LCID string for use in filenames.
pub fn lang_code_to_lcid(lang_code: &str) -> &str {
    LANG_MAP.iter()
        .find(|(code, _)| *code == lang_code)
        .map(|(_, lcid)| *lcid)
        .unwrap_or("1033")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_mui_companion_system_events_en() {
        #[cfg(windows)]
        {
            let path = r"C:\Windows\System32\Microsoft-Windows-System-Events.dll";
            if !std::path::Path::new(path).exists() {
                return;
            }
            let mui = find_mui_companion(path, "en-US");
            assert!(mui.is_some(), "should find en-US MUI companion");
            let mui = mui.unwrap();
            assert!(mui.exists(), "MUI path should exist");
            assert!(mui.to_string_lossy().contains("en-US"));
            assert!(mui.to_string_lossy().ends_with(".mui"));
            eprintln!("  en-US MUI: {}", mui.display());
        }
    }

    #[test]
    fn test_find_mui_companion_system_events_zh() {
        #[cfg(windows)]
        {
            let path = r"C:\Windows\System32\Microsoft-Windows-System-Events.dll";
            if !std::path::Path::new(path).exists() {
                return;
            }
            let mui = find_mui_companion(path, "zh-CN");
            if let Some(ref mui) = mui {
                eprintln!("  zh-CN MUI: {}", mui.display());
            } else {
                eprintln!("  zh-CN MUI not found (expected on non-Chinese systems)");
            }
        }
    }

    #[test]
    fn test_find_mui_companion_nonexistent_dll() {
        let result = find_mui_companion(r"C:\nonexistent.dll", "en-US");
        assert!(result.is_none());
    }

    #[test]
    fn test_lang_code_to_lcid() {
        assert_eq!(lang_code_to_lcid("en-US"), "1033");
        assert_eq!(lang_code_to_lcid("zh-CN"), "2052");
        assert_eq!(lang_code_to_lcid("fr-FR"), "1033"); // fallback
    }
}
```

- [ ] **Step 2: Add MUI resolution logic to cmd_dump.rs**

The key change: after `extract_messages()` returns empty for a DLL, check for MUI companion → extract from MUI → parse CRIM → join → write.

Add a helper function:

```rust
/// Try to extract messages from MUI companion files when the main DLL
/// uses WEVT_TEMPLATE (no RT_MESSAGETABLE).
///
/// Returns messages keyed by event_id (resolved through CRIM mapping).
fn extract_mui_messages(
    dll_path: &str,
    provider_name: &str,
    provider_guid: Option<&str>,
    lang: &str,
) -> Result<Vec<PeMessage>> {
    // 1. Get raw CRIM blobs from main DLL
    let crim_blobs = hotech_evtx_dumper::extract_raw_crim_blobs(dll_path)?;

    // 2. Find MUI companion file
    let mui_path = mui_helper::find_mui_companion(dll_path, lang)
        .ok_or_else(|| format!("no MUI companion for lang '{}'", lang))?;

    // 3. Extract messages from MUI (keyed by message_identifier → text)
    let mui_messages = hotech_evtx_dumper::extract_messages(
        &mui_path.to_string_lossy()
    )?;

    if mui_messages.is_empty() {
        return Err(format!("MUI file '{}' has no messages", mui_path.display()));
    }

    // 4. Parse CRIM to get event_id → message_identifier mapping
    // Each provider in CRIM has events with (event_id, message_identifier)
    // that maps to the MUI MESSAGETABLE entries.
    for blob in &crim_blobs {
        if let Ok(manifest) = evtx::wevt_templates::manifest::CrimManifest::parse(blob) {
            for provider in &manifest.providers {
                // Match by GUID or take all if no GUID provided
                let guid_match = match provider_guid {
                    Some(g) => provider.guid.to_string().contains(g),
                    None => true, // accept any if no guid filter
                };
                if !guid_match {
                    continue;
                }

                // Build metadata name maps from CRIM elements
                // (task_id → task_name, opcode_id → opcode_name, etc.)
                let tasks: std::collections::HashMap<u32, String> = provider.wevt.elements.tasks
                    .as_ref()
                    .map(|t| t.tasks.iter()
                        .filter_map(|task| task.name.clone().map(|n| (task.identifier, n)))
                        .collect())
                    .unwrap_or_default();

                let opcodes: std::collections::HashMap<u32, String> = provider.wevt.elements.opcodes
                    .as_ref()
                    .map(|o| o.opcodes.iter()
                        .filter_map(|op| op.name.clone().map(|n| (op.identifier, n)))
                        .collect())
                    .unwrap_or_default();

                let levels: std::collections::HashMap<u32, String> = provider.wevt.elements.levels
                    .as_ref()
                    .map(|l| l.levels.iter()
                        .filter_map(|lv| lv.name.clone().map(|n| (lv.identifier, n)))
                        .collect())
                    .unwrap_or_default();

                // Collect keyword names (bitmask-based)
                // Build template parameter names
                let mut template_params: std::collections::HashMap<u32, Vec<String>> = std::collections::HashMap::new();
                if let Some(ref ttbl) = provider.wevt.elements.templates {
                    for tpl in &ttbl.templates {
                        let param_names: Vec<String> = tpl.items.iter()
                            .filter_map(|item| item.name.clone())
                            .collect();
                        if !param_names.is_empty() {
                            template_params.insert(tpl.offset, param_names);
                        }
                    }
                }

                // Get events
                if let Some(ref events) = provider.wevt.elements.events {
                    let mut resolved = Vec::new();
                    for ev in &events.events {
                        let eid = ev.identifier as u32;
                        let msg_id = ev.message_identifier;

                        // Look up message text from MUI by message_identifier
                        if let Some(mui_msg) = mui_messages.get(&msg_id) {
                            let task_name = tasks.get(&(ev.task as u32)).cloned();
                            let opcode_name = opcodes.get(&(ev.opcode as u32)).cloned();
                            let level_name = levels.get(&(ev.level as u32)).cloned();

                            // Build parameters map from template
                            let params = ev.template_offset.and_then(|toff| {
                                template_params.get(&toff).map(|names| {
                                    names.iter().enumerate()
                                        .map(|(i, n)| ((i + 1).to_string(), n.clone()))
                                        .collect::<std::collections::HashMap<String, String>>()
                                })
                            });

                            resolved.push(PeMessage {
                                event_id: eid,
                                message: mui_msg.message.clone(),
                                level: Some(ev.level),
                                task: task_name,
                                opcode: opcode_name,
                                keywords: level_name, // Use level name as keyword hint
                                parameters: params,
                                provider_name: Some(provider_name.to_string()),
                            });
                        }
                    }

                    if !resolved.is_empty() {
                        return Ok(resolved);
                    }
                }
            }
        }
    }

    Err(format!("no matching CRIM events found for provider '{}'", provider_name))
}
```

- [ ] **Step 3: Modify `run_dump()` to call MUI extraction when normal extraction returns empty**

In the main provider loop (around line 83), after the normal `extract_messages` call:

```rust
// If no messages from normal extraction, try MUI + CRIM path
if msgs.is_empty() && dll_path.to_string_lossy().contains(".dll") {
    eprintln!("[info]   no RT_MESSAGETABLE, trying MUI+CRIM path...");
    match extract_mui_messages(
        &dll_str,
        provider.name.as_str(),
        provider.guid.as_deref(),
        primary_lang,
    ) {
        Ok(mui_msgs) => {
            eprintln!("[info]   extracted {} messages via MUI+CRIM", mui_msgs.len());
            for msg in mui_msgs {
                let eid = msg.event_id;
                let mut msg = msg;
                msg.provider_name = Some(provider.name.clone());
                let dll_key = dll_name.clone();
                let entry = per_dll_messages.entry(dll_key).or_default();
                entry.insert(eid, msg.clone());
                all_messages.push(msg);
            }
        }
        Err(e) => {
            eprintln!("[warn]   MUI+CRIM also failed: {e}");
        }
    }
}
```

Note: This needs to be AFTER the current block that adds messages to `all_messages` and `per_dll_messages`. The cleanest way is to restructure the provider loop slightly.

- [ ] **Step 4: Compile and fix issues**

```bash
cargo check -p hotech-evtx 2>&1 | head -30
```

Iterate on compilation errors until clean.

- [ ] **Step 5: Commit**

```bash
git add hotech-evtx/src/cmd_dump.rs hotech-evtx/src/mui_helper.rs hotech-evtx/Cargo.toml
git commit -m "feat(dump): resolve MUI+CRIM messages for EventManifest providers"
```

---

### Task 4: Integration test — verify System.evtx message extraction

**Files:**
- Run: E2E test

- [ ] **Step 1: Full dump test**

```bash
cargo run --release -p hotech-evtx -- dump --out-dir D:/temp/mui_test/ --providers "Microsoft-Windows-System-Events" --split-dlls --lang 1033 2>&1
```

Expected: show messages extracted from MUI+CRIM path (should be > 0, ideally ~616)

- [ ] **Step 2: Run full existing test suite**

```bash
cargo test -p hotech-evtx-dumper 2>&1 | tail -10
cargo test -p hotech-evtx-core 2>&1 | tail -10
```

Expected: all tests pass

- [ ] **Step 3: Commit final**

```bash
git add -A
git commit -m "test(dump): verify MUI+CRIM resolution for System events"
```

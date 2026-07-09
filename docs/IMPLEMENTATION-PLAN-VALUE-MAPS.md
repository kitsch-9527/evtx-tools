# %N Parameter Enumeration Value Resolution -- Implementation Plan (Historical)

> **⚠️ Note:** The Rust dumper implementation described here has been rewritten in Go.
> This document is kept for historical reference. See the [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) (binary `msgdb-dump`) repository for current implementation.

## Problem

BITS event 209 message template:
```
High performance property for BITS job "%1" with ID "%2" %3.
```

- `%1` = jobName ("Font Download"), `%2` = jobId ("{guid}"), `%3` = isRoaming (1)
- hotech current output: `%3` = `"1"` (raw integer from event_data)
- Windows Event Viewer: `%3` = `"Enabled"` (via value map)

Root cause: hotech-evtx has no mechanism to resolve numeric enumeration values
to display strings during message substitution.

---

## Architecture

Two layers, applied in order (first match wins, fall through to next):

```
raw numeric param value
  |
  v
Layer 1: Standard type conventions (heuristic)   -- Phase 1 (substitute.rs)
  |  win:Boolean-like fields: 0/false -> "Disabled", 1/true -> "Enabled"
  |  field names containing: bool, enable, roam, flag (case-insensitive)
  v (fall through)
Layer 2: CRIM VMAP resolved at dump time          -- Phase 2 (dumper + schema)
  |  DB table value_maps: (provider, map_name, raw_value) -> display_string
  |  populated during dump by resolving VMAP message_ids against DLL message table
  v (fall through)
raw value (current behaviour, unchanged)
```

No new crates, no new files.

---

## Phase 1: Standard Type Conventions (substitute.rs, ~50 lines)

### File changed
- `E:/Code/rust/hotech-analyzer-gui/crates/hotech-evtx/hotech-evtx-core/src/substitute.rs`

### New function

```rust
/// Resolve a raw value to a display string using standard type conventions.
///
/// Matches win:Boolean-like fields by heuristic name matching:
/// - field name contains "bool", "enable", "roam", "flag" (case-insensitive)
/// - raw = "0" | "false"  -> Some("Disabled")
/// - raw = "1" | "true"   -> Some("Enabled")
/// - otherwise -> None (pass through)
pub fn resolve_enum(raw: &str, field: &str) -> Option<String>;
```

This is conservative: no false positives for event IDs, counters, or other
numeric fields. Only triggers on field names that clearly suggest boolean
semantics.

### Tests (5 new, in substitute.rs)

| Test name | Input | Expected |
|-----------|-------|----------|
| `resolve_enum_bool_field_true` | `resolve_enum("1", "isRoaming")` | `Some("Enabled")` |
| `resolve_enum_bool_field_false` | `resolve_enum("0", "enableFlag")` | `Some("Disabled")` |
| `resolve_enum_bool_field_false_str` | `resolve_enum("false", "RoamingEnabled")` | `Some("Disabled")` |
| `resolve_enum_non_bool_field` | `resolve_enum("1", "ProcessId")` | `None` |
| `resolve_enum_non_boolean_value` | `resolve_enum("42", "isActive")` | `None` |

---

## Phase 2: CRIM VMAP + Dumper + Parser (~180 lines)

### File changed: extractor.rs

#### 2a. CrimProviderData -- new field

```rust
pub struct CrimProviderData {
    // ... existing fields unchanged ...
    /// Raw value maps from CRIM MAPS element: map_name -> (raw_value, message_id).
    /// message_id is the RT_MESSAGETABLE resource ID for the display string.
    /// Option<u32>: None means 0xFFFFFFFF (no message).
    pub value_maps: HashMap<String, Vec<(u32, Option<u32>)>>,
}
```

#### 2b. MAPS element dispatch in parse_crim_provider_data()

In the element dispatch loop (around line 432), add:

```rust
} else if sig == b"MAPS" {
    result.value_maps = parse_maps_block(crim_data, elem_off);
}
```

New function `parse_maps_block()`:

```rust
/// Parse the MAPS element from a CRIM blob to extract VMAP definitions.
///
/// MAPS element format (at elem_off in crim_data):
///   [0..4)  "MAPS" signature
///   [4..8)  total_size (u32) -- size of entire MAPS element
///   [8..12) count (u32) -- number of map definitions
///   [12..)  map definitions, each as a sub-block
///
/// Each VMAP sub-block (found by scanning for "VMAP" signature within MAPS):
///   [0..4)  "VMAP" signature
///   [4..8)  subblock_size (u32) -- total size including header
///   [8..12) map_string_offset (u32) -- offset to CRIM-sized UTF-16LE name,
///           relative to subblock start
///   [12..16) entry_count (u32)
///   [16..)  entries (8 bytes each):
///     [0..4) identifier (u32) -- raw enum value
///     [4..8) message_identifier (u32) -- RT_MESSAGETABLE resource ID
///   After entries, at map_string_offset from subblock start:
///     CRIM-sized UTF-16LE string (the field name, e.g. "RoamingState")
///
/// Returns map_name -> Vec<(raw_value, Option<message_id>)>.
/// BMAP sub-blocks are ignored (not needed for boolean-like value maps).
fn parse_maps_block(
    crim_data: &[u8],
    elem_off: usize,
) -> HashMap<String, Vec<(u32, Option<u32>)>>;
```

Parsing strategy:
1. Verify "MAPS" signature at elem_off
2. Read total_size to bound the search region
3. Scan the MAPS data region for "VMAP" signature bytes (same pattern as `parse_crim_evnt_blocks` scans for "EVNT")
4. For each "VMAP" found, parse the sub-block:
   a. Read map_string_offset, resolve the map name using `read_crim_utf16_string()`
   b. Read entry_count, iterate entries
   c. For each entry: if message_identifier == 0xFFFF_FFFF, store None; otherwise store Some(msg_id)
5. Return the accumulated map

Uses existing `read_crim_utf16_string()` for name string parsing -- no new infrastructure.

### File changed: cmd_dump.rs

#### 2c. Resolve VMAP entries at dump time

In `extract_single_dll()`, after the CRIM LEVL/KEYW enrichment loop (line 384-413),
add VMAP resolution. The DLL's message table (`combined.messages`) already indexes
ALL RT_MESSAGETABLE entries by resource_id -- use it to resolve VMAP message_ids
to display strings at dump time.

```rust
// --- Resolve VMAP value maps from CRIM blobs ---
let mut resolved_maps: HashMap<String, HashMap<String, HashMap<u32, String>>> = HashMap::new();
if !combined.crim_blobs.is_empty() && !combined.messages.is_empty() {
    for provider in providers {
        if let Some(guid) = &provider.guid {
            for blob in &combined.crim_blobs {
                if let Some(crim) = parse_crim_provider_data(blob, guid) {
                    if !crim.value_maps.is_empty() {
                        let mut prov_maps: HashMap<String, HashMap<u32, String>> = HashMap::new();
                        for (map_name, entries) in &crim.value_maps {
                            let mut resolved: HashMap<u32, String> = HashMap::new();
                            for &(raw_val, msg_id_opt) in entries {
                                let display = msg_id_opt
                                    .and_then(|mid| combined.messages.get(&mid))
                                    .map(|msg| msg.message.clone())
                                    .unwrap_or_else(|| raw_val.to_string());
                                resolved.insert(raw_val, display);
                            }
                            prov_maps.insert(map_name.clone(), resolved);
                        }
                        resolved_maps.insert(provider.name.clone(), prov_maps);
                        break; // first matching blob
                    }
                }
            }
        }
    }
}
```

Alternative simpler approach (if per-provider grouping is not needed):

```rust
/// Resolve all VMAP entries for a given CRIM blob against the DLL message table.
/// Returns map_name -> (raw_value -> display_string).
fn resolve_crim_vmaps(
    crim: &CrimProviderData,
    dll_messages: &HashMap<u32, PeMessage>,
) -> HashMap<String, HashMap<u32, String>>;
```

Add this function to extractor.rs (since it depends on CrimProviderData).

Add a `value_maps_by_provider` field to DllResult:

```rust
struct DllResult {
    // ... existing fields ...
    /// provider_name -> (field_name -> (raw_value -> display_string))
    value_maps_by_provider: HashMap<String, HashMap<String, HashMap<u32, String>>>,
}
```

In `run_dump()`, collect these into a flat list and write to the DB (same pattern
as `all_param_refs`):

```rust
let mut all_value_maps: Vec<(String, HashMap<String, HashMap<u32, String>>)> = Vec::new();
for result in &dll_results {
    for (provider_name, maps) in &result.value_maps_by_provider {
        if !maps.is_empty() {
            all_value_maps.push((provider_name.clone(), maps.clone()));
        }
    }
}
```

Then write after the main DB creation (alongside parameter_refs, step 10):

```rust
// --- Write value_maps ---
if !all_value_maps.is_empty() {
    for (provider_name, maps) in &all_value_maps {
        insert_value_maps(&conn, provider_name, maps)?;
    }
}
```

### File changed: writer_sys.rs

#### 2d. value_maps table DDL

Add a new table constant:

```sql
CREATE TABLE IF NOT EXISTS value_maps (
    provider_name TEXT NOT NULL,
    map_name      TEXT NOT NULL,
    raw_value     INTEGER NOT NULL,
    display_string TEXT,
    PRIMARY KEY (provider_name, map_name, raw_value)
);
```

Add `CREATE_VALUE_MAPS_TABLE` to the schema creation in `create_v4_schema()`.

#### 2e. insert_value_maps() function

```rust
/// Insert resolved value map entries into the value_maps table.
pub fn insert_value_maps(
    conn: &Connection,
    provider_name: &str,
    value_maps: &HashMap<String, HashMap<u32, String>>,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO value_maps (provider_name, map_name, raw_value, display_string)
         VALUES (?1, ?2, ?3, ?4)"
    )?;
    for (map_name, entries) in value_maps {
        for (&raw_val, display) in entries {
            stmt.execute(params![provider_name, map_name, raw_val, display])?;
        }
    }
    Ok(())
}
```

Re-export from `lib.rs`:
```rust
pub use writer_sys::{..., insert_value_maps};
```

### File changed: resolver.rs

#### 2f. MessageDb -- load value_maps at open()

New field:

```rust
pub struct MessageDb {
    conn: Connection,
    cache: HashMap<(String, u32), MessageRow>,
    param_refs: HashMap<u32, String>,
    /// (provider_name, map_name, raw_value) -> display_string
    value_maps: HashMap<(String, String, u32), String>,
}
```

New method:

```rust
impl MessageDb {
    pub fn load_value_maps(&mut self) -> Result<()> {
        // Gracefully handle missing table (older DBs)
        let table_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master \
             WHERE type='table' AND name='value_maps'",
            [], |row| row.get(0),
        ).unwrap_or(false);
        if !table_exists { return Ok(()); }

        let mut stmt = self.conn.prepare(
            "SELECT provider_name, map_name, raw_value, display_string \
             FROM value_maps"
        )?;
        let rows = stmt.query_map([], |row| {
            let provider: String = row.get(0)?;
            let map_name: String = row.get(1)?;
            let raw_val: i64 = row.get(2)?;
            let display: Option<String> = row.get(3)?;
            Ok(((provider, map_name, raw_val as u32), display))
        })?;
        for row in rows {
            if let Ok((key, Some(display))) = row {
                self.value_maps.insert(key, display);
            }
        }
        Ok(())
    }

    pub fn get_value_maps(&self) -> &HashMap<(String, String, u32), String> {
        &self.value_maps
    }
}
```

Call `load_value_maps()` from `cmd_parse.rs::run_parse()` alongside `load_param_refs()`:

```rust
if let Err(e) = db.load_value_maps() {
    eprintln!("[warn] failed to load value maps: {e}");
}
```

### File changed: cmd_parse.rs

#### 2g. Wire value_maps into process_parallel_batch()

In `process_parallel_batch()`, after `substitute()` produces the message
(line ~288), add a second pass: scan the resolved params for numeric values
that have value_map entries, and apply them.

The cleanest integration point is between `extract_subst_params()` and `substitute()`:

```rust
// --- Existing: extract params ---
let params = extract_subst_params(
    &event.event_data,
    event_data_value.as_ref(),
    &row.parameters,
);

// --- New: resolve params via value_maps ---
let resolved_params: Vec<String> = if let Ok(guard) = db.lock() {
    let provider = match event.provider_name.as_ref() {
        Some(p) => p.as_str(),
        None => params.iter().map(|s| s.to_string()).collect(),
    };
    let vmaps = guard.get_value_maps();
    params.iter().enumerate().map(|(i, val)| {
        let param_idx = i + 1;
        // Phase 1: standard type conventions
        if let Some(name) = row.parameters.as_ref()
            .and_then(|p| p.get(&param_idx.to_string()))
        {
            if let Some(resolved) = resolve_enum(val, name) {
                return resolved;
            }
        }
        // Phase 2: value maps from DB
        let key = (provider.to_string(), param_idx.to_string(), val.parse().unwrap_or(0));
        if let Some(display) = vmaps.get(&key) {
            return display.clone();
        }
        val.to_string()
    }).collect()
} else {
    params.iter().map(|s| s.to_string()).collect()
};

// --- Existing: substitute ---
let param_refs: Vec<&str> = resolved_params.iter().map(|s| s.as_str()).collect();
event.message = Some(substitute(&row.template, &param_refs));
```

**Note on look-up granularity:** The value_maps key is `(provider_name, map_name, raw_value)`.
The map_name in VMAP is the field name (e.g. "isRoaming"). For the param index -> field name
lookup, we use `row.parameters` (which maps "1" -> "TargetUserName", "2" -> "TargetDomain",
etc.). The actual look-up key should be `(provider, row.parameters["N"], raw_value)`.

Revised look-up:

```rust
// Phase 2: value maps from DB
if let Some(field_name) = row.parameters.as_ref()
    .and_then(|p| p.get(&param_idx.to_string()))
{
    let vmap_key = (provider.to_string(), field_name.clone(), val.parse().unwrap_or(0));
    if let Some(display) = vmaps.get(&vmap_key) {
        return display.clone();
    }
}
```

### Test strategy (Phase 2)

| Test | File | What |
|------|------|------|
| `test_parse_maps_block_synthetic` | extractor.rs | Build synthetic MAPS with one VMAP, verify parsing |
| `test_parse_maps_block_empty` | extractor.rs | Empty/corrupt MAPS returns empty |
| `test_value_maps_table_schema` | writer_sys.rs | Table creation, column types, PK constraint |
| `test_insert_value_maps_roundtrip` | writer_sys.rs | Insert entries, read back |
| `test_message_db_load_value_maps` | resolver.rs | Create DB with value_maps table, load via MessageDb |
| `test_resolve_enum_wired_in_substitute` | substitute.rs | Phase 1 + Phase 2 full chain (unit with mocked vmaps) |

### Commit messages

Phase 1:
```
feat(%N): Layer 1 enum resolution -- standard type conventions

Add resolve_enum() to substitute.rs: heuristic boolean field detection.
Fields containing "bool", "enable", "roam", "flag" map 0/false -> "Disabled".
BITS event 209 isRoaming now shows "Enabled" instead of "1".
```

Phase 2:
```
feat(CRIM): parse MAPS element and resolve value maps in dumper+schema

- Add parse_maps_block() to extractor.rs for CRIM MAPS element parsing
- Resolve VMAP message_ids against DLL message table at dump time
- Add value_maps table and insert_value_maps() to writer_sys.rs
- Add value_maps field and load_value_maps() to resolver.rs MessageDb
- Wire value_map resolution into process_parallel_batch() in cmd_parse.rs
- Re-export new types from hotech-evtx-dumper lib.rs
```

---

## Summary of all changes

| File | Phase | Change |
|------|-------|--------|
| `hotech-evtx-core/src/substitute.rs` | 1 | Add `resolve_enum()` + 5 tests |
| `hotech-evtx-dumper/src/extractor.rs` | 2a, 2b | Add `value_maps` to CrimProviderData, `parse_maps_block()`, `resolve_crim_vmaps()` |
| `hotech-evtx-dumper/src/writer_sys.rs` | 2d, 2e | Add `value_maps` table DDL, `insert_value_maps()`, wire into `create_v4_schema()` |
| `hotech-evtx-dumper/src/lib.rs` | 2e | Re-export `insert_value_maps` |
| `hotech-evtx-core/src/resolver.rs` | 2f | Add `value_maps` field, `load_value_maps()`, `get_value_maps()` |
| `hotech-evtx/src/cmd_dump.rs` | 2c | Resolve VMAP in `extract_single_dll()`, collect in `run_dump()`, write to DB |
| `hotech-evtx/src/cmd_parse.rs` | 2g | Apply value_maps + resolve_enum in `process_parallel_batch()` |

Lines of code: Phase 1 ~50, Phase 2 ~180 = ~230 total.
New crates: 0. New files: 0.

//! Message resolution engine for Windows Event Log messages.
//!
//! Provides [`MessageDb`], an in-memory cache over a SQLite message database
//! created by the external [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) tool.
//! The cache is populated via [`MessageDb::warmup`], which does a single batched SELECT query.
//!
//! # Schema
//!
//! The expected SQLite `messages` table uses this schema:
//!
//! ```sql
//! CREATE TABLE messages (
//!     provider_name TEXT NOT NULL,
//!     lang          TEXT NOT NULL DEFAULT 'en-US',
//!     event_id      INTEGER NOT NULL,
//!     message       TEXT NOT NULL,
//!     level         INTEGER,
//!     task          TEXT,
//!     opcode        TEXT,
//!     keywords      TEXT,
//!     parameters    TEXT,
//!     PRIMARY KEY (provider_name, lang, event_id)
//! );
//! ```

use rusqlite::Connection;

use crate::{substitute::expand_param_refs, MessageRow, Result};

use std::collections::{HashMap, HashSet};

/// An in-memory cached view of a SQLite message database.
///
/// # Usage
///
/// 1. [`open`](MessageDb::open) the database file.
/// 2. (Optional) Call [`load_param_refs`](MessageDb::load_param_refs) once to
///    populate the `%%N` parameter reference cache.
/// 3. Call [`warmup`](MessageDb::warmup) with the provider/event-ID pairs the
///    application needs (typically gathered from parsed EVTX events).
/// 4. Look up resolved messages with [`get`](MessageDb::get), then expand
///    `%%N` references with [`expand_message`](MessageDb::expand_message).
pub struct MessageDb {
    pub(crate) conn: Connection,
    /// Variant cache: VariantKey -> MessageRow (single-file v7, 8-tuple precise).
    /// The legacy single-template `cache` is deleted (ADR 0003).
    pub(crate) variant_cache: HashMap<crate::VariantKey, MessageRow>,
    /// `%%N` parameter reference cache: provider_name → (param_id → expanded text).
    /// R17（2026-09-04）：provider 维度。全局平铺会让 provider 私有 %%N 与 Win32
    /// 系统错误码互撞（SCM %%2 要 Win32 的 2，某 provider %%2 是私有文本）。
    /// 特殊键 "Win32-System-Error" 作回退层（provider 私有缺 id 时补）。
    param_refs: HashMap<String, HashMap<u32, String>>,
    /// CRIM value map cache: map_name → {value_str → display_name}.
    /// Populated from the `value_maps` table (VMAP/BMAP blocks).
    value_maps: HashMap<String, HashMap<String, String>>,
    /// Set of map names that are bitmaps (is_bitmap=1 in DB).
    bitmap_names: HashSet<String>,
    /// Registered event source names (R12 / ADR 0004): classic EventLog
    /// sources ∪ WINEVT publisher names. Optional table — an absent or empty
    /// snapshot means "registry state unknown": `is_registered_source` then
    /// reports everything registered and the N/A-parity path stays off.
    registry_sources: HashSet<String>,
    /// Per-parameter TDH InType/OutType hints (R10 / ADR 0005):
    /// (provider, event_id) → param_index → (in:out hex pair, param_name).
    /// The param_name makes the hint template-scoped: variants whose param at
    /// that index has a different name must not inherit it.
    param_types_cache: HashMap<(String, u32), HashMap<String, (String, String)>>,
}

impl MessageDb {
    /// Open a single-file message database (schema_version 7, ADR 0003).
    ///
    /// The database is expected to have been created by the
    /// [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) tool and contain the
    /// `provider_message_variants` table. Anything else is rejected.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| format!("failed to open messages db '{path}': {e}"))?;
        let version: String = conn
            .query_row(
                "SELECT value FROM metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("messages db '{path}' has no schema_version (not a v8 single-file DB): {e}"))?;
        if version != "8" {
            return Err(format!(
                "messages db '{path}' schema_version={version}, want 8: re-run msgdb-dump (single-file, ADR 0003/0004/0005)"
            ));
        }
        Ok(MessageDb {
            conn,
            variant_cache: HashMap::new(),
            param_refs: HashMap::new(),
            value_maps: HashMap::new(),
            bitmap_names: HashSet::new(),
            registry_sources: HashSet::new(),
            param_types_cache: HashMap::new(),
        })
    }

    /// Load per-parameter TDH InType/OutType hints from the `param_types`
    /// table (R10 / ADR 0005). The table is part of the v8 schema — absence
    /// is a hard error (re-run msgdb-dump). No rows for a cohort = passthrough.
    pub fn load_param_types(&mut self) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT provider_name, event_id, param_index, param_name, in_type, out_type FROM param_types")
            .map_err(|e| format!("messages db lacks param_types (want schema_version 8, re-run msgdb-dump, ADR 0005): {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let provider: String = row.get(0)?;
                let eid: i64 = row.get(1)?;
                let idx: String = row.get(2)?;
                let pname: String = row.get(3)?;
                let intype: i64 = row.get(4)?;
                let outtype: i64 = row.get(5)?;
                Ok((provider, eid as u32, idx, pname, intype as u8, outtype as u8))
            })
            .map_err(|e| format!("failed to query param_types: {e}"))?;
        for row in rows {
            let Ok((provider, eid, idx, pname, intype, outtype)) = row else { continue };
            self.param_types_cache
                .entry((provider, eid))
                .or_default()
                .insert(idx, (format!("{intype:02x}:{outtype:02x}"), pname));
        }
        Ok(())
    }

    /// Win32 system error message (R15 table, "Win32-System-Error" group).
    /// Used by HRESULT typed rendering (R10): a code like 0x8007007B carries
    /// the Win32 code 0x7B in its low word.
    pub fn lookup_win32_error(&self, code: u32) -> Option<String> {
        self.param_refs
            .get("Win32-System-Error")?
            .get(&code)
            .cloned()
    }

    /// NTSTATUS message text (Phase E table, "NTSTATUS-System-Error" group).
    /// ntdll's RT_MESSAGETABLE extracted at dump time; used by typed
    /// rendering for OutType 0x1F (Ntfs 140 `Failure status`).
    pub fn lookup_ntstatus(&self, code: u32) -> Option<String> {
        self.param_refs
            .get("NTSTATUS-System-Error")?
            .get(&code)
            .cloned()
    }

    /// Load the registered event source snapshot from the `eventlog_sources`
    /// table (R12 / ADR 0004). The table is part of the v8 schema — absence
    /// is a hard error (re-run msgdb-dump). An empty snapshot (collection
    /// failure) reads as "registry state unknown": `registry_known()` is then
    /// false and the N/A-parity path stays off.
    pub fn load_registry_sources(&mut self) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT source_name FROM eventlog_sources")
            .map_err(|e| format!("messages db lacks eventlog_sources (want schema_version 8, re-run msgdb-dump, ADR 0004): {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("failed to query eventlog_sources: {e}"))?;
        for row in rows {
            if let Ok(name) = row {
                self.registry_sources.insert(name);
            }
        }
        Ok(())
    }

    /// True when the registry snapshot is known (table present and non-empty).
    pub fn registry_known(&self) -> bool {
        !self.registry_sources.is_empty()
    }

    /// Whether `name` is a registered event source. An unknown registry state
    /// (empty snapshot) conservatively reports every source as registered.
    pub fn is_registered_source(&self, name: &str) -> bool {
        if self.registry_sources.is_empty() {
            return true;
        }
        self.registry_sources.contains(name)
    }

    /// Load all `%%N` parameter references from the `parameter_refs` table.
    ///
    /// This populates the in-memory `param_refs` cache used by
    /// [`expand_message`](MessageDb::expand_message). Single-file v7 always
    /// has this table; a missing table is a hard error.
    pub fn load_param_refs(&mut self) -> Result<()> {
        // R17（2026-09-04）：按 provider 分组加载。R15 的 Win32-System-Error 全局行
        // 与 provider 私有 %%N 同 param_id 互撞（SCM %%2 要 Win32 的 2，私有 %%2 是别的文本），
        // 平铺无法区分。Win32-System-Error 单独成组作回退层。
        let mut stmt = self
            .conn
            .prepare("SELECT provider_name, param_id, message FROM parameter_refs")
            .map_err(|e| format!("failed to prepare param_refs query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let provider: String = row.get(0)?;
                let param_id: i64 = row.get(1)?;
                let message: String = row.get(2)?;
                Ok((provider, param_id as u32, message))
            })
            .map_err(|e| format!("failed to query parameter_refs: {e}"))?;

        for row in rows {
            if let Ok((provider, param_id, message)) = row {
                self.param_refs.entry(provider).or_default().insert(param_id, message);
            }
        }

        Ok(())
    }

    /// Expand `%%N` parameter references in a message string.
    ///
    /// R17：provider 私有 %%N 优先，Win32-System-Error 作回退（仅补私有缺失的 id）。
    /// If the cache is empty, the message is returned unchanged.
    pub fn expand_message(&self, message: &str, provider_name: &str) -> String {
        if self.param_refs.is_empty() {
            return message.to_string();
        }
        if !message.contains("%%") {
            return message.to_string();
        }
        let mut merged: HashMap<u32, String> = HashMap::new();
        if let Some(priv_map) = self.param_refs.get(provider_name) {
            merged.extend(priv_map.iter().map(|(k, v)| (*k, v.clone())));
        }
        if let Some(sys) = self.param_refs.get("Win32-System-Error") {
            for (k, v) in sys {
                merged.entry(*k).or_insert_with(|| v.clone());
            }
        }
        if merged.is_empty() {
            return message.to_string();
        }
        expand_param_refs(message, &merged)
    }

    /// Load all value maps from the `value_maps` table.
    ///
    /// Populates the in-memory `value_maps` cache used by
    /// [`get_value_map`](MessageDb::get_value_map) and other resolution methods.
    /// Also populates `bitmap_names` for map entries where `is_bitmap=1`.
    /// Single-file v7 always has this table; a missing table is a hard error.
    pub fn load_value_maps(&mut self) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT map_name, entry_value, display_name, is_bitmap FROM value_maps")
            .map_err(|e| format!("failed to prepare value_maps query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let map_name: String = row.get(0)?;
                let entry_value: String = row.get(1)?;
                let display_name: String = row.get(2)?;
                let is_bitmap: i32 = row.get(3)?;
                Ok((map_name, entry_value, display_name, is_bitmap))
            })
            .map_err(|e| format!("failed to query value_maps: {e}"))?;

        for row in rows {
            if let Ok((map_name, entry_value, display_name, is_bitmap)) = row {
                if is_bitmap != 0 {
                    self.bitmap_names.insert(map_name.clone());
                }
                self.value_maps
                    .entry(map_name)
                    .or_default()
                    .insert(entry_value, display_name);
            }
        }

        Ok(())
    }

    /// Look up a value in the value_maps cache.
    ///
    /// Returns `None` if the map_name or value is not found.
    pub fn get_value_map(&self, map_name: &str, value: &str) -> Option<&str> {
        self.value_maps
            .get(map_name)
            .and_then(|entries| entries.get(value))
            .map(|s| s.as_str())
    }

    /// Get value maps for a set of parameter names.
    ///
    /// Given a `param_map` (e.g. `{"1": "LogonType", "2": "TargetUserName"}`),
    /// returns a map of field_name → {value_str → display_name} for any field
    /// names that have value map entries. This allows the caller to clone only
    /// the relevant value maps without holding the lock.
    pub fn get_param_value_maps(
        &self,
        param_map: &std::collections::HashMap<String, String>,
    ) -> HashMap<String, HashMap<String, String>> {
        let mut result = HashMap::new();
        for (_idx, field_name) in param_map {
            if let Some(entries) = self.value_maps.get(field_name) {
                result.insert(field_name.clone(), entries.clone());
            }
        }
        result
    }

    /// Resolve a raw numeric enumeration value to its display string using
    /// the value_maps cache.
    ///
    /// `provider` is reserved for future per-provider scoping (currently
    /// unused — the cache is keyed by `map_name` alone).
    /// `map_name` is the CRIM VMAP/BMAP name (e.g. "LogonType").
    /// `raw_value` is the raw integer value from the event data.
    ///
    /// Returns `None` when the map_name or raw_value is not found.
    pub fn resolve_value(&self, _provider: &str, map_name: &str, raw_value: u32) -> Option<String> {
        self.value_maps
            .get(map_name)
            .and_then(|entries| entries.get(&raw_value.to_string()))
            .cloned()
    }

    /// Returns true when value maps are loaded and available.
    pub fn has_value_maps(&self) -> bool {
        !self.value_maps.is_empty()
    }

    /// Returns the set of bitmap map names loaded from `value_maps`.
    pub fn get_bitmap_names(&self) -> HashSet<String> {
        self.bitmap_names.clone()
    }

    /// Attach per-parameter value map references to warmed variant rows.
    ///
    /// Single-file v7: the legacy single-template `cache` is gone. Variant rows
    /// are warmed by `warmup_variants`; this loads `param_value_maps` for the
    /// (provider, event_id) cohorts and attaches them to every variant row.
    pub fn warmup(&mut self, keys: &[crate::VariantKey], lang: &str) -> Result<()> {
        self.warmup_variants(keys, lang)?;
        let mut cohorts: Vec<(String, u32)> = Vec::new();
        let mut seen: HashSet<(String, u32)> = HashSet::new();
        for k in keys {
            let c = (k.provider_name.clone(), k.event_id);
            if seen.insert(c.clone()) {
                cohorts.push(c);
            }
        }
        let refs: Vec<(&str, u32)> = cohorts.iter().map(|(p, e)| (p.as_str(), *e)).collect();
        self.load_param_value_maps(&refs)?;
        self.attach_param_types(&refs)
    }

    /// Attach param_types hints to every warmed variant row of each
    /// (provider, event_id) cohort (R10 / ADR 0005). A hint is template-
    /// scoped: if the collected param_name disagrees with the variant's own
    /// name at that index (same event, different template variant), the hint
    /// is skipped — KGA 1 index 3 is "Reason" in v3 but "TimeDeltaInMs" in v4.
    fn attach_param_types(&mut self, providers: &[(&str, u32)]) -> Result<()> {
        for (provider, event_id) in providers {
            let Some(types) = self.param_types_cache.get(&((*provider).to_string(), *event_id))
            else {
                continue;
            };
            for (k, v) in self.variant_cache.iter_mut() {
                if k.provider_name == *provider && k.event_id == *event_id {
                    let mut matched = HashMap::new();
                    for (idx, (spec, pname)) in types {
                        let name_ok = match v.parameters.as_ref().and_then(|pm| pm.get(idx)) {
                            // empty collected name = legacy/unknown → trust
                            Some(actual) => pname.is_empty() || actual == pname,
                            None => true, // no parameters JSON — trust the hint
                        };
                        if name_ok {
                            matched.insert(idx.clone(), spec.clone());
                        }
                    }
                    v.param_types = matched;
                }
            }
        }
        Ok(())
    }

    /// Load per-parameter value map references from the param_value_maps
    /// table for the warmed (provider, event_id) pairs.
    ///
    /// Each row maps a param index to the value_maps key holding that
    /// parameter's display entries. Single-file v7 always has this table;
    /// a missing table is a hard error.
    fn load_param_value_maps(&mut self, providers: &[(&str, u32)]) -> Result<()> {
        if providers.is_empty() {
            return Ok(());
        }

        let n = providers.len();
        let mut conditions: Vec<String> = Vec::with_capacity(n);
        let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(n * 2);
        for (i, (provider, event_id)) in providers.iter().enumerate() {
            let p_idx = i * 2 + 1;
            let e_idx = i * 2 + 2;
            conditions.push(format!("(provider_name = ?{p_idx} AND event_id = ?{e_idx})"));
            params.push(rusqlite::types::Value::Text(provider.to_string()));
            params.push(rusqlite::types::Value::Integer(*event_id as i64));
        }

        let query = format!(
            "SELECT provider_name, event_id, param_index, param_name, map_name, is_bitmap              FROM param_value_maps WHERE ({})",
            conditions.join(" OR ")
        );
        let mut stmt = self.conn.prepare(&query)
            .map_err(|e| format!("param_value_maps query failed (want v7 single-file DB): {e}"))?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let provider: String = row.get(0)?;
            let event_id: i64 = row.get(1)?;
            let param_index: String = row.get(2)?;
            let param_name: String = row.get(3)?;
            let map_name: String = row.get(4)?;
            let is_bitmap: i64 = row.get(5)?;
            Ok((
                provider,
                event_id as u32,
                param_index,
                param_name,
                map_name,
                is_bitmap != 0,
            ))
        })
        .map_err(|e| format!("param_value_maps query failed: {e}"))?;

        for row in rows {
            let Ok((provider, event_id, param_index, param_name, map_name, is_bitmap)) = row else {
                continue;
            };
            // Attach to every variant row of the (provider, event_id) cohort.
            // The link is template-scoped (batch 1, 2026-09-08): skip when the
            // variant's own name at this index disagrees with the collected
            // param_name — KGA 1 index 3 is "Reason" in v3 but
            // "TimeDeltaInMs" in v4; a v3 link must not map the v4 delta.
            for (k, v) in self.variant_cache.iter_mut() {
                if k.provider_name == provider && k.event_id == event_id {
                    let name_ok = match v.parameters.as_ref().and_then(|pm| pm.get(&param_index)) {
                        // empty collected name = legacy/unknown → trust
                        Some(actual) => param_name.is_empty() || actual == &param_name,
                        None => true, // no parameters JSON — trust the link
                    };
                    if !name_ok {
                        continue;
                    }
                    v.param_map_refs.insert(param_index.clone(), map_name.clone());
                    if is_bitmap {
                        v.param_bitmap_refs.insert(param_index.clone());
                    }
                }
            }
        }
        Ok(())
    }

    /// Resolve event-scoped parameter value maps for a message row.
    ///
    /// Maps each param index through row.param_map_refs to a map name, then
    /// to the entries loaded from the value_maps table. Returns the map of
    /// field_name to entries plus the set of field names that are bitmaps.
    ///
    /// This is precise: the association comes from the CRIM TEMP item
    /// descriptor (+8 field), so "Reason" resolves to the exact map the
    /// provider's manifest references (e.g. Pop:PowerStateEvent), avoiding
    /// the ambiguity of name-based matching.
    pub fn resolve_param_value_maps(
        &self,
        row: &MessageRow,
    ) -> (HashMap<String, HashMap<String, String>>, HashSet<String>) {
        let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut bitmaps: HashSet<String> = HashSet::new();
        let Some(params) = &row.parameters else {
            return (result, bitmaps);
        };
        for (idx, map_name) in &row.param_map_refs {
            let Some(field_name) = params.get(idx) else {
                continue;
            };
            let Some(entries) = self.value_maps.get(map_name) else {
                continue;
            };
            result.insert(field_name.clone(), entries.clone());
            if row.param_bitmap_refs.contains(idx) {
                bitmaps.insert(field_name.clone());
            }
        }
        (result, bitmaps)
    }

    /// Returns true when `%%N` parameter references are loaded and available.
    pub fn has_param_refs(&self) -> bool {
        !self.param_refs.is_empty()
    }

    /// Return a reference to the underlying SQLite connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use crate::VariantKey;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DB_SEQ: AtomicU64 = AtomicU64::new(0);

    /// Unique temp file path per test DB (MessageDb::open only opens files).
    fn tmp_path(name: &str) -> PathBuf {
        let n = TEST_DB_SEQ.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("evtx_v7_{}_{}_{}.db", name, std::process::id(), n))
    }

    /// Create the 7 single-file v7 tables (ADR 0003, schema_version 7).
    fn create_v8_schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE dll_entries (
                dll_id      INTEGER PRIMARY KEY AUTOINCREMENT,
                dll_name    TEXT NOT NULL,
                dll_path    TEXT NOT NULL UNIQUE,
                pe_version  TEXT NOT NULL DEFAULT '0.0.0.0'
            );
            CREATE TABLE providers (
                provider_name TEXT NOT NULL PRIMARY KEY,
                guid          TEXT,
                channel       TEXT,
                dll_id        INTEGER NOT NULL REFERENCES dll_entries(dll_id)
            );
            CREATE TABLE provider_message_variants (
                provider_name      TEXT NOT NULL,
                provider_guid      TEXT NOT NULL,
                lang               TEXT NOT NULL DEFAULT 'en-US',
                event_id           INTEGER NOT NULL,
                version            INTEGER NOT NULL DEFAULT 0,
                level              INTEGER NOT NULL DEFAULT 0,
                opcode             INTEGER NOT NULL DEFAULT 0,
                task               INTEGER NOT NULL DEFAULT 0,
                keywords           TEXT NOT NULL DEFAULT '0',
                channel            INTEGER NOT NULL DEFAULT 0,
                message_identifier INTEGER NOT NULL,
                dll_id             INTEGER NOT NULL REFERENCES dll_entries(dll_id),
                message            TEXT NOT NULL,
                parameters         TEXT,
                level_name         TEXT,
                keywords_name      TEXT,
                PRIMARY KEY (provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)
            );
            CREATE TABLE metadata (
                key   TEXT NOT NULL PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE parameter_refs (
                provider_name TEXT NOT NULL,
                param_id      INTEGER NOT NULL,
                message       TEXT NOT NULL,
                lang          TEXT NOT NULL DEFAULT 'en-US',
                PRIMARY KEY (provider_name, param_id, lang)
            );
            CREATE TABLE value_maps (
                provider_name TEXT NOT NULL,
                map_name      TEXT NOT NULL,
                entry_value   TEXT NOT NULL,
                display_name  TEXT NOT NULL,
                is_bitmap     INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (provider_name, map_name, entry_value)
            );
            CREATE TABLE param_value_maps (
                provider_name TEXT NOT NULL,
                event_id      INTEGER NOT NULL,
                param_index   TEXT NOT NULL,
                param_name    TEXT NOT NULL DEFAULT '',
                map_name      TEXT NOT NULL,
                is_bitmap     INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (provider_name, event_id, param_index)
            );
            CREATE TABLE eventlog_sources (
                source_name TEXT NOT NULL PRIMARY KEY,
                source_type TEXT NOT NULL DEFAULT 'classic'
            );
            CREATE TABLE param_types (
                provider_name TEXT NOT NULL,
                event_id      INTEGER NOT NULL,
                param_index   TEXT NOT NULL,
                param_name    TEXT NOT NULL DEFAULT '',
                in_type       INTEGER NOT NULL,
                out_type      INTEGER NOT NULL,
                PRIMARY KEY (provider_name, event_id, param_index)
            );",
        )
        .unwrap();
    }

    fn insert_metadata_8(conn: &Connection) {
        conn.execute(
            "INSERT INTO metadata(key, value) VALUES('schema_version', '8')",
            [],
        )
        .unwrap();
    }

    /// Standard v7 fixture file DB: 1 DLL + Test-Provider with one 4624 and
    /// one 4625 variant, one parameter_ref, one LogonType value map entry
    /// and the (Test-Provider, 4624, param 1 -> LogonType) link.
    fn setup_v8_db(path: &Path) {
        let _ = std::fs::remove_file(path);
        let conn = Connection::open(path).unwrap();
        create_v8_schema(&conn);
        insert_metadata_8(&conn);
        conn.execute(
            "INSERT INTO dll_entries(dll_id, dll_name, dll_path) VALUES(1, 'test.dll', 'C:/test.dll')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO providers(provider_name, guid, channel, dll_id) VALUES('Test-Provider', '{TP-GUID}', 'Security', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message, parameters) VALUES('Test-Provider', '{TP-GUID}', 'en-US', 4624, 0, 4, 0, 0, '0', 0, 1001, 1, 'An account was successfully logged on. Account: %1', '{"1":"TargetUserName"}')"#,
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message) VALUES('Test-Provider', '{TP-GUID}', 'en-US', 4625, 0, 4, 0, 0, '0', 0, 1002, 1, 'An account failed to log on.')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO parameter_refs(provider_name, param_id, message, lang) VALUES('Test-Provider', 1843, '虚拟帐户', 'en-US')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO value_maps(provider_name, map_name, entry_value, display_name, is_bitmap) VALUES('Test-Provider', 'LogonType', '2', 'Interactive', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO param_value_maps(provider_name, event_id, param_index, param_name, map_name, is_bitmap) VALUES('Test-Provider', 4624, '1', 'TargetUserName', 'LogonType', 0)",
            [],
        )
        .unwrap();
        drop(conn);
    }

    /// VariantKey matching the 4624 fixture row in setup_v7_db.
    fn key_4624() -> VariantKey {
        VariantKey {
            provider_name: "Test-Provider".into(),
            event_id: 4624,
            version: 0,
            level: 4,
            opcode: 0,
            task: 0,
            keywords: 0,
            channel: 0,
            message_id: 1001,
        }
    }

    /// VariantKey matching the 4625 fixture row in setup_v7_db.
    fn key_4625() -> VariantKey {
        VariantKey {
            provider_name: "Test-Provider".into(),
            event_id: 4625,
            version: 0,
            level: 4,
            opcode: 0,
            task: 0,
            keywords: 0,
            channel: 0,
            message_id: 1002,
        }
    }

    #[test]
    fn message_db_open_nonexistent_fails() {
        let result = MessageDb::open("/nonexistent/path/to/messages.db");
        assert!(result.is_err());
    }

    #[test]
    fn v8_open_rejects_legacy_v7_and_metadata_less_db() {
        // Legacy DB: old single-template table + schema_version=5.
        let legacy = tmp_path("legacy_v5");
        let _ = std::fs::remove_file(&legacy);
        {
            let conn = Connection::open(&legacy).unwrap();
            conn.execute_batch(
                "CREATE TABLE dll_messages(dll_id INTEGER, lang TEXT, event_id INTEGER, message TEXT);
                 CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO metadata(key, value) VALUES('schema_version', '5')",
                [],
            )
            .unwrap();
            drop(conn);
        }
        assert!(MessageDb::open(legacy.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(&legacy);

        // Pre-R12/R10 v7 DB: right tables, old version stamp — rejected, re-run dump.
        let v7 = tmp_path("old_v7");
        let _ = std::fs::remove_file(&v7);
        {
            let conn = Connection::open(&v7).unwrap();
            create_v8_schema(&conn);
            conn.execute("DROP TABLE eventlog_sources", []).unwrap();
            conn.execute("DROP TABLE param_types", []).unwrap();
            conn.execute(
                "INSERT INTO metadata(key, value) VALUES('schema_version', '7')",
                [],
            )
            .unwrap();
            drop(conn);
        }
        let err = match MessageDb::open(v7.to_str().unwrap()) {
            Err(e) => e,
            Ok(_) => panic!("v7 DB must be rejected"),
        };
        assert!(
            err.contains("want 8"),
            "v7 DB must be rejected with a re-run hint, got: {err}"
        );
        let _ = std::fs::remove_file(&v7);

        // DB without a metadata table at all.
        let no_meta = tmp_path("no_metadata");
        let _ = std::fs::remove_file(&no_meta);
        {
            let conn = Connection::open(&no_meta).unwrap();
            conn.execute_batch(
                "CREATE TABLE provider_message_variants(provider_name TEXT, lang TEXT, event_id INTEGER, message TEXT);",
            )
            .unwrap();
            drop(conn);
        }
        assert!(MessageDb::open(no_meta.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(&no_meta);
    }

    #[test]
    fn warmup_and_resolve() {
        let tmp = tmp_path("warmup_resolve");
        setup_v8_db(&tmp);
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_value_maps().unwrap();
        db.warmup(&[key_4624()], "en-US").unwrap();

        let row = db.resolve_variant(&key_4624()).expect("4624 variant resolves");
        assert!(row.template.contains("%1"));
        assert_eq!(
            row.parameters.as_ref().unwrap().get("1").map(|s| s.as_str()),
            Some("TargetUserName")
        );

        // Event-scoped param->map reference attached per cohort during warmup.
        assert_eq!(
            row.param_map_refs.get("1").map(|s| s.as_str()),
            Some("LogonType")
        );
        assert!(!row.param_bitmap_refs.contains("1"));

        // Field name comes from parameters[idx], map key from param_map_refs[idx].
        let (vms, bitmaps) = db.resolve_param_value_maps(row);
        let entries = vms.get("TargetUserName").expect("TargetUserName should resolve");
        assert_eq!(entries.get("2").map(|s| s.as_str()), Some("Interactive"));
        assert!(!bitmaps.contains("TargetUserName"));

        let _ = std::fs::remove_file(&tmp);
    }

    /// SPP 16388/1016 同构回归：事件 task=0（wevtapi Task=N/A），同 EID 下双 task variant
    /// （如 16388 task=64 `The current time is` mid=1073758212 vs task=192 `Authorized
    /// upgrade` mid=3221241860）。live 取证 wevtapi 选 message_identifier 最小者。
    /// ladder 平分必须按 mid 仲裁，不得依赖 HashMap 迭代顺序。
    #[test]
    fn ladder_tiebreak_prefers_smallest_message_id() {
        let tmp = tmp_path("ladder_tiebreak");
        let _ = std::fs::remove_file(&tmp);
        let conn = Connection::open(&tmp).unwrap();
        create_v8_schema(&conn);
        insert_metadata_8(&conn);
        conn.execute(
            "INSERT INTO dll_entries(dll_id, dll_name, dll_path) VALUES(1, 'spp.dll', 'C:/spp.dll')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO providers(provider_name, guid, channel, dll_id) VALUES('SPP', '{SPP-GUID}', 'Application', 1)",
            [],
        )
        .unwrap();
        // 插入顺序故意与期望相反（先 192 后 64）：复现生产 DB 行序，ladder 不得依赖行序/迭代序。
        conn.execute(
            "INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message) VALUES('SPP', '{SPP-GUID}', 'en-US', 16388, 0, 0, 192, 0, '36028797018963968', 0, 3221241860, 1, 'Authorized upgrade for PID: %1.')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message) VALUES('SPP', '{SPP-GUID}', 'en-US', 16388, 0, 0, 64, 0, '36028797018963968', 0, 1073758212, 1, 'The current time is %1 : %2.')",
            [],
        )
        .unwrap();
        drop(conn);
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.warmup(
            &[VariantKey {
                provider_name: "SPP".into(),
                event_id: 16388,
                version: 0,
                level: 4,
                opcode: 0,
                task: 0,
                keywords: 0,
                channel: 0,
                message_id: 0,
            }],
            "en-US",
        )
        .unwrap();
        // 确定性：同 key 解析 N 次必须每次都命中同一模板（HashMap 迭代顺序不得影响结果）。
        for _ in 0..50 {
            let row = db
                .resolve_variant(&VariantKey {
                    provider_name: "SPP".into(),
                    event_id: 16388,
                    version: 0,
                    level: 4,
                    opcode: 0,
                    task: 0,
                    keywords: 0,
                    channel: 0,
                    message_id: 0,
                })
                .expect("16388 must resolve");
            assert!(
                row.template.contains("The current time is"),
                "tiebreak must prefer smallest message_identifier, got: {}",
                row.template
            );
        }
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn warmup_empty_keys() {
        let tmp = tmp_path("warmup_empty");
        setup_v8_db(&tmp);
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.warmup(&[], "en-US").unwrap();
        assert!(db.resolve_variant(&key_4624()).is_none());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn cohort_miss_resolve_returns_none() {
        let tmp = tmp_path("cohort_miss");
        setup_v8_db(&tmp);
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        // Only the 4624 cohort is warmed; other cohorts must miss.
        db.warmup(&[key_4624()], "en-US").unwrap();
        assert!(db.resolve_variant(&key_4625()).is_none());
        let unknown = VariantKey {
            provider_name: "Unknown-Provider".into(),
            event_id: 4624,
            version: 0,
            level: 4,
            opcode: 0,
            task: 0,
            keywords: 0,
            channel: 0,
            message_id: 1001,
        };
        assert!(db.resolve_variant(&unknown).is_none());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_registry_sources_with_data() {
        // R12 / ADR 0004: rows present → snapshot known, membership exact.
        let tmp = tmp_path("registry_sources_data");
        let _ = std::fs::remove_file(&tmp);
        {
            let conn = Connection::open(&tmp).unwrap();
            create_v8_schema(&conn);
            insert_metadata_8(&conn);
            conn.execute_batch(
                "INSERT INTO eventlog_sources(source_name, source_type) VALUES
                    ('Chrome', 'classic'),
                    ('Microsoft-Windows-AppXDeploymentServer', 'winevt');",
            )
            .unwrap();
            drop(conn);
        }
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_registry_sources().unwrap();
        assert!(db.registry_known(), "snapshot must be known with rows");
        assert!(db.is_registered_source("Chrome"));
        assert!(db.is_registered_source("Microsoft-Windows-AppXDeploymentServer"));
        assert!(!db.is_registered_source("Totally-Unregistered-Source"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_registry_sources_missing_table_is_hard_error() {
        // v8 contract (ADR 0004, no old-DB compatibility): a DB whose metadata
        // claims v8 but lacks the table is a broken DB — hard error.
        let tmp = tmp_path("registry_sources_missing");
        let _ = std::fs::remove_file(&tmp);
        {
            let conn = Connection::open(&tmp).unwrap();
            create_v8_schema(&conn);
            insert_metadata_8(&conn);
            conn.execute("DROP TABLE eventlog_sources", []).unwrap();
            drop(conn);
        }
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        assert!(
            db.load_registry_sources().is_err(),
            "missing eventlog_sources in a v8 DB must be a hard error"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_registry_sources_empty_table_is_unknown_state() {
        // A collection failure writes no rows: empty snapshot must read as
        // unknown (never as "everything unregistered"), per ADR 0004.
        // create_v8_schema already leaves the table empty.
        let tmp = tmp_path("registry_sources_empty");
        let _ = std::fs::remove_file(&tmp);
        {
            let conn = Connection::open(&tmp).unwrap();
            create_v8_schema(&conn);
            insert_metadata_8(&conn);
            drop(conn);
        }
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_registry_sources().unwrap();
        assert!(!db.registry_known(), "empty snapshot = unknown state");
        assert!(db.is_registered_source("Anything-At-All"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_value_maps_with_data() {
        let tmp = tmp_path("value_maps_data");
        setup_v8_db(&tmp);
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_value_maps().unwrap();
        assert!(db.has_value_maps());
        assert_eq!(db.get_value_map("LogonType", "2"), Some("Interactive"));
        assert_eq!(db.get_value_map("LogonType", "99"), None);
        assert_eq!(db.get_value_map("UnknownMap", "2"), None);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_value_maps_missing_table_fails() {
        // v8 metadata present, but the value_maps table itself is absent.
        let tmp = tmp_path("value_maps_missing");
        let _ = std::fs::remove_file(&tmp);
        {
            let conn = Connection::open(&tmp).unwrap();
            conn.execute_batch(
                "CREATE TABLE provider_message_variants(provider_name TEXT, lang TEXT, event_id INTEGER, message TEXT);
                 CREATE TABLE param_value_maps(provider_name TEXT, event_id INTEGER, param_index TEXT, map_name TEXT, is_bitmap INTEGER);
                 CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO metadata(key, value) VALUES('schema_version', '8')",
                [],
            )
            .unwrap();
            drop(conn);
        }
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        assert!(!db.has_value_maps());
        assert!(db.load_value_maps().is_err());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_param_refs_missing_table_fails() {
        // v8 metadata present, but the parameter_refs table itself is absent.
        let tmp = tmp_path("param_refs_missing");
        let _ = std::fs::remove_file(&tmp);
        {
            let conn = Connection::open(&tmp).unwrap();
            conn.execute_batch(
                "CREATE TABLE provider_message_variants(provider_name TEXT, lang TEXT, event_id INTEGER, message TEXT);
                 CREATE TABLE value_maps(provider_name TEXT, map_name TEXT, entry_value TEXT, display_name TEXT, is_bitmap INTEGER);
                 CREATE TABLE param_value_maps(provider_name TEXT, event_id INTEGER, param_index TEXT, map_name TEXT, is_bitmap INTEGER);
                 CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO metadata(key, value) VALUES('schema_version', '8')",
                [],
            )
            .unwrap();
            drop(conn);
        }
        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        assert!(db.load_param_refs().is_err());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_substitute_with_resolved_message() {
        // Integration-style test: use the substitute engine with a resolved
        // MessageRow to verify the full pipeline works end-to-end.
        let row = MessageRow {
            template: "User %1 logged on to %2".to_string(),
            level: None,
            task: Some("Logon".into()),
            opcode: Some("Info".into()),
            keywords: Some("Audit Success".into()),
            parameters: Some(HashMap::from([
                ("1".into(), "UserName".into()),
                ("2".into(), "Workstation".into()),
            ])),
            level_name: None,
            keywords_name: None,
            param_map_refs: HashMap::new(),
            param_bitmap_refs: HashSet::new(),
            param_types: HashMap::new(),
        };

        let resolved = crate::substitute::substitute(
            &row.template,
            &["admin", "PC-01"],
        );
        assert_eq!(resolved, "User admin logged on to PC-01");
    }

    /// v7 fixture for the %%N regression tests: a variants row whose template
    /// carries %%1843 / %%18 refs plus the matching parameter_refs rows.
    fn setup_param_refs_db(path: &Path, include_param_data: bool) {
        let _ = std::fs::remove_file(path);
        let conn = Connection::open(path).unwrap();
        create_v8_schema(&conn);
        insert_metadata_8(&conn);
        conn.execute(
            "INSERT INTO dll_entries(dll_id, dll_name, dll_path) VALUES(1, 'adtschema.dll', 'C:/adtschema.dll')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO providers(provider_name, dll_id) VALUES('Microsoft-Windows-Security-Auditing', 1)",
            [],
        )
        .unwrap();
        // Template with %%N refs (simulating real 4624 zh-CN template).
        // Positional slots use %t and %n per substitute conventions.
        conn.execute(
            r#"INSERT INTO provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message, parameters) VALUES('Microsoft-Windows-Security-Auditing', '{SEC-GUID}', 'zh-CN', 4624, 0, 0, 0, 0, '0', 0, 4624, 1, '登录类型:%t%5%n虚拟帐户:%t%%1843%n提升的令牌:%t%%18%n帐户名:%t%1', '{"1":"TargetUserName","5":"LogonType"}')"#,
            [],
        )
        .unwrap();
        if include_param_data {
            conn.execute(
            "INSERT INTO parameter_refs(provider_name, param_id, message, lang) VALUES('Microsoft-Windows-Security-Auditing', 1843, '虚拟帐户', 'zh-CN')",
                [],
            )
            .unwrap();
            conn.execute(
            "INSERT INTO parameter_refs(provider_name, param_id, message, lang) VALUES('Microsoft-Windows-Security-Auditing', 18, '提升的令牌', 'zh-CN')",
                [],
            )
            .unwrap();
        }
        drop(conn);
    }

    fn key_security_4624() -> VariantKey {
        VariantKey {
            provider_name: "Microsoft-Windows-Security-Auditing".into(),
            event_id: 4624,
            version: 0,
            level: 0,
            opcode: 0,
            task: 0,
            keywords: 0,
            channel: 0,
            message_id: 4624,
        }
    }

    #[test]
    fn test_expand_param_refs_before_substitute() {
        // Regression test for the bug where %%N expansion happened AFTER
        // substitute_format_message, which consumes %% to %, making %%N
        // references unrecoverable.

        let tmp = tmp_path("param_refs_before_sub");
        setup_param_refs_db(&tmp, true);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_param_refs().unwrap();
        db.warmup(&[key_security_4624()], "zh-CN").unwrap();

        // Get the message row via the v7 variant ladder.
        let row = db.resolve_variant(&key_security_4624()).cloned().expect("4624 variant resolves");

        // --- Correct order: expand %%N FIRST, then substitute ---
        let expanded = db.expand_message(&row.template, "Microsoft-Windows-Security-Auditing");
        assert!(!expanded.contains("%%1843"), "%%1843 should be expanded");
        assert!(expanded.contains("虚拟帐户"), "%%1843 should become '虚拟帐户'");
        assert!(!expanded.contains("%%18"), "%%18 should be expanded");
        assert!(expanded.contains("提升的令牌"), "%%18 should become '提升的令牌'");

        // Now substitute %1-%99 positional params on the already-expanded template.
        // Need at least 5 params for %5 (LogonType).
        let params = vec!["admin", "DOMAIN", "S-1-5-18", "0x3E7", "2"];
        let param_refs: Vec<&str> = params.iter().map(|s| *s).collect();
        let result = crate::substitute::substitute_format_message(&expanded, &param_refs);

        // Verify the final message is correct
        assert!(
            result.contains("虚拟帐户"),
            "%%1843 should resolve to '虚拟帐户'"
        );
        assert!(
            result.contains("提升的令牌"),
            "%%18 should resolve to '提升的令牌'"
        );
        assert!(result.contains("登录类型:\t2"), "%%5 should be substituted, got: {result}");
        assert!(result.contains("帐户名:\tadmin"), "%%1 should be substituted, got: {result}");

        // --- Demonstrate the BUG: wrong order produces corrupted output ---
        // Before the fix: substitute_format_message(raw_template, ...)
        //     → pass 1: %%1843 → %%→%, literal "1843" → "%1843"
        //     → pass 2: %18   → out-of-bounds param 18 → empty, "43" literal
        //     → pass 2: %1843 → %18 (OOB) + literal "43"
        // Result: bare "43" where the resolved value should appear.
        //
        // With the correct order (expand → substitute), the output has the
        // resolved VALUE ("虚拟帐户") after the LABEL ("虚拟帐户:") instead of "43".
        let correct_result = &result;
        let wrong_result =
            crate::substitute::substitute_format_message(&row.template, &param_refs);
        assert!(
            wrong_result.contains('\t') && wrong_result.contains("43"),
            "without prior %%N expansion, %%1843 should leave '43' as residue, got: {wrong_result}"
        );
        assert_ne!(
            wrong_result, *correct_result,
            "wrong order (no expand) should differ from correct order (expand first)"
        );
        // The correct result has "44" — not "43" — because %5 → params[4] = "2" ← wait, that's "2".
        // Actually the correct result contains the resolved "虚拟帐户" value.
        assert!(
            correct_result.contains("登录类型:\t2"),
            "correct path should substitute %%5 → 2, got: {correct_result}"
        );

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_expand_param_refs_empty_cache_no_corruption() {
        // When the parameter_refs table has no rows, expand_message
        // must leave the template untouched.

        let tmp = tmp_path("param_refs_empty");
        setup_param_refs_db(&tmp, false);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_param_refs().unwrap();
        assert!(!db.has_param_refs(), "cache should be empty with no data rows");
        db.warmup(&[key_security_4624()], "zh-CN").unwrap();

        let row = db.resolve_variant(&key_security_4624()).cloned().expect("4624 variant resolves");
        assert!(row.template.contains("%%1843"), "template should have %%1843");

        // With empty cache, expand_message returns template unchanged
        let expanded = db.expand_message(&row.template, "Microsoft-Windows-Security-Auditing");
        assert_eq!(expanded, row.template, "empty cache should preserve template");

        // substitute_format_message with %%N but no expansion data.
        // Need at least 5 params for %5 (LogonType).
        let params = vec!["admin", "DOMAIN", "S-1-5-18", "0x3E7", "2"];
        let param_refs: Vec<&str> = params.iter().map(|s| *s).collect();
        let result =
            crate::substitute::substitute_format_message(&expanded, &param_refs);

        // %%1843 → substitute pass 1: %%→%, "1843" literal → "%1843"
        //         → pass 2: %18 treated as param 18, out of bounds → empty + "43" literal
        // This is expected when no param_refs are available.
        // "43" is the corruption remnant where %%1843's resolved value should be.
        assert!(
            result.contains('4') && result.contains('3'),
            "without param_refs data, %%1843 should leave '43' remnant, got: {result}"
        );
        // But %1 and %5 positional substitution should still work
        assert!(result.contains("admin"), "%1 should still be substituted");
        assert!(result.contains('\t') && result.contains('2'), "%5 should still be substituted");

        let _ = std::fs::remove_file(&tmp);
    }
}

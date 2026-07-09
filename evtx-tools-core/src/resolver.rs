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
    conn: Connection,
    cache: HashMap<(String, u32), MessageRow>,
    /// `%%N` parameter reference cache: param_id → expanded text.
    param_refs: HashMap<u32, String>,
    /// CRIM value map cache: map_name → {value_str → display_name}.
    /// Populated from the `value_maps` table (VMAP/BMAP blocks).
    value_maps: HashMap<String, HashMap<String, String>>,
    /// Set of map names that are bitmaps (is_bitmap=1 in DB).
    bitmap_names: HashSet<String>,
}

impl MessageDb {
    /// Open a SQLite message database at the given path.
    ///
    /// The database is expected to have been created by the
    /// [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) tool and contain a `messages` table.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| format!("failed to open messages db '{path}': {e}"))?;

        Ok(MessageDb {
            conn,
            cache: HashMap::new(),
            param_refs: HashMap::new(),
            value_maps: HashMap::new(),
            bitmap_names: HashSet::new(),
        })
    }

    /// Load all `%%N` parameter references from the `parameter_refs` table.
    ///
    /// This populates the in-memory `param_refs` cache used by
    /// [`expand_message`](MessageDb::expand_message).  The table may not exist
    /// in older DBs — missing table is handled gracefully (empty cache).
    pub fn load_param_refs(&mut self) -> Result<()> {
        // Check if the table exists; older DBs may lack it.
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master \
                 WHERE type='table' AND name='parameter_refs'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !table_exists {
            return Ok(());
        }

        let mut stmt = self
            .conn
            .prepare("SELECT param_id, message FROM parameter_refs")
            .map_err(|e| format!("failed to prepare param_refs query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let param_id: i64 = row.get(0)?;
                let message: String = row.get(1)?;
                Ok((param_id as u32, message))
            })
            .map_err(|e| format!("failed to query parameter_refs: {e}"))?;

        for row in rows {
            if let Ok((param_id, message)) = row {
                self.param_refs.insert(param_id, message);
            }
        }

        Ok(())
    }

    /// Expand `%%N` parameter references in a message string.
    ///
    /// Calls [`expand_param_refs`] with the loaded `param_refs` cache.
    /// If the cache is empty, the message is returned unchanged.
    pub fn expand_message(&self, message: &str) -> String {
        if self.param_refs.is_empty() {
            return message.to_string();
        }
        expand_param_refs(message, &self.param_refs)
    }

    /// Load all value maps from the `value_maps` table.
    ///
    /// Populates the in-memory `value_maps` cache used by
    /// [`get_value_map`](MessageDb::get_value_map) and other resolution methods.
    /// Also populates `bitmap_names` for map entries where `is_bitmap=1`.
    /// The table may not exist in older DBs — missing table is handled gracefully
    /// (empty cache).
    pub fn load_value_maps(&mut self) -> Result<()> {
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master \
                 WHERE type='table' AND name='value_maps'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !table_exists {
            return Ok(());
        }

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

    /// Batch-load message rows for the given `(provider, event_id)` pairs
    /// using `en-US` as the language filter.
    ///
    /// This is a convenience wrapper around [`warmup_lang`](MessageDb::warmup_lang).
    pub fn warmup(&mut self, providers: &[(&str, u32)]) -> Result<()> {
        self.warmup_lang(providers, "en-US")
    }

    /// Batch-load message rows for the given `(provider, event_id)` pairs,
    /// filtering by `lang`.
    ///
    /// Query uses v4 normalized schema: `dll_messages JOIN providers`.
    pub fn warmup_lang(&mut self, providers: &[(&str, u32)], lang: &str) -> Result<()> {
        if providers.is_empty() {
            return Ok(());
        }

        let n = providers.len();
        let mut conditions: Vec<String> = Vec::with_capacity(n);
        let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(n * 2 + 1);

        params.push(rusqlite::types::Value::Text(lang.to_string()));
        for (i, (provider, event_id)) in providers.iter().enumerate() {
            let p_idx = i * 2 + 2;
            let e_idx = i * 2 + 3;
            conditions.push(format!(
                "(p.provider_name = ?{p_idx} AND m.event_id = ?{e_idx})"
            ));
            params.push(rusqlite::types::Value::Text(provider.to_string()));
            params.push(rusqlite::types::Value::Integer(*event_id as i64));
        }

        let query = format!(
            "SELECT p.provider_name, m.event_id, m.message, m.task, m.opcode, \
                m.keywords, m.parameters, m.level_name, m.keywords_name
             FROM dll_messages m
             JOIN providers p ON p.dll_id = m.dll_id
             WHERE m.lang = ?1 AND ({})",
            conditions.join(" OR ")
        );

        let mut stmt = self
            .conn
            .prepare(&query)
            .map_err(|e| format!("failed to prepare v4 warmup query: {e}"))?;

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let provider_name: String = row.get(0)?;
                let event_id: i64 = row.get(1)?;
                let message: String = row.get(2)?;
                let task: Option<String> = row.get(3)?;
                let opcode: Option<String> = row.get(4)?;
                let keywords: Option<String> = row.get(5)?;
                let params_json: Option<String> = row.get(6)?;
                let level_name: Option<String> = row.get(7)?;
                let keywords_name: Option<String> = row.get(8)?;

                let parameters = params_json
                    .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok());

                Ok((
                    provider_name,
                    event_id as u32,
                    MessageRow {
                        template: message,
                        task,
                        opcode,
                        keywords,
                        parameters,
                        level_name,
                        keywords_name,
                    },
                ))
            })
            .map_err(|e| format!("failed to execute v4 warmup query: {e}"))?;

        for row in rows {
            let (provider_name, event_id, msg_row) =
                row.map_err(|e| format!("failed to read v4 warmup row: {e}"))?;
            self.cache.insert((provider_name, event_id), msg_row);
        }

        Ok(())
    }

    /// Look up a previously warmed-up message row.
    ///
    /// Returns `None` if the `(provider, event_id)` pair was not loaded
    /// during [`warmup`](MessageDb::warmup) or is not present in the database.
    pub fn get(&self, provider: &str, event_id: u32) -> Option<&MessageRow> {
        self.cache.get(&(provider.to_string(), event_id))
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
    use std::collections::HashMap;

    /// Create a v4-schema messages database and return it.
    fn setup_test_db_v4() -> Connection {
        let conn = Connection::open_in_memory().unwrap();

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
            CREATE TABLE dll_messages (
                dll_id        INTEGER NOT NULL REFERENCES dll_entries(dll_id),
                lang          TEXT NOT NULL DEFAULT 'en-US',
                event_id      INTEGER NOT NULL,
                message       TEXT NOT NULL,
                level         INTEGER,
                task          TEXT,
                opcode        TEXT,
                keywords      TEXT,
                parameters    TEXT,
                level_name    TEXT,
                keywords_name TEXT,
                PRIMARY KEY (dll_id, lang, event_id)
            );",
        )
        .unwrap();

        // Insert DLL entry
        conn.execute(
            "INSERT INTO dll_entries (dll_id, dll_name, dll_path, pe_version) VALUES (1, 'test.dll', 'C:\\test.dll', '1.0.0.0')",
            [],
        )
        .unwrap();

        // Insert providers
        conn.execute(
            "INSERT INTO providers (provider_name, guid, channel, dll_id) VALUES ('Test-Provider', '{TP-GUID}', 'Application', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO providers (provider_name, guid, channel, dll_id) VALUES ('Security-Provider', '{SP-GUID}', 'Security', 1)",
            [],
        )
        .unwrap();

        // Insert messages
        conn.execute(
            "INSERT INTO dll_messages (dll_id, lang, event_id, message, task, opcode, keywords, parameters)
             VALUES (1, 'en-US', 4624, 'Account logged on. %1\\%2', 'Logon', 'Info', 'Audit Success', '{\"1\":\"TargetUserName\",\"2\":\"TargetDomain\"}')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO dll_messages (dll_id, lang, event_id, message, task, opcode, keywords, parameters)
             VALUES (1, 'en-US', 4625, 'Failed logon for %1', 'Logon', 'Failure', 'Audit Failure', NULL)",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO dll_messages (dll_id, lang, event_id, message, level, task, opcode, keywords, parameters)
             VALUES (1, 'en-US', 1001, 'Simple event', 0, NULL, NULL, NULL, NULL)",
            [],
        )
        .unwrap();

        conn
    }

    #[test]
    fn message_db_open_in_memory() {
        let tmp = std::env::temp_dir().join("test_msg_db_open.db");
        let _ = std::fs::remove_file(&tmp);

        let setup_conn = Connection::open(&tmp).unwrap();
        setup_conn
            .execute_batch(
                "CREATE TABLE dll_entries (
                    dll_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    dll_name TEXT NOT NULL, dll_path TEXT NOT NULL UNIQUE,
                    pe_version TEXT NOT NULL DEFAULT '0.0.0.0'
                );
                CREATE TABLE providers (
                    provider_name TEXT NOT NULL PRIMARY KEY,
                    guid TEXT, channel TEXT,
                    dll_id INTEGER NOT NULL REFERENCES dll_entries(dll_id)
                );
                CREATE TABLE dll_messages (
                    dll_id INTEGER NOT NULL REFERENCES dll_entries(dll_id),
                    lang TEXT NOT NULL DEFAULT 'en-US',
                    event_id INTEGER NOT NULL,
                    message TEXT NOT NULL,
                    task TEXT, opcode TEXT, keywords TEXT,
                    parameters TEXT, level_name TEXT, keywords_name TEXT,
                    PRIMARY KEY (dll_id, lang, event_id)
                );",
            )
            .unwrap();
        drop(setup_conn);

        let db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        assert!(db.conn().is_autocommit());

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn message_db_open_nonexistent_fails() {
        let result = MessageDb::open("/nonexistent/path/to/messages.db");
        assert!(result.is_err());
    }

    #[test]
    fn message_db_warmup_and_get() {
        let tmp = std::env::temp_dir().join("test_msg_db_warmup.db");
        let _ = std::fs::remove_file(&tmp);

        let conn = Connection::open(&tmp).unwrap();
        conn.execute_batch(
            "CREATE TABLE dll_entries (
                dll_id INTEGER PRIMARY KEY AUTOINCREMENT,
                dll_name TEXT NOT NULL, dll_path TEXT NOT NULL UNIQUE,
                pe_version TEXT NOT NULL DEFAULT '0.0.0.0'
            );
            CREATE TABLE providers (
                provider_name TEXT NOT NULL PRIMARY KEY,
                guid TEXT, channel TEXT,
                dll_id INTEGER NOT NULL REFERENCES dll_entries(dll_id)
            );
            CREATE TABLE dll_messages (
                dll_id INTEGER NOT NULL REFERENCES dll_entries(dll_id),
                lang TEXT NOT NULL DEFAULT 'en-US',
                event_id INTEGER NOT NULL, message TEXT NOT NULL,
                task TEXT, opcode TEXT, keywords TEXT,
                parameters TEXT, level_name TEXT, keywords_name TEXT,
                PRIMARY KEY (dll_id, lang, event_id)
            );",
        ).unwrap();
        conn.execute("INSERT INTO dll_entries(dll_id,dll_name,dll_path) VALUES(1,'test.dll','C:\\test.dll')", []).unwrap();
        conn.execute("INSERT INTO providers(provider_name,dll_id) VALUES('Test-Provider',1)", []).unwrap();
        conn.execute(
            "INSERT INTO dll_messages (dll_id,lang,event_id,message,task,opcode,keywords,parameters)
             VALUES (1,'en-US',4624,'Account logged on. %1\\%2','Logon','Info','Audit Success','{\"1\":\"TargetUserName\",\"2\":\"TargetDomain\"}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO dll_messages (dll_id,lang,event_id,message,task,opcode,keywords,parameters)
             VALUES (1,'en-US',4625,'Failed logon for %1','Logon','Failure','Audit Failure',NULL)",
            [],
        ).unwrap();
        drop(conn);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.warmup(&[("Test-Provider", 4624), ("Test-Provider", 4625)]).unwrap();

        let row_4624 = db.get("Test-Provider", 4624).unwrap();
        assert_eq!(row_4624.template, "Account logged on. %1\\%2");
        assert_eq!(row_4624.task.as_deref(), Some("Logon"));
        assert_eq!(row_4624.opcode.as_deref(), Some("Info"));
        assert_eq!(row_4624.keywords.as_deref(), Some("Audit Success"));
        assert!(row_4624.parameters.is_some());
        assert_eq!(
            row_4624.parameters.as_ref().unwrap().get("1").map(|s| s.as_str()),
            Some("TargetUserName")
        );

        let row_4625 = db.get("Test-Provider", 4625).unwrap();
        assert_eq!(row_4625.template, "Failed logon for %1");
        assert_eq!(row_4625.task.as_deref(), Some("Logon"));
        assert!(row_4625.parameters.is_none());

        assert!(db.get("Test-Provider", 9999).is_none());
        assert!(db.get("Unknown-Provider", 4624).is_none());

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn message_db_warmup_empty() {
        let tmp = std::env::temp_dir().join("test_msg_db_warmup_empty.db");
        let _ = std::fs::remove_file(&tmp);

        let c = Connection::open(&tmp).unwrap();
        c.execute_batch(
            "CREATE TABLE dll_entries(dll_id INTEGER PRIMARY KEY,dll_name TEXT,dll_path TEXT UNIQUE,pe_version TEXT);
             CREATE TABLE providers(provider_name TEXT PRIMARY KEY,guid TEXT,channel TEXT,dll_id INTEGER REFERENCES dll_entries(dll_id));
             CREATE TABLE dll_messages(dll_id INTEGER,lang TEXT,event_id INTEGER,message TEXT,PRIMARY KEY(dll_id,lang,event_id));",
        ).unwrap();
        drop(c);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.warmup(&[]).unwrap();
        assert!(db.get("anything", 0).is_none());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn message_db_warmup_partial_hits() {
        let tmp = std::env::temp_dir().join("test_msg_db_warmup_partial.db");
        let _ = std::fs::remove_file(&tmp);

        let c = Connection::open(&tmp).unwrap();
        c.execute_batch(
            "CREATE TABLE dll_entries(dll_id INTEGER PRIMARY KEY,dll_name TEXT,dll_path TEXT UNIQUE,pe_version TEXT);
             CREATE TABLE providers(provider_name TEXT PRIMARY KEY,guid TEXT,channel TEXT,dll_id INTEGER REFERENCES dll_entries(dll_id));
             CREATE TABLE dll_messages(dll_id INTEGER,lang TEXT,event_id INTEGER,message TEXT,task TEXT,opcode TEXT,keywords TEXT,parameters TEXT,level_name TEXT,keywords_name TEXT,PRIMARY KEY(dll_id,lang,event_id));",
        ).unwrap();
        c.execute("INSERT INTO dll_entries(dll_id,dll_name,dll_path) VALUES(1,'a.dll','C:\\a.dll')", []).unwrap();
        c.execute("INSERT INTO providers(provider_name,dll_id) VALUES('Provider-A',1)", []).unwrap();
        c.execute("INSERT INTO dll_messages(dll_id,lang,event_id,message) VALUES(1,'en-US',10,'Event A')", []).unwrap();
        drop(c);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.warmup(&[("Provider-A", 10), ("Provider-A", 99), ("Provider-B", 10)]).unwrap();

        assert!(db.get("Provider-A", 10).is_some());
        assert!(db.get("Provider-A", 99).is_none());
        assert!(db.get("Provider-B", 10).is_none());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn message_db_get_no_warmup_returns_none() {
        let tmp = std::env::temp_dir().join("test_msg_db_no_warmup.db");
        let _ = std::fs::remove_file(&tmp);

        let c = Connection::open(&tmp).unwrap();
        c.execute_batch(
            "CREATE TABLE dll_entries(dll_id INTEGER PRIMARY KEY,dll_name TEXT,dll_path TEXT UNIQUE,pe_version TEXT);
             CREATE TABLE providers(provider_name TEXT PRIMARY KEY,guid TEXT,channel TEXT,dll_id INTEGER REFERENCES dll_entries(dll_id));
             CREATE TABLE dll_messages(dll_id INTEGER,lang TEXT,event_id INTEGER,message TEXT,task TEXT,opcode TEXT,keywords TEXT,parameters TEXT,level_name TEXT,keywords_name TEXT,PRIMARY KEY(dll_id,lang,event_id));",
        ).unwrap();
        c.execute("INSERT INTO dll_entries(dll_id,dll_name,dll_path) VALUES(1,'a.dll','C:\\a.dll')", []).unwrap();
        c.execute("INSERT INTO providers(provider_name,dll_id) VALUES('Provider-A',1)", []).unwrap();
        c.execute("INSERT INTO dll_messages(dll_id,lang,event_id,message) VALUES(1,'en-US',10,'Event A')", []).unwrap();
        drop(c);

        let db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        assert!(db.get("Provider-A", 10).is_none());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn message_db_warmup_with_level_name() {
        let tmp = std::env::temp_dir().join("test_msg_db_level_name.db");
        let _ = std::fs::remove_file(&tmp);

        let c = Connection::open(&tmp).unwrap();
        c.execute_batch(
            "CREATE TABLE dll_entries(dll_id INTEGER PRIMARY KEY,dll_name TEXT,dll_path TEXT UNIQUE,pe_version TEXT);
             CREATE TABLE providers(provider_name TEXT PRIMARY KEY,guid TEXT,channel TEXT,dll_id INTEGER REFERENCES dll_entries(dll_id));
             CREATE TABLE dll_messages(dll_id INTEGER,lang TEXT,event_id INTEGER,message TEXT,task TEXT,opcode TEXT,keywords TEXT,parameters TEXT,level_name TEXT,keywords_name TEXT,PRIMARY KEY(dll_id,lang,event_id));",
        ).unwrap();
        c.execute("INSERT INTO dll_entries(dll_id,dll_name,dll_path) VALUES(1,'t.dll','C:\\t.dll')", []).unwrap();
        c.execute("INSERT INTO providers(provider_name,dll_id) VALUES('Test-Prov',1)", []).unwrap();
        c.execute(
            "INSERT INTO dll_messages(dll_id,lang,event_id,message,level_name,keywords_name) VALUES(1,'en-US',100,'Msg with %%14592','win:Error','Audit Success')",
            [],
        ).unwrap();
        drop(c);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.warmup(&[("Test-Prov", 100)]).unwrap();

        let row = db.get("Test-Prov", 100).unwrap();
        assert_eq!(row.level_name.as_deref(), Some("win:Error"));
        assert_eq!(row.keywords_name.as_deref(), Some("Audit Success"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_substitute_with_resolved_message() {
        // Integration-style test: use the substitute engine with a resolved
        // MessageRow to verify the full pipeline works end-to-end.
        let row = MessageRow {
            template: "User %1 logged on to %2".to_string(),
            task: Some("Logon".into()),
            opcode: Some("Info".into()),
            keywords: Some("Audit Success".into()),
            parameters: Some(HashMap::from([
                ("1".into(), "UserName".into()),
                ("2".into(), "Workstation".into()),
            ])),
            level_name: None,
            keywords_name: None,
        };

        let resolved = crate::substitute::substitute(
            &row.template,
            &["admin", "PC-01"],
        );
        assert_eq!(resolved, "User admin logged on to PC-01");
    }

    #[test]
    fn message_db_load_value_maps_empty() {
        let tmp = std::env::temp_dir().join("test_vm_load_empty.db");
        let _ = std::fs::remove_file(&tmp);

        let c = Connection::open(&tmp).unwrap();
        c.execute_batch(
            "CREATE TABLE dll_entries(dll_id INTEGER PRIMARY KEY,dll_name TEXT,dll_path TEXT UNIQUE,pe_version TEXT);
             CREATE TABLE providers(provider_name TEXT PRIMARY KEY,guid TEXT,channel TEXT,dll_id INTEGER REFERENCES dll_entries(dll_id));
             CREATE TABLE dll_messages(dll_id INTEGER,lang TEXT,event_id INTEGER,message TEXT,PRIMARY KEY(dll_id,lang,event_id));",
        ).unwrap();
        drop(c);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_value_maps().unwrap();
        assert!(!db.has_value_maps());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn message_db_load_value_maps_with_data() {
        let tmp = std::env::temp_dir().join("test_vm_load_data.db");
        let _ = std::fs::remove_file(&tmp);

        let c = Connection::open(&tmp).unwrap();
        c.execute_batch(
            "CREATE TABLE dll_entries(dll_id INTEGER PRIMARY KEY,dll_name TEXT,dll_path TEXT UNIQUE,pe_version TEXT);
             CREATE TABLE providers(provider_name TEXT PRIMARY KEY,guid TEXT,channel TEXT,dll_id INTEGER REFERENCES dll_entries(dll_id));
             CREATE TABLE dll_messages(dll_id INTEGER,lang TEXT,event_id INTEGER,message TEXT,PRIMARY KEY(dll_id,lang,event_id));
             CREATE TABLE value_maps(
                 provider_name TEXT NOT NULL,
                 map_name TEXT NOT NULL,
                 entry_value TEXT NOT NULL,
                 display_name TEXT NOT NULL,
                 is_bitmap INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (provider_name, map_name, entry_value)
             );",
        ).unwrap();
        c.execute(
            "INSERT INTO value_maps(provider_name, map_name, entry_value, display_name)
             VALUES ('Test-Prov', 'LogonType', '2', 'Interactive')",
            [],
        ).unwrap();
        c.execute(
            "INSERT INTO value_maps(provider_name, map_name, entry_value, display_name)
             VALUES ('Test-Prov', 'LogonType', '10', 'RemoteInteractive')",
            [],
        ).unwrap();
        c.execute(
            "INSERT INTO value_maps(provider_name, map_name, entry_value, display_name, is_bitmap)
             VALUES ('Test-Prov', 'AccessMask', '0', 'Read', 1)",
            [],
        ).unwrap();
        drop(c);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_value_maps().unwrap();
        assert!(db.has_value_maps());

        assert_eq!(db.get_value_map("LogonType", "2"), Some("Interactive"));
        assert_eq!(db.get_value_map("LogonType", "10"), Some("RemoteInteractive"));
        assert_eq!(db.get_value_map("LogonType", "99"), None);
        assert_eq!(db.get_value_map("AccessMask", "0"), Some("Read"));
        assert_eq!(db.get_value_map("UnknownMap", "2"), None);

        let param_map = HashMap::from([
            ("1".into(), "LogonType".into()),
            ("2".into(), "TargetUserName".into()),
        ]);
        let relevant = db.get_param_value_maps(&param_map);
        assert!(relevant.contains_key("LogonType"));
        assert!(!relevant.contains_key("TargetUserName"));
        assert_eq!(
            relevant.get("LogonType").and_then(|m| m.get("2")).map(|s| s.as_str()),
            Some("Interactive")
        );

        let _ = std::fs::remove_file(&tmp);
    }

    // -----------------------------------------------------------------------
    // Regression: %%N expansion must happen BEFORE substitute
    // -----------------------------------------------------------------------

    /// Create a v4-schema messages.db with parameter_refs table.
    fn setup_db_with_param_refs(include_param_data: bool) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE dll_entries (dll_id INTEGER PRIMARY KEY AUTOINCREMENT, dll_name TEXT NOT NULL, dll_path TEXT NOT NULL UNIQUE, pe_version TEXT NOT NULL DEFAULT '0.0.0.0');
             CREATE TABLE providers (provider_name TEXT NOT NULL PRIMARY KEY, guid TEXT, channel TEXT, dll_id INTEGER NOT NULL REFERENCES dll_entries(dll_id));
             CREATE TABLE dll_messages (dll_id INTEGER NOT NULL, lang TEXT NOT NULL DEFAULT 'en-US', event_id INTEGER NOT NULL, message TEXT NOT NULL, task TEXT, opcode TEXT, keywords TEXT, parameters TEXT, level_name TEXT, keywords_name TEXT, PRIMARY KEY (dll_id, lang, event_id));
             CREATE TABLE parameter_refs (param_id INTEGER PRIMARY KEY, message TEXT NOT NULL);",
        ).unwrap();
        conn.execute("INSERT INTO dll_entries (dll_id, dll_name, dll_path) VALUES (1, 'adtschema.dll', 'C:\\adtschema.dll')", []).unwrap();
        conn.execute("INSERT INTO providers (provider_name, dll_id) VALUES ('Microsoft-Windows-Security-Auditing', 1)", []).unwrap();

        // Template with %%N refs (simulating real 4624 zh-CN template).
        // Use %t (→tab) and %n (→CRLF) per substitute conventions.
        conn.execute(
            "INSERT INTO dll_messages (dll_id, lang, event_id, message, parameters) VALUES (1, 'zh-CN', 4624,
             '登录类型:%t%5%n虚拟帐户:%t%%1843%n提升的令牌:%t%%18%n帐户名:%t%1',
             '{\"1\":\"TargetUserName\",\"5\":\"LogonType\"}')",
            [],
        ).unwrap();

        if include_param_data {
            conn.execute("INSERT INTO parameter_refs (param_id, message) VALUES (1843, '虚拟帐户')", []).unwrap();
            conn.execute("INSERT INTO parameter_refs (param_id, message) VALUES (18, '提升的令牌')", []).unwrap();
        }
        conn
    }

    #[test]
    fn test_expand_param_refs_before_substitute() {
        // Regression test for the bug where %%N expansion happened AFTER
        // substitute_format_message, which consumes %% → %, making %%N
        // references unrecoverable (e.g. %%1843 → %18 + 43 corruption).

        let conn = setup_db_with_param_refs(true);

        // Write to temp file and open as MessageDb
        let tmp = std::env::temp_dir().join("test_param_refs_before_sub.db");
        let _ = std::fs::remove_file(&tmp);
        // VACUUM INTO saves in-memory DB to file
        conn.execute("VACUUM INTO ?1", [tmp.to_str().unwrap()]).unwrap();
        drop(conn);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        db.load_param_refs().unwrap();
        db.warmup_lang(&[("Microsoft-Windows-Security-Auditing", 4624)], "zh-CN").unwrap();

        // Get the message row
        let row = db.get("Microsoft-Windows-Security-Auditing", 4624).unwrap();

        // --- Correct order: expand %%N FIRST, then substitute ---
        let expanded = db.expand_message(&row.template);
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
        //     → pass 1: %%1843 → %%→%, literal "1843" → "%1843"
        //     → pass 2: %18   → out-of-bounds param 18 → empty, "43" literal
        //     → pass 2: %1843 → %18 (OOB) + literal "43"
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
        // When parameter_refs table is empty, expand_message should be a no-op
        // and substitute should still work correctly %1 positional params.

        let conn = setup_db_with_param_refs(false);

        let tmp = std::env::temp_dir().join("test_param_refs_empty.db");
        let _ = std::fs::remove_file(&tmp);
        conn.execute("VACUUM INTO ?1", [tmp.to_str().unwrap()]).unwrap();
        drop(conn);

        let mut db = MessageDb::open(tmp.to_str().unwrap()).unwrap();
        // Table exists but is empty → cache stays empty
        db.load_param_refs().unwrap();
        assert!(!db.has_param_refs(), "cache should be empty with no data rows");
        db.warmup_lang(&[("Microsoft-Windows-Security-Auditing", 4624)], "zh-CN").unwrap();

        let row = db.get("Microsoft-Windows-Security-Auditing", 4624).unwrap();
        assert!(row.template.contains("%%1843"), "template should have %%1843");

        // With empty cache, expand_message returns template unchanged
        let expanded = db.expand_message(&row.template);
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

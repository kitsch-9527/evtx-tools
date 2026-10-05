//! EVTX file parser wrapper around the [`evtx`] crate.
//!
//! Provides convenience functions for opening EVTX files, detecting their
//! OS build number, iterating over events, and extracting all 20+ fields
//! into [`ParsedEvent`] structs.
//!
//! [`evtx`]: https://docs.rs/evtx

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use evtx::{EvtxParser, ParserSettings};
use serde_json::Value;

use crate::{severity_from_level, ParsedEvent, Result};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Open an EVTX file for reading.
///
/// `num_threads` controls parallelism via the evtx crate's internal thread
/// pool. Pass `0` to delegate to rayon's default (typically == CPU count).
/// Pass `Some(1)` for single-threaded (ordered) iteration.
///
/// Returns a ready-to-use [`EvtxParser`]. The parser owns the file handle and
/// is used by [`iter_events`] to walk all records.
pub fn open_evtx(path: &str, num_threads: usize) -> Result<EvtxParser<File>> {
    let settings = ParserSettings::default().num_threads(num_threads);
    EvtxParser::from_path(path)
        .map(|parser| parser.with_configuration(settings))
        .map_err(|e| format!("failed to open evtx file '{path}': {e}"))
}

/// Detect the Windows build number from an EVTX file header.
///
/// Reads the `major_version` field at byte offset `0x40` in the EVTX file header.
/// This value is the Windows build number (e.g. 22621 for Windows 11 22H2,
/// 19041 for Windows 10 20H1).
pub fn detect_evtx_build(path: &str) -> Option<u32> {
    let mut file = File::open(path).ok()?;
    file.seek(SeekFrom::Start(0x40)).ok()?;
    let mut buf = [0u8; 2];
    file.read_exact(&mut buf).ok()?;
    Some(u16::from_le_bytes(buf) as u32)
}

/// Scan an EVTX file and collect unique channel names.
///
/// Opens the EVTX file, iterates all records, and extracts the `System/Channel`
/// field from each. Returns a deduplicated, alphabetically-sorted list of
/// channel names found in the file.
///
/// This is a lightweight operation — it reads only the Channel field from each
/// record without doing full field extraction or message substitution. Suitable
/// for GUI workflows that need to determine which rules may apply before
/// running the full parse pipeline.
///
/// `num_threads` controls parallelism (same semantics as [`open_evtx`]).
///
/// # Errors
///
/// Returns `Err` if the EVTX file cannot be opened or if any record fails to
/// parse.
///
/// # Example
///
/// ```no_run
/// use evtx_tools_core::parser::scan_evtx_channels;
///
/// let channels = scan_evtx_channels("Security.evtx", 1).unwrap();
/// assert!(channels.contains(&"Security".to_string()));
/// ```
pub fn scan_evtx_channels(path: &str, num_threads: usize) -> Result<Vec<String>> {
    let mut parser = open_evtx(path, num_threads)?;
    let mut channels = std::collections::BTreeSet::new();
    for raw in iter_raw_json_values(&mut parser) {
        let (value, _record_id) = raw?;
        if let Some(ch) = value["Event"]["System"]["Channel"].as_str() {
            channels.insert(ch.to_string());
        }
    }
    Ok(channels.into_iter().collect())
}

/// Iterate over all events in an EVTX file, yielding `(ParsedEvent, EventData Value)` tuples.
///
/// Each record is rendered to a JSON [`Value`], then all 22 fields are
/// extracted from that JSON tree. Uses `records_json_value()` to avoid an
/// unnecessary bytes→Value serialization round trip.
///
/// The second tuple element is the raw `EventData` sub-tree (cloned from the
/// parser output). It is passed along so that downstream message resolution
/// ([`extract_subst_params`]) can work directly on the parsed [`Value`] instead
/// of round-tripping through JSON string serialisation.
///
/// The `file_name` is stored in every output event for provenance tracking.
///
/// Like [`iter_events`] but yields raw `(Value, event_record_id)` tuples
/// **without** calling [`extract_fields`]. Processing is deferred to the
/// caller (e.g. inside a rayon parallel section) so that the 22-field JSON
/// traversal runs on multiple cores alongside message resolution.
///
/// The caller owns the `Value` and must call [`extract_fields`] on each tuple
/// to obtain a [`ParsedEvent`].
pub fn iter_raw_json_values(
    parser: &mut EvtxParser<File>,
) -> impl Iterator<Item = Result<(Value, u64)>> + '_ {
    parser.records_json_value().map(|result| match result {
        Err(e) => Err(format!("evtx error: {e}")),
        Ok(record) => Ok((record.data, record.event_record_id)),
    })
}

/// Iterate over all events in an EVTX file, yielding `(ParsedEvent, EventData Value)` tuples.
///
/// Each record is rendered to a JSON [`Value`], then all 22 fields are
/// extracted from that JSON tree. Uses `records_json_value()` to avoid an
/// unnecessary bytes→Value serialization round trip.
///
/// The second tuple element is the raw `EventData` sub-tree (cloned from the
/// parser output). It is passed along so that downstream message resolution
/// ([`extract_subst_params`]) can work directly on the parsed [`Value`] instead
/// of round-tripping through JSON string serialisation.
///
/// The `file_name` is stored in every output event for provenance tracking.
///
/// # Errors
///
/// Individual records that fail to render or parse are yielded as `Err` —
/// iteration continues with subsequent records.
pub fn iter_events(
    parser: &mut EvtxParser<File>,
    file_name: String,
) -> impl Iterator<Item = Result<(ParsedEvent, Option<Value>)>> + '_ {
    parser.records_json_value().map(move |result| {
        match result {
            Err(e) => Err(format!("evtx error: {e}")),
            Ok(record) => {
                let (event, event_data_value) =
                    extract_fields(record.data, record.event_record_id, &file_name);
                Ok((event, event_data_value))
            }
        }
    })
}

/// Like [`iter_events`], but also attaches each record's raw XML
/// (`<Event xmlns="...">...`, the evtx crate's own serialization) to
/// `ParsedEvent.raw_xml`. Used by `parse --with-xml`.
///
/// The evtx crate serializes a record either to JSON or to XML, never both,
/// so a second parser over the same file walks the records again and the two
/// streams are zipped by `event_record_id`. Both iterators yield records in
/// file order; a record whose XML counterpart is missing (parse error on
/// either side) is still emitted, with `raw_xml = None`.
pub fn iter_events_with_xml<'a>(
    parser: &'a mut EvtxParser<File>,
    xml_parser: &'a mut EvtxParser<File>,
    file_name: String,
) -> impl Iterator<Item = Result<(ParsedEvent, Option<Value>)>> + 'a {
    iter_raw_json_values_with_xml(parser, xml_parser).map(move |result| match result {
        Err(e) => Err(e),
        Ok((data, rid, raw_xml)) => {
            let (mut event, event_data_value) = extract_fields(data, rid, &file_name);
            event.raw_xml = raw_xml;
            Ok((event, event_data_value))
        }
    })
}

/// Like [`iter_events_with_xml`], but yields raw `(Value, event_record_id,
/// raw_xml)` tuples **without** calling [`extract_fields`] — the 22-field
/// JSON traversal is deferred to the caller so it can run inside a rayon
/// parallel section (see [`extract_batch_parallel`]). The `raw_xml` payload
/// comes from the same record-id zip as [`iter_events_with_xml`]: a record
/// whose XML counterpart is missing (parse error on either side) yields
/// `raw_xml = None`.
pub fn iter_raw_json_values_with_xml<'a>(
    parser: &'a mut EvtxParser<File>,
    xml_parser: &'a mut EvtxParser<File>,
) -> impl Iterator<Item = Result<(Value, u64, Option<String>)>> + 'a {
    let mut xml_iter = xml_parser.records();
    // Records whose XML arrived before the matching JSON event (e.g. the JSON
    // side skipped an item via an error path) wait here for their record id.
    let mut pending_xml: std::collections::BTreeMap<u64, String> = std::collections::BTreeMap::new();
    parser.records_json_value().map(move |result| {
        match result {
            Err(e) => Err(format!("evtx error: {e}")),
            Ok(record) => {
                let rid = record.event_record_id;
                let mut raw_xml: Option<String> = pending_xml.remove(&rid);
                if raw_xml.is_none() {
                    for item in xml_iter.by_ref() {
                        match item {
                            Ok(x) if x.event_record_id < rid => continue,
                            Ok(x) if x.event_record_id == rid => {
                                raw_xml = Some(x.data);
                                break;
                            }
                            Ok(x) => {
                                pending_xml.insert(x.event_record_id, x.data);
                                break;
                            }
                            Err(_) => continue,
                        }
                    }
                }
                Ok((record.data, rid, raw_xml))
            }
        }
    })
}

/// Parallel counterpart of the [`iter_events`] extract stage: run the
/// 22-field JSON traversal ([`extract_fields`]) across a raw batch on the
/// rayon pool instead of the serial producer thread.
///
/// Order is preserved (rayon indexed collect), so callers can keep assigning
/// sequence numbers and collecting warmup keys in stream order after the
/// batch returns. `raw_xml` payloads produced by
/// [`iter_raw_json_values_with_xml`] are attached to their records here.
pub fn extract_batch_parallel(
    raw: Vec<(Value, u64, Option<String>)>,
    file_name: &str,
) -> Vec<(ParsedEvent, Option<Value>)> {
    use rayon::prelude::*;
    raw.into_par_iter()
        .map(|(data, rid, raw_xml)| {
            let (mut event, event_data_value) = extract_fields(data, rid, file_name);
            event.raw_xml = raw_xml;
            (event, event_data_value)
        })
        .collect()
}

/// Extract all fields from a JSON EVTX record into a [`ParsedEvent`].
///
/// Also returns the raw `EventData` sub-tree so that downstream message
/// resolution can avoid a Value→String→Value round trip.
///
/// The `data` parameter is the `"Event"` root node of the rendered JSON.
/// `event_record_id` comes from the EVTX chunk header.
pub fn extract_fields(
    data: Value,
    event_record_id: u64,
    file_name: &str,
) -> (ParsedEvent, Option<Value>) {
    let system = data
        .get("Event")
        .and_then(|v| v.get("System"));

    // ---- helpers ----

    /// Get a string value from a JSON node, handling String, Number, and
    /// object-with-`$`/`#text` variants (the evtx crate uses either, depending
    /// on whether the XML element has attributes).
    fn get_str(v: Option<&Value>) -> Option<String> {
        v.and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            // evtx crate may emit `{"$": "value"}` or `{"#text": "value"}`
            Value::Object(obj) => obj.get("$").or_else(|| obj.get("#text"))
                .and_then(|inner| get_str(Some(inner))),
            _ => None,
        })
    }

    /// Extract an attribute from an object's `#attributes` sub-object.
    fn get_attr(parent: Option<&Value>, attr: &str) -> Option<String> {
        get_str(parent.and_then(|o| o.get("#attributes")).and_then(|a| a.get(attr)))
    }

    /// Convert UserData JSON (from the evtx crate) into a flat key-value object
    /// suitable for parameter extraction.
    ///
    /// UserData format (evtx 0.12):
    /// ```json
    /// {"CbsPackageChangeState": {"#attributes": {"xmlns": "..."},
    ///   "PackageIdentifier": "KB5087051", "IntendedPackageStateTextized": "Absent"}}
    /// ```
    /// → Output: `{"PackageIdentifier": "KB5087051", "IntendedPackageStateTextized": "Absent"}`
    ///
    /// P4（2026-09-30）nested-UserData structs: when a direct child is an object
    /// WITHOUT a `#text`/`$` leaf (nested containers, e.g. Resource-Exhaustion
    /// Detector 2004 `MemoryExhaustionInfo` → `SystemInfo`/`ProcessInfo`), the
    /// legacy code dropped it entirely → `event_data=None` → all substitution
    /// params empty. Such containers are now flattened, shape-gated:
    /// - depth-2 leaf fields keep their bare name (`SystemInfo.SystemCommitLimit`
    ///   → `"SystemCommitLimit"` — matches the msgdb parameter table exactly);
    /// - depth-3 struct members get the struct name as prefix
    ///   (`ProcessInfo.Process_1.Name` → `"Process_1_Name"` — the msgdb naming
    ///   for template slots %21/%22/%24 etc.);
    /// - `null` leaves produce no key; anything deeper stays untouched.
    /// Single-level UserData (Setup channel) behaves exactly as before.
    ///
    /// Returns `None` if UserData is not in the expected format.
    fn extract_userdata_as_eventdata(userdata: &Value) -> Option<Value> {
        // Flatten one nested container (an object without a #text/$ leaf).
        // `prefix` is None for the container's direct children (depth-2: bare
        // field name) and the struct name for depth-3 members.
        fn flatten_container(container: &serde_json::Map<String, Value>, prefix: Option<&str>, flat: &mut serde_json::Map<String, Value>) {
            for (k, v) in container.iter() {
                // Skip XML attribute wrappers (#attributes, xmlns, etc.)
                if k.starts_with('#') || k.starts_with('@') {
                    continue;
                }
                let key = match prefix {
                    Some(p) => format!("{p}_{k}"),
                    None => k.clone(),
                };
                match v {
                    Value::String(_) | Value::Number(_) => {
                        flat.insert(key, v.clone());
                    }
                    Value::Object(o) => {
                        // {"#text": ...}/{"$": ...} leaf wrapper → take the leaf
                        if let Some(inner) = o.get("#text").or_else(|| o.get("$")) {
                            flat.insert(key, inner.clone());
                        } else if prefix.is_none() {
                            // One more nesting level (e.g. Process_1 → {Name, ID,...}):
                            // struct name becomes the key prefix. Deeper nesting is
                            // left untouched (conservative shape gate).
                            flatten_container(o, Some(&key), flat);
                        }
                    }
                    _ => {
                        // null / bool / array leaves: no key (null Process_N.Name etc.)
                    }
                }
            }
        }

        let userdata_obj = userdata.as_object()?;
        // UserData always contains exactly one child element (the event-specific XML element)
        let (_event_name, event_data_value) = userdata_obj.iter().next()?;
        // The child element is an object with key-value pairs (and possibly #attributes)
        match event_data_value {
            Value::Object(obj) => {
                let mut flat = serde_json::Map::new();
                for (k, v) in obj.iter() {
                    // Skip XML attribute wrappers (#attributes, xmlns, etc.)
                    if k.starts_with('#') || k.starts_with('@') {
                        continue;
                    }
                    // Extract string/number values directly, handle #text wrapper
                    let extracted = match v {
                        Value::String(s) => Some(Value::String(s.clone())),
                        Value::Number(n) => Some(Value::Number(n.clone())),
                        Value::Object(o) => {
                            // Handle {"#text": "value"} wrapper from evtx crate
                            match o.get("#text").or_else(|| o.get("$")) {
                                Some(inner) => Some(inner.clone()),
                                // P4: nested container without a leaf — flatten it
                                // (shape gate: this previously produced no key at all)
                                None => {
                                    flatten_container(o, None, &mut flat);
                                    None
                                }
                            }
                        }
                        _ => None,
                    };
                    if let Some(val) = extracted {
                        flat.insert(k.clone(), val);
                    }
                }
                if flat.is_empty() { None } else { Some(Value::Object(flat)) }
            }
            _ => None,
        }
    }

    // Helper to get a top-level field from the `System` section.
    let sys_get = |key: &str| -> Option<String> { get_str(system.and_then(|s| s.get(key))) };

    // ---- Provider ----
    let provider = system.and_then(|s| s.get("Provider"));
    let provider_name = get_attr(provider, "Name");
    let provider_guid = get_attr(provider, "Guid");

    // ---- Core event identifiers ----
    let event_id = sys_get("EventID").and_then(|s| s.parse::<u32>().ok());
    // Qualifiers attribute on EventID — used for classic EventLog providers
    // to compute the full message_id: (qualifiers << 16) | event_id
    let qualifiers: Option<u16> = system
        .and_then(|s| s.get("EventID"))
        .and_then(|v| get_attr(Some(v), "Qualifiers"))
        .and_then(|s| s.parse::<u16>().ok());
    let level = sys_get("Level").and_then(|s| s.parse::<u8>().ok());
    let task_category = sys_get("Task");
    let opcode = sys_get("Opcode");
    let keywords = sys_get("Keywords");
    let channel = sys_get("Channel");
    let computer = sys_get("Computer");

    // ---- Timestamps ----
    let timestamp = get_attr(system.and_then(|s| s.get("TimeCreated")), "SystemTime")
        .map(|s| convert_timestamp_local(&s));

    // ---- Execution (Process / Thread) ----
    let execution = system.and_then(|s| s.get("Execution"));
    let process_id = get_attr(execution, "ProcessID").and_then(|s| s.parse::<u32>().ok());
    let thread_id = get_attr(execution, "ThreadID").and_then(|s| s.parse::<u32>().ok());

    // ---- Correlation ----
    let correlation = system.and_then(|s| s.get("Correlation"));
    let activity_id = get_attr(correlation, "ActivityID");
    let related_activity_id = get_attr(correlation, "RelatedActivityID");

    // ---- Derived fields ----
    let severity = level.map(|l| severity_from_level(l).to_string());

    // ---- EventData / UserData (evtx 0.12 returns flat key-value or UserData) ----
    // evtx 0.12 format: {"SubjectUserSid":"S-1-5-...","SubjectUserName":"SYSTEM",...}
    // OLD evtx 0.8 format: {"Data":[{"#attributes":{"Name":"..."},"$":"..."}]}
    // UserData format: {"CbsEventName": {"#attributes":{...}, "Key1":"val1", "Key2":"val2"}}
    let event = data.get("Event");
    let event_data_ref = event.and_then(|v| v.get("EventData"));

    // Fall back to UserData when EventData is not present (Setup channel events, etc.)
    let event_data_value = if event_data_ref.is_some() {
        event_data_ref.cloned()
    } else {
        // Extract UserData content as a flat object
        event.and_then(|v| v.get("UserData")).and_then(extract_userdata_as_eventdata)
    };
    let event_data = event_data_value.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());

    // ---- EventData 白名单字段（源头宽表抽取：树遍历时顺手取值，零额外解析）----
    // 值统一字符串化（JSON 数字 2 → "2"，与文本 "2" 同值——查询不赌编码）
    let logon_type = event_data_value.as_ref().and_then(|v| v.get("LogonType")).and_then(|v| {
        match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    });

    // ---- user_name / user_sid from EventData/UserData (evtx 0.12) ----
    // In evtx 0.12, System.Security is null — UserID/UserName are in EventData
    // as SubjectUserName / SubjectUserSid (or TargetUserName for logon events).
    // Also handles UserData format (Setup channel events).
    // Fall back to old System.Security/@UserID for evtx 0.8 compatibility.
    let (user_name, user_sid) = match event_data_value.as_ref().and_then(|v| v.as_object()) {
        Some(obj) => {
            let un = obj.get("SubjectUserName")
                .or_else(|| obj.get("TargetUserName"))
                .and_then(|v| get_str(Some(v)));
            let sid = obj.get("SubjectUserSid")
                .or_else(|| obj.get("TargetUserSid"))
                .and_then(|v| get_str(Some(v)));
            (un, sid)
        }
        None => {
            // evtx 0.8 fallback: System.Security/@UserID
            let security = system.and_then(|s| s.get("Security"));
            let sid = get_attr(security, "UserID");
            (None, sid)
        }
    };

    // ---- Fields not available from JSON ----
    let message = None; // Resolved later via the MessageDb resolver
    let raw_xml = None; // Not available from records_json_value path

    let parsed = ParsedEvent {
        event_record_id: Some(event_record_id),
        event_time: timestamp,
        provider_name,
        provider_guid,
        event_id,
        qualifiers,
        version: system.and_then(|s| s.get("Version")).and_then(|v| get_str(Some(v))).and_then(|s| s.parse::<u8>().ok()),
        level,
        severity,
        task_category,
        opcode,
        keywords,
        channel,
        computer,
        process_id,
        thread_id,
        user_name,
        user_sid,
        activity_id,
        related_activity_id,
        message,
        level_name: None,
        keywords_name: None,
        event_data,
        raw_xml,
        file_name: Some(file_name.to_string()),
        sequence_number: None,
        logon_type,
        // 渲染链路由 process_parallel_batch 标注（提取期未渲染）
        render_via: None,
    };
    (parsed, event_data_value)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Timestamp conversion
// ---------------------------------------------------------------------------

/// Convert a UTC ISO 8601 timestamp string to local time.
///
/// Input:  `"2024-01-15T10:30:00.000Z"` (any RFC 3339 / ISO 8601 UTC format)
/// Output: `"2024-01-15 18:30:00.000000"` (local time, no `T`/`Z`, microsecond precision)
///
/// If parsing fails, returns the original string unchanged.
pub fn convert_timestamp_local(ts: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(ts) {
        Ok(dt) => dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S.%6f").to_string(),
        Err(_) => ts.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc, Local};
    use super::*;

    // ── Timestamp conversion tests ──

    #[test]
    fn test_convert_timestamp_local_utc_input() {
        let result = convert_timestamp_local("2024-01-15T10:30:00.000Z");
        // Must use space not T, must not end with Z
        assert!(!result.contains('T'), "should not contain T separator");
        assert!(!result.ends_with('Z'), "should not end with Z");
        // Must contain space separator
        assert!(result.contains(' '), "should have space separator");
        // Must have microsecond precision (at least 6 digits after last dot)
        let parts: Vec<&str> = result.split('.').collect();
        assert!(parts.len() == 2, "should have fractional seconds: {result}");
        assert_eq!(parts[1].len(), 6, "should have 6-digit microsecond precision");
        // Result must not equal the input (format changed and timezone applied)
        assert_ne!(result, "2024-01-15T10:30:00.000Z");
    }

    #[test]
    fn test_convert_timestamp_local_invariant_format() {
        let result = convert_timestamp_local("2024-01-15T10:30:00.000Z");
        // Verify the overall format pattern: YYYY-MM-DD HH:MM:SS.ffffff
        assert!(result.len() == 26, "expected 26 chars, got '{result}' (len={})", result.len());
        assert_eq!(&result[4..5], "-", "position 4 should be -");
        assert_eq!(&result[7..8], "-", "position 7 should be -");
        assert_eq!(&result[10..11], " ", "position 10 should be space");
        assert_eq!(&result[13..14], ":", "position 13 should be :");
        assert_eq!(&result[16..17], ":", "position 16 should be :");
        assert_eq!(&result[19..20], ".", "position 19 should be .");
    }

    #[test]
    fn test_convert_timestamp_local_offset_applied() {
        // Verify the timezone was actually applied — local time differs from UTC
        let utc_dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let local_dt = utc_dt.with_timezone(&Local);
        let expected = local_dt.format("%Y-%m-%d %H:%M:%S.%6f").to_string();
        assert_eq!(convert_timestamp_local("2024-01-15T10:30:00.000Z"), expected);
    }

    #[test]
    fn test_convert_timestamp_local_invalid_input() {
        // Non-parseable input should be returned unchanged
        assert_eq!(convert_timestamp_local("not-a-timestamp"), "not-a-timestamp");
    }

    #[test]
    fn test_convert_timestamp_local_empty() {
        assert_eq!(convert_timestamp_local(""), "");
    }

    #[test]
    fn test_convert_timestamp_local_microsecond_precision() {
        // Input with full microseconds
        let result = convert_timestamp_local("2024-01-15T10:30:00.123456Z");
        let parts: Vec<&str> = result.split('.').collect();
        assert_eq!(parts[1].len(), 6);
        // The fractional part after conversion should still be 123456
        // (timezone offset doesn't affect sub-second precision)
        assert_eq!(parts[1], "123456", "microseconds should be preserved");
    }

    /// Create a minimal mock EVTX JSON value for field-extraction testing.
    fn make_mock_json() -> Value {
        serde_json::from_str(r##"{
            "Event": {
                "#attributes": { "xmlns": "http://schemas.microsoft.com/win/2004/08/events/event" },
                "System": {
                    "Provider": {
                        "#attributes": {
                            "Name": "Microsoft-Windows-Security-Auditing",
                            "Guid": "{54849625-5478-4994-A5BA-3E3B0328C30D}"
                        }
                    },
                    "EventID": 4624,
                    "Version": 0,
                    "Level": 0,
                    "Task": 12544,
                    "Opcode": 0,
                    "Keywords": "0x8020000000000000",
                    "TimeCreated": {
                        "#attributes": {
                            "SystemTime": "2024-01-15T10:30:00.000Z"
                        }
                    },
                    "EventRecordID": 1001,
                    "Execution": {
                        "#attributes": {
                            "ProcessID": 500,
                            "ThreadID": 1234
                        }
                    },
                    "Channel": "Security",
                    "Computer": "PC-01.domain.local",
                    "Security": {
                        "#attributes": {
                            "UserID": "S-1-5-18"
                        }
                    },
                    "Correlation": {
                        "#attributes": {
                            "ActivityID": "{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}",
                            "RelatedActivityID": "{00000000-0000-0000-0000-000000000000}"
                        }
                    }
                },
                "EventData": {
                    "SubjectUserSid": "S-1-5-21-...",
                    "SubjectUserName": "SYSTEM"
                }
            }
        }"##).unwrap()
    }

    #[test]
    fn extract_fields_from_json() {
        let value = make_mock_json();
        let (event, event_data_val) = extract_fields(value, 1001, "test_file.evtx");

        // Core identifiers
        assert_eq!(event.event_record_id, Some(1001));
        let provider = "Microsoft-Windows-Security-Auditing";
        assert_eq!(event.provider_name.as_deref(), Some(provider));
        let guid = "{54849625-5478-4994-A5BA-3E3B0328C30D}";
        assert_eq!(event.provider_guid.as_deref(), Some(guid));
        assert_eq!(event.event_id, Some(4624));
        assert_eq!(event.level, Some(0));
        assert_eq!(event.severity.as_deref(), Some("信息"));

        // System fields
        assert_eq!(event.channel.as_deref(), Some("Security"));
        assert_eq!(event.computer.as_deref(), Some("PC-01.domain.local"));
        let expected_ts = convert_timestamp_local("2024-01-15T10:30:00.000Z");
        assert_eq!(event.event_time.as_deref(), Some(expected_ts.as_str()));

        // Execution
        assert_eq!(event.process_id, Some(500));
        assert_eq!(event.thread_id, Some(1234));

        // Security — evtx 0.12 puts UserID in EventData.SubjectUserSid
        assert_eq!(event.user_sid.as_deref(), Some("S-1-5-21-..."));

        // Correlation
        let activity = "{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}";
        assert_eq!(event.activity_id.as_deref(), Some(activity));
        let related = "{00000000-0000-0000-0000-000000000000}";
        assert_eq!(event.related_activity_id.as_deref(), Some(related));

        // EventData should be serialized as a flat JSON object (evtx 0.12 format)
        assert!(event.event_data.is_some());
        let event_data_str = event.event_data.as_deref().unwrap();
        assert!(event_data_str.contains("SubjectUserSid"));
        assert!(event_data_str.contains("SYSTEM"));

        // user_name extracted from EventData.SubjectUserName
        assert_eq!(event.user_name.as_deref(), Some("SYSTEM"));
        // user_sid extracted from EventData.SubjectUserSid
        assert_eq!(event.user_sid.as_deref(), Some("S-1-5-21-..."));

        // EventData Value should also be returned (for round-trip avoidance)
        assert!(event_data_val.is_some());
        let val = event_data_val.as_ref().unwrap();
        assert!(val.is_object());
        assert_eq!(val.get("SubjectUserName").and_then(|v| v.as_str()), Some("SYSTEM"));

        // Fields that are not extracted from JSON
        assert_eq!(event.message, None);
        assert_eq!(event.raw_xml, None);

        // File name
        let fname = "test_file.evtx";
        assert_eq!(event.file_name.as_deref(), Some(fname));
    }

    #[test]
    fn extract_fields_minimal() {
        // Test with a minimal event that has no optional fields
        let value: Value = serde_json::from_str(r##"{
            "Event": {
                "System": {
                    "Provider": {
                        "#attributes": { "Name": "Minimal" }
                    },
                    "EventID": 100,
                    "EventRecordID": 42
                }
            }
        }"##).unwrap();

        let (event, event_data_val) = extract_fields(value, 42, "minimal_file.evtx");

        assert_eq!(event.event_record_id, Some(42));
        assert_eq!(event.provider_name.as_deref(), Some("Minimal"));
        assert_eq!(event.event_id, Some(100));

        // No EventData in minimal event
        assert!(event_data_val.is_none());

        // All optional fields should be None
        assert_eq!(event.level, None);
        assert_eq!(event.severity, None);
        assert_eq!(event.channel, None);
        assert_eq!(event.computer, None);
        assert_eq!(event.process_id, None);
        assert_eq!(event.thread_id, None);
        assert_eq!(event.user_sid, None);
        assert_eq!(event.activity_id, None);
        assert_eq!(event.related_activity_id, None);
        assert_eq!(event.user_name, None);
        assert_eq!(event.message, None);
        assert_eq!(event.raw_xml, None);
        assert_eq!(event.event_data, None);
    }

    #[test]
    fn extract_fields_empty_event() {
        // Test with literally empty Event
        let value: Value = serde_json::from_str(r#"{"Event": {}}"#).unwrap();
        let (event, event_data_val) = extract_fields(value, 0, "empty_file.evtx");

        assert_eq!(event.event_record_id, Some(0));
        assert_eq!(event.provider_name, None);
        assert_eq!(event.event_id, None);
        assert!(event_data_val.is_none());
        assert_eq!(event.file_name.as_deref(), Some("empty_file.evtx"));
    }

    // ── P4（2026-09-30）：嵌套 UserData 结构展平（Resource-Exhaustion-Detector 2004 ×9）──
    //
    // 形态取证（probe 对 baseline-evtx/System.evtx 真实记录 + wevtutil raw XML 双向实证）：
    // UserData.MemoryExhaustionInfo 下是嵌套容器（SystemInfo/ProcessInfo/...），evtx 0.12
    // JSON 里是**无 #text/$ 叶子的嵌套 Object**，旧逻辑整支丢弃 → event_data=None →
    // 模板命中但 63 个参数全空（"() 使用了 字节"）。
    // 展平规则（只对"根子节点为无叶子嵌套 Object"的形态生效，单层 Setup 形态不变）：
    //   深度2 容器名丢弃：SystemInfo.SystemCommitLimit → "SystemCommitLimit"
    //   深度3 结构体名保留为前缀：ProcessInfo.Process_1.Name → "Process_1_Name"
    // msgdb 参数表（63 项）按名精确命中模板引用的 9 槽：
    //   %21=Process_1_Name %22=Process_1_ID %24=Process_1_CommitCharge
    //   %28/%29/%31、%35/%36/%38 同构（Process_2/3）。
    #[test]
    fn test_extract_fields_userdata_nested_struct_flattened() {
        // 形态与 baseline-evtx/System.evtx EventRecordID=3614 的 evtx-0.12 输出逐字段一致
        //（数字保持 JSON Number，Process_4.Name 为 null）
        let value: Value = serde_json::from_str(r##"{
            "Event": {
                "System": {
                    "Provider": {"#attributes": {"Name": "Microsoft-Windows-Resource-Exhaustion-Detector"}},
                    "EventID": 2004,
                    "Channel": "System"
                },
                "UserData": {
                    "MemoryExhaustionInfo": {
                        "#attributes": {"xmlns": "http://www.microsoft.com/Windows/Resource/Exhaustion/Detector/Events"},
                        "SystemInfo": {
                            "SystemCommitLimit": 65288781824,
                            "SystemCommitCharge": 65226944512,
                            "PagedPoolUsage": 6296014848,
                            "Processes": 1923
                        },
                        "PagedPoolInfo": {
                            "Tag_1": {"Name": "Toke", "PoolUsed": 2343021056}
                        },
                        "ProcessInfo": {
                            "Process_1": {
                                "Name": "agent-win.exe",
                                "ID": 18268,
                                "CreationTime": "2026-09-07T08:24:56.170740Z",
                                "CommitCharge": 7402741760,
                                "HandleCount": 302,
                                "Version": "1.3.11.0",
                                "TypeInfo": 201
                            },
                            "Process_2": {"Name": "Quicker.exe", "ID": 7860, "CommitCharge": 1702064128},
                            "Process_3": {"Name": "ZCode.exe", "ID": 36156, "CommitCharge": 801992704},
                            "Process_4": {"Name": null, "ID": 0, "CommitCharge": 0}
                        },
                        "ExhaustionEventInfo": {"Time": "2026-09-09T15:00:29.655227Z"}
                    }
                }
            }
        }"##).unwrap();
        let (event, event_data_val) = extract_fields(value, 3614, "System.evtx");

        // 解析层不再空置：event_data 必须携带展平后的对象
        let ed_str = event.event_data.as_deref().expect("event_data must be Some for nested UserData");
        let ed: Value = serde_json::from_str(ed_str).unwrap();
        let obj = ed.as_object().unwrap();
        // 数字叶子在 JSON 序列化后仍是 Number（下游 extract_subst_params_ext 会 to_string）
        let s = |k: &str| -> Option<String> {
            obj.get(k).map(|v| match v {
                Value::String(x) => x.clone(),
                Value::Number(n) => n.to_string(),
                other => other.to_string(),
            })
        };

        // 深度2：容器名丢弃，叶子键为字段名（与 msgdb 参数表精确同名）
        assert_eq!(s("SystemCommitLimit"), Some("65288781824".to_string()));
        assert_eq!(s("SystemCommitCharge"), Some("65226944512".to_string()));
        assert_eq!(s("PagedPoolUsage"), Some("6296014848".to_string()));
        assert_eq!(s("Time"), Some("2026-09-09T15:00:29.655227Z".to_string()));

        // 深度3：结构体名作前缀（msgdb %21/%22/%24 命名即此形态）
        assert_eq!(s("Process_1_Name"), Some("agent-win.exe".to_string()));
        assert_eq!(s("Process_1_ID"), Some("18268".to_string()));
        assert_eq!(s("Process_1_CommitCharge"), Some("7402741760".to_string()));
        assert_eq!(s("Process_2_Name"), Some("Quicker.exe".to_string()));
        assert_eq!(s("Process_2_ID"), Some("7860".to_string()));
        assert_eq!(s("Process_2_CommitCharge"), Some("1702064128".to_string()));
        assert_eq!(s("Process_3_Name"), Some("ZCode.exe".to_string()));
        assert_eq!(s("Process_3_CommitCharge"), Some("801992704".to_string()));

        // Tag 结构体同规则（模板未引用，但展平不丢）；null 叶子不产出键
        assert_eq!(s("Tag_1_Name"), Some("Toke".to_string()));
        assert_eq!(s("Tag_1_PoolUsed"), Some("2343021056".to_string()));
        assert!(!obj.contains_key("Process_4_Name"), "null leaf must not produce a key");

        // 返回给下游的 event_data_value 与序列化结果同源
        assert!(event_data_val.is_some());
    }

    #[test]
    fn test_extract_fields_userdata_single_level_flat_preserved() {
        // Setup 通道既有形态（单层 UserData）必须完全不受展平影响
        let value: Value = serde_json::from_str(r##"{
            "Event": {
                "System": {
                    "Provider": {"#attributes": {"Name": "Microsoft-Windows-Servicing"}},
                    "EventID": 1,
                    "Channel": "Setup"
                },
                "UserData": {
                    "CbsPackageChangeState": {
                        "#attributes": {"xmlns": "http://manifests.microsoft.com/win/2008/08/windows/setup_events"},
                        "PackageIdentifier": "KB5087051",
                        "IntendedPackageStateTextized": "Absent"
                    }
                }
            }
        }"##).unwrap();
        let (event, event_data_val) = extract_fields(value, 1, "Setup.evtx");

        let ed_str = event.event_data.expect("single-level UserData must still flatten");
        let ed: Value = serde_json::from_str(&ed_str).unwrap();
        let obj = ed.as_object().unwrap();
        assert_eq!(obj.len(), 2, "exactly the two direct leaf fields");
        assert_eq!(obj.get("PackageIdentifier").and_then(|v| v.as_str()), Some("KB5087051"));
        assert_eq!(obj.get("IntendedPackageStateTextized").and_then(|v| v.as_str()), Some("Absent"));
        assert!(event_data_val.is_some());
    }

    #[test]
    fn test_extract_fields_userdata_nested_object_with_text_leaf() {
        // 嵌套容器的儿子若带 #text/$ 叶子，仍按"容器键=键名"取叶子（不递归再深）
        let value: Value = serde_json::from_str(r##"{
            "Event": {
                "System": {"Provider": {"#attributes": {"Name": "X"}}, "EventID": 1},
                "UserData": {
                    "Root": {
                        "Wrapper": {"#text": "leaf-value"},
                        "Dollar": {"$": "dollar-value"}
                    }
                }
            }
        }"##).unwrap();
        let (event, _) = extract_fields(value, 2, "t.evtx");
        let ed_str = event.event_data.expect("text-wrapped nested objects stay extractable");
        let ed: Value = serde_json::from_str(&ed_str).unwrap();
        let obj = ed.as_object().unwrap();
        assert_eq!(obj.get("Wrapper").and_then(|v| v.as_str()), Some("leaf-value"));
        assert_eq!(obj.get("Dollar").and_then(|v| v.as_str()), Some("dollar-value"));
    }

    #[test]
    fn test_detect_evtx_build_invalid_path() {
        assert_eq!(detect_evtx_build("/nonexistent/file.evtx"), None);
    }

    // -----------------------------------------------------------------------
    // scan_evtx_channels tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_scan_evtx_channels_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/test-dataset/evtx/DE_WinEventLogSvc_Crash_System_7036.evtx"
        );
        let channels = scan_evtx_channels(path, 1).unwrap();
        assert!(!channels.is_empty(), "expected at least one channel");
        assert!(channels.contains(&"System".to_string()),
            "expected System channel in '{}', got: {:?}", path, channels);
    }

    #[test]
    fn test_scan_evtx_channels_not_found() {
        let result = scan_evtx_channels("/nonexistent/file.evtx", 1);
        assert!(result.is_err(), "expected Err for non-existent file");
    }

    // -----------------------------------------------------------------------
    // iter_events_with_xml tests (--with-xml)
    // -----------------------------------------------------------------------

    #[test]
    fn test_iter_events_with_xml_populates_raw_xml() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/test-dataset/evtx/DE_WinEventLogSvc_Crash_System_7036.evtx"
        );
        let mut parser = open_evtx(path, 1).unwrap();
        let mut xml_parser = open_evtx(path, 1).unwrap();
        let events: Vec<_> = iter_events_with_xml(&mut parser, &mut xml_parser, "t.evtx".into())
            .map(|r| r.expect("event should parse"))
            .collect();
        assert!(!events.is_empty(), "fixture should yield events");

        // Same record set as the plain iterator (no events lost/added).
        let plain_count = {
            let mut p = open_evtx(path, 1).unwrap();
            iter_events(&mut p, "t.evtx".into())
                .map(|r| r.expect("event should parse"))
                .count()
        };
        assert_eq!(events.len(), plain_count, "xml pass must not drop events");

        for (ev, _) in &events {
            let rid = ev.event_record_id.expect("record id present");
            let xml = ev.raw_xml.as_ref().unwrap_or_else(|| {
                panic!("raw_xml must be populated for record {rid} (--with-xml)")
            });
            assert!(xml.contains("<Event"), "record {rid}: not Event XML: {xml:.80}");
        }
    }

    #[test]
    fn test_iter_events_with_xml_record_ids_match() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/test-dataset/evtx/DE_WinEventLogSvc_Crash_System_7036.evtx"
        );
        // Independent source of truth: the embedded <EventRecordID> of each
        // record from a plain XML pass. The zip must reproduce this exact
        // sequence — one XML payload per event, in stream order.
        let xml_ids = |data: &str| -> Option<String> {
            let tag = "<EventRecordID>";
            let start = data.find(tag)? + tag.len();
            let end = data[start..].find('<')? + start;
            Some(data[start..end].to_string())
        };
        let expected: Vec<String> = {
            let mut p0 = open_evtx(path, 1).unwrap();
            p0.records()
                .filter_map(|r| r.ok())
                .filter_map(|r| xml_ids(&r.data))
                .collect()
        };
        assert!(!expected.is_empty(), "fixture XML should carry EventRecordID");
        let mut parser = open_evtx(path, 1).unwrap();
        let mut xml_parser = open_evtx(path, 1).unwrap();
        let got: Vec<String> = iter_events_with_xml(&mut parser, &mut xml_parser, "t.evtx".into())
            .filter_map(|r| r.ok())
            .filter_map(|(ev, _)| ev.raw_xml)
            .filter_map(|xml| xml_ids(&xml))
            .collect();
        assert_eq!(got, expected, "zip misaligned: raw_xml stream != XML stream order");
    }

    // -------------------------------------------------------------------
    // extract_batch_parallel / iter_raw_json_values_with_xml
    // (R38 生产者并行化：22 字段 JSON 遍历前移进 rayon fork-join 段)
    // -------------------------------------------------------------------

    fn xml_fixture_path() -> &'static str {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/test-dataset/evtx/DE_WinEventLogSvc_Crash_System_7036.evtx"
        )
    }

    #[test]
    fn test_extract_batch_parallel_matches_serial_iter_events() {
        let path = xml_fixture_path();
        let serial: Vec<(ParsedEvent, Option<Value>)> = {
            let mut p = open_evtx(path, 1).unwrap();
            iter_events(&mut p, "t.evtx".into())
                .map(|r| r.expect("event should parse"))
                .collect()
        };
        assert!(!serial.is_empty(), "fixture should yield events");

        let raw: Vec<(Value, u64)> = {
            let mut p = open_evtx(path, 1).unwrap();
            iter_raw_json_values(&mut p)
                .map(|r| r.expect("event should parse"))
                .collect()
        };
        assert_eq!(raw.len(), serial.len(), "raw and serial must cover the same records");

        let raw3: Vec<(Value, u64, Option<String>)> =
            raw.into_iter().map(|(v, rid)| (v, rid, None)).collect();
        let parallel = extract_batch_parallel(raw3, "t.evtx");

        for ((se, sv), (pe, pv)) in serial.into_iter().zip(parallel) {
            assert_eq!(se.event_record_id, pe.event_record_id, "record order must be preserved");
            let sj = serde_json::to_string(&se).unwrap();
            let pj = serde_json::to_string(&pe).unwrap();
            assert_eq!(sj, pj, "ParsedEvent mismatch at record {:?}", pe.event_record_id);
            assert_eq!(sv, pv, "EventData value mismatch at record {:?}", pe.event_record_id);
        }
    }

    #[test]
    fn test_iter_raw_json_values_with_xml_zip_matches_iter_events_with_xml() {
        let path = xml_fixture_path();
        // Reference: the established zip+extract iterator.
        let reference: Vec<(Option<u64>, Option<String>)> = {
            let mut p = open_evtx(path, 1).unwrap();
            let mut xp = open_evtx(path, 1).unwrap();
            iter_events_with_xml(&mut p, &mut xp, "t.evtx".into())
                .map(|r| r.expect("event should parse"))
                .map(|(e, _)| (e.event_record_id, e.raw_xml))
                .collect()
        };
        // New raw zip: identical (rid, raw_xml) pairing, unextracted Value.
        let raw: Vec<(Value, u64, Option<String>)> = {
            let mut p = open_evtx(path, 1).unwrap();
            let mut xp = open_evtx(path, 1).unwrap();
            iter_raw_json_values_with_xml(&mut p, &mut xp)
                .map(|r| r.expect("event should parse"))
                .collect()
        };
        assert_eq!(raw.len(), reference.len(), "zip must not drop events");
        for ((v, rid, xml), (ref_rid, ref_xml)) in raw.into_iter().zip(reference) {
            assert_eq!(Some(rid), ref_rid, "record id zip misaligned");
            assert_eq!(xml, ref_xml, "raw_xml zip misaligned at record {rid}");
            assert!(v.get("Event").is_some(), "raw Value must be the full record");
        }
    }

    /// Perf probe (manual, `#[ignore]`): splits the serial producer cost into
    /// the evtx crate's JSON tree build vs our `extract_fields` traversal —
    /// the extract share is what [`extract_batch_parallel`] can move onto the
    /// rayon pool.
    ///
    /// Run: `EVTX_PROBE_FILE=<path> cargo test --release -p evtx-tools-core
    /// --lib -- --ignored probe_serial_split`
    #[test]
    #[ignore = "manual perf probe: set EVTX_PROBE_FILE"]
    fn probe_serial_split_tree_build_vs_extract() {
        let Ok(path) = std::env::var("EVTX_PROBE_FILE") else { return };
        let t0 = std::time::Instant::now();
        let mut p = open_evtx(&path, 1).unwrap();
        let mut tree_only = 0u64;
        for raw in iter_raw_json_values(&mut p) {
            if raw.is_ok() {
                tree_only += 1;
            }
        }
        let tree = t0.elapsed();

        let t1 = std::time::Instant::now();
        let mut p = open_evtx(&path, 1).unwrap();
        let mut full = 0u64;
        for ev in iter_events(&mut p, "probe.evtx".into()) {
            if ev.is_ok() {
                full += 1;
            }
        }
        let both = t1.elapsed();

        let extract = both.saturating_sub(tree);
        eprintln!("[probe] {path}");
        eprintln!("[probe] records: tree_only={tree_only} full={full}");
        eprintln!(
            "[probe] tree_build={tree:?} full_iter={both:?} extract≈{extract:?} ({:.0}% of serial)",
            100.0 * extract.as_secs_f64() / both.as_secs_f64().max(1e-9)
        );
    }
}

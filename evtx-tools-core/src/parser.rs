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

    // ---- EventData (evtx 0.12 returns a flat key-value object) ----
    // evtx 0.12 format: {"SubjectUserSid":"S-1-5-...","SubjectUserName":"SYSTEM",...}
    // OLD evtx 0.8 format: {"Data":[{"#attributes":{"Name":"..."},"$":"..."}]}
    // We handle both by checking the actual structure.
    let event_data_ref = data
        .get("Event")
        .and_then(|v| v.get("EventData"));

    // Clone once: serialise to String for output AND keep a Value for message
    // resolution (avoids Value→String→Value round trip in extract_subst_params).
    let event_data_value = event_data_ref.cloned();
    let event_data = event_data_ref.map(|v| serde_json::to_string(v).unwrap_or_default());

    // ---- user_name / user_sid from EventData (evtx 0.12) ----
    // In evtx 0.12, System.Security is null — UserID/UserName are in EventData
    // as SubjectUserName / SubjectUserSid (or TargetUserName for logon events).
    // Fall back to old System.Security/@UserID for evtx 0.8 compatibility.
    let (user_name, user_sid) = match event_data_ref.and_then(|v| v.as_object()) {
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

    #[test]
    fn test_detect_evtx_build_invalid_path() {
        assert_eq!(detect_evtx_build("/nonexistent/file.evtx"), None);
    }
}

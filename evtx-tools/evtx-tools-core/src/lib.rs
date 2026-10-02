use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

pub type Result<T> = std::result::Result<T, String>;

// ---------------------------------------------------------------------------
// Severity mapping
// ---------------------------------------------------------------------------

/// Map a Windows Event Log level value to a Chinese severity string.
///
/// | Level | Meaning   | Severity |
/// |-------|-----------|----------|
/// | 0     |           | 信息     |
/// | 1     | Critical  | 关键     |
/// | 2     | Error     | 错误     |
/// | 3     | Warning   | 警告     |
/// | 4     |           | 信息     |
/// | 5     | Verbose   | 详细     |
/// | other |           | 信息     |
pub fn severity_from_level(level: u8) -> &'static str {
    match level {
        1 => "关键",
        2 => "错误",
        3 => "警告",
        5 => "详细",
        _ => "信息",
    }
}

/// Compute the full message ID for classic EventLog providers.
///
/// Windows Event Log stores the message resource identifier as a 32-bit value:
/// ```text
/// message_id = (qualifiers << 16) | event_id
/// ```
/// For EventManifest providers (no qualifiers), the event_id is used directly.
pub fn compute_message_id(event_id: u32, qualifiers: Option<u16>) -> u32 {
    match qualifiers {
        Some(q) => (q as u32) << 16 | event_id,
        None => event_id,
    }
}

// ---------------------------------------------------------------------------
// Core data types
// ---------------------------------------------------------------------------

/// A parsed EVTX event with all 22 columns.
///
/// All fields are `Option` because not every event provides every field.
/// This struct is the single output data model for the entire crate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedEvent {
    pub event_record_id: Option<u64>,
    pub event_time: Option<String>,
    pub provider_name: Option<String>,
    pub provider_guid: Option<String>,
    pub event_id: Option<u32>,
    /// Qualifiers attribute from EventID (classic EventLog providers only).
    /// Used to compute the full message_id: `(qualifiers << 16) | event_id`.
    pub qualifiers: Option<u16>,
    /// Version from System.Version (CRIM EventEntry.Version).
    pub version: Option<u8>,
    pub level: Option<u8>,
    pub severity: Option<String>,
    pub task_category: Option<String>,
    pub opcode: Option<String>,
    pub keywords: Option<String>,
    pub channel: Option<String>,
    pub computer: Option<String>,
    pub process_id: Option<u32>,
    pub thread_id: Option<u32>,
    pub user_name: Option<String>,
    pub user_sid: Option<String>,
    pub activity_id: Option<String>,
    pub related_activity_id: Option<String>,
    pub message: Option<String>,
    /// Human-readable Level name from CRIM LEVL block (e.g. "Information", "Error").
    pub level_name: Option<String>,
    /// Human-readable Keyword names from CRIM KEYW block (e.g. "Audit Success").
    pub keywords_name: Option<String>,
    pub event_data: Option<String>,
    pub raw_xml: Option<String>,
    pub file_name: Option<String>,
    /// Global sequence number — monotonically incrementing parse order.
    /// Preserves Event Viewer sequence even across EVTX files.
    pub sequence_number: Option<u64>,
}

/// Variant identity — 8-tuple Message Variant (CONTEXT.md: Variant Identity).
/// Mirrors msgdb-dump/crim.VariantKey. Two variants share a Key iff all 8 fields equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VariantKey {
    pub provider_name: String,
    pub event_id: u32,
    pub version: u8,
    pub level: u8,
    pub opcode: u8,
    pub task: u16,
    pub keywords: u64,
    pub channel: u8,
    pub message_id: u32,
}

/// A cached message resolution result from messages.db.
///
/// `template` is the message template string with `%1`, `%2` etc. placeholders.
/// `parameters` maps placeholder numbers to human-readable parameter names
/// (e.g., `{"1": "SubjectUserSid", "2": "SubjectUserName"}`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageRow {
    pub template: String,
    pub level: Option<u8>,
    pub task: Option<String>,
    pub opcode: Option<String>,
    pub keywords: Option<String>,
    pub parameters: Option<HashMap<String, String>>,
    pub level_name: Option<String>,
    pub keywords_name: Option<String>,
    /// Per-parameter value map references from the param_value_maps table:
    /// param_index ("1", "2", ...) -> map_name (key into the value_maps cache).
    /// Derived from the CRIM TEMP item +8 field (manifest data map attribute).
    #[serde(default)]
    pub param_map_refs: HashMap<String, String>,
    /// Param indices (keys of param_map_refs) whose map is a bitmap (BMAP).
    #[serde(default)]
    pub param_bitmap_refs: std::collections::HashSet<String>,
    /// Per-parameter TDH InType/OutType hints (R10 / ADR 0005):
    /// param_index ("1", "2", ...) -> "in:out" hex byte pair (e.g. "14:12").
    /// Empty map = no type information → passthrough rendering.
    #[serde(default)]
    pub param_types: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Module declarations
// ---------------------------------------------------------------------------

pub mod storage;

pub mod stats;
pub mod substitute;
pub mod parser;
pub mod pipeline;
pub mod resolver;
pub mod variant_resolver;
pub mod routing;
pub mod shard;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_message_id_no_qualifiers() {
        assert_eq!(compute_message_id(4624, None), 4624);
        assert_eq!(compute_message_id(100, None), 100);
        assert_eq!(compute_message_id(0, None), 0);
    }

    #[test]
    fn test_compute_message_id_with_qualifiers() {
        // hcmon EventID=0, Qualifiers=32768:
        //   32768 << 16 | 0 = 0x80000000 = 2147483648
        assert_eq!(
            compute_message_id(0, Some(32768)),
            0x80000000u32 as u32
        );
        // Qualifiers=0, EventID=100:
        //   0 << 16 | 100 = 100
        assert_eq!(compute_message_id(100, Some(0)), 100);
        // Qualifiers=1, EventID=1:
        //   1 << 16 | 1 = 65537
        assert_eq!(compute_message_id(1, Some(1)), 65537);
    }

    #[test]
    fn test_severity_mapping_critical() {
        assert_eq!(severity_from_level(1), "关键");
    }

    #[test]
    fn test_severity_mapping_error() {
        assert_eq!(severity_from_level(2), "错误");
    }

    #[test]
    fn test_severity_mapping_warning() {
        assert_eq!(severity_from_level(3), "警告");
    }

    #[test]
    fn test_severity_mapping_info() {
        assert_eq!(severity_from_level(0), "信息");
        assert_eq!(severity_from_level(4), "信息");
    }

    #[test]
    fn test_severity_mapping_verbose() {
        assert_eq!(severity_from_level(5), "详细");
    }

    #[test]
    fn test_severity_mapping_unknown_defaults_to_info() {
        assert_eq!(severity_from_level(99), "信息");
        assert_eq!(severity_from_level(255), "信息");
    }

    #[test]
    fn test_parsed_event_creation_roundtrip() {
        let event = ParsedEvent {
            event_record_id: Some(1),
            event_time: Some("2024-01-01T00:00:00Z".into()),
            provider_name: Some("Test-Provider".into()),
            provider_guid: Some("{00000000-0000-0000-0000-000000000000}".into()),
            event_id: Some(4624),
            qualifiers: None,
            version: None,
            level: Some(0),
            severity: Some("信息".into()),
            task_category: None,
            opcode: None,
            keywords: None,
            channel: Some("Security".into()),
            computer: Some("PC-1".into()),
            process_id: Some(4),
            thread_id: Some(123),
            user_name: Some("SYSTEM".into()),
            user_sid: Some("S-1-5-18".into()),
            activity_id: None,
            related_activity_id: None,
            message: Some("已成功登录帐户。".into()),
            level_name: None,
            keywords_name: None,
            event_data: Some(r#"{"TargetUserName":"admin"}"#.into()),
            raw_xml: None,
            sequence_number: None,
            file_name: Some("test.evtx".into()),
        };

        assert_eq!(event.event_record_id, Some(1));
        assert_eq!(event.provider_name.as_deref(), Some("Test-Provider"));
        assert_eq!(event.event_id, Some(4624));
        assert_eq!(event.severity.as_deref(), Some("信息"));
        assert_eq!(event.process_id, Some(4));
        assert_eq!(event.thread_id, Some(123));
        assert!(event.task_category.is_none());
        assert!(event.activity_id.is_none());
        assert!(event.raw_xml.is_none());

        // Verify serialization roundtrip
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ParsedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_record_id, Some(1));
        assert_eq!(deserialized.provider_name, Some("Test-Provider".into()));
    }

    #[test]
    fn test_parsed_event_all_none() {
        let event = ParsedEvent {
            event_record_id: None,
            event_time: None,
            provider_name: None,
            provider_guid: None,
            event_id: None,
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
            sequence_number: None,
            file_name: None,
        };

        // Verify serde roundtrip with all None
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ParsedEvent = serde_json::from_str(&json).unwrap();
        assert!(deserialized.event_record_id.is_none());
        assert!(deserialized.provider_name.is_none());
        assert!(deserialized.event_id.is_none());
        assert!(deserialized.message.is_none());
    }

    #[test]
    fn test_message_row_creation() {
        let row = MessageRow {
            template: "已成功登录帐户。%n%n帐户名: %1".into(),
            level: None,
            task: Some("Logon".into()),
            opcode: Some("Info".into()),
            keywords: Some("Audit Success".into()),
            parameters: Some(HashMap::from([
                ("1".into(), "TargetUserName".into()),
                ("2".into(), "TargetDomainName".into()),
            ])),
            level_name: None,
            keywords_name: None,
            param_map_refs: HashMap::new(),
            param_bitmap_refs: HashSet::new(),
            param_types: HashMap::new(),
        };

        assert_eq!(row.template, "已成功登录帐户。%n%n帐户名: %1");
        assert_eq!(row.task.as_deref(), Some("Logon"));
        assert_eq!(row.opcode.as_deref(), Some("Info"));
        assert_eq!(row.keywords.as_deref(), Some("Audit Success"));
        assert!(row.parameters.as_ref().unwrap().contains_key("1"));
        assert_eq!(
            row.parameters.as_ref().unwrap().get("1").map(|s| s.as_str()),
            Some("TargetUserName")
        );
    }

    #[test]
    fn test_result_type_works_with_string_error() {
        fn fallible(ok: bool) -> Result<i32> {
            if ok {
                Ok(42)
            } else {
                Err("something went wrong".into())
            }
        }

        assert_eq!(fallible(true).unwrap(), 42);
        assert_eq!(fallible(false).unwrap_err(), "something went wrong");
    }
}

use std::io::Write;

use crate::{ParsedEvent, Result};

// ---------------------------------------------------------------------------
// Column order
// ---------------------------------------------------------------------------

/// The canonical column order for CSV output, matching the `events` table.
const CSV_HEADER: &[&str] = &[
    "event_record_id",
    "event_time",
    "provider_name",
    "provider_guid",
    "event_id",
    "level",
    "severity",
    "task_category",
    "opcode",
    "keywords",
    "channel",
    "computer",
    "process_id",
    "thread_id",
    "user_name",
    "user_sid",
    "activity_id",
    "related_activity_id",
    "message",
    "level_name",
    "keywords_name",
    "event_data",
    "raw_xml",
    "file_name",
    "sequence_number",
];

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Write a batch of `ParsedEvent` rows as CSV, including a UTF-8 BOM.
///
/// The CSV uses `csv::WriterBuilder` with standard options (comma delimiter,
/// double-quote quoting). A UTF-8 BOM (`\u{FEFF}`) is written before the
/// header so that Chinese characters display correctly in Excel.
pub fn write_events<W: Write>(mut writer: W, events: &[ParsedEvent]) -> Result<()> {
    write_csv_header(&mut writer)?;
    write_event_rows(writer, events)
}

/// Write the UTF-8 BOM + header row. Call exactly once per output file,
/// before the first [`write_event_rows`] call.
pub fn write_csv_header<W: Write>(mut writer: W) -> Result<()> {
    // Write UTF-8 BOM before csv::Writer so Excel displays Chinese correctly
    writer
        .write_all(&[0xEF, 0xBB, 0xBF])
        .map_err(|e| format!("failed to write BOM: {e}"))?;

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b',')
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(&mut writer);

    wtr.write_record(CSV_HEADER)
        .map_err(|e| format!("failed to write csv header: {e}"))?;

    wtr.flush().map_err(|e| format!("failed to flush csv writer: {e}"))
}

/// Append a batch of `ParsedEvent` rows without BOM or header.
///
/// flush_batch calls this once per producer batch; the header must have been
/// emitted exactly once up front (run_parse), otherwise batch boundaries
/// would grow duplicate header rows (Security ≈ 102 batches ≈ 102 junk rows).
pub fn write_event_rows<W: Write>(writer: W, events: &[ParsedEvent]) -> Result<()> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b',')
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(writer);

    for event in events {
        wtr.write_record(&[
            opt_u64_str(&event.event_record_id),
            opt_str(&event.event_time),
            opt_str(&event.provider_name),
            opt_str(&event.provider_guid),
            opt_u32_str(&event.event_id),
            opt_u8_str(&event.level),
            opt_str(&event.severity),
            opt_str(&event.task_category),
            opt_str(&event.opcode),
            opt_str(&event.keywords),
            opt_str(&event.channel),
            opt_str(&event.computer),
            opt_u32_str(&event.process_id),
            opt_u32_str(&event.thread_id),
            opt_str(&event.user_name),
            opt_str(&event.user_sid),
            opt_str(&event.activity_id),
            opt_str(&event.related_activity_id),
            opt_str(&event.message),
            opt_str(&event.level_name),
            opt_str(&event.keywords_name),
            opt_str(&event.event_data),
            opt_str(&event.raw_xml),
            opt_str(&event.file_name),
            opt_u64_str(&event.sequence_number),
        ])
        .map_err(|e| format!("failed to write csv row: {e}"))?;
    }

    wtr.flush().map_err(|e| format!("failed to flush csv writer: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn opt_str(v: &Option<String>) -> String {
    v.as_deref().unwrap_or("").to_string()
}

fn opt_u64_str(v: &Option<u64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

fn opt_u32_str(v: &Option<u32>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

fn opt_u8_str(v: &Option<u8>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_event(event_record_id: u64) -> ParsedEvent {
        ParsedEvent {
            event_record_id: Some(event_record_id),
            event_time: Some("2024-01-01T00:00:00Z".into()),
            provider_name: Some("Test-Provider".into()),
            provider_guid: None,
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
            user_name: None,
            user_sid: None,
            activity_id: None,
            related_activity_id: None,
            message: Some("已成功登录帐户。".into()),
            level_name: None,
            keywords_name: None,
            event_data: Some(r#"{"TargetUserName":"admin"}"#.into()),
            raw_xml: None,
            sequence_number: None,
            file_name: Some("test.evtx".into()),
        }
    }

    #[test]
    fn test_csv_batched_appends_do_not_repeat_header() {
        // 回归：flush_batch 曾逐批复用 write_events，每批都重写 BOM+表头
        // （实测 Application 34,255 事件产出 3 条表头，Security 约 102 条，
        // pandas/Excel 读取在批次边界报错）。契约：调用方预写一次表头，
        // 各批次经 write_event_rows 只追加数据行。
        let mut output = Vec::new();
        write_csv_header(&mut output).unwrap();
        write_event_rows(&mut output, &[make_test_event(1)]).unwrap();
        write_event_rows(&mut output, &[make_test_event(2), make_test_event(3)]).unwrap();
        let content = String::from_utf8(output).unwrap();

        assert_eq!(
            content.matches('\u{FEFF}').count(),
            1,
            "BOM 必须只出现一次，不得随批次重复"
        );
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 4); // header + 3 data rows
        assert!(lines[0].contains("event_record_id"));
        assert!(lines[1].contains("Test-Provider"));
    }

    #[test]
    fn test_csv_header() {
        let mut output = Vec::new();
        write_events(&mut output, &[]).unwrap();
        let content = String::from_utf8(output).unwrap();

        // Verify BOM is present
        assert!(content.starts_with('\u{FEFF}'));

        // Verify header line
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("event_record_id"));
        assert!(lines[0].contains("provider_name"));
        assert!(lines[0].contains("file_name"));
        assert_eq!(lines[0].matches(',').count(), 24); // 25 columns = 24 commas
    }

    #[test]
    fn test_csv_single_event() {
        let mut output = Vec::new();
        let event = make_test_event(1);
        write_events(&mut output, &[event]).unwrap();
        let content = String::from_utf8(output).unwrap();

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("Test-Provider"));
        assert!(lines[1].contains("4624"));
        assert!(lines[1].contains("信息"));
        assert!(lines[1].contains("PC-1"));
    }

    #[test]
    fn test_csv_multiple_events() {
        let mut output = Vec::new();
        let events: Vec<ParsedEvent> = (1..=3).map(make_test_event).collect();
        write_events(&mut output, &events).unwrap();
        let content = String::from_utf8(output).unwrap();

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 4); // header + 3 data rows
    }

    #[test]
    fn test_csv_null_fields_empty() {
        let mut output = Vec::new();
        let event = ParsedEvent {
            event_record_id: Some(1),
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
        write_events(&mut output, &[event]).unwrap();
        let content = String::from_utf8(output).unwrap();

        let lines: Vec<&str> = content.lines().collect();
        let fields: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(fields.len(), 25); // 23 original + level_name + keywords_name + sequence_number
        assert_eq!(fields[0], "1"); // event_record_id
        for f in &fields[1..] {
            assert_eq!(*f, "", "expected empty field after event_record_id, got: {f}");
        }
    }

    #[test]
    fn test_csv_unicode() {
        let mut output = Vec::new();
        let mut event = make_test_event(1);
        event.message = Some("已成功登录帐户。".into());
        write_events(&mut output, &[event]).unwrap();
        let content = String::from_utf8(output).unwrap();

        assert!(content.contains("已成功登录帐户。"));
    }

    #[test]
    fn test_csv_empty_batch_only_header() {
        let mut output = Vec::new();
        write_events(&mut output, &[]).unwrap();
        let content = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1); // only header
    }

    #[test]
    fn test_csv_roundtrip_all_fields() {
        let mut output = Vec::new();
        let event = ParsedEvent {
            event_record_id: Some(42),
            event_time: Some("2025-06-15T10:30:00Z".into()),
            provider_name: Some("Microsoft-Windows-Security-Auditing".into()),
            provider_guid: Some("{54849625-5478-4994-A5BA-3E3B0328C30D}".into()),
            event_id: Some(4625),
            qualifiers: None,
            version: None,
            level: Some(3),
            severity: Some("警告".into()),
            task_category: Some("Logon".into()),
            opcode: Some("Failure".into()),
            keywords: Some("Audit Failure".into()),
            channel: Some("Security".into()),
            computer: Some("DC-01.domain.local".into()),
            process_id: Some(532),
            thread_id: Some(1234),
            user_name: Some("ANONYMOUS LOGON".into()),
            user_sid: Some("S-1-5-7".into()),
            activity_id: Some("{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}".into()),
            related_activity_id: Some("{00000000-0000-0000-0000-000000000000}".into()),
            message: Some("帐户登录失败。".into()),
            level_name: None,
            keywords_name: None,
            event_data: Some(r#"{"TargetUserName":"admin"}"#.into()),
            raw_xml: Some("<Event>...</Event>".into()),
            file_name: Some("security.evtx".into()),
            sequence_number: None,
        };
        write_events(&mut output, &[event]).unwrap();
        let content = String::from_utf8(output).unwrap();

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("42"));
        assert!(lines[1].contains("Microsoft-Windows-Security-Auditing"));
        assert!(lines[1].contains("4625"));
        assert!(lines[1].contains("警告"));
        assert!(lines[1].contains("DC-01.domain.local"));
        assert!(lines[1].contains("帐户登录失败。"));
        assert!(lines[1].contains("security.evtx"));
    }
}

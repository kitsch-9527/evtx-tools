//! Template string substitution engine for Windows Event Log messages.
//!
//! Replaces `%1` – `%99` placeholders with positional parameters,
//! and handles special character codes (`%n`, `%r`, `%t`, `%%`).
//!
//! # Examples
//!
//! ```
//! use evtx_tools_core::substitute::substitute;
//!
//! assert_eq!(substitute("Hello %1", &["World"]), "Hello World");
//! assert_eq!(substitute("%n%t%%", &[]), "\r\n\t%");
//! ```

use std::collections::{HashMap, HashSet};

/// Format a resolved message for display, matching Windows Event Viewer conventions.
///
/// Post-processing steps:
/// 1. Strip `\r` from CRLF → LF (Windows Event Viewer does this)
/// 2. Normalize GUIDs to `{XXXXXXXX-XXXX-...}` (uppercase, braces)
/// 3. Convert decimal HRESULT/status codes (≥0x80000000) to hex `0x%08X`
///
/// This should be called after `substitute()` and `expand_param_refs()`.
pub fn format_for_display(message: &str) -> String {
    let mut result = message.to_string();

    // Step 1: Strip \r (Windows Event Viewer strips CR from CRLF)
    result = result.replace('\r', "");

    // Step 2: Normalize GUIDs to {UPPERCASE-with-braces}
    // Scan for GUID-like patterns: 8-4-4-4-12 hex digits with dashes
    let mut guid_result = String::with_capacity(result.len());
    let bytes = result.as_bytes();
    let mut pos = 0;
    while pos < result.len() {
        // Check for GUID (optionally wrapped in braces)
        let has_brace = bytes[pos] == b'{';
        let start = if has_brace { pos + 1 } else { pos };
        if let Some(end) = scan_guid(&result, start) {
            let raw = &result[start..end];
            if raw.len() == 36
                && raw.chars().filter(|&c| c == '-').count() == 4
                && raw.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
            {
                // If input had opening brace, include closing after GUID
                if has_brace && end < result.len() && bytes[end] == b'}' {
                    guid_result.push('{');
                    for c in raw.chars() {
                        guid_result.push(c.to_ascii_uppercase());
                    }
                    guid_result.push('}');
                    pos = end + 1;
                    continue;
                }
                // No braces in input — check if between %7B/%7D (URL-encoded braces).
                // If so, don't add extra braces — just lowercase. The %7B/%7D are
                // already in the output (pushed character by character before/after
                // the GUID scan) and will be consumed by the second FormatMessageW pass.
                if pos >= 3 && end + 3 <= result.len()
                    && bytes[pos - 3..pos].eq_ignore_ascii_case(b"%7B")
                    && bytes[end..end + 3].eq_ignore_ascii_case(b"%7D")
                {
                    for c in raw.chars() {
                        guid_result.push(c.to_ascii_uppercase());
                    }
                } else {
                    guid_result.push('{');
                    for c in raw.chars() {
                        guid_result.push(c.to_ascii_uppercase());
                    }
                    guid_result.push('}');
                }
                pos = end;
                continue;
            }
        }
        // Use char_indices to safely handle multi-byte UTF-8
        let (_, ch) = result[pos..].char_indices().next().unwrap();
        let clen = ch.len_utf8();
        guid_result.push_str(&result[pos..pos + clen]);
        pos += clen;
    }
    result = guid_result;

    // Step 3: Normalize hex values and convert large decimals.
    // Iterate using char_indices() for safe UTF-8 handling.
    let mut hex_result = String::with_capacity(result.len());
    let chars: Vec<(usize, char)> = result.char_indices().collect();
    let mut ci = 0;
    while ci < chars.len() {
        let (byte_idx, ch) = chars[ci];
        // Skip braced groups
        if ch == '{' {
            if let Some(close_byte) = result[byte_idx..].find('}') {
                let end_byte = byte_idx + close_byte + 1;
                hex_result.push_str(&result[byte_idx..end_byte]);
                let end_ci = result[..end_byte].chars().count();
                ci = end_ci;
                continue;
            }
        }
        // Match "0x" hex values → uppercase
        if ch == '0' && ci + 1 < chars.len() && chars[ci + 1].1 == 'x' {
            let start_byte = byte_idx;
            let mut hex_end = ci + 2;
            while hex_end < chars.len() && chars[hex_end].1.is_ascii_hexdigit() {
                hex_end += 1;
            }
            hex_result.push_str("0x");
            if hex_end > ci + 2 {
                let hex_start = chars[ci + 2].0;
                let hex_last_end = chars[hex_end - 1].0 + chars[hex_end - 1].1.len_utf8();
                hex_result.push_str(&result[hex_start..hex_last_end].to_uppercase());
            }
            ci = hex_end;
            continue;
        }
        // Match decimal numbers
        if ch.is_ascii_digit() {
            let start_byte = byte_idx;
            let start_ci = ci;
            while ci < chars.len() && chars[ci].1.is_ascii_digit() {
                ci += 1;
            }
            let end_byte = if ci < chars.len() { chars[ci].0 } else { result.len() };
            let num_str = &result[start_byte..end_byte];
            // Check SID context
            let is_sid_context = start_byte > 0
                && result.as_bytes()[start_byte - 1] == b'-'
                && result[..start_byte].contains("S-");
            let is_sid_direct = start_ci >= 2 && chars[start_ci - 2].1 == 'S' && chars[start_ci - 1].1 == '-';
            let is_guid_context = start_byte > 0 && ci < chars.len()
                && (result.as_bytes()[start_byte - 1].is_ascii_hexdigit() || result.as_bytes()[start_byte - 1] == b'-')
                && chars[ci].1.is_ascii_hexdigit();
            if !is_sid_context && !is_sid_direct && !is_guid_context {
                if let Ok(val) = num_str.parse::<u64>() {
                    if val >= 0x80000000 {
                        hex_result.push_str(&format!("0x{:08X}", val));
                        continue;
                    }
                }
            }
            hex_result.push_str(num_str);
            continue;
        }
        // Any other character — copy as-is (safe for all UTF-8)
        let clen = ch.len_utf8();
        hex_result.push_str(&result[byte_idx..byte_idx + clen]);
        ci += 1;
    }
    result = hex_result;

    result
}

/// Return the byte width of a UTF-8 character from its leading byte.
#[inline]
fn utf8_char_width(leading: u8) -> usize {
    if leading & 0x80 == 0 {
        1
    } else if leading & 0xE0 == 0xC0 {
        2
    } else if leading & 0xF0 == 0xE0 {
        3
    } else if leading & 0xF8 == 0xF0 {
        4
    } else {
        1 // fallback
    }
}

/// Scan forward from `start` for the end of a GUID string.
/// Returns `Some(end_pos)` if a GUID-like pattern (length 32-36 + dashes) is found,
/// or `None` if the character at `start` doesn't look like a GUID start.
fn scan_guid(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if start >= s.len() || !bytes[start].is_ascii_hexdigit() {
        return None;
    }
    let mut end = start;
    let mut dash_count = 0;
    let mut hex_count = 0;
    while end < s.len() {
        let c = bytes[end];
        if c.is_ascii_hexdigit() {
            hex_count += 1;
            end += 1;
        } else if c == b'-' {
            dash_count += 1;
            end += 1;
        } else {
            break;
        }
    }
    if hex_count == 32 && dash_count == 4 {
        Some(end)
    } else {
        None
    }
}

/// Substitute `%1`–`%99` placeholders and special codes in a message template.
///
/// Rules (applied in order within a single left-to-right scan):
/// * `%%` → literal `%`
/// * `%n` → `\r\n`
/// * `%r` → `\r`
/// * `%t` → `\t`
/// * `%1` – `%99` → `params[N-1]` (empty string if index out of bounds)
/// * Lone `%` not followed by a recognised code → emitted as-is
pub fn substitute(template: &str, params: &[&str]) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '%' {
            result.push(c);
            continue;
        }

        match chars.next() {
            Some('%') => result.push('%'),
            Some('n') => result.push_str("\r\n"),
            Some('r') => result.push('\r'),
            Some('t') => result.push('\t'),
            Some(d) if d.is_ascii_digit() => {
                let mut num_str = String::with_capacity(2);
                num_str.push(d);
                // Try to read a second digit for two-digit placeholders (e.g. %10)
                if let Some(&d2) = chars.peek() {
                    if d2.is_ascii_digit() {
                        num_str.push(d2);
                        chars.next();
                    }
                }
                let index: usize = num_str.parse().unwrap_or(0);
                if index > 0 && index <= params.len() {
                    result.push_str(params[index - 1]);
                }
                // index == 0 or out of bounds → emit nothing (empty substitution)
            }
            Some(other) => {
                // Unrecognised escape — emit both the `%` and the following char
                result.push('%');
                result.push(other);
            }
            None => {
                // Trailing `%` at end of string
                result.push('%');
            }
        }
    }

    result
}

/// Simulate Windows FormatMessageW two-pass substitution behavior.
///
/// Windows `EvtFormatMessage` performs two `FormatMessageW` passes:
/// 1. Substitute `%1`–`%99` in the message template with parameter values.
/// 2. Substitute `%1`–`%99` in the *result of pass 1* with the same values.
///
/// Pass 2 catches `%N` patterns that were introduced by substituted parameter
/// values. For example, a URL parameter containing URL-encoded braces
/// (`%7B`/`%7D`) will have `%7` interpreted as parameter 7 on the second pass:
/// `%7B` → `%7` (param 7) + `B` (literal), which matches how Windows Event
/// Viewer / Get-WinEvent renders these events.
///
/// Between the two passes, [`format_for_display`] is called to normalize
/// GUIDs and hex values. This prevents pattern corruption where the second
/// pass's substitution merges with adjacent GUID hex digits.
///
/// This function performs the two passes using the same `params` slice,
/// without calling any Windows API — it is a pure Rust simulation.
pub fn substitute_format_message(template: &str, params: &[&str]) -> String {
    let pass1 = substitute(template, params);
    // Normalize GUIDs and hex BEFORE the second pass, so that URL-encoded
    // GUIDs (e.g. %7B430FD4D0-...%7D) are recognized and braced before
    // being split apart by the second pass's %N substitutions.
    let normalized = format_for_display(&pass1);
    substitute(&normalized, params)
}

// ---------------------------------------------------------------------------
// %%N parameter reference expansion
// ---------------------------------------------------------------------------

/// Expand `%%N` parameter references in a message template.
///
/// Scans for `%%` followed by one or more ASCII digits. The digits are parsed
/// as a `u32` parameter ID and looked up in `refs`. If found, the entire `%%N`
/// sequence is replaced with the looked-up message text. If not found, the
/// original `%%N` text is preserved as-is.
///
/// This function is designed to run **before** [`substitute`], so that
/// parameter-reference expansion and positional-parameter substitution can be
/// handled in separate passes.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use evtx_tools_core::substitute::expand_param_refs;
///
/// let mut refs = HashMap::new();
/// refs.insert(14592, "SUCCESS".to_string());
/// assert_eq!(expand_param_refs("%%14592", &refs), "SUCCESS");
/// ```
pub fn expand_param_refs(message: &str, refs: &HashMap<u32, String>) -> String {
    let mut result = String::with_capacity(message.len());
    let mut chars = message.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '%' {
            result.push(c);
            continue;
        }

        match chars.next() {
            Some('%') => {
                // Got '%%', now look for digits
                let mut num_str = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        num_str.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }

                if num_str.is_empty() {
                    // '%%' not followed by digits — emit both percents
                    // for `substitute()` to interpret (%% → literal %)
                    result.push('%');
                    result.push('%');
                } else {
                    let param_id: u32 = match num_str.parse() {
                        Ok(id) => id,
                        Err(_) => {
                            // Shouldn't happen since we only collected digits
                            result.push('%');
                            result.push('%');
                            result.push_str(&num_str);
                            continue;
                        }
                    };

                    match refs.get(&param_id) {
                        Some(replacement) => result.push_str(replacement),
                        None => {
                            // Not found — keep as-is for later processing
                            result.push('%');
                            result.push('%');
                            result.push_str(&num_str);
                        }
                    }
                }
            }
            Some(other) => {
                // Single '%' followed by non-'%' — pass through for substitute()
                result.push('%');
                result.push(other);
            }
            None => {
                // Trailing '%' at end of string
                result.push('%');
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Enumeration value resolution — Layer 1: Standard type conventions
// ---------------------------------------------------------------------------

/// Resolve a raw enumeration value to its display string using standard
/// Windows type conventions.
///
/// Currently handles `win:Boolean` heuristics — field names that suggest
/// boolean semantics (is\*, has\*, \*Enabled, etc.) map `0`/`false` to
/// `"Disabled"` and `1`/`true` to `"Enabled"`.
///
/// Returns `None` when the field name does not match boolean heuristics
/// or the value is not a recognised boolean literal (conservative fall-through).
pub fn resolve_enum(value: &str, field_name: &str) -> Option<String> {
    if !is_boolean_field(field_name) {
        return None;
    }
    match value {
        "0" | "false" => Some("Disabled".to_string()),
        "1" | "true" => Some("Enabled".to_string()),
        _ => None,
    }
}

/// Apply enum resolution to a list of substitution parameters using the
/// parameter name map from the message DB.
///
/// Each parameter's field name is looked up from `param_map` (which maps
/// positional indices like `"1"`, `"2"` to field names like `"isRoaming"`).
/// If the field name matches a boolean heuristic, the value is resolved
/// from raw `0`/`1` to `"Disabled"`/`"Enabled"`.
///
/// If `param_map` is `None` or empty, the params are returned unchanged.
pub fn apply_param_enums(
    params: &[String],
    param_map: &Option<HashMap<String, String>>,
) -> Vec<String> {
    let Some(map) = param_map else {
        return params.to_vec();
    };
    if map.is_empty() {
        return params.to_vec();
    }

    params
        .iter()
        .enumerate()
        .map(|(i, value)| {
            // param_map keys are 1-indexed strings ("1", "2", ...)
            let idx_str = (i + 1).to_string();
            match map.get(&idx_str) {
                Some(field_name) => {
                    resolve_enum(value, field_name).unwrap_or_else(|| value.clone())
                }
                None => value.clone(),
            }
        })
        .collect()
}

/// Apply CRIM value-map resolution to substitution parameters.
///
/// For each parameter whose field name matches a value/map name in `value_maps`,
/// the raw value is replaced with the display name from the map.
/// For example, if parameter 2 is named "LogonType" and value_maps contains
/// `{"LogonType": {"2" → "Interactive", "10" → "RemoteInteractive"}}`,
/// then value "2" becomes "Interactive".
///
/// For bitmap maps (names listed in `bitmap_names`), the value is treated as a
/// bitmask and decomposed into individual set bit positions, with each matched
/// name joined by ", " (e.g. value "0x3" + map {"0": "Read", "1": "Write"} →
/// "Read, Write").
///
/// If `param_map` is `None` or empty, the params are returned unchanged.
/// If `value_maps` is empty, the params are returned unchanged.
pub fn apply_value_maps(
    params: &[String],
    param_map: &Option<HashMap<String, String>>,
    value_maps: &HashMap<String, HashMap<String, String>>,
    bitmap_names: &HashSet<String>,
) -> Vec<String> {
    let Some(map) = param_map else {
        return params.to_vec();
    };
    if map.is_empty() || value_maps.is_empty() {
        return params.to_vec();
    }

    params
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let idx_str = (i + 1).to_string();
            match map.get(&idx_str) {
                Some(field_name) => match value_maps.get(field_name) {
                    Some(entries) => {
                        if bitmap_names.contains(field_name) {
                            // Bitmap: decompose value into set bits
                            match_bitmap(value, entries)
                                .unwrap_or_else(|| value.clone())
                        } else {
                            // Regular VMAP: exact match
                            entries.get(value)
                                .cloned()
                                .unwrap_or_else(|| value.clone())
                        }
                    }
                    None => value.clone(),
                },
                None => value.clone(),
            }
        })
        .collect()
}

/// Decompose a bitmask value into human-readable names using a bitmap map.
///
/// Parses `value` (e.g. `"3"`, `"0x3"`, `"0x8020000000000000"`) as a u64,
/// iterates all set bit positions, and looks up each position in `entries`.
/// Matched display names are joined with ", " sorted by bit position.
///
/// Returns `None` if the value can't be parsed as u64, value is zero, or
/// no set bit matches any entry.
pub fn match_bitmap(
    value: &str,
    entries: &std::collections::HashMap<String, String>,
) -> Option<String> {
    // Parse value as u64 (support decimal and 0x-prefixed hex)
    let val = if let Some(hex) = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()?
    } else {
        value.parse::<u64>().ok()?
    };

    if val == 0 {
        return None;
    }

    // Collect matched bit names with their positions for sorted output
    let mut matched: Vec<(u64, &str)> = Vec::new();
    for (bit_str, name) in entries {
        if let Ok(bit_pos) = bit_str.parse::<u64>() {
            if bit_pos < 64 && (val & (1u64 << bit_pos)) != 0 {
                matched.push((bit_pos, name.as_str()));
            }
        }
    }

    if matched.is_empty() {
        return None;
    }

    matched.sort_by_key(|(pos, _)| *pos);
    Some(matched.into_iter().map(|(_, name)| name).collect::<Vec<_>>().join(", "))
}

/// Check whether `name` looks like a boolean-typed field.
///
/// Conservative detection to avoid false positives for numeric event IDs,
/// counters, or process IDs. Matches Hungarian-style verb prefixes and
/// common Windows Event Log boolean suffix conventions.
fn is_boolean_field(name: &str) -> bool {
    let lower = name.to_lowercase();

    // Prefix patterns used in Windows event parameter names:
    //   isRoaming, isEnabled, hasAccess, canDelete, shouldLog, willExpire
    if lower.starts_with("is")
        || lower.starts_with("has")
        || lower.starts_with("can")
        || lower.starts_with("should")
        || lower.starts_with("will")
    {
        return true;
    }

    // Suffix patterns for common boolean field name endings:
    //   RoamingEnabled, AuditDisabled, ConnectionActive, ErrorFlag, ActiveState, WinBool
    if lower.ends_with("enabled")
        || lower.ends_with("disabled")
        || lower.ends_with("active")
        || lower.ends_with("flag")
        || lower.ends_with("state")
        || lower.ends_with("bool")
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // substitute_format_message tests
    // -----------------------------------------------------------------------

    #[test]
    fn fmtmsg_basic_two_pass() {
        // Param value containing %7B should be caught by second pass
        let params = ["file_val_7"];
        let result = substitute_format_message("test %1", &params);
        assert_eq!(result, "test file_val_7");
    }

    #[test]
    fn fmtmsg_url_percent_braces() {
        // Simulate BITS event 61: %4=URL with %7B...%7D, %7=fileTime
        // format_for_display between passes: just lowercases GUID, no braces added
        // (because GUID is between %7B/%7D, it's URL-encoded, not bare).
        // Pass 2: %7B → fileTime + B, %7D → fileTime + D
        // Result matches PowerShell: filetimeZBguid...DfiletimeZD
        let params = &[
            "{guid-1}",           // %1
            "jobname",            // %2
            "{guid-2}",           // %3
            "https://test?appid=%7B430FD4D0-B729-4F61-AA34-91526481799D%7D",  // %4 = URL
            "",                   // %5
            "0x80072EE7",         // %6
            "1601-01-01T00:00:00.0000000Z",  // %7 = fileTime
        ];
        let result = substitute_format_message("Job %4 status %6", params);
        // %7B → fileTime + B (literal)
        // %7D → fileTime + D (literal)
        assert!(result.contains("1601-01-01T00:00:00.0000000Z"), "should contain fileTime from %7");
        assert!(result.contains("B430FD4D0"), "should have B literal from %%7B before GUID");
        assert!(result.contains("1601-01-01T00:00:00.0000000ZD"), "should have D literal from %%7D");
        assert!(!result.contains("%7B"), "%%7B should be resolved");
        assert!(!result.contains("%7D"), "%%7D should be resolved");
        // GUID hex should not be corrupted by hex conversion (no 0x prefix)
        assert!(!result.contains("0x154F674787"), "GUID numeric segment should not be hex-converted");
    }

    #[test]
    fn fmtmsg_url_no_double_braces_after_format_display() {
        // format_for_display should not add extra braces when GUID is between %7B/%7D.
        // Result: %7B430FD4D0-...%7D (uppercase, no extra braces, URL encoding preserved)
        let msg = "appid=%7B430FD4D0-B729-4F61-AA34-91526481799D%7D";
        let formatted = format_for_display(msg);
        assert!(!formatted.contains("{"), "should NOT add braces when between %%7B/%%7D");
        assert!(formatted.contains("%7B430FD4D0"), "should uppercase GUID, keep %%7B");
        assert!(formatted.contains("91526481799D%7D"), "should preserve GUID + %%7D");
    }

    #[test]
    fn fmtmsg_no_change_when_no_extra_patterns() {
        let params = &["admin", "PC-1"];
        let result = substitute_format_message("User %1 from %2", params);
        assert_eq!(result, "User admin from PC-1");
    }

    #[test]
    fn fmtmsg_empty_params() {
        let result = substitute_format_message("no params here", &[] as &[&str]);
        assert_eq!(result, "no params here");
    }

    #[test]
    fn fmtmsg_percent_pattern_in_template_not_in_params() {
        let params = &["a", "b"];
        let result = substitute_format_message("test %1 %4", params);
        assert_eq!(result, "test a ");
    }

    #[test]
    fn fmtmsg_percent_literal_escaped() {
        let params = &["hello"];
        let result = substitute_format_message("50%% %1", params);
        assert_eq!(result, "50% hello");
    }

    #[test]
    fn fmtmsg_handles_special_codes() {
        let params = &["hello", "world%1"];
        let result = substitute_format_message("%1%n%2", params);
        // format_for_display between passes strips \r, leaving \n
        assert!(!result.contains("\r\n"), "CRLF should be normalized to LF between passes");
        assert!(result.contains("\n"), "should have LF");
        assert!(!result.contains("%1"), "%1 in param value should be resolved on pass 2");
    }

    #[test]
    fn substitute_basic() {
        assert_eq!(substitute("%1", &["hello"]), "hello");
    }

    #[test]
    fn substitute_multiple() {
        assert_eq!(substitute("%1 %2", &["a", "b"]), "a b");
    }

    #[test]
    fn substitute_special() {
        // Template has spaces between codes: %n[space]%t[space]%%
        assert_eq!(substitute("%n %t %%", &[]), "\r\n \t %");
    }

    #[test]
    fn substitute_missing() {
        assert_eq!(substitute("%99", &["a", "b"]), "");
    }

    #[test]
    fn substitute_two_digit() {
        assert_eq!(
            substitute("%10", &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]),
            "j"
        );
    }

    #[test]
    fn substitute_no_placeholders() {
        assert_eq!(substitute("plain text", &[]), "plain text");
    }

    #[test]
    fn substitute_mixed() {
        assert_eq!(
            substitute(
                "User %1 logged on from %2%n%t%%",
                &["admin", "192.168.1.1"]
            ),
            "User admin logged on from 192.168.1.1\r\n\t%"
        );
    }

    #[test]
    fn substitute_lone_percent() {
        assert_eq!(substitute("50%", &[]), "50%");
    }

    #[test]
    fn substitute_percent_at_end() {
        assert_eq!(substitute("end%", &[]), "end%");
    }

    #[test]
    fn substitute_unrecognized_escape() {
        assert_eq!(substitute("%z", &[]), "%z");
    }

    #[test]
    fn substitute_empty_template() {
        assert_eq!(substitute("", &["x"]), "");
    }

    // -----------------------------------------------------------------------
    // expand_param_refs tests
    // -----------------------------------------------------------------------

    #[test]
    fn expand_param_refs_found() {
        let mut refs = HashMap::new();
        refs.insert(14592, "SUCCESS".to_string());
        assert_eq!(expand_param_refs("%%14592", &refs), "SUCCESS");
    }

    #[test]
    fn expand_param_refs_not_found() {
        let refs = HashMap::new();
        assert_eq!(
            expand_param_refs("%%99999", &refs),
            "%%99999"
        );
    }

    #[test]
    fn expand_param_refs_no_refs_in_text() {
        let refs = HashMap::new();
        assert_eq!(
            expand_param_refs("plain text", &refs),
            "plain text"
        );
    }

    #[test]
    fn expand_param_refs_multiple() {
        let mut refs = HashMap::new();
        refs.insert(14592, "SUCCESS".to_string());
        refs.insert(14593, "FAILURE".to_string());
        assert_eq!(
            expand_param_refs(
                "Status: %%14592, Prev: %%14593",
                &refs,
            ),
            "Status: SUCCESS, Prev: FAILURE"
        );
    }

    #[test]
    fn expand_param_refs_mixed_with_substitution() {
        // %%N should be resolved; %1 should be left for substitute()
        let mut refs = HashMap::new();
        refs.insert(14592, "SUCCESS".to_string());
        assert_eq!(
            expand_param_refs("Operation %1: %%14592", &refs),
            "Operation %1: SUCCESS"
        );
    }

    #[test]
    fn expand_param_refs_literal_percent_pass_through() {
        // %% not followed by digits should pass through for substitute()
        let refs = HashMap::new();
        assert_eq!(
            expand_param_refs("50%% complete", &refs),
            "50%% complete"
        );
    }

    #[test]
    fn expand_param_refs_digits_then_text() {
        let mut refs = HashMap::new();
        refs.insert(14592, "SUCCESS".to_string());
        assert_eq!(
            expand_param_refs("%%14592 suffix", &refs),
            "SUCCESS suffix"
        );
    }

    #[test]
    fn expand_param_refs_large_param_id() {
        let mut refs = HashMap::new();
        refs.insert(4294967295, "MAX".to_string());
        assert_eq!(
            expand_param_refs("%%4294967295", &refs),
            "MAX"
        );
    }

    #[test]
    fn expand_param_refs_trailing_lone_percent() {
        let refs = HashMap::new();
        assert_eq!(
            expand_param_refs("end%", &refs),
            "end%"
        );
    }

    // -----------------------------------------------------------------------
    // resolve_enum tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_enum_is_field_true() {
        assert_eq!(resolve_enum("1", "isRoaming"), Some("Enabled".to_string()));
    }

    #[test]
    fn resolve_enum_is_field_false() {
        assert_eq!(
            resolve_enum("0", "isEnabled"),
            Some("Disabled".to_string())
        );
    }

    #[test]
    fn resolve_enum_non_matching_field() {
        assert_eq!(resolve_enum("1", "ProcessId"), None);
        assert_eq!(resolve_enum("0", "Path"), None);
    }

    #[test]
    fn resolve_enum_non_boolean_value() {
        assert_eq!(resolve_enum("42", "isActive"), None);
        assert_eq!(resolve_enum("-1", "isValid"), None);
    }

    #[test]
    fn resolve_enum_has_prefix() {
        assert_eq!(
            resolve_enum("1", "hasAccess"),
            Some("Enabled".to_string())
        );
        assert_eq!(
            resolve_enum("0", "hasAccess"),
            Some("Disabled".to_string())
        );
    }

    #[test]
    fn resolve_enum_boolean_suffix() {
        assert_eq!(
            resolve_enum("1", "RoamingEnabled"),
            Some("Enabled".to_string())
        );
        assert_eq!(
            resolve_enum("0", "AuditFlag"),
            Some("Disabled".to_string())
        );
        assert_eq!(
            resolve_enum("1", "ActiveState"),
            Some("Enabled".to_string())
        );
        assert_eq!(
            resolve_enum("0", "winBool"),
            Some("Disabled".to_string())
        );
    }

    #[test]
    fn resolve_enum_string_true_false() {
        assert_eq!(
            resolve_enum("true", "isActive"),
            Some("Enabled".to_string())
        );
        assert_eq!(
            resolve_enum("false", "isActive"),
            Some("Disabled".to_string())
        );
    }

    #[test]
    fn resolve_enum_case_insensitive() {
        assert_eq!(
            resolve_enum("1", "ISROAMING"),
            Some("Enabled".to_string())
        );
        assert_eq!(
            resolve_enum("0", "HasAccess"),
            Some("Disabled".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // apply_param_enums tests
    // -----------------------------------------------------------------------

    #[test]
    fn apply_param_enums_with_map() {
        let param_map = Some(HashMap::from([
            ("1".into(), "ProcessId".into()),
            ("2".into(), "isRoaming".into()),
            ("3".into(), "Status".into()),
        ]));
        let params = vec!["1234".into(), "1".into(), "0".into()];
        let result = apply_param_enums(&params, &param_map);
        assert_eq!(result.len(), 3);
        // ProcessId — not boolean, unchanged
        assert_eq!(result[0], "1234");
        // isRoaming -> "1" -> "Enabled"
        assert_eq!(result[1], "Enabled");
        // Status — not boolean, unchanged
        assert_eq!(result[2], "0");
    }

    #[test]
    fn apply_param_enums_no_map() {
        let params = vec!["1".into(), "0".into()];
        let result = apply_param_enums(&params, &None);
        assert_eq!(result, vec!["1", "0"]);
    }

    #[test]
    fn apply_param_enums_empty_map() {
        let param_map = Some(HashMap::new());
        let params = vec!["hello".into()];
        let result = apply_param_enums(&params, &param_map);
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn apply_param_enums_mixed_resolution() {
        // Test that all matching boolean fields are resolved,
        // mismatched fields keep their raw values.
        let param_map = Some(HashMap::from([
            ("1".into(), "isRoaming".into()),
            ("2".into(), "hasAccess".into()),
            ("3".into(), "EnabledFlag".into()),
            ("4".into(), "TcpCount".into()),
        ]));
        let params = vec![
            "0".into(),
            "1".into(),
            "0".into(),
            "5".into(),
        ];
        let result = apply_param_enums(&params, &param_map);
        assert_eq!(result[0], "Disabled"); // isRoaming -> "0" -> "Disabled"
        assert_eq!(result[1], "Enabled");  // hasAccess -> "1" -> "Enabled"
        assert_eq!(result[2], "Disabled"); // EnabledFlag -> "0" -> "Disabled"
        assert_eq!(result[3], "5");        // TcpCount — not boolean, unchanged
    }

    // -----------------------------------------------------------------------
    // apply_value_maps tests
    // -----------------------------------------------------------------------

    #[test]
    fn apply_value_maps_with_map() {
        let param_map = Some(HashMap::from([
            ("1".into(), "ProcessId".into()),
            ("2".into(), "LogonType".into()),
            ("3".into(), "Status".into()),
        ]));
        let mut value_maps: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut logon_map = HashMap::new();
        logon_map.insert("2".to_string(), "Interactive".to_string());
        logon_map.insert("10".to_string(), "RemoteInteractive".to_string());
        value_maps.insert("LogonType".to_string(), logon_map);

        let bitmap_names = HashSet::new();
        let params = vec!["1234".into(), "2".into(), "0".into()];
        let result = apply_value_maps(&params, &param_map, &value_maps, &bitmap_names);
        assert_eq!(result.len(), 3);
        // ProcessId — not in value map, unchanged
        assert_eq!(result[0], "1234");
        // LogonType -> "2" -> "Interactive"
        assert_eq!(result[1], "Interactive");
        // Status — not in value map, unchanged
        assert_eq!(result[2], "0");
    }

    #[test]
    fn apply_value_maps_no_map() {
        let params = vec!["2".into(), "10".into()];
        let value_maps = HashMap::new();
        let bitmap_names = HashSet::new();
        let result = apply_value_maps(&params, &None, &value_maps, &bitmap_names);
        assert_eq!(result, vec!["2", "10"]);
    }

    #[test]
    fn apply_value_maps_empty_maps() {
        let param_map = Some(HashMap::from([
            ("1".into(), "LogonType".into()),
        ]));
        let value_maps: HashMap<String, HashMap<String, String>> = HashMap::new();
        let bitmap_names = HashSet::new();
        let params = vec!["2".into()];
        let result = apply_value_maps(&params, &param_map, &value_maps, &bitmap_names);
        assert_eq!(result, vec!["2"]);
    }

    #[test]
    fn apply_value_maps_no_match_keeps_original() {
        let param_map = Some(HashMap::from([
            ("1".into(), "LogonType".into()),
        ]));
        let mut value_maps: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut logon_map = HashMap::new();
        logon_map.insert("2".to_string(), "Interactive".to_string());
        value_maps.insert("LogonType".to_string(), logon_map);

        let bitmap_names = HashSet::new();

        // Value "99" is not in the map — keep original
        let params = vec!["99".into()];
        let result = apply_value_maps(&params, &param_map, &value_maps, &bitmap_names);
        assert_eq!(result, vec!["99"]);
    }

    #[test]
    fn apply_value_maps_multiple_params_with_different_maps() {
        let param_map = Some(HashMap::from([
            ("1".into(), "LogonType".into()),
            ("2".into(), "LogonType".into()),
            ("3".into(), "OtherField".into()),
        ]));
        let mut value_maps: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut logon_map = HashMap::new();
        logon_map.insert("2".to_string(), "Interactive".to_string());
        logon_map.insert("10".to_string(), "RemoteInteractive".to_string());
        value_maps.insert("LogonType".to_string(), logon_map);

        let bitmap_names = HashSet::new();

        let params = vec!["2".into(), "10".into(), "hello".into()];
        let result = apply_value_maps(&params, &param_map, &value_maps, &bitmap_names);
        assert_eq!(result, vec!["Interactive", "RemoteInteractive", "hello"]);
    }

    // -----------------------------------------------------------------------
    // format_for_display tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_strips_carriage_return() {
        assert_eq!(format_for_display("hello\r\nworld"), "hello\nworld");
    }

    #[test]
    fn format_guid_uppercase_no_braces_to_uppercase_braces() {
        let input = "GUID BE8ABC0D-3E7F-4DF4-84A5-B0F41A6857A1 here";
        let expected = "GUID {BE8ABC0D-3E7F-4DF4-84A5-B0F41A6857A1} here";
        assert_eq!(format_for_display(input), expected);
    }

    #[test]
    fn format_guid_with_braces_uppercase_to_uppercase() {
        let input = "{BE8ABC0D-3E7F-4DF4-84A5-B0F41A6857A1}";
        let expected = "{BE8ABC0D-3E7F-4DF4-84A5-B0F41A6857A1}";
        assert_eq!(format_for_display(input), expected);
    }

    #[test]
    fn format_guid_multiple() {
        let input = "{AABBCCDD-EEFF-0011-2233-445566778899} and {FFEEDDCC-BBAA-1100-3322-554433221100}";
        let expected = "{AABBCCDD-EEFF-0011-2233-445566778899} and {FFEEDDCC-BBAA-1100-3322-554433221100}";
        assert_eq!(format_for_display(input), expected);
    }

    #[test]
    fn format_hex_conversion() {
        assert_eq!(format_for_display("code 2147954407"), "code 0x80072EE7");
    }

    #[test]
    fn format_small_integer_unchanged() {
        assert_eq!(format_for_display("value 100"), "value 100");
    }

    #[test]
    fn format_mixed_content() {
        let input = "Job cancelled.\r\nID: BE8ABC0D-3E7F-4DF4-84A5-B0F41A6857A1\r\nStatus: 2147954407";
        let expected = "Job cancelled.\nID: {BE8ABC0D-3E7F-4DF4-84A5-B0F41A6857A1}\nStatus: 0x80072EE7";
        assert_eq!(format_for_display(input), expected);
    }

    #[test]
    fn format_empty_string() {
        assert_eq!(format_for_display(""), "");
    }

    #[test]
    fn format_no_changes_needed() {
        assert_eq!(format_for_display("plain text"), "plain text");
    }

    #[test]
    fn format_no_false_positive_hex() {
        assert_eq!(format_for_display("count 100 items"), "count 100 items");
    }

    #[test]
    fn format_hex_prefix_keeps_lowercase_x() {
        assert_eq!(format_for_display("Logon ID: 0x3e7"), "Logon ID: 0x3E7");
    }

    #[test]
    fn format_sid_preserved() {
        let input = "Security ID: S-1-5-21-3461203602-4096304019-2269080069-1000";
        assert_eq!(format_for_display(input), input);
    }

    #[test]
    fn format_chinese_text_no_panic() {
        // Chinese multi-byte UTF-8 must not panic
        let _ = format_for_display("已成功登录帐户。");
        let _ = format_for_display("特权被分配到新登录。");
        let _ = format_for_display("事件 4624 (Security) 已记录");
        // Also test the garbled text that caused the original panic
        let _ = format_for_display("资源升级转移成功。");
        let _ = format_for_display("防病毒保护服务重新启动。");
    }

    #[test]
    fn format_chinese_text_preserved() {
        let input = "安全事件";
        assert_eq!(format_for_display(input), input);
        let input2 = "特权被分配到新登录";
        assert_eq!(format_for_display(input2), input2);
    }

    // -----------------------------------------------------------------------
    // match_bitmap tests
    // -----------------------------------------------------------------------

    #[test]
    fn match_bitmap_single_bit() {
        let mut entries = std::collections::HashMap::new();
        entries.insert("0".to_string(), "Read".to_string());
        entries.insert("1".to_string(), "Write".to_string());
        // value 1 = bit 0 set → "Read"
        let result = crate::substitute::match_bitmap("1", &entries);
        assert_eq!(result, Some("Read".to_string()));
    }

    #[test]
    fn match_bitmap_multiple_bits() {
        let mut entries = std::collections::HashMap::new();
        entries.insert("0".to_string(), "Read".to_string());
        entries.insert("1".to_string(), "Write".to_string());
        // value 3 = bits 0 and 1 set → "Read, Write" (sorted by bit position)
        let result = crate::substitute::match_bitmap("3", &entries);
        assert_eq!(result, Some("Read, Write".to_string()));
    }

    #[test]
    fn match_bitmap_no_match() {
        let mut entries = std::collections::HashMap::new();
        entries.insert("0".to_string(), "Read".to_string());
        // value 2 = bit 1 set, but only entry for bit 0 exists
        let result = crate::substitute::match_bitmap("2", &entries);
        assert_eq!(result, None);
    }

    #[test]
    fn match_bitmap_hex_input() {
        let mut entries = std::collections::HashMap::new();
        entries.insert("0".to_string(), "Read".to_string());
        entries.insert("1".to_string(), "Write".to_string());
        assert_eq!(crate::substitute::match_bitmap("0x3", &entries), Some("Read, Write".to_string()));
        assert_eq!(crate::substitute::match_bitmap("0X1", &entries), Some("Read".to_string()));
    }

    #[test]
    fn match_bitmap_zero_value() {
        let mut entries = std::collections::HashMap::new();
        entries.insert("0".to_string(), "Read".to_string());
        assert_eq!(crate::substitute::match_bitmap("0", &entries), None);
    }

    #[test]
    fn match_bitmap_large_mask() {
        let mut entries = std::collections::HashMap::new();
        entries.insert("63".to_string(), "HighestBit".to_string());
        // 0x8000000000000000 has bit 63 set
        assert_eq!(
            crate::substitute::match_bitmap("0x8000000000000000", &entries),
            Some("HighestBit".to_string())
        );
        // 0x1 has only bit 0 set
        let result = crate::substitute::match_bitmap("0x1", &entries);
        assert_eq!(result, None);
    }

    #[test]
    fn match_bitmap_invalid_value() {
        let mut entries = std::collections::HashMap::new();
        entries.insert("0".to_string(), "Read".to_string());
        assert_eq!(crate::substitute::match_bitmap("abc", &entries), None);
        assert_eq!(crate::substitute::match_bitmap("-1", &entries), None);
        assert_eq!(crate::substitute::match_bitmap("", &entries), None);
    }

    // -----------------------------------------------------------------------
    // apply_value_maps with bitmap_names tests
    // -----------------------------------------------------------------------

    #[test]
    fn apply_value_maps_bitmap_resolution() {
        let param_map = Some(std::collections::HashMap::from([
            ("1".into(), "LogonType".into()),
            ("2".into(), "AccessMask".into()),
        ]));
        let mut vmaps: std::collections::HashMap<String, std::collections::HashMap<String, String>> =
            std::collections::HashMap::new();
        let mut logon_map = std::collections::HashMap::new();
        logon_map.insert("2".to_string(), "Interactive".to_string());
        vmaps.insert("LogonType".to_string(), logon_map);
        let mut access_map = std::collections::HashMap::new();
        access_map.insert("0".to_string(), "Read".to_string());
        access_map.insert("1".to_string(), "Write".to_string());
        vmaps.insert("AccessMask".to_string(), access_map);

        let bitmap_names = std::collections::HashSet::from(["AccessMask".to_string()]);

        let params = vec!["2".into(), "0x3".into()];
        let result = crate::substitute::apply_value_maps(&params, &param_map, &vmaps, &bitmap_names);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "Interactive");  // VMAP: exact match
        assert_eq!(result[1], "Read, Write");  // BMAP: bit decomposition
    }
}

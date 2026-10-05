//! Event routing engine — matches events against rule profiles.
//!
//! Routes parsed events to scenario tables based on rule filters:
//!
//! 1. `event_ids` — fast integer match on `ParsedEvent.event_id`
//! 2. `channels` — exact string match on `ParsedEvent.channel`
//! 3. `provider_like` — SQL LIKE pattern on `ParsedEvent.provider_name`
//! 4. `message_like` — SQL LIKE pattern on `ParsedEvent.message` (after substitution)
//!
//! All specified filter fields must match (AND). Within a field, any value matches (OR).

use crate::ParsedEvent;

// ---------------------------------------------------------------------------
// Regex wrapper — compiled at rule-load time
// ---------------------------------------------------------------------------

/// A pre-compiled regular expression stored inside a rule filter.
///
/// Compiled once during YAML deserialization so that every `.is_match()`
/// call on each event is fast (no re-compilation).
#[derive(Debug, Clone)]
pub struct CompiledRegex(pub regex::Regex);

impl<'de> serde::Deserialize<'de> for CompiledRegex {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        regex::Regex::new(&s)
            .map(CompiledRegex)
            .map_err(|e| serde::de::Error::custom(format!("invalid regex pattern: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Rule definition (YAML-deserializable)
// ---------------------------------------------------------------------------

/// A single scenario rule that defines which events go to which table.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Rule {
    /// Display name (from YAML `name` field).
    pub name: String,
    /// Target table name (from YAML `table` field).
    pub table: String,
    /// Optional column whitelist (from YAML `columns` field).
    pub columns: Option<Vec<String>>,
    /// Filter conditions — flattened from YAML so all filter keys are direct.
    #[serde(flatten)]
    pub filter: RuleFilter,
}

/// Filter conditions for a rule.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuleFilter {
    /// Match if `event_id` is in this list.
    pub event_ids: Option<Vec<u32>>,
    /// Match if `channel` equals one of these values.
    pub channels: Option<Vec<String>>,
    /// Match if `level` is in this list of numeric values.
    pub levels: Option<Vec<u8>>,
    /// Match if `provider_name` matches at least one SQL LIKE pattern (YAML: `provider_name`).
    /// Legacy alias `provider_like` still accepted for backward compatibility.
    #[serde(alias = "provider_like", rename = "provider_name")]
    pub provider_like: Option<Vec<String>>,
    /// Match if `message` matches at least one SQL LIKE pattern (YAML: `message`).
    /// Legacy alias `message_like` still accepted for backward compatibility.
    #[serde(alias = "message_like", rename = "message")]
    pub message_like: Option<Vec<String>>,
    /// Match if `provider_name` matches at least one regex (YAML: `provider_name|re`).
    #[serde(alias = "provider_name|re")]
    pub provider_name_re: Option<Vec<CompiledRegex>>,
    /// Match if `message` matches at least one regex (YAML: `message|re`).
    #[serde(alias = "message|re")]
    pub message_re: Option<Vec<CompiledRegex>>,
}

impl RuleFilter {
    /// Check whether a parsed event matches this filter.
    ///
    /// All specified fields must match (AND). Fields that are `None` are not
    /// checked — they accept any value.
    pub fn matches(&self, event: &ParsedEvent) -> bool {
        if let Some(ref ids) = self.event_ids {
            match event.event_id {
                Some(eid) if ids.contains(&eid) => {}
                _ => return false,
            }
        }
        if let Some(ref channels) = self.channels {
            match event.channel.as_ref() {
                Some(ch) if channels.iter().any(|c| c == ch) => {}
                _ => return false,
            }
        }
        if let Some(ref ids) = self.levels {
            match event.level {
                Some(lvl) if ids.contains(&lvl) => {}
                _ => return false,
            }
        }
        if let Some(ref patterns) = self.provider_like {
            match event.provider_name.as_ref() {
                Some(prov) if patterns_match(prov, patterns) => {}
                _ => return false,
            }
        }
        if let Some(ref patterns) = self.message_like {
            match event.message.as_ref() {
                Some(msg) if patterns_match(msg, patterns) => {}
                _ => return false,
            }
        }
        if let Some(ref patterns) = self.provider_name_re {
            match event.provider_name.as_ref() {
                Some(prov) if patterns.iter().any(|p| p.0.is_match(prov)) => {}
                _ => return false,
            }
        }
        if let Some(ref patterns) = self.message_re {
            match event.message.as_ref() {
                Some(msg) if patterns.iter().any(|p| p.0.is_match(msg)) => {}
                _ => return false,
            }
        }
        true
    }
}

/// Check if `value` matches at least one SQL LIKE pattern.
///
/// Supports `%` as a wildcard matching any sequence of characters
/// (including empty). No other LIKE syntax (`_`, `[charlist]`) is supported.
fn patterns_match(value: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pat| like_match(value, pat))
}

/// Simple SQL LIKE pattern matching.
///
/// - `%` matches any sequence of characters (including empty).
/// - All other characters match literally.
/// - Case-insensitive (ASCII only).
fn like_match(value: &str, pattern: &str) -> bool {
    let value = value.to_lowercase();
    let pattern = pattern.to_lowercase();

    let mut vi = 0usize;
    let mut pi = 0usize;
    let vbytes = value.as_bytes();
    let pbytes = pattern.as_bytes();

    let mut star_vi: Option<usize> = None;
    let mut star_pi: Option<usize> = None;

    while vi < vbytes.len() {
        if pi < pbytes.len() && (pbytes[pi] == b'%') {
            // % matches zero or more characters
            star_vi = Some(vi);
            star_pi = Some(pi);
            pi += 1;
        } else if pi < pbytes.len() && pbytes[pi] == vbytes[vi] {
            vi += 1;
            pi += 1;
        } else if let (Some(sv), Some(sp)) = (star_vi, star_pi) {
            // backtrack: let % consume one more character
            vi = sv + 1;
            pi = sp + 1;
            star_vi = Some(vi);
        } else {
            return false;
        }
    }

    // skip trailing % in pattern
    while pi < pbytes.len() && pbytes[pi] == b'%' {
        pi += 1;
    }

    pi == pbytes.len()
}

// ---------------------------------------------------------------------------
// Rule loading
// ---------------------------------------------------------------------------

/// Parse a single rule from YAML string.
pub fn rule_from_yaml(yaml: &str) -> std::result::Result<Rule, String> {
    let rule: Rule = serde_yaml::from_str(yaml).map_err(|e| format!("failed to parse rule: {e}"))?;
    if rule.columns.as_ref().map_or(true, |c| c.is_empty()) {
        return Err("rule must have a non-empty 'columns' field (e.g. columns: [event_id, message])".into());
    }
    Ok(rule)
}

/// Parse multiple YAML rules from (name, yaml_content) pairs.
pub fn rules_from_embedded<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<Rule> {
    pairs.into_iter().filter_map(|(name, yaml)| {
        match rule_from_yaml(yaml) {
            Ok(rule) => Some(rule),
            Err(e) => {
                eprintln!("[warn] failed to load built-in rule '{name}': {e}");
                None
            }
        }
    }).collect()
}

// ---------------------------------------------------------------------------
// SceneRouter — evaluate events against multiple rules
// ---------------------------------------------------------------------------

/// Routes parsed events to matching scenario tables.
///
/// Holds a list of `Rule` definitions. For each event, `route()` returns
/// all rules whose filter matches (zero, one, or many).
#[derive(Clone)]
pub struct SceneRouter {
    rules: Vec<Rule>,
}

impl SceneRouter {
    /// Create a new router with the given rules.
    pub fn new(rules: Vec<Rule>) -> Self {
        SceneRouter { rules }
    }

    /// Whether this router has any rules.
    pub fn has_rules(&self) -> bool {
        !self.rules.is_empty()
    }

    /// Access the loaded rules (for debugging).
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Evaluate an event against all rules.
    ///
    /// Returns references to every rule whose filter matches.
    pub fn route<'a>(&'a self, event: &ParsedEvent) -> Vec<&'a Rule> {
        self.rules.iter().filter(|r| r.filter.matches(event)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParsedEvent;

    fn make_event(event_id: Option<u32>, channel: Option<&str>,
                  provider: Option<&str>, message: Option<&str>) -> ParsedEvent {
        ParsedEvent {
            event_record_id: Some(1),
            event_id,
            channel: channel.map(|s| s.to_string()),
            provider_name: provider.map(|s| s.to_string()),
            message: message.map(|s| s.to_string()),
            // Everything else default
            event_time: None,
            provider_guid: None,
            qualifiers: None,
            version: None,
            level: None,
            severity: None,
            task_category: None,
            opcode: None,
            keywords: None,
            computer: None,
            process_id: None,
            thread_id: None,
            user_name: None,
            user_sid: None,
            activity_id: None,
            related_activity_id: None,
            level_name: None,
            keywords_name: None,
            event_data: None,
            raw_xml: None,
            sequence_number: None,
            logon_type: None,
            render_via: None,
            file_name: None,
        }
    }

    // ── Cycle 1: event_ids matching ──

    #[test]
    fn test_matches_event_id_in_list() {
        let filter = RuleFilter {
            event_ids: Some(vec![4624, 4625, 7045]),
            channels: None,
            provider_like: None,
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(Some(4624), None, None, None);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_rejects_event_id_not_in_list() {
        let filter = RuleFilter {
            event_ids: Some(vec![4624, 4625]),
            channels: None,
            provider_like: None,
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(Some(4688), None, None, None);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_no_event_ids_accepts_any() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(Some(9999), None, None, None);
        assert!(filter.matches(&event));
    }

    // ── Cycle 2: event_ids + channels matching ──

    #[test]
    fn test_matches_event_id_and_channel() {
        let filter = RuleFilter {
            event_ids: Some(vec![4624]),
            channels: Some(vec!["Security".into()]),
            provider_like: None,
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(Some(4624), Some("Security"), None, None);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_wrong_channel_rejects() {
        let filter = RuleFilter {
            event_ids: Some(vec![4624]),
            channels: Some(vec!["Security".into()]),
            provider_like: None,
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(Some(4624), Some("System"), None, None);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_multiple_channels_any_matches() {
        let filter = RuleFilter {
            event_ids: Some(vec![4624]),
            channels: Some(vec!["Security".into(), "System".into()]),
            provider_like: None,
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(Some(4624), Some("System"), None, None);
        assert!(filter.matches(&event));
    }

    // ── Cycle 3: provider_like matching ──

    #[test]
    fn test_matches_provider_prefix() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: Some(vec!["MSSQL%".into()]),
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(None, None, Some("MSSQLSERVER"), None);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_provider_like_case_insensitive() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: Some(vec!["asp.net%".into()]),
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(None, None, Some("ASP.NET 4.0"), None);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_provider_like_no_match() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: Some(vec!["MSSQL%".into()]),
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(None, None, Some("ASP.NET"), None);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_provider_like_multiple_patterns() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: Some(vec!["MSSQL%".into(), "ASP.NET%".into()]),
            message_like: None,
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event1 = make_event(None, None, Some("MSSQLSERVER"), None);
        let event2 = make_event(None, None, Some("ASP.NET 2.0"), None);
        assert!(filter.matches(&event1));
        assert!(filter.matches(&event2));
    }

    // ── Cycle 4: message_like matching ──

    #[test]
    fn test_matches_message_contains() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: Some(vec!["%登录%".into()]),
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(None, None, None, Some("用户已成功登录"));
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_message_like_no_match() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: Some(vec!["%AnyDesk%".into()]),
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(None, None, None, Some("普通信息事件"));
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_message_like_multiple_patterns() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: Some(vec!["%login%".into(), "%登录%".into()]),
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event_en = make_event(None, None, None, Some("User login successful"));
        let event_zh = make_event(None, None, None, Some("用户已成功登录"));
        assert!(filter.matches(&event_en));
        assert!(filter.matches(&event_zh));
    }

    // ── Cycle 5: combined multi-field matching ──

    #[test]
    fn test_combined_all_fields_match() {
        let filter = RuleFilter {
            event_ids: Some(vec![18456]),
            channels: Some(vec!["Application".into()]),
            provider_like: Some(vec!["MSSQL%".into()]),
            message_like: Some(vec!["%登录%".into()]),
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = make_event(
            Some(18456), Some("Application"), Some("MSSQLSERVER"),
            Some("登录失败 for user 'sa'"),
        );
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_combined_one_field_mismatch_rejects() {
        let filter = RuleFilter {
            event_ids: Some(vec![18456]),
            channels: Some(vec!["Application".into()]),
            provider_like: Some(vec!["MSSQL%".into()]),
            message_like: Some(vec!["%登录%".into()]),
            levels: None,
            provider_name_re: None,
            message_re: None,
        };
        // Wrong channel
        let event = make_event(
            Some(18456), Some("System"), Some("MSSQLSERVER"),
            Some("登录失败"),
        );
        assert!(!filter.matches(&event));
    }

    // ── Cycle 6: Rule deserialization ──

    #[test]
    fn test_deserialize_rule_yaml() {
        let yaml = r#"
name: "测试规则"
table: "test_events"
columns:
  - event_id
  - message
event_ids: [4624, 4625]
channels: ["Security"]
"#;
        let rule: Rule = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(rule.name, "测试规则");
        assert_eq!(rule.table, "test_events");
        assert_eq!(rule.columns, Some(vec!["event_id".into(), "message".into()]));
        assert_eq!(rule.filter.event_ids, Some(vec![4624, 4625]));
    }

    // ── SceneRouter: multi-rule routing ──

    fn make_security_rule() -> Rule {
        Rule {
            name: "安全审计".into(),
            table: "security_events".into(),
            columns: Some(vec!["event_id".into(), "message".into()]),
            filter: RuleFilter {
                event_ids: Some(vec![4624, 4625, 4648, 4672, 4720]),
                channels: Some(vec!["Security".into()]),
                provider_like: None,
                message_like: None,
                provider_name_re: None,
                message_re: None,
                levels: None,
            },
        }
    }

    fn make_persistence_rule() -> Rule {
        Rule {
            name: "持久化".into(),
            table: "persistence_events".into(),
            columns: None,
            filter: RuleFilter {
                event_ids: Some(vec![7045, 4697, 4698]),
                channels: Some(vec!["System".into(), "Security".into()]),
                provider_like: None,
                message_like: None,
                provider_name_re: None,
                message_re: None,
                levels: None,
            },
        }
    }

    #[test]
    fn test_router_single_rule_matches() {
        let router = SceneRouter::new(vec![make_security_rule()]);
        let event = make_event(Some(4624), Some("Security"), None, None);
        let matched = router.route(&event);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].table, "security_events");
    }

    #[test]
    fn test_router_single_rule_no_match() {
        let router = SceneRouter::new(vec![make_security_rule()]);
        let event = make_event(Some(7045), Some("System"), None, None);
        let matched = router.route(&event);
        assert!(matched.is_empty());
    }

    #[test]
    fn test_router_multiple_rules_one_matches() {
        let router = SceneRouter::new(vec![make_security_rule(), make_persistence_rule()]);
        let event = make_event(Some(7045), Some("System"), None, None);
        let matched = router.route(&event);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].table, "persistence_events");
    }

    #[test]
    fn test_router_multiple_rules_all_match() {
        // Persistence rule allows Security+System channels, 7045 in Security
        let router = SceneRouter::new(vec![make_security_rule(), make_persistence_rule()]);
        // 7045 on Security channel matches persistence (7045 in Security/System)
        let event = make_event(Some(7045), Some("Security"), None, None);
        let matched = router.route(&event);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].table, "persistence_events");
    }

    #[test]
    fn test_router_no_match_empty() {
        let router = SceneRouter::new(vec![make_security_rule(), make_persistence_rule()]);
        let event = make_event(Some(9999), Some("Unknown"), None, None);
        let matched = router.route(&event);
        assert!(matched.is_empty());
    }

    #[test]
    fn test_router_empty_rules_no_match() {
        let router: SceneRouter = SceneRouter::new(vec![]);
        let event = make_event(Some(4624), Some("Security"), None, None);
        let matched = router.route(&event);
        assert!(matched.is_empty());
    }

    // ── Rule loading from YAML ──

    #[test]
    fn test_rule_from_yaml_full_rule() {
        let yaml = r#"
name: "登录审计"
table: "logon_events"
columns:
  - event_id
  - event_time
  - user_name
event_ids: [4624, 4625]
channels: ["Security"]
"#;
        let rule = rule_from_yaml(yaml).unwrap();
        assert_eq!(rule.name, "登录审计");
        assert_eq!(rule.table, "logon_events");
        assert_eq!(rule.filter.event_ids, Some(vec![4624, 4625]));
    }

    #[test]
    fn test_rule_from_yaml_invalid() {
        let yaml = "not: valid: yaml: structure: broken";
        assert!(rule_from_yaml(yaml).is_err());
    }

    #[test]
    fn test_rule_from_yaml_minimal() {
        // Only name + table + event_ids is enough
        let yaml = r#"
name: "minimal"
table: "t"
columns: [event_id]
event_ids: [1]
"#;
        let rule = rule_from_yaml(yaml).unwrap();
        assert_eq!(rule.name, "minimal");
        assert_eq!(rule.table, "t");
        assert_eq!(rule.filter.event_ids, Some(vec![1]));
        assert!(rule.filter.channels.is_none());
    }

    /// Test that `#[serde(flatten)]` correctly pushes event_ids/channels into
    /// `RuleFilter` and NOT into the `columns` or top-level fields.
    #[test]
    fn test_flatten_not_leaking_filter_into_columns() {
        let yaml = r#"
name: "test"
table: "t"
columns:
  - event_id
  - message
event_ids: [4624]
channels: ["Security"]
"#;
        let rule: Rule = serde_yaml::from_str(yaml).unwrap();
        // columns should be Some(["event_id", "message"])
        assert_eq!(rule.columns, Some(vec!["event_id".into(), "message".into()]));
        // event_ids should be in the filter, NOT in columns
        assert_eq!(rule.filter.event_ids, Some(vec![4624]));
        assert_eq!(rule.filter.channels, Some(vec!["Security".into()]));
        // channels should NOT contain "event_id" or "message"
        if let Some(ref cols) = rule.columns {
            assert!(!cols.contains(&"event_ids".to_string()));
            assert!(!cols.contains(&"channels".to_string()));
        }
    }

    // ── Cycle 7: levels filtering ──

    #[test]
    fn test_matches_level_in_list() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            levels: Some(vec![1, 2, 3]),
            provider_like: None,
            message_like: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = ParsedEvent {
            level: Some(2),
            ..make_event(None, None, None, None)
        };
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_rejects_level_not_in_list() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            levels: Some(vec![1, 2]),
            provider_like: None,
            message_like: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = ParsedEvent {
            level: Some(5),
            ..make_event(None, None, None, None)
        };
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_no_levels_accepts_any() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            levels: None,
            provider_like: None,
            message_like: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = ParsedEvent {
            level: Some(99),
            ..make_event(None, None, None, None)
        };
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_missing_level_rejects() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            levels: Some(vec![1, 2]),
            provider_like: None,
            message_like: None,
            provider_name_re: None,
            message_re: None,
        };
        let event = ParsedEvent {
            level: None,
            ..make_event(None, None, None, None)
        };
        assert!(!filter.matches(&event));
    }

    // ── Cycle 8: regex matching (|re modifier) ──

    #[test]
    fn test_matches_message_regex() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: None,
            message_re: Some(vec![super::CompiledRegex(regex::Regex::new("失败$").unwrap())]),
            provider_name_re: None,
            levels: None,
        };
        let event = make_event(None, None, None, Some("用户已成功登录失败"));
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_rejects_message_regex() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: None,
            message_re: Some(vec![super::CompiledRegex(regex::Regex::new("失败$").unwrap())]),
            provider_name_re: None,
            levels: None,
        };
        let event = make_event(None, None, None, Some("用户已成功登录"));
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_matches_provider_name_regex() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: None,
            message_re: None,
            provider_name_re: Some(vec![super::CompiledRegex(regex::Regex::new("^MSSQL").unwrap())]),
            levels: None,
        };
        let event = make_event(None, None, Some("MSSQLSERVER"), None);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_rejects_provider_name_regex() {
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: None,
            message_re: None,
            provider_name_re: Some(vec![super::CompiledRegex(regex::Regex::new("^MSSQL").unwrap())]),
            levels: None,
        };
        let event = make_event(None, None, Some("ASP.NET"), None);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_message_regex_multiple_patterns_or() {
        // Multiple regex patterns — any should match (OR)
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: None,
            message_re: Some(vec![
                super::CompiledRegex(regex::Regex::new("失败$").unwrap()),
                super::CompiledRegex(regex::Regex::new("^错误").unwrap()),
            ]),
            provider_name_re: None,
            levels: None,
        };
        let event_match_first = make_event(None, None, None, Some("用户已成功登录失败"));
        let event_match_second = make_event(None, None, None, Some("错误代码: 0x80070005"));
        let event_no_match = make_event(None, None, None, Some("普通信息事件"));
        assert!(filter.matches(&event_match_first));
        assert!(filter.matches(&event_match_second));
        assert!(!filter.matches(&event_no_match));
    }

    #[test]
    fn test_regex_and_like_and() {
        // message_re + message_like must both match (AND)
        let filter = RuleFilter {
            event_ids: None,
            channels: None,
            provider_like: None,
            message_like: Some(vec!["%登录%".into()]),
            message_re: Some(vec![super::CompiledRegex(regex::Regex::new("失败$").unwrap())]),
            provider_name_re: None,
            levels: None,
        };
        let event_ok = make_event(None, None, None, Some("登录失败"));
        let event_no_like = make_event(None, None, None, Some("安装失败"));
        let event_no_re = make_event(None, None, None, Some("用户已成功登录"));
        assert!(filter.matches(&event_ok));
        assert!(!filter.matches(&event_no_like));
        assert!(!filter.matches(&event_no_re));
    }

    #[test]
    fn test_invalid_regex_pattern_rejected() {
        // YAML with invalid regex should fail to deserialize
        let yaml = r#"
name: "bad regex"
table: "t"
columns: [event_id]
message|re: ["("]
"#;
        let result = rule_from_yaml(yaml);
        assert!(result.is_err(), "expected error for invalid regex, got: {result:?}");
        let err = result.unwrap_err();
        assert!(err.contains("invalid regex"), "error should mention regex, got: {err}");
    }

    #[test]
    fn test_missing_columns_rejected() {
        let yaml = r#"
name: "no columns"
table: "t"
event_ids: [1]
"#;
        let result = rule_from_yaml(yaml);
        assert!(result.is_err(), "expected error for missing columns, got: {result:?}");
        let err = result.unwrap_err();
        assert!(err.contains("columns"), "error should mention columns, got: {err}");
    }

    #[test]
    fn test_backward_compat_old_like_suffix() {
        // Legacy message_like / provider_like should still be accepted
        let yaml = r#"
name: "legacy"
table: "t"
columns: [event_id]
message_like: ["%login%"]
provider_like: ["MSSQL%"]
"#;
        let rule = rule_from_yaml(yaml).unwrap();
        assert_eq!(rule.filter.message_like, Some(vec!["%login%".into()]));
        assert_eq!(rule.filter.provider_like, Some(vec!["MSSQL%".into()]));
    }

    #[test]
    fn test_rule_from_yaml_with_provider_like() {
        let yaml = r#"
name: "MSSQL"
table: "mssql_events"
columns: [event_id, message]
channels: ["Application"]
provider_name: ["MSSQL%"]
message: ["登录", "login"]
"#;
        let rule = rule_from_yaml(yaml).unwrap();
        assert_eq!(rule.filter.provider_like, Some(vec!["MSSQL%".into()]));
        assert_eq!(rule.filter.message_like, Some(vec!["登录".into(), "login".into()]));
    }
}

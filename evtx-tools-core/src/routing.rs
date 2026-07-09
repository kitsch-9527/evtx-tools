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
    /// Match if `provider_name` matches at least one SQL LIKE pattern.
    pub provider_like: Option<Vec<String>>,
    /// Match if `message` matches at least one SQL LIKE pattern.
    pub message_like: Option<Vec<String>>,
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
    serde_yaml::from_str(yaml).map_err(|e| format!("failed to parse rule: {e}"))
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
            timestamp: None,
            provider_guid: None,
            qualifiers: None,
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
  - timestamp
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

    #[test]
    fn test_rule_from_yaml_with_provider_like() {
        let yaml = r#"
name: "MSSQL"
table: "mssql_events"
channels: ["Application"]
provider_like: ["MSSQL%"]
message_like: ["登录", "login"]
"#;
        let rule = rule_from_yaml(yaml).unwrap();
        assert_eq!(rule.filter.provider_like, Some(vec!["MSSQL%".into()]));
        assert_eq!(rule.filter.message_like, Some(vec!["登录".into(), "login".into()]));
    }
}

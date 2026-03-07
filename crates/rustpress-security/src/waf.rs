//! Web Application Firewall (WAF) engine with configurable rules.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// Action to take when a WAF rule matches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WafAction {
    /// Block the request entirely.
    Block,
    /// Log the match but allow the request.
    Log,
    /// Challenge the user (e.g., CAPTCHA).
    Challenge,
}

/// Result of a WAF check on an incoming request.
#[derive(Debug, Clone, PartialEq)]
pub enum WafResult {
    /// Request is allowed through.
    Allow,
    /// Request is blocked by a rule.
    Block {
        rule_id: String,
        reason: String,
    },
    /// Request matched a logging rule.
    Log {
        rule_id: String,
        reason: String,
    },
    /// Request requires a challenge.
    Challenge {
        rule_id: String,
        reason: String,
    },
}

/// A single WAF rule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WafRule {
    /// Unique identifier for the rule.
    pub id: String,
    /// Human-readable name for the rule.
    pub name: String,
    /// Regex pattern to match against request data.
    pub pattern: String,
    /// Action to take when the rule matches.
    pub action: WafAction,
    /// Whether this rule is currently enabled.
    pub enabled: bool,
}

/// Compiled version of a WAF rule with a pre-compiled regex.
struct CompiledRule {
    rule: WafRule,
    regex: Regex,
}

/// The WAF engine that checks incoming requests against rules.
pub struct WafEngine {
    rules: Vec<CompiledRule>,
}

impl WafEngine {
    /// Create a new WAF engine with no rules.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a new WAF engine pre-loaded with default security rules.
    pub fn with_default_rules() -> Self {
        let mut engine = Self::new();
        for rule in Self::default_rules() {
            engine.add_rule(rule);
        }
        engine
    }

    /// Add a rule to the engine. Returns false if the regex pattern is invalid.
    pub fn add_rule(&mut self, rule: WafRule) -> bool {
        match Regex::new(&rule.pattern) {
            Ok(regex) => {
                info!(rule_id = %rule.id, rule_name = %rule.name, "WAF rule added");
                self.rules.push(CompiledRule { rule, regex });
                true
            }
            Err(e) => {
                warn!(rule_id = %rule.id, error = %e, "Failed to compile WAF rule pattern");
                false
            }
        }
    }

    /// Remove a rule by its ID. Returns true if a rule was removed.
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.rule.id != rule_id);
        self.rules.len() < before
    }

    /// Enable or disable a rule by ID.
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) -> bool {
        for compiled in &mut self.rules {
            if compiled.rule.id == rule_id {
                compiled.rule.enabled = enabled;
                return true;
            }
        }
        false
    }

    /// Return the number of rules loaded.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Check an incoming request against all enabled rules.
    ///
    /// The engine concatenates the method, path, query string, body, and header
    /// values into a single payload and matches each enabled rule against it.
    /// The first matching rule determines the result.
    pub fn check_request(
        &self,
        method: &str,
        path: &str,
        query: &str,
        body: &str,
        headers: &HashMap<String, String>,
    ) -> WafResult {
        // Build combined payload for matching.
        let header_values: String = headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let payload = format!(
            "{}\n{}\n{}\n{}\n{}",
            method, path, query, body, header_values
        );

        for compiled in &self.rules {
            if !compiled.rule.enabled {
                continue;
            }

            if compiled.regex.is_match(&payload) {
                let reason = format!(
                    "Matched WAF rule '{}' ({})",
                    compiled.rule.name, compiled.rule.id
                );

                match compiled.rule.action {
                    WafAction::Block => {
                        warn!(
                            rule_id = %compiled.rule.id,
                            rule_name = %compiled.rule.name,
                            path = %path,
                            "WAF blocked request"
                        );
                        return WafResult::Block {
                            rule_id: compiled.rule.id.clone(),
                            reason,
                        };
                    }
                    WafAction::Log => {
                        info!(
                            rule_id = %compiled.rule.id,
                            rule_name = %compiled.rule.name,
                            path = %path,
                            "WAF logged suspicious request"
                        );
                        return WafResult::Log {
                            rule_id: compiled.rule.id.clone(),
                            reason,
                        };
                    }
                    WafAction::Challenge => {
                        info!(
                            rule_id = %compiled.rule.id,
                            rule_name = %compiled.rule.name,
                            path = %path,
                            "WAF challenging request"
                        );
                        return WafResult::Challenge {
                            rule_id: compiled.rule.id.clone(),
                            reason,
                        };
                    }
                }
            }
        }

        WafResult::Allow
    }

    /// Return the built-in default rules.
    fn default_rules() -> Vec<WafRule> {
        vec![
            // SQL Injection rules
            WafRule {
                id: "sqli-001".into(),
                name: "SQL Injection - UNION SELECT".into(),
                pattern: r"(?i)\bunion\s+(all\s+)?select\b".into(),
                action: WafAction::Block,
                enabled: true,
            },
            WafRule {
                id: "sqli-002".into(),
                name: "SQL Injection - OR/AND boolean".into(),
                pattern: r"(?i)(\bor\b|\band\b)\s+\d+\s*=\s*\d+".into(),
                action: WafAction::Block,
                enabled: true,
            },
            WafRule {
                id: "sqli-003".into(),
                name: "SQL Injection - Comment injection".into(),
                pattern: r"(?i)(\b(select|insert|update|delete|drop|alter)\b.*--)|(\/\*.*\*\/)".into(),
                action: WafAction::Block,
                enabled: true,
            },
            WafRule {
                id: "sqli-004".into(),
                name: "SQL Injection - SLEEP/BENCHMARK".into(),
                pattern: r"(?i)\b(sleep|benchmark|waitfor\s+delay)\s*\(".into(),
                action: WafAction::Block,
                enabled: true,
            },
            // XSS rules
            WafRule {
                id: "xss-001".into(),
                name: "XSS - Script tag".into(),
                pattern: r"(?i)<\s*script[^>]*>".into(),
                action: WafAction::Block,
                enabled: true,
            },
            WafRule {
                id: "xss-002".into(),
                name: "XSS - Event handler attribute".into(),
                pattern: r"(?i)\bon(load|error|click|mouseover|focus|blur|submit|change|input)\s*=".into(),
                action: WafAction::Block,
                enabled: true,
            },
            WafRule {
                id: "xss-003".into(),
                name: "XSS - JavaScript URI".into(),
                pattern: r"(?i)javascript\s*:".into(),
                action: WafAction::Block,
                enabled: true,
            },
            // Directory traversal
            WafRule {
                id: "lfi-001".into(),
                name: "Directory Traversal".into(),
                pattern: r"(\.\.[\\/]){2,}".into(),
                action: WafAction::Block,
                enabled: true,
            },
            // File inclusion
            WafRule {
                id: "rfi-001".into(),
                name: "Remote File Inclusion".into(),
                pattern: r"(?i)(include|require)(_once)?\s*\(\s*['\"]?(https?|ftp|php|data):".into(),
                action: WafAction::Block,
                enabled: true,
            },
            // Command injection
            WafRule {
                id: "cmdi-001".into(),
                name: "Command Injection".into(),
                pattern: r"[;&|`]\s*(cat|ls|pwd|whoami|id|uname|wget|curl|nc|bash|sh|python|perl|ruby)\b".into(),
                action: WafAction::Block,
                enabled: true,
            },
            // WordPress-specific
            WafRule {
                id: "wp-001".into(),
                name: "WP Config Access".into(),
                pattern: r"(?i)wp-config\.php".into(),
                action: WafAction::Block,
                enabled: true,
            },
            WafRule {
                id: "wp-002".into(),
                name: "PHP File Upload Attempt".into(),
                pattern: r"(?i)\.(php[345s]?|phtml|phar)\s*$".into(),
                action: WafAction::Log,
                enabled: true,
            },
        ]
    }
}

impl Default for WafEngine {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_headers() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn test_allow_clean_request() {
        let engine = WafEngine::with_default_rules();
        let result = engine.check_request("GET", "/hello-world", "", "", &empty_headers());
        assert_eq!(result, WafResult::Allow);
    }

    #[test]
    fn test_block_sql_injection_union() {
        let engine = WafEngine::with_default_rules();
        let result = engine.check_request(
            "GET",
            "/page",
            "id=1 UNION SELECT * FROM users",
            "",
            &empty_headers(),
        );
        match result {
            WafResult::Block { rule_id, .. } => assert_eq!(rule_id, "sqli-001"),
            other => panic!("Expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_block_xss_script_tag() {
        let engine = WafEngine::with_default_rules();
        let result = engine.check_request(
            "POST",
            "/comment",
            "",
            "<script>alert('xss')</script>",
            &empty_headers(),
        );
        match result {
            WafResult::Block { rule_id, .. } => assert_eq!(rule_id, "xss-001"),
            other => panic!("Expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_block_directory_traversal() {
        let engine = WafEngine::with_default_rules();
        let result = engine.check_request(
            "GET",
            "/../../etc/passwd",
            "",
            "",
            &empty_headers(),
        );
        match result {
            WafResult::Block { rule_id, .. } => assert_eq!(rule_id, "lfi-001"),
            other => panic!("Expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_disabled_rule_does_not_match() {
        let mut engine = WafEngine::with_default_rules();
        engine.set_rule_enabled("sqli-001", false);
        // UNION SELECT should no longer be blocked by sqli-001
        let result = engine.check_request(
            "GET",
            "/page",
            "id=1 UNION SELECT username FROM users",
            "",
            &empty_headers(),
        );
        // It might still be caught by another rule, but not sqli-001
        if let WafResult::Block { rule_id, .. } = &result {
            assert_ne!(rule_id, "sqli-001");
        }
    }

    #[test]
    fn test_add_custom_rule() {
        let mut engine = WafEngine::new();
        let ok = engine.add_rule(WafRule {
            id: "custom-001".into(),
            name: "Block admin path".into(),
            pattern: r"/secret-admin".into(),
            action: WafAction::Block,
            enabled: true,
        });
        assert!(ok);
        assert_eq!(engine.rule_count(), 1);

        let result = engine.check_request("GET", "/secret-admin", "", "", &empty_headers());
        match result {
            WafResult::Block { rule_id, .. } => assert_eq!(rule_id, "custom-001"),
            other => panic!("Expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_remove_rule() {
        let mut engine = WafEngine::with_default_rules();
        let count_before = engine.rule_count();
        assert!(engine.remove_rule("sqli-001"));
        assert_eq!(engine.rule_count(), count_before - 1);
        assert!(!engine.remove_rule("nonexistent"));
    }

    #[test]
    fn test_block_command_injection() {
        let engine = WafEngine::with_default_rules();
        let result = engine.check_request(
            "POST",
            "/api/exec",
            "",
            "file=test; cat /etc/passwd",
            &empty_headers(),
        );
        match result {
            WafResult::Block { rule_id, .. } => assert_eq!(rule_id, "cmdi-001"),
            other => panic!("Expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_default_rules_count() {
        let engine = WafEngine::with_default_rules();
        assert!(engine.rule_count() >= 10);
    }

    #[test]
    fn test_invalid_regex_not_added() {
        let mut engine = WafEngine::new();
        let ok = engine.add_rule(WafRule {
            id: "bad-001".into(),
            name: "Bad regex".into(),
            pattern: r"[invalid".into(),
            action: WafAction::Block,
            enabled: true,
        });
        assert!(!ok);
        assert_eq!(engine.rule_count(), 0);
    }
}

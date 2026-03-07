use serde::{Deserialize, Serialize};

/// Rewrite rule from WordPress .htaccess or nginx config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteRule {
    pub pattern: String,
    pub replacement: String,
    pub flags: Vec<String>,
}

/// Redirect rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectRule {
    pub from: String,
    pub to: String,
    pub status_code: u16,
}

/// Parse Apache .htaccess rewrite rules.
pub fn parse_htaccess(content: &str) -> Vec<RewriteRule> {
    let mut rules = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("RewriteRule") {
            let parts: Vec<&str> = trimmed.splitn(4, ' ').collect();
            if parts.len() >= 3 {
                let flags = if parts.len() >= 4 {
                    parts[3]
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect()
                } else {
                    Vec::new()
                };

                rules.push(RewriteRule {
                    pattern: parts[1].to_string(),
                    replacement: parts[2].to_string(),
                    flags,
                });
            }
        }
    }

    rules
}

/// Parse nginx rewrite/redirect rules.
pub fn parse_nginx_conf(content: &str) -> Vec<RewriteRule> {
    let mut rules = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim().trim_end_matches(';');

        if trimmed.starts_with("rewrite") {
            let parts: Vec<&str> = trimmed.splitn(4, ' ').collect();
            if parts.len() >= 3 {
                let flags = if parts.len() >= 4 {
                    vec![parts[3].to_string()]
                } else {
                    Vec::new()
                };
                rules.push(RewriteRule {
                    pattern: parts[1].to_string(),
                    replacement: parts[2].to_string(),
                    flags,
                });
            }
        }
    }

    rules
}

/// Convert rewrite rules to RustPress route configuration.
pub fn rules_to_rustpress_config(rules: &[RewriteRule]) -> String {
    let mut config = String::new();
    config.push_str("# RustPress Rewrite Configuration\n");
    config.push_str("# Auto-generated from WordPress rewrite rules\n\n");

    config.push_str("[rewrites]\n");
    for (i, rule) in rules.iter().enumerate() {
        config.push_str(&format!("# Rule {}\n", i + 1));
        config.push_str(&format!("pattern_{} = \"{}\"\n", i, rule.pattern));
        config.push_str(&format!("target_{} = \"{}\"\n", i, rule.replacement));
        if !rule.flags.is_empty() {
            config.push_str(&format!("flags_{} = \"{}\"\n", i, rule.flags.join(",")));
        }
        config.push('\n');
    }

    config
}

/// Extract redirect rules from parsed rewrite rules.
pub fn extract_redirects(rules: &[RewriteRule]) -> Vec<RedirectRule> {
    rules
        .iter()
        .filter(|r| {
            r.flags.iter().any(|f| {
                f.starts_with("R=") || f == "R" || f == "redirect" || f == "permanent"
            })
        })
        .map(|r| {
            let status = if r.flags.iter().any(|f| f == "R=301" || f == "permanent") {
                301
            } else if r.flags.iter().any(|f| f == "R=302" || f == "redirect") {
                302
            } else {
                301
            };

            RedirectRule {
                from: r.pattern.clone(),
                to: r.replacement.clone(),
                status_code: status,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_htaccess() {
        let htaccess = r#"
# WordPress Rewrite Rules
RewriteEngine On
RewriteBase /
RewriteRule ^index\.php$ - [L]
RewriteRule ^(.*)$ /index.php?p=$1 [L,QSA]
"#;
        let rules = parse_htaccess(htaccess);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].pattern, "^index\\.php$");
        assert_eq!(rules[1].replacement, "/index.php?p=$1");
        assert!(rules[1].flags.contains(&"L".to_string()));
    }

    #[test]
    fn test_parse_nginx() {
        let nginx = r#"
location / {
    rewrite ^/old-page$ /new-page permanent;
    rewrite ^/blog/(.*)$ /articles/$1 last;
}
"#;
        let rules = parse_nginx_conf(nginx);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].pattern, "^/old-page$");
        assert_eq!(rules[0].replacement, "/new-page");
    }

    #[test]
    fn test_extract_redirects() {
        let rules = vec![
            RewriteRule {
                pattern: "^/old$".to_string(),
                replacement: "/new".to_string(),
                flags: vec!["R=301".to_string(), "L".to_string()],
            },
            RewriteRule {
                pattern: "^(.*)$".to_string(),
                replacement: "/index.php".to_string(),
                flags: vec!["L".to_string()],
            },
        ];
        let redirects = extract_redirects(&rules);
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].status_code, 301);
    }

    #[test]
    fn test_rules_to_config() {
        let rules = vec![RewriteRule {
            pattern: "^/old$".to_string(),
            replacement: "/new".to_string(),
            flags: vec!["R=301".to_string()],
        }];
        let config = rules_to_rustpress_config(&rules);
        assert!(config.contains("pattern_0"));
        assert!(config.contains("target_0"));
    }
}

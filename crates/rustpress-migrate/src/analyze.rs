use serde::{Deserialize, Serialize};
use tracing::info;

/// Compatibility analysis result for a WordPress site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub wordpress_version: String,
    pub db_version: String,
    pub compatibility_score: u8,
    pub post_count: u64,
    pub page_count: u64,
    pub user_count: u64,
    pub comment_count: u64,
    pub attachment_count: u64,
    pub active_theme: String,
    pub active_plugins: Vec<PluginCompat>,
    pub issues: Vec<CompatIssue>,
    pub recommendations: Vec<String>,
}

/// Plugin compatibility status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCompat {
    pub name: String,
    pub status: PluginCompatStatus,
    pub alternative: Option<String>,
}

/// Plugin compatibility level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginCompatStatus {
    /// Native RustPress equivalent available
    NativeAvailable,
    /// Can be converted via AI conversion service
    Convertible,
    /// Not compatible, manual work needed
    Incompatible,
    /// Unknown plugin
    Unknown,
}

/// A compatibility issue found during analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatIssue {
    pub severity: IssueSeverity,
    pub area: String,
    pub description: String,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    Critical,
    Warning,
    Info,
}

/// Known WordPress plugins and their RustPress equivalents.
fn known_plugin_mappings() -> Vec<(&'static str, PluginCompatStatus, Option<&'static str>)> {
    vec![
        ("yoast-seo", PluginCompatStatus::NativeAvailable, Some("rustpress-seo")),
        ("wordpress-seo", PluginCompatStatus::NativeAvailable, Some("rustpress-seo")),
        ("rank-math", PluginCompatStatus::NativeAvailable, Some("rustpress-seo")),
        ("contact-form-7", PluginCompatStatus::NativeAvailable, Some("rustpress-forms")),
        ("wpforms", PluginCompatStatus::NativeAvailable, Some("rustpress-forms")),
        ("gravity-forms", PluginCompatStatus::NativeAvailable, Some("rustpress-forms")),
        ("woocommerce", PluginCompatStatus::NativeAvailable, Some("rustpress-commerce")),
        ("advanced-custom-fields", PluginCompatStatus::NativeAvailable, Some("rustpress-fields")),
        ("acf", PluginCompatStatus::NativeAvailable, Some("rustpress-fields")),
        ("wordfence", PluginCompatStatus::NativeAvailable, Some("rustpress-security")),
        ("sucuri-scanner", PluginCompatStatus::NativeAvailable, Some("rustpress-security")),
        ("akismet", PluginCompatStatus::Convertible, None),
        ("jetpack", PluginCompatStatus::Incompatible, None),
        ("elementor", PluginCompatStatus::Incompatible, None),
        ("wp-super-cache", PluginCompatStatus::NativeAvailable, Some("rustpress-cache (built-in)")),
        ("w3-total-cache", PluginCompatStatus::NativeAvailable, Some("rustpress-cache (built-in)")),
        ("wp-rocket", PluginCompatStatus::NativeAvailable, Some("rustpress-cache (built-in)")),
    ]
}

/// Analyze a plugin name and determine compatibility.
pub fn analyze_plugin(name: &str) -> PluginCompat {
    let normalized = name.to_lowercase().replace(' ', "-");

    for (known_name, status, alternative) in known_plugin_mappings() {
        if normalized.contains(known_name) {
            return PluginCompat {
                name: name.to_string(),
                status,
                alternative: alternative.map(|s| s.to_string()),
            };
        }
    }

    PluginCompat {
        name: name.to_string(),
        status: PluginCompatStatus::Unknown,
        alternative: None,
    }
}

/// Analyze WordPress version compatibility.
pub fn analyze_wp_version(version: &str) -> (u8, Vec<CompatIssue>) {
    let mut issues = Vec::new();

    let major_minor: Vec<&str> = version.split('.').collect();
    let major: u32 = major_minor.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = major_minor.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let score = match (major, minor) {
        (6, 0..=9) => {
            info!(version, "Tier 1: Full compatibility");
            95
        }
        (5, 0..=9) => {
            issues.push(CompatIssue {
                severity: IssueSeverity::Info,
                area: "WordPress Version".to_string(),
                description: format!("WordPress {} is Tier 2 (basic compatibility)", version),
                resolution: "Most features will work. Some newer block editor features may differ.".to_string(),
            });
            80
        }
        (4, 4..=9) => {
            issues.push(CompatIssue {
                severity: IssueSeverity::Warning,
                area: "WordPress Version".to_string(),
                description: format!("WordPress {} is Tier 3 (legacy compatibility)", version),
                resolution: "Classic Editor content will work. Consider upgrading WordPress before migrating.".to_string(),
            });
            60
        }
        _ => {
            issues.push(CompatIssue {
                severity: IssueSeverity::Critical,
                area: "WordPress Version".to_string(),
                description: format!("WordPress {} is not supported", version),
                resolution: "Please upgrade to WordPress 4.4 or later before migrating.".to_string(),
            });
            20
        }
    };

    (score, issues)
}

/// Generate a human-readable compatibility report.
pub fn format_report(report: &CompatibilityReport) -> String {
    let mut output = String::new();
    output.push_str("RustPress Compatibility Report\n");
    output.push_str(&"=".repeat(50));
    output.push('\n');
    output.push_str(&format!("WordPress Version: {}\n", report.wordpress_version));
    output.push_str(&format!("DB Version: {}\n", report.db_version));
    output.push_str(&format!("Compatibility Score: {}%\n\n", report.compatibility_score));

    output.push_str("Content Summary:\n");
    output.push_str(&format!("  Posts: {}\n", report.post_count));
    output.push_str(&format!("  Pages: {}\n", report.page_count));
    output.push_str(&format!("  Users: {}\n", report.user_count));
    output.push_str(&format!("  Comments: {}\n", report.comment_count));
    output.push_str(&format!("  Attachments: {}\n\n", report.attachment_count));

    output.push_str(&format!("Active Theme: {}\n\n", report.active_theme));

    if !report.active_plugins.is_empty() {
        output.push_str("Plugin Compatibility:\n");
        for plugin in &report.active_plugins {
            let status_str = match plugin.status {
                PluginCompatStatus::NativeAvailable => "NATIVE",
                PluginCompatStatus::Convertible => "CONVERT",
                PluginCompatStatus::Incompatible => "INCOMPAT",
                PluginCompatStatus::Unknown => "UNKNOWN",
            };
            let alt = plugin
                .alternative
                .as_deref()
                .map(|a| format!(" -> {}", a))
                .unwrap_or_default();
            output.push_str(&format!("  [{}] {}{}\n", status_str, plugin.name, alt));
        }
        output.push('\n');
    }

    if !report.issues.is_empty() {
        output.push_str("Issues:\n");
        for issue in &report.issues {
            let severity = match issue.severity {
                IssueSeverity::Critical => "CRITICAL",
                IssueSeverity::Warning => "WARNING",
                IssueSeverity::Info => "INFO",
            };
            output.push_str(&format!("  [{}] {}: {}\n", severity, issue.area, issue.description));
            output.push_str(&format!("    Resolution: {}\n", issue.resolution));
        }
        output.push('\n');
    }

    if !report.recommendations.is_empty() {
        output.push_str("Recommendations:\n");
        for rec in &report.recommendations {
            output.push_str(&format!("  - {}\n", rec));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_known_plugin() {
        let result = analyze_plugin("Yoast SEO");
        assert_eq!(result.status, PluginCompatStatus::NativeAvailable);
        assert_eq!(result.alternative, Some("rustpress-seo".to_string()));
    }

    #[test]
    fn test_analyze_unknown_plugin() {
        let result = analyze_plugin("My Custom Plugin");
        assert_eq!(result.status, PluginCompatStatus::Unknown);
    }

    #[test]
    fn test_analyze_wp6() {
        let (score, issues) = analyze_wp_version("6.9");
        assert_eq!(score, 95);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_analyze_wp5() {
        let (score, issues) = analyze_wp_version("5.9");
        assert_eq!(score, 80);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_analyze_wp4_legacy() {
        let (score, issues) = analyze_wp_version("4.5");
        assert_eq!(score, 60);
        assert_eq!(issues[0].severity, IssueSeverity::Warning);
    }

    #[test]
    fn test_analyze_unsupported() {
        let (score, issues) = analyze_wp_version("3.9");
        assert_eq!(score, 20);
        assert_eq!(issues[0].severity, IssueSeverity::Critical);
    }

    #[test]
    fn test_format_report() {
        let report = CompatibilityReport {
            wordpress_version: "6.9".to_string(),
            db_version: "58975".to_string(),
            compatibility_score: 95,
            post_count: 100,
            page_count: 10,
            user_count: 5,
            comment_count: 50,
            attachment_count: 200,
            active_theme: "twentytwentyfive".to_string(),
            active_plugins: vec![analyze_plugin("Yoast SEO")],
            issues: vec![],
            recommendations: vec!["Backup your database before migrating.".to_string()],
        };
        let output = format_report(&report);
        assert!(output.contains("Compatibility Score: 95%"));
        assert!(output.contains("Posts: 100"));
        assert!(output.contains("rustpress-seo"));
    }
}

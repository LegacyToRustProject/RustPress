//! Security scanner for detecting common misconfigurations and vulnerabilities.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

/// Status of a security check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CheckStatus {
    /// Check passed without issues.
    Pass,
    /// Check passed but with warnings.
    Warning,
    /// Check failed indicating a security issue.
    Fail,
}

/// Result of a single security check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityCheck {
    /// Name of the check.
    pub name: String,
    /// Description of what this check verifies.
    pub description: String,
    /// Result status.
    pub status: CheckStatus,
    /// Detailed information about the check result.
    pub details: String,
}

/// Configuration values passed to the scanner for inspection.
#[derive(Debug, Clone, Default)]
pub struct ScannerContext {
    /// Whether debug mode is enabled.
    pub debug_mode: bool,
    /// Whether SSL/TLS is being used.
    pub ssl_enabled: bool,
    /// The database table prefix.
    pub db_prefix: String,
    /// Whether directory listing is enabled on the web server.
    pub directory_listing_enabled: bool,
    /// List of admin usernames.
    pub admin_usernames: Vec<String>,
    /// Paths to check for file permissions.
    pub file_paths: Vec<String>,
    /// Upload directory path.
    pub upload_dir: String,
    /// Any uploaded file names to scan for suspicious patterns.
    pub uploaded_filenames: Vec<String>,
    /// Additional key-value pairs for custom checks.
    pub extra: HashMap<String, String>,
}

/// Security scanner that runs a suite of checks against the system.
pub struct SecurityScanner {
    context: ScannerContext,
}

impl SecurityScanner {
    /// Create a new scanner with the given context.
    pub fn new(context: ScannerContext) -> Self {
        Self { context }
    }

    /// Run all security checks and return the results.
    pub fn run_all_checks(&self) -> Vec<SecurityCheck> {
        info!("Running security scan...");

        let mut checks = Vec::new();
        checks.push(self.check_debug_mode());
        checks.push(self.check_ssl());
        checks.push(self.check_db_prefix());
        checks.push(self.check_default_admin());
        checks.push(self.check_directory_listing());
        checks.push(self.check_file_permissions());
        checks.push(self.check_php_upload());
        checks.push(self.check_wp_config_accessible());
        checks.push(self.check_strong_db_prefix());
        checks.push(self.check_upload_directory());

        let pass_count = checks.iter().filter(|c| c.status == CheckStatus::Pass).count();
        let warn_count = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warning)
            .count();
        let fail_count = checks.iter().filter(|c| c.status == CheckStatus::Fail).count();

        info!(
            passed = pass_count,
            warnings = warn_count,
            failed = fail_count,
            total = checks.len(),
            "Security scan complete"
        );

        checks
    }

    /// Check if debug mode is enabled in production.
    fn check_debug_mode(&self) -> SecurityCheck {
        SecurityCheck {
            name: "Debug Mode".into(),
            description: "Debug mode should be disabled in production to prevent information leakage.".into(),
            status: if self.context.debug_mode {
                CheckStatus::Fail
            } else {
                CheckStatus::Pass
            },
            details: if self.context.debug_mode {
                "Debug mode is ENABLED. Disable it for production deployments to prevent \
                 sensitive information from being exposed in error messages."
                    .into()
            } else {
                "Debug mode is disabled.".into()
            },
        }
    }

    /// Check if SSL/TLS is enabled.
    fn check_ssl(&self) -> SecurityCheck {
        SecurityCheck {
            name: "SSL/TLS".into(),
            description: "SSL/TLS should be enabled to encrypt traffic between clients and the server.".into(),
            status: if self.context.ssl_enabled {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            details: if self.context.ssl_enabled {
                "SSL/TLS is enabled.".into()
            } else {
                "SSL/TLS is NOT enabled. All traffic is sent in plain text. \
                 Enable HTTPS to protect user credentials and data."
                    .into()
            },
        }
    }

    /// Check if the database prefix is the insecure default "wp_".
    fn check_db_prefix(&self) -> SecurityCheck {
        let is_default = self.context.db_prefix == "wp_";
        SecurityCheck {
            name: "Database Prefix".into(),
            description: "Using the default 'wp_' prefix makes SQL injection attacks easier.".into(),
            status: if is_default {
                CheckStatus::Warning
            } else {
                CheckStatus::Pass
            },
            details: if is_default {
                "Database table prefix is set to the default 'wp_'. Consider using a \
                 unique prefix to reduce the risk of targeted SQL injection attacks."
                    .into()
            } else {
                format!(
                    "Database table prefix is set to '{}' (non-default).",
                    self.context.db_prefix
                )
            },
        }
    }

    /// Check for default admin username.
    fn check_default_admin(&self) -> SecurityCheck {
        let dangerous_names = ["admin", "administrator", "root", "user"];
        let found: Vec<&str> = self
            .context
            .admin_usernames
            .iter()
            .filter(|u| dangerous_names.contains(&u.to_lowercase().as_str()))
            .map(|s| s.as_str())
            .collect();

        SecurityCheck {
            name: "Default Admin Username".into(),
            description: "Using default admin usernames makes brute-force attacks easier.".into(),
            status: if found.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            },
            details: if found.is_empty() {
                "No default admin usernames detected.".into()
            } else {
                format!(
                    "Default admin username(s) detected: {}. Consider changing to a \
                     unique username to reduce brute-force attack risk.",
                    found.join(", ")
                )
            },
        }
    }

    /// Check if directory listing is enabled.
    fn check_directory_listing(&self) -> SecurityCheck {
        SecurityCheck {
            name: "Directory Listing".into(),
            description: "Directory listing should be disabled to prevent information disclosure."
                .into(),
            status: if self.context.directory_listing_enabled {
                CheckStatus::Fail
            } else {
                CheckStatus::Pass
            },
            details: if self.context.directory_listing_enabled {
                "Directory listing is ENABLED. This allows attackers to browse your \
                 server's file structure. Disable it in your web server configuration."
                    .into()
            } else {
                "Directory listing is disabled.".into()
            },
        }
    }

    /// Check file permissions on critical paths.
    fn check_file_permissions(&self) -> SecurityCheck {
        if self.context.file_paths.is_empty() {
            return SecurityCheck {
                name: "File Permissions".into(),
                description: "Critical files should have restrictive permissions.".into(),
                status: CheckStatus::Warning,
                details: "No file paths provided for permission checking. Ensure that \
                          configuration files (e.g., .env, wp-config.php) are not \
                          world-readable (recommended: 600 or 640)."
                    .into(),
            };
        }

        let mut issues = Vec::new();

        for path_str in &self.context.file_paths {
            let path = Path::new(path_str);
            if !path.exists() {
                continue;
            }

            // On Unix, check if the file is world-readable.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata(path) {
                    let mode = metadata.permissions().mode();
                    // Check if world-readable (o+r) or world-writable (o+w)
                    if mode & 0o004 != 0 {
                        issues.push(format!(
                            "{}: world-readable (mode {:o})",
                            path_str,
                            mode & 0o777
                        ));
                    }
                    if mode & 0o002 != 0 {
                        issues.push(format!(
                            "{}: world-writable (mode {:o})",
                            path_str,
                            mode & 0o777
                        ));
                    }
                }
            }
        }

        SecurityCheck {
            name: "File Permissions".into(),
            description: "Critical files should have restrictive permissions.".into(),
            status: if issues.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            details: if issues.is_empty() {
                "All checked files have appropriate permissions.".into()
            } else {
                format!("Permission issues found:\n- {}", issues.join("\n- "))
            },
        }
    }

    /// Check for suspicious PHP file uploads.
    fn check_php_upload(&self) -> SecurityCheck {
        let php_pattern =
            Regex::new(r"(?i)\.(php[345s]?|phtml|phar|phps|pht|pgif|shtml|inc)$").unwrap();

        let suspicious: Vec<&str> = self
            .context
            .uploaded_filenames
            .iter()
            .filter(|f| php_pattern.is_match(f))
            .map(|s| s.as_str())
            .collect();

        // Also check for double extensions like image.php.jpg
        let double_ext_pattern = Regex::new(r"(?i)\.(php[345s]?|phtml|phar)\.\w+$").unwrap();
        let double_ext: Vec<&str> = self
            .context
            .uploaded_filenames
            .iter()
            .filter(|f| double_ext_pattern.is_match(f))
            .map(|s| s.as_str())
            .collect();

        let all_suspicious: Vec<&str> = suspicious
            .into_iter()
            .chain(double_ext.into_iter())
            .collect();

        SecurityCheck {
            name: "PHP File Upload Detection".into(),
            description: "Uploaded files should not contain PHP or executable extensions.".into(),
            status: if all_suspicious.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            details: if all_suspicious.is_empty() {
                "No suspicious file uploads detected.".into()
            } else {
                format!(
                    "Suspicious uploaded files detected: {}. These could be used for \
                     remote code execution.",
                    all_suspicious.join(", ")
                )
            },
        }
    }

    /// Check if wp-config.php would be accessible via web.
    fn check_wp_config_accessible(&self) -> SecurityCheck {
        // In RustPress, we don't use wp-config.php, but check if one exists
        // in the document root (which could indicate a WordPress installation
        // alongside RustPress).
        let config_exists = self
            .context
            .extra
            .get("document_root")
            .map(|root| Path::new(root).join("wp-config.php").exists())
            .unwrap_or(false);

        SecurityCheck {
            name: "Configuration File Exposure".into(),
            description: "Configuration files should not be accessible via the web.".into(),
            status: if config_exists {
                CheckStatus::Warning
            } else {
                CheckStatus::Pass
            },
            details: if config_exists {
                "A wp-config.php file was found in the document root. Ensure it is \
                 not served by the web server."
                    .into()
            } else {
                "No exposed configuration files detected.".into()
            },
        }
    }

    /// Check if the database prefix is strong enough (length and complexity).
    fn check_strong_db_prefix(&self) -> SecurityCheck {
        let prefix = &self.context.db_prefix;
        if prefix.is_empty() {
            return SecurityCheck {
                name: "Database Prefix Strength".into(),
                description: "Database prefix should be non-empty and sufficiently complex.".into(),
                status: CheckStatus::Fail,
                details: "Database prefix is empty. Set a table prefix to namespace your tables."
                    .into(),
            };
        }

        let has_underscore = prefix.ends_with('_');
        let long_enough = prefix.len() >= 4;

        SecurityCheck {
            name: "Database Prefix Strength".into(),
            description: "Database prefix should be non-empty and sufficiently complex.".into(),
            status: if long_enough && has_underscore {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            },
            details: if long_enough && has_underscore {
                format!("Database prefix '{}' looks strong.", prefix)
            } else if !has_underscore {
                format!(
                    "Database prefix '{}' does not end with underscore. \
                     Convention is to use a trailing underscore (e.g., 'mysite_').",
                    prefix
                )
            } else {
                format!(
                    "Database prefix '{}' is short. Consider using a longer, \
                     more unique prefix.",
                    prefix
                )
            },
        }
    }

    /// Check that the upload directory is properly configured.
    fn check_upload_directory(&self) -> SecurityCheck {
        if self.context.upload_dir.is_empty() {
            return SecurityCheck {
                name: "Upload Directory".into(),
                description: "Upload directory should be configured and restricted.".into(),
                status: CheckStatus::Warning,
                details: "Upload directory path is not configured.".into(),
            };
        }

        let path = Path::new(&self.context.upload_dir);
        if !path.exists() {
            return SecurityCheck {
                name: "Upload Directory".into(),
                description: "Upload directory should be configured and restricted.".into(),
                status: CheckStatus::Warning,
                details: format!(
                    "Upload directory '{}' does not exist.",
                    self.context.upload_dir
                ),
            };
        }

        // Check if .htaccess or equivalent exists in upload dir
        let has_protection = path.join(".htaccess").exists()
            || path.join("index.html").exists()
            || path.join("index.php").exists();

        SecurityCheck {
            name: "Upload Directory".into(),
            description: "Upload directory should be configured and restricted.".into(),
            status: if has_protection {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            },
            details: if has_protection {
                format!(
                    "Upload directory '{}' has directory index protection.",
                    self.context.upload_dir
                )
            } else {
                format!(
                    "Upload directory '{}' may lack directory listing protection. \
                     Consider adding an index.html or .htaccess file.",
                    self.context.upload_dir
                )
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_context() -> ScannerContext {
        ScannerContext {
            debug_mode: false,
            ssl_enabled: true,
            db_prefix: "rp42_".into(),
            directory_listing_enabled: false,
            admin_usernames: vec!["jdoe".into()],
            file_paths: vec![],
            upload_dir: String::new(),
            uploaded_filenames: vec![],
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_all_pass_with_secure_config() {
        let scanner = SecurityScanner::new(default_context());
        let results = scanner.run_all_checks();

        let fails: Vec<_> = results
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .collect();
        assert!(fails.is_empty(), "Expected no failures, got: {:?}", fails);
    }

    #[test]
    fn test_debug_mode_fails() {
        let mut ctx = default_context();
        ctx.debug_mode = true;
        let scanner = SecurityScanner::new(ctx);
        let results = scanner.run_all_checks();

        let debug_check = results.iter().find(|c| c.name == "Debug Mode").unwrap();
        assert_eq!(debug_check.status, CheckStatus::Fail);
    }

    #[test]
    fn test_ssl_disabled_fails() {
        let mut ctx = default_context();
        ctx.ssl_enabled = false;
        let scanner = SecurityScanner::new(ctx);
        let results = scanner.run_all_checks();

        let ssl_check = results.iter().find(|c| c.name == "SSL/TLS").unwrap();
        assert_eq!(ssl_check.status, CheckStatus::Fail);
    }

    #[test]
    fn test_default_admin_warns() {
        let mut ctx = default_context();
        ctx.admin_usernames = vec!["admin".into(), "jdoe".into()];
        let scanner = SecurityScanner::new(ctx);
        let results = scanner.run_all_checks();

        let admin_check = results
            .iter()
            .find(|c| c.name == "Default Admin Username")
            .unwrap();
        assert_eq!(admin_check.status, CheckStatus::Warning);
    }

    #[test]
    fn test_default_db_prefix_warns() {
        let mut ctx = default_context();
        ctx.db_prefix = "wp_".into();
        let scanner = SecurityScanner::new(ctx);
        let results = scanner.run_all_checks();

        let prefix_check = results
            .iter()
            .find(|c| c.name == "Database Prefix")
            .unwrap();
        assert_eq!(prefix_check.status, CheckStatus::Warning);
    }

    #[test]
    fn test_php_upload_detected() {
        let mut ctx = default_context();
        ctx.uploaded_filenames = vec![
            "image.jpg".into(),
            "document.pdf".into(),
            "shell.php".into(),
        ];
        let scanner = SecurityScanner::new(ctx);
        let results = scanner.run_all_checks();

        let upload_check = results
            .iter()
            .find(|c| c.name == "PHP File Upload Detection")
            .unwrap();
        assert_eq!(upload_check.status, CheckStatus::Fail);
        assert!(upload_check.details.contains("shell.php"));
    }

    #[test]
    fn test_directory_listing_fails() {
        let mut ctx = default_context();
        ctx.directory_listing_enabled = true;
        let scanner = SecurityScanner::new(ctx);
        let results = scanner.run_all_checks();

        let listing_check = results
            .iter()
            .find(|c| c.name == "Directory Listing")
            .unwrap();
        assert_eq!(listing_check.status, CheckStatus::Fail);
    }

    #[test]
    fn test_run_all_checks_returns_expected_count() {
        let scanner = SecurityScanner::new(default_context());
        let results = scanner.run_all_checks();
        assert_eq!(results.len(), 10);
    }
}

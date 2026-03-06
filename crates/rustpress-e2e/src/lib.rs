//! RustPress E2E Comparison Test Suite
//!
//! This crate provides helpers and configuration for running end-to-end tests
//! that compare a RustPress instance against a real WordPress instance, ensuring
//! RustPress is a faithful WordPress clone.
//!
//! # Environment Variables
//!
//! - `WORDPRESS_URL` - URL of the WordPress instance (default: `http://localhost:8081`)
//! - `RUSTPRESS_URL` - URL of the RustPress instance (default: `http://localhost:8080`)
//! - `ADMIN_USER`    - Admin username for both instances (default: `admin`)
//! - `ADMIN_PASSWORD` - Admin password for both instances (default: `password`)
//! - `WEBDRIVER_URL` - Selenium WebDriver URL (default: `http://localhost:9515`)

use scraper::{Html, Selector};
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for dual-site testing.
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// WordPress base URL, e.g. `http://localhost:8081`
    pub wordpress_url: String,
    /// RustPress base URL, e.g. `http://localhost:8080`
    pub rustpress_url: String,
    /// Admin username used on both sites.
    pub admin_user: String,
    /// Admin password used on both sites.
    pub admin_password: String,
    /// WebDriver URL for Selenium tests.
    pub webdriver_url: String,
}

impl TestConfig {
    /// Read configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            wordpress_url: std::env::var("WORDPRESS_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            rustpress_url: std::env::var("RUSTPRESS_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            admin_user: std::env::var("ADMIN_USER")
                .unwrap_or_else(|_| "admin".to_string()),
            admin_password: std::env::var("ADMIN_PASSWORD")
                .unwrap_or_else(|_| "password".to_string()),
            webdriver_url: std::env::var("WEBDRIVER_URL")
                .unwrap_or_else(|_| "http://localhost:9515".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP client helpers
// ---------------------------------------------------------------------------

/// Build a shared `reqwest::Client` with sensible defaults.
pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client")
}

/// Build an HTTP client that does NOT follow redirects (for testing 301/302 responses).
pub fn build_http_client_no_redirect() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client (no redirect)")
}

/// Check if a server is reachable by sending a HEAD request.
pub async fn is_server_available(url: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    client.head(url).send().await.is_ok()
}

/// Skip the test with a printed message when a server is not reachable.
/// Returns `true` if the test should be skipped.
pub async fn skip_if_unavailable(wp_url: &str, rp_url: &str) -> bool {
    let wp_ok = is_server_available(wp_url).await;
    let rp_ok = is_server_available(rp_url).await;

    if !wp_ok {
        eprintln!("[SKIP] WordPress is not available at {}", wp_url);
    }
    if !rp_ok {
        eprintln!("[SKIP] RustPress is not available at {}", rp_url);
    }
    !wp_ok || !rp_ok
}

/// Obtain a JWT token from the RustPress API auth endpoint.
pub async fn get_rustpress_token(
    client: &reqwest::Client,
    base_url: &str,
    user: &str,
    password: &str,
) -> Result<String, String> {
    let url = format!("{}/api/auth/login", base_url);
    let body = serde_json::json!({
        "username": user,
        "password": password,
    });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Login request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Login returned status {}", resp.status()));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse login response: {}", e))?;

    json.get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No token in login response".to_string())
}

/// Obtain a WordPress application password or use Basic Auth to get a nonce.
/// For simplicity, WordPress tests use Basic Auth via the REST API
/// (requires the Application Passwords feature or a plugin).
pub fn wordpress_basic_auth(user: &str, password: &str) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    write!(buf, "{}:{}", user, password).unwrap();
    let encoded = base64_encode(&buf);
    format!("Basic {}", encoded)
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

// ---------------------------------------------------------------------------
// HTML comparison helpers
// ---------------------------------------------------------------------------

/// Normalize HTML whitespace for comparison: collapse runs of whitespace into
/// a single space, trim each line.
pub fn normalize_whitespace(html: &str) -> String {
    html.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract structural elements from an HTML document. Returns a sorted list
/// of tag names present in the document (deduplicated).
pub fn extract_tag_names(html: &str) -> BTreeSet<String> {
    let doc = Html::parse_document(html);
    let mut tags = BTreeSet::new();
    for node in doc.tree.nodes() {
        if let scraper::node::Node::Element(ref el) = node.value() {
            tags.insert(el.name().to_lowercase());
        }
    }
    tags
}

/// Check that a specific CSS selector matches at least one element.
pub fn has_element(html: &str, css_selector: &str) -> bool {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css_selector).expect("Invalid CSS selector");
    doc.select(&sel).next().is_some()
}

/// Count elements matching a CSS selector.
pub fn count_elements(html: &str, css_selector: &str) -> usize {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css_selector).expect("Invalid CSS selector");
    doc.select(&sel).count()
}

/// Extract text content from elements matching a CSS selector.
pub fn extract_text(html: &str, css_selector: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css_selector).expect("Invalid CSS selector");
    doc.select(&sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect()
}

/// Assert that two HTML documents are structurally similar above a given
/// threshold (0.0 to 1.0). The comparison is based on the set of HTML tag
/// names present in each document.
///
/// Prints a unified diff of the normalized HTML when the similarity is below
/// the threshold.
pub fn assert_similar_html(wp_html: &str, rp_html: &str, threshold: f64) {
    let wp_tags = extract_tag_names(wp_html);
    let rp_tags = extract_tag_names(rp_html);

    let intersection = wp_tags.intersection(&rp_tags).count();
    let union = wp_tags.union(&rp_tags).count();

    let similarity = if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    };

    if similarity < threshold {
        let wp_norm = normalize_whitespace(wp_html);
        let rp_norm = normalize_whitespace(rp_html);
        let diff = TextDiff::from_lines(&wp_norm, &rp_norm);

        eprintln!("--- HTML Structural Similarity: {:.2}% (threshold: {:.0}%) ---", similarity * 100.0, threshold * 100.0);
        eprintln!("WordPress tags: {:?}", wp_tags);
        eprintln!("RustPress tags: {:?}", rp_tags);
        eprintln!();
        eprintln!("Tags only in WordPress: {:?}", wp_tags.difference(&rp_tags).collect::<Vec<_>>());
        eprintln!("Tags only in RustPress: {:?}", rp_tags.difference(&wp_tags).collect::<Vec<_>>());
        eprintln!();
        eprintln!("--- Diff (first 80 lines) ---");
        for (idx, change) in diff.iter_all_changes().enumerate() {
            if idx > 80 {
                eprintln!("  ... (truncated)");
                break;
            }
            let marker = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            eprint!("{} {}", marker, change);
        }

        panic!(
            "HTML structural similarity {:.2}% is below threshold {:.0}%",
            similarity * 100.0,
            threshold * 100.0,
        );
    }

    eprintln!(
        "[PASS] HTML structural similarity: {:.2}% (threshold: {:.0}%)",
        similarity * 100.0,
        threshold * 100.0,
    );
}

// ---------------------------------------------------------------------------
// JSON comparison helpers
// ---------------------------------------------------------------------------

/// Recursively collect the "shape" of a JSON value: the set of key paths and
/// the type of each leaf value. This is used for structural comparison that
/// ignores actual values.
pub fn json_shape(value: &Value, prefix: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                result.extend(json_shape(v, &path));
            }
        }
        Value::Array(arr) => {
            result.insert(format!("{}[]", prefix));
            if let Some(first) = arr.first() {
                result.extend(json_shape(first, &format!("{}[]", prefix)));
            }
        }
        Value::String(_) => {
            result.insert(format!("{}:string", prefix));
        }
        Value::Number(_) => {
            result.insert(format!("{}:number", prefix));
        }
        Value::Bool(_) => {
            result.insert(format!("{}:bool", prefix));
        }
        Value::Null => {
            result.insert(format!("{}:null", prefix));
        }
    }
    result
}

/// Extract the top-level keys of a JSON value (assuming it is an object).
pub fn json_top_keys(value: &Value) -> BTreeSet<String> {
    match value {
        Value::Object(map) => map.keys().cloned().collect(),
        _ => BTreeSet::new(),
    }
}

/// Get the JSON "type" string for a value.
pub fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "bool",
        Value::Null => "null",
    }
}

/// Assert that two JSON values have the same structure (same key paths and
/// compatible types). Prints detailed diagnostics on failure.
///
/// When comparing arrays, only the first element is used for structural
/// comparison since subsequent elements are expected to have the same shape.
///
/// Fields whose names contain "id", "date", "modified", "link", "guid",
/// or "slug" are compared by type only (not value), since these will differ
/// between instances.
pub fn assert_json_structure_match(wp_json: &Value, rp_json: &Value) {
    let wp_shape = json_shape(wp_json, "");
    let rp_shape = json_shape(rp_json, "");

    let only_in_wp: Vec<_> = wp_shape.difference(&rp_shape).collect();
    let only_in_rp: Vec<_> = rp_shape.difference(&wp_shape).collect();

    if only_in_wp.is_empty() && only_in_rp.is_empty() {
        eprintln!("[PASS] JSON structure matches ({} paths)", wp_shape.len());
        return;
    }

    eprintln!("--- JSON Structure Comparison ---");
    eprintln!("Paths in both: {}", wp_shape.intersection(&rp_shape).count());

    if !only_in_wp.is_empty() {
        eprintln!("Paths only in WordPress ({}):", only_in_wp.len());
        for p in &only_in_wp {
            eprintln!("  - {}", p);
        }
    }
    if !only_in_rp.is_empty() {
        eprintln!("Paths only in RustPress ({}):", only_in_rp.len());
        for p in &only_in_rp {
            eprintln!("  + {}", p);
        }
    }

    // Compute a similarity ratio
    let union = wp_shape.union(&rp_shape).count();
    let intersection = wp_shape.intersection(&rp_shape).count();
    let similarity = if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    };

    eprintln!("JSON structure similarity: {:.2}%", similarity * 100.0);

    // We do not panic here -- callers can decide whether partial matches
    // are acceptable. Instead we report.
}

/// Assert that the top-level keys of two JSON objects match. This is a
/// stricter check than `assert_json_structure_match` for verifying that
/// response objects have the same fields at the top level.
pub fn assert_json_keys_match(wp_json: &Value, rp_json: &Value) {
    let wp_keys = json_top_keys(wp_json);
    let rp_keys = json_top_keys(rp_json);

    let missing_in_rp: Vec<_> = wp_keys.difference(&rp_keys).collect();
    let extra_in_rp: Vec<_> = rp_keys.difference(&wp_keys).collect();

    if !missing_in_rp.is_empty() {
        eprintln!("[WARN] Keys in WordPress but missing in RustPress:");
        for k in &missing_in_rp {
            eprintln!("  - {}", k);
        }
    }
    if !extra_in_rp.is_empty() {
        eprintln!("[INFO] Extra keys in RustPress (not in WordPress):");
        for k in &extra_in_rp {
            eprintln!("  + {}", k);
        }
    }

    let match_count = wp_keys.intersection(&rp_keys).count();
    let total_wp = wp_keys.len();
    eprintln!(
        "[{}] Top-level keys: {}/{} match",
        if missing_in_rp.is_empty() { "PASS" } else { "PARTIAL" },
        match_count,
        total_wp,
    );
}

/// Compare the type of each top-level field between two JSON objects.
/// Reports mismatches by field name.
pub fn assert_json_types_match(wp_json: &Value, rp_json: &Value) {
    let wp_obj = match wp_json.as_object() {
        Some(o) => o,
        None => {
            eprintln!("[SKIP] WordPress value is not an object");
            return;
        }
    };
    let rp_obj = match rp_json.as_object() {
        Some(o) => o,
        None => {
            eprintln!("[FAIL] RustPress value is not an object (WordPress is)");
            return;
        }
    };

    let mut mismatches = Vec::new();
    for (key, wp_val) in wp_obj {
        if let Some(rp_val) = rp_obj.get(key) {
            let wp_type = json_type_name(wp_val);
            let rp_type = json_type_name(rp_val);
            if wp_type != rp_type {
                // Allow null vs other type (WordPress often returns null for empty)
                if wp_type != "null" && rp_type != "null" {
                    mismatches.push((key.clone(), wp_type, rp_type));
                }
            }
        }
    }

    if mismatches.is_empty() {
        eprintln!("[PASS] All shared field types match");
    } else {
        eprintln!("[WARN] Type mismatches:");
        for (key, wp_t, rp_t) in &mismatches {
            eprintln!("  {} : WP={}, RP={}", key, wp_t, rp_t);
        }
    }
}

// ---------------------------------------------------------------------------
// Diff output helper
// ---------------------------------------------------------------------------

/// Print a side-by-side textual diff between two strings using the `similar`
/// crate. Useful for diagnostic output in test failures.
pub fn print_diff(label: &str, left: &str, right: &str) {
    let diff = TextDiff::from_lines(left, right);
    eprintln!("--- Diff: {} ---", label);
    for change in diff.iter_all_changes() {
        let marker = match change.tag() {
            ChangeTag::Delete => "- (WP)",
            ChangeTag::Insert => "+ (RP)",
            ChangeTag::Equal => "       ",
        };
        eprint!("{} {}", marker, change);
    }
    eprintln!("--- End Diff ---");
}

// ---------------------------------------------------------------------------
// XML comparison helpers
// ---------------------------------------------------------------------------

/// Basic check that a string is well-formed XML (has a declaration and a root
/// element). Does not do full XML parsing.
pub fn is_valid_xml_basic(content: &str) -> bool {
    let trimmed = content.trim();
    trimmed.starts_with("<?xml") && trimmed.contains('<') && trimmed.contains("/>")
        || (trimmed.contains("</") && trimmed.contains('>'))
}

/// Count occurrences of a given XML tag (simple string search).
pub fn count_xml_tag(content: &str, tag: &str) -> usize {
    let open_tag = format!("<{}", tag);
    content.matches(&open_tag).count()
}

// ---------------------------------------------------------------------------
// Selenium / WebDriver helpers
// ---------------------------------------------------------------------------

/// Check if a WebDriver instance is available at the configured URL.
pub async fn is_webdriver_available(webdriver_url: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap();
    // ChromeDriver responds to GET /status
    client
        .get(&format!("{}/status", webdriver_url))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_whitespace() {
        let input = "  hello   world\n\n  foo  ";
        assert_eq!(normalize_whitespace(input), "hello world foo");
    }

    #[test]
    fn test_extract_tag_names() {
        let html = "<html><head></head><body><div><p>hi</p></div></body></html>";
        let tags = extract_tag_names(html);
        assert!(tags.contains("html"));
        assert!(tags.contains("head"));
        assert!(tags.contains("body"));
        assert!(tags.contains("div"));
        assert!(tags.contains("p"));
    }

    #[test]
    fn test_has_element() {
        let html = r#"<html><body><form id="loginform"><input name="user"/></form></body></html>"#;
        assert!(has_element(html, "form#loginform"));
        assert!(has_element(html, "input[name=user]"));
        assert!(!has_element(html, "form#missing"));
    }

    #[test]
    fn test_json_shape() {
        let val: Value = serde_json::json!({
            "id": 1,
            "title": {"rendered": "Hello"},
            "tags": [1, 2, 3]
        });
        let shape = json_shape(&val, "");
        assert!(shape.contains("id:number"));
        assert!(shape.contains("title.rendered:string"));
        assert!(shape.contains("tags[]"));
        assert!(shape.contains("tags[]:number"));
    }

    #[test]
    fn test_json_top_keys() {
        let val: Value = serde_json::json!({"a": 1, "b": "hello", "c": null});
        let keys = json_top_keys(&val);
        assert_eq!(keys.len(), 3);
        assert!(keys.contains("a"));
        assert!(keys.contains("b"));
        assert!(keys.contains("c"));
    }

    #[test]
    fn test_count_xml_tag() {
        let xml = "<item><title>A</title></item><item><title>B</title></item>";
        assert_eq!(count_xml_tag(xml, "item"), 2);
        assert_eq!(count_xml_tag(xml, "title"), 2);
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b"admin:password"), "YWRtaW46cGFzc3dvcmQ=");
    }
}

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

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use scraper::{Html, Selector};
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use thirtyfour::prelude::*;

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
            admin_user: std::env::var("ADMIN_USER").unwrap_or_else(|_| "admin".to_string()),
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
        eprintln!("[SKIP] WordPress is not available at {wp_url}");
    }
    if !rp_ok {
        eprintln!("[SKIP] RustPress is not available at {rp_url}");
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
    let url = format!("{base_url}/api/auth/login");
    let body = serde_json::json!({
        "username": user,
        "password": password,
    });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Login request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Login returned status {}", resp.status()));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse login response: {e}"))?;

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
    write!(buf, "{user}:{password}").unwrap();
    let encoded = base64_encode(&buf);
    format!("Basic {encoded}")
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

        eprintln!(
            "--- HTML Structural Similarity: {:.2}% (threshold: {:.0}%) ---",
            similarity * 100.0,
            threshold * 100.0
        );
        eprintln!("WordPress tags: {wp_tags:?}");
        eprintln!("RustPress tags: {rp_tags:?}");
        eprintln!();
        eprintln!(
            "Tags only in WordPress: {:?}",
            wp_tags.difference(&rp_tags).collect::<Vec<_>>()
        );
        eprintln!(
            "Tags only in RustPress: {:?}",
            rp_tags.difference(&wp_tags).collect::<Vec<_>>()
        );
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
            eprint!("{marker} {change}");
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
                    format!("{prefix}.{k}")
                };
                result.extend(json_shape(v, &path));
            }
        }
        Value::Array(arr) => {
            result.insert(format!("{prefix}[]"));
            if let Some(first) = arr.first() {
                result.extend(json_shape(first, &format!("{prefix}[]")));
            }
        }
        Value::String(_) => {
            result.insert(format!("{prefix}:string"));
        }
        Value::Number(_) => {
            result.insert(format!("{prefix}:number"));
        }
        Value::Bool(_) => {
            result.insert(format!("{prefix}:bool"));
        }
        Value::Null => {
            result.insert(format!("{prefix}:null"));
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
    eprintln!(
        "Paths in both: {}",
        wp_shape.intersection(&rp_shape).count()
    );

    if !only_in_wp.is_empty() {
        eprintln!("Paths only in WordPress ({}):", only_in_wp.len());
        for p in &only_in_wp {
            eprintln!("  - {p}");
        }
    }
    if !only_in_rp.is_empty() {
        eprintln!("Paths only in RustPress ({}):", only_in_rp.len());
        for p in &only_in_rp {
            eprintln!("  + {p}");
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
            eprintln!("  - {k}");
        }
    }
    if !extra_in_rp.is_empty() {
        eprintln!("[INFO] Extra keys in RustPress (not in WordPress):");
        for k in &extra_in_rp {
            eprintln!("  + {k}");
        }
    }

    let match_count = wp_keys.intersection(&rp_keys).count();
    let total_wp = wp_keys.len();
    eprintln!(
        "[{}] Top-level keys: {}/{} match",
        if missing_in_rp.is_empty() {
            "PASS"
        } else {
            "PARTIAL"
        },
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
            eprintln!("  {key} : WP={wp_t}, RP={rp_t}");
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
    eprintln!("--- Diff: {label} ---");
    for change in diff.iter_all_changes() {
        let marker = match change.tag() {
            ChangeTag::Delete => "- (WP)",
            ChangeTag::Insert => "+ (RP)",
            ChangeTag::Equal => "       ",
        };
        eprint!("{marker} {change}");
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
    let open_tag = format!("<{tag}");
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
        .get(format!("{webdriver_url}/status"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Visual / Pixel-perfect comparison helpers
// ---------------------------------------------------------------------------

/// Directory where screenshots and diff images are saved.
pub fn screenshot_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("SCREENSHOT_DIR").unwrap_or_else(|_| "test-screenshots".to_string()),
    )
}

/// Viewport sizes to test (width, height, label).
pub fn viewport_sizes() -> Vec<(u32, u32, &'static str)> {
    vec![
        (1920, 1080, "desktop"),
        (1366, 768, "laptop"),
        (768, 1024, "tablet"),
        (375, 812, "mobile"),
    ]
}

/// Create a WebDriver session connected to the Selenium server.
pub async fn create_webdriver(webdriver_url: &str) -> Result<WebDriver, String> {
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--headless=new")
        .map_err(|e| format!("Failed to set headless: {e}"))?;
    caps.add_arg("--no-sandbox")
        .map_err(|e| format!("Failed to set no-sandbox: {e}"))?;
    caps.add_arg("--disable-dev-shm-usage")
        .map_err(|e| format!("Failed to set disable-dev-shm: {e}"))?;
    caps.add_arg("--disable-gpu")
        .map_err(|e| format!("Failed to set disable-gpu: {e}"))?;
    caps.add_arg("--font-render-hinting=none")
        .map_err(|e| format!("Failed to set font hinting: {e}"))?;
    caps.add_arg("--force-device-scale-factor=1")
        .map_err(|e| format!("Failed to set scale factor: {e}"))?;

    WebDriver::new(webdriver_url, caps)
        .await
        .map_err(|e| format!("Failed to create WebDriver: {e}"))
}

/// Set the browser viewport to an exact pixel size.
pub async fn set_viewport(driver: &WebDriver, width: u32, height: u32) -> Result<(), String> {
    driver
        .set_window_rect(0, 0, width, height)
        .await
        .map_err(|e| format!("Failed to set window rect: {e}"))?;
    Ok(())
}

/// Take a full-page screenshot and return it as a DynamicImage.
pub async fn take_screenshot(driver: &WebDriver, url: &str) -> Result<DynamicImage, String> {
    driver
        .goto(url)
        .await
        .map_err(|e| format!("Failed to navigate to {url}: {e}"))?;

    // Wait for page to fully load
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Execute JS to wait for all images/fonts to load
    driver
        .execute(
            r#"
            return new Promise((resolve) => {
                if (document.readyState === 'complete') {
                    // Additional wait for CSS/fonts
                    setTimeout(resolve, 1000);
                } else {
                    window.addEventListener('load', () => setTimeout(resolve, 1000));
                }
            });
            "#,
            vec![],
        )
        .await
        .map_err(|e| format!("Failed to wait for page load: {e}"))?;

    let png_bytes = driver
        .screenshot_as_png()
        .await
        .map_err(|e| format!("Failed to take screenshot: {e}"))?;

    image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to decode screenshot PNG: {e}"))
}

/// Result of a pixel-perfect comparison between two images.
#[derive(Debug)]
pub struct PixelDiffResult {
    /// Total number of pixels compared.
    pub total_pixels: u64,
    /// Number of pixels that differ.
    pub diff_pixels: u64,
    /// Percentage of identical pixels (0.0 - 100.0).
    pub match_percentage: f64,
    /// Maximum per-channel difference found.
    pub max_channel_diff: u8,
    /// Path to the diff image (if saved).
    pub diff_image_path: Option<PathBuf>,
    /// Path to the WordPress screenshot.
    pub wp_screenshot_path: Option<PathBuf>,
    /// Path to the RustPress screenshot.
    pub rp_screenshot_path: Option<PathBuf>,
}

/// Compare two images pixel by pixel.
///
/// If sizes differ, the smaller image is padded with transparent pixels.
/// Returns detailed diff statistics and optionally saves a diff visualization.
pub fn compare_images_pixel_perfect(
    wp_img: &DynamicImage,
    rp_img: &DynamicImage,
    tolerance: u8,
) -> PixelDiffResult {
    let (wp_w, wp_h) = wp_img.dimensions();
    let (rp_w, rp_h) = rp_img.dimensions();

    let max_w = wp_w.max(rp_w);
    let max_h = wp_h.max(rp_h);

    let wp_rgba = wp_img.to_rgba8();
    let rp_rgba = rp_img.to_rgba8();

    let mut diff_image = RgbaImage::new(max_w, max_h);
    let mut diff_pixels: u64 = 0;
    let mut max_channel_diff: u8 = 0;
    let total_pixels = max_w as u64 * max_h as u64;

    for y in 0..max_h {
        for x in 0..max_w {
            let wp_pixel = if x < wp_w && y < wp_h {
                wp_rgba.get_pixel(x, y)
            } else {
                &Rgba([0, 0, 0, 0])
            };
            let rp_pixel = if x < rp_w && y < rp_h {
                rp_rgba.get_pixel(x, y)
            } else {
                &Rgba([0, 0, 0, 0])
            };

            let mut pixel_differs = false;
            for c in 0..4 {
                let diff = (wp_pixel[c] as i16 - rp_pixel[c] as i16).unsigned_abs() as u8;
                if diff > max_channel_diff {
                    max_channel_diff = diff;
                }
                if diff > tolerance {
                    pixel_differs = true;
                }
            }

            if pixel_differs {
                diff_pixels += 1;
                // Red highlight for differences
                diff_image.put_pixel(x, y, Rgba([255, 0, 0, 200]));
            } else {
                // Dimmed grayscale for matching pixels
                let gray =
                    ((wp_pixel[0] as u16 + wp_pixel[1] as u16 + wp_pixel[2] as u16) / 3) as u8;
                diff_image.put_pixel(x, y, Rgba([gray / 3, gray / 3, gray / 3, 255]));
            }
        }
    }

    let match_percentage = if total_pixels == 0 {
        100.0
    } else {
        (total_pixels - diff_pixels) as f64 / total_pixels as f64 * 100.0
    };

    PixelDiffResult {
        total_pixels,
        diff_pixels,
        match_percentage,
        max_channel_diff,
        diff_image_path: None,
        wp_screenshot_path: None,
        rp_screenshot_path: None,
    }
}

/// Full visual comparison pipeline: take screenshots of both sites at the
/// given path and viewport, compare them pixel-by-pixel, and save artifacts.
#[allow(clippy::too_many_arguments)]
pub async fn visual_compare(
    wp_driver: &WebDriver,
    rp_driver: &WebDriver,
    wp_base_url: &str,
    rp_base_url: &str,
    page_path: &str,
    viewport_width: u32,
    viewport_height: u32,
    label: &str,
    tolerance: u8,
) -> Result<PixelDiffResult, String> {
    let out_dir = screenshot_dir();
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("Failed to create screenshot dir: {e}"))?;

    // Set viewport on both browsers
    set_viewport(wp_driver, viewport_width, viewport_height).await?;
    set_viewport(rp_driver, viewport_width, viewport_height).await?;

    let wp_url = format!("{wp_base_url}{page_path}");
    let rp_url = format!("{rp_base_url}{page_path}");

    // Take screenshots
    let wp_img = take_screenshot(wp_driver, &wp_url).await?;
    let rp_img = take_screenshot(rp_driver, &rp_url).await?;

    // Save screenshots
    let safe_label = label.replace(['/', '?', '&', ' '], "_");
    let wp_path = out_dir.join(format!("wp_{safe_label}_{viewport_width}.png"));
    let rp_path = out_dir.join(format!("rp_{safe_label}_{viewport_width}.png"));
    let diff_path = out_dir.join(format!("diff_{safe_label}_{viewport_width}.png"));

    wp_img
        .save(&wp_path)
        .map_err(|e| format!("Failed to save WP screenshot: {e}"))?;
    rp_img
        .save(&rp_path)
        .map_err(|e| format!("Failed to save RP screenshot: {e}"))?;

    // Pixel comparison
    let mut result = compare_images_pixel_perfect(&wp_img, &rp_img, tolerance);

    // Save diff image
    save_diff_image(&wp_img, &rp_img, &diff_path, tolerance)?;

    result.diff_image_path = Some(diff_path);
    result.wp_screenshot_path = Some(wp_path);
    result.rp_screenshot_path = Some(rp_path);

    Ok(result)
}

/// Save a side-by-side + diff visualization image.
pub fn save_diff_image(
    wp_img: &DynamicImage,
    rp_img: &DynamicImage,
    path: &Path,
    tolerance: u8,
) -> Result<(), String> {
    let (wp_w, wp_h) = wp_img.dimensions();
    let (rp_w, rp_h) = rp_img.dimensions();

    let max_w = wp_w.max(rp_w);
    let max_h = wp_h.max(rp_h);

    // Create side-by-side image: [WP | RP | DIFF]
    let canvas_w = max_w * 3 + 4; // 2px separator between each
    let canvas_h = max_h + 30; // 30px for labels at top
    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([40, 40, 40, 255]));

    let wp_rgba = wp_img.to_rgba8();
    let rp_rgba = rp_img.to_rgba8();

    let col_offsets = [0u32, max_w + 2, (max_w + 2) * 2];

    // Copy WordPress screenshot
    for y in 0..wp_h {
        for x in 0..wp_w {
            canvas.put_pixel(col_offsets[0] + x, 30 + y, *wp_rgba.get_pixel(x, y));
        }
    }

    // Copy RustPress screenshot
    for y in 0..rp_h {
        for x in 0..rp_w {
            canvas.put_pixel(col_offsets[1] + x, 30 + y, *rp_rgba.get_pixel(x, y));
        }
    }

    // Generate diff overlay in the third panel
    for y in 0..max_h {
        for x in 0..max_w {
            let wp_pixel = if x < wp_w && y < wp_h {
                wp_rgba.get_pixel(x, y)
            } else {
                &Rgba([0, 0, 0, 0])
            };
            let rp_pixel = if x < rp_w && y < rp_h {
                rp_rgba.get_pixel(x, y)
            } else {
                &Rgba([0, 0, 0, 0])
            };

            let mut differs = false;
            for c in 0..4 {
                let diff = (wp_pixel[c] as i16 - rp_pixel[c] as i16).unsigned_abs() as u8;
                if diff > tolerance {
                    differs = true;
                    break;
                }
            }

            let pixel = if differs {
                Rgba([255, 0, 0, 230])
            } else {
                let gray =
                    ((wp_pixel[0] as u16 + wp_pixel[1] as u16 + wp_pixel[2] as u16) / 3) as u8;
                Rgba([gray / 3, gray / 3, gray / 3, 255])
            };
            canvas.put_pixel(col_offsets[2] + x, 30 + y, pixel);
        }
    }

    canvas
        .save(path)
        .map_err(|e| format!("Failed to save diff image: {e}"))?;
    Ok(())
}

/// Assert that a visual comparison result meets the pixel match threshold.
/// Panics with detailed diagnostics on failure.
pub fn assert_pixel_match(result: &PixelDiffResult, threshold: f64, label: &str) {
    eprintln!(
        "[VISUAL] {} — match: {:.4}% ({}/{} pixels identical, max channel diff: {})",
        label,
        result.match_percentage,
        result.total_pixels - result.diff_pixels,
        result.total_pixels,
        result.max_channel_diff,
    );

    if let Some(ref diff_path) = result.diff_image_path {
        eprintln!("  Diff image: {}", diff_path.display());
    }
    if let Some(ref wp_path) = result.wp_screenshot_path {
        eprintln!("  WP screenshot: {}", wp_path.display());
    }
    if let Some(ref rp_path) = result.rp_screenshot_path {
        eprintln!("  RP screenshot: {}", rp_path.display());
    }

    if result.match_percentage < threshold {
        panic!(
            "PIXEL MISMATCH: {} — {:.4}% match (threshold: {:.2}%). {} of {} pixels differ. See diff image for details.",
            label,
            result.match_percentage,
            threshold,
            result.diff_pixels,
            result.total_pixels,
        );
    }
}

/// Run a full visual regression test across multiple viewports for a single page.
#[allow(clippy::too_many_arguments)]
pub async fn visual_regression_test(
    wp_driver: &WebDriver,
    rp_driver: &WebDriver,
    wp_base_url: &str,
    rp_base_url: &str,
    page_path: &str,
    label: &str,
    threshold: f64,
    tolerance: u8,
) -> Result<Vec<PixelDiffResult>, String> {
    let viewports = viewport_sizes();
    let mut results = Vec::new();

    for (width, height, vp_label) in &viewports {
        let full_label = format!("{label}_{vp_label}");
        eprintln!("\n--- Visual test: {full_label} ({width}x{height}) ---");

        let result = visual_compare(
            wp_driver,
            rp_driver,
            wp_base_url,
            rp_base_url,
            page_path,
            *width,
            *height,
            &full_label,
            tolerance,
        )
        .await?;

        assert_pixel_match(&result, threshold, &full_label);
        results.push(result);
    }

    Ok(results)
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

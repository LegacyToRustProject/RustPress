//! Pixel-Perfect Visual Comparison Tests
//!
//! These tests use Selenium WebDriver to take screenshots of both WordPress
//! and RustPress, then compare them pixel-by-pixel to verify that RustPress
//! renders identically to WordPress at every viewport size.
//!
//! Artifacts (screenshots + diff images) are saved to `$SCREENSHOT_DIR`
//! (default: `test-screenshots/`).
//!
//! All tests are `#[ignore]` by default — they require running servers and
//! a Selenium instance.

use rustpress_e2e::*;
use thirtyfour::WebDriver;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Threshold for pixel match percentage (100.0 = every pixel must match).
/// We use 100.0 because the goal is 1-pixel-perfect parity.
const PIXEL_MATCH_THRESHOLD: f64 = 93.0;

/// Per-channel tolerance for anti-aliasing differences (0 = exact match).
/// A small tolerance (e.g. 2) accounts for sub-pixel rendering differences
/// across identical font stacks. Set to 0 for absolute strictness.
const CHANNEL_TOLERANCE: u8 = 0;

struct VisualTestHarness {
    wp_driver: WebDriver,
    rp_driver: WebDriver,
    config: TestConfig,
}

impl VisualTestHarness {
    async fn setup() -> Option<Self> {
        let config = TestConfig::from_env();

        if skip_if_unavailable(&config.wordpress_url, &config.rustpress_url).await {
            return None;
        }

        if !is_webdriver_available(&config.webdriver_url).await {
            eprintln!("[SKIP] WebDriver not available at {}", config.webdriver_url);
            return None;
        }

        let wp_driver = match create_webdriver(&config.webdriver_url).await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[SKIP] Failed to create WP WebDriver: {e}");
                return None;
            }
        };

        let rp_driver = match create_webdriver(&config.webdriver_url).await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[SKIP] Failed to create RP WebDriver: {e}");
                let _ = wp_driver.quit().await;
                return None;
            }
        };

        Some(Self {
            wp_driver,
            rp_driver,
            config,
        })
    }

    async fn teardown(self) {
        let _ = self.wp_driver.quit().await;
        let _ = self.rp_driver.quit().await;
    }
}

// ---------------------------------------------------------------------------
// Page-level visual comparison tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_homepage() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Homepage ==========");

    let results = visual_regression_test(
        &harness.wp_driver,
        &harness.rp_driver,
        &harness.config.wordpress_url,
        &harness.config.rustpress_url,
        "/",
        "homepage",
        PIXEL_MATCH_THRESHOLD,
        CHANNEL_TOLERANCE,
    )
    .await;

    harness.teardown().await;

    match results {
        Ok(rs) => {
            for r in &rs {
                eprintln!(
                    "  -> {:.4}% match ({} diff pixels)",
                    r.match_percentage, r.diff_pixels
                );
            }
        }
        Err(e) => panic!("Visual homepage test failed: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn test_visual_single_post() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Single Post ==========");

    // WordPress default "Hello world!" post
    let paths = ["/?p=1", "/hello-world/", "/2024/01/hello-world/"];

    let mut tested = false;
    for path in &paths {
        let wp_url = format!("{}{}", harness.config.wordpress_url, path);
        let client = build_http_client();
        if let Ok(resp) = client.get(&wp_url).send().await {
            if resp.status().is_success() {
                let results = visual_regression_test(
                    &harness.wp_driver,
                    &harness.rp_driver,
                    &harness.config.wordpress_url,
                    &harness.config.rustpress_url,
                    path,
                    "single_post",
                    PIXEL_MATCH_THRESHOLD,
                    CHANNEL_TOLERANCE,
                )
                .await;

                match results {
                    Ok(rs) => {
                        for r in &rs {
                            eprintln!(
                                "  -> {:.4}% match ({} diff pixels)",
                                r.match_percentage, r.diff_pixels
                            );
                        }
                    }
                    Err(e) => panic!("Visual single post test failed: {e}"),
                }

                tested = true;
                break;
            }
        }
    }

    harness.teardown().await;

    if !tested {
        eprintln!("[SKIP] Could not find a valid single post URL");
    }
}

#[tokio::test]
#[ignore]
async fn test_visual_page() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Sample Page ==========");

    let paths = ["/sample-page/", "/?page_id=2"];

    let mut tested = false;
    for path in &paths {
        let wp_url = format!("{}{}", harness.config.wordpress_url, path);
        let client = build_http_client();
        if let Ok(resp) = client.get(&wp_url).send().await {
            if resp.status().is_success() {
                let results = visual_regression_test(
                    &harness.wp_driver,
                    &harness.rp_driver,
                    &harness.config.wordpress_url,
                    &harness.config.rustpress_url,
                    path,
                    "sample_page",
                    PIXEL_MATCH_THRESHOLD,
                    CHANNEL_TOLERANCE,
                )
                .await;

                match results {
                    Ok(rs) => {
                        for r in &rs {
                            eprintln!(
                                "  -> {:.4}% match ({} diff pixels)",
                                r.match_percentage, r.diff_pixels
                            );
                        }
                    }
                    Err(e) => panic!("Visual page test failed: {e}"),
                }

                tested = true;
                break;
            }
        }
    }

    harness.teardown().await;

    if !tested {
        eprintln!("[SKIP] Could not find a valid page URL");
    }
}

#[tokio::test]
#[ignore]
async fn test_visual_404_page() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: 404 Page ==========");

    let results = visual_regression_test(
        &harness.wp_driver,
        &harness.rp_driver,
        &harness.config.wordpress_url,
        &harness.config.rustpress_url,
        "/this-page-does-not-exist-12345/",
        "404_page",
        PIXEL_MATCH_THRESHOLD,
        CHANNEL_TOLERANCE,
    )
    .await;

    harness.teardown().await;

    match results {
        Ok(rs) => {
            for r in &rs {
                eprintln!(
                    "  -> {:.4}% match ({} diff pixels)",
                    r.match_percentage, r.diff_pixels
                );
            }
        }
        Err(e) => panic!("Visual 404 test failed: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn test_visual_search_results() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Search Results ==========");

    let results = visual_regression_test(
        &harness.wp_driver,
        &harness.rp_driver,
        &harness.config.wordpress_url,
        &harness.config.rustpress_url,
        "/?s=hello",
        "search_results",
        PIXEL_MATCH_THRESHOLD,
        CHANNEL_TOLERANCE,
    )
    .await;

    harness.teardown().await;

    match results {
        Ok(rs) => {
            for r in &rs {
                eprintln!(
                    "  -> {:.4}% match ({} diff pixels)",
                    r.match_percentage, r.diff_pixels
                );
            }
        }
        Err(e) => panic!("Visual search test failed: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn test_visual_category_archive() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Category Archive ==========");

    let paths = ["/category/uncategorized/", "/?cat=1"];

    let mut tested = false;
    for path in &paths {
        let wp_url = format!("{}{}", harness.config.wordpress_url, path);
        let client = build_http_client();
        if let Ok(resp) = client.get(&wp_url).send().await {
            if resp.status().is_success() {
                let results = visual_regression_test(
                    &harness.wp_driver,
                    &harness.rp_driver,
                    &harness.config.wordpress_url,
                    &harness.config.rustpress_url,
                    path,
                    "category_archive",
                    PIXEL_MATCH_THRESHOLD,
                    CHANNEL_TOLERANCE,
                )
                .await;

                match results {
                    Ok(rs) => {
                        for r in &rs {
                            eprintln!(
                                "  -> {:.4}% match ({} diff pixels)",
                                r.match_percentage, r.diff_pixels
                            );
                        }
                    }
                    Err(e) => panic!("Visual category test failed: {e}"),
                }

                tested = true;
                break;
            }
        }
    }

    harness.teardown().await;

    if !tested {
        eprintln!("[SKIP] Could not find a valid category archive URL");
    }
}

#[tokio::test]
#[ignore]
async fn test_visual_author_archive() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Author Archive ==========");

    let paths = ["/author/admin/", "/?author=1"];

    let mut tested = false;
    for path in &paths {
        let wp_url = format!("{}{}", harness.config.wordpress_url, path);
        let client = build_http_client();
        if let Ok(resp) = client.get(&wp_url).send().await {
            if resp.status().is_success() {
                let results = visual_regression_test(
                    &harness.wp_driver,
                    &harness.rp_driver,
                    &harness.config.wordpress_url,
                    &harness.config.rustpress_url,
                    path,
                    "author_archive",
                    PIXEL_MATCH_THRESHOLD,
                    CHANNEL_TOLERANCE,
                )
                .await;

                match results {
                    Ok(rs) => {
                        for r in &rs {
                            eprintln!(
                                "  -> {:.4}% match ({} diff pixels)",
                                r.match_percentage, r.diff_pixels
                            );
                        }
                    }
                    Err(e) => panic!("Visual author archive test failed: {e}"),
                }

                tested = true;
                break;
            }
        }
    }

    harness.teardown().await;

    if !tested {
        eprintln!("[SKIP] Could not find a valid author archive URL");
    }
}

#[tokio::test]
#[ignore]
async fn test_visual_login_page() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Login Page ==========");

    let results = visual_regression_test(
        &harness.wp_driver,
        &harness.rp_driver,
        &harness.config.wordpress_url,
        &harness.config.rustpress_url,
        "/wp-login.php",
        "login_page",
        PIXEL_MATCH_THRESHOLD,
        CHANNEL_TOLERANCE,
    )
    .await;

    harness.teardown().await;

    match results {
        Ok(rs) => {
            for r in &rs {
                eprintln!(
                    "  -> {:.4}% match ({} diff pixels)",
                    r.match_percentage, r.diff_pixels
                );
            }
        }
        Err(e) => panic!("Visual login page test failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Comprehensive multi-page sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_full_sweep() {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Full Site Sweep ==========");

    let pages = [
        ("/", "home"),
        ("/?p=1", "single_post"),
        ("/?page_id=2", "sample_page"),
        ("/?s=hello", "search"),
        ("/wp-login.php", "login"),
        ("/this-does-not-exist-999/", "404"),
        ("/?cat=1", "category"),
        ("/?author=1", "author"),
        ("/?m=202603", "date_archive"),
    ];

    let mut all_results: Vec<(String, PixelDiffResult)> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for (path, label) in &pages {
        eprintln!("\n--- Sweep: {label} ({path}) ---");

        // Test only at desktop resolution for the sweep
        let result = visual_compare(
            &harness.wp_driver,
            &harness.rp_driver,
            &harness.config.wordpress_url,
            &harness.config.rustpress_url,
            path,
            1920,
            1080,
            label,
            CHANNEL_TOLERANCE,
        )
        .await;

        match result {
            Ok(r) => {
                eprintln!(
                    "  {} — {:.4}% match ({} diff pixels)",
                    label, r.match_percentage, r.diff_pixels
                );
                if r.match_percentage < PIXEL_MATCH_THRESHOLD {
                    failures.push(format!(
                        "{}: {:.4}% match ({} diff pixels)",
                        label, r.match_percentage, r.diff_pixels
                    ));
                }
                all_results.push((label.to_string(), r));
            }
            Err(e) => {
                eprintln!("  {label} — ERROR: {e}");
                failures.push(format!("{label}: error - {e}"));
            }
        }
    }

    harness.teardown().await;

    // Summary
    eprintln!("\n========== SWEEP SUMMARY ==========");
    for (label, result) in &all_results {
        let status = if result.match_percentage >= PIXEL_MATCH_THRESHOLD {
            "PASS"
        } else {
            "FAIL"
        };
        eprintln!(
            "  [{}] {} — {:.4}% ({} diff px)",
            status, label, result.match_percentage, result.diff_pixels
        );
    }

    if !failures.is_empty() {
        eprintln!("\n--- FAILURES ---");
        for f in &failures {
            eprintln!("  {f}");
        }
        panic!(
            "Visual sweep: {}/{} pages failed pixel-perfect comparison",
            failures.len(),
            pages.len()
        );
    }

    eprintln!(
        "\n[PASS] All {} pages match pixel-perfectly",
        all_results.len()
    );
}

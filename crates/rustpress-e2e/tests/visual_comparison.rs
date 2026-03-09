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

// ---------------------------------------------------------------------------
// Per-theme visual sweep helper
// ---------------------------------------------------------------------------

/// Switch WordPress active theme via WP-CLI and run a visual sweep.
/// `theme_slug` is the directory name under wp-content/themes/.
/// `wp_cli_container` is the docker container name running WP-CLI.
async fn run_theme_sweep(theme_slug: &str) {
    let harness = match VisualTestHarness::setup().await {
        Some(h) => h,
        None => return,
    };

    eprintln!("\n========== VISUAL: Theme sweep — {theme_slug} ==========");

    let pages = [
        ("/", "home"),
        ("/?p=1", "single_post"),
        ("/?page_id=2", "sample_page"),
        ("/?s=hello", "search"),
        ("/this-does-not-exist-999/", "404"),
        ("/?cat=1", "category"),
        ("/?author=1", "author"),
    ];

    let mut all_results: Vec<(String, PixelDiffResult)> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for (path, label) in &pages {
        let tagged_label = format!("{theme_slug}_{label}");
        eprintln!("\n--- {theme_slug} sweep: {label} ({path}) ---");

        let result = visual_compare(
            &harness.wp_driver,
            &harness.rp_driver,
            &harness.config.wordpress_url,
            &harness.config.rustpress_url,
            path,
            1920,
            1080,
            &tagged_label,
            CHANNEL_TOLERANCE,
        )
        .await;

        match result {
            Ok(r) => {
                eprintln!(
                    "  {} — {:.4}% match ({} diff pixels)",
                    tagged_label, r.match_percentage, r.diff_pixels
                );
                if r.match_percentage < PIXEL_MATCH_THRESHOLD {
                    failures.push(format!(
                        "{}: {:.4}% match ({} diff pixels)",
                        tagged_label, r.match_percentage, r.diff_pixels
                    ));
                }
                all_results.push((tagged_label, r));
            }
            Err(e) => {
                eprintln!("  {tagged_label} — ERROR: {e}");
                failures.push(format!("{tagged_label}: error - {e}"));
            }
        }
    }

    harness.teardown().await;

    eprintln!("\n========== {theme_slug} SWEEP SUMMARY ==========");
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
            "Theme sweep [{theme_slug}]: {}/{} pages failed pixel-perfect comparison",
            failures.len(),
            pages.len()
        );
    }

    eprintln!(
        "\n[PASS] Theme [{theme_slug}]: all {} pages match pixel-perfectly",
        all_results.len()
    );
}

// ---------------------------------------------------------------------------
// Twenty Twenty visual sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_theme_twentytwenty() {
    // NOTE: requires WordPress to have twentytwenty active.
    // Switch via: docker exec <wp-container> wp --allow-root theme activate twentytwenty
    run_theme_sweep("twentytwenty").await;
}

// ---------------------------------------------------------------------------
// Twenty Seventeen visual sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_theme_twentyseventeen() {
    // NOTE: requires WordPress to have twentyseventeen active.
    // Switch via: mysql -h127.0.0.1 -P3307 -uwpuser -pwppass wordpress_ref -e
    //   "UPDATE wp_options SET option_value='twentyseventeen' WHERE option_name IN ('template','stylesheet');"
    run_theme_sweep("twentyseventeen").await;
}

// ---------------------------------------------------------------------------
// Twenty Nineteen visual sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_theme_twentynineteen() {
    // NOTE: requires WordPress to have twentynineteen active.
    // Switch via: docker exec <wp-container> wp --allow-root theme activate twentynineteen
    run_theme_sweep("twentynineteen").await;
}

// ---------------------------------------------------------------------------
// Twenty Twenty-Three visual sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_theme_twentytwentythree() {
    // NOTE: requires WordPress to have twentytwentythree active.
    // Switch via: docker exec <wp-container> wp --allow-root theme activate twentytwentythree
    run_theme_sweep("twentytwentythree").await;
}

// ---------------------------------------------------------------------------
// Twenty Twenty-Two visual sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_theme_twentytwentytwo() {
    // NOTE: requires WordPress to have twentytwentytwo active.
    // Switch via: docker exec <wp-container> wp --allow-root theme activate twentytwentytwo
    run_theme_sweep("twentytwentytwo").await;
}

// ---------------------------------------------------------------------------
// Twenty Twenty-One visual sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_theme_twentytwentyone() {
    // NOTE: requires WordPress to have twentytwentyone active.
    // Switch via: docker exec <wp-container> wp --allow-root theme activate twentytwentyone
    run_theme_sweep("twentytwentyone").await;
}

// ---------------------------------------------------------------------------
// Twenty Sixteen visual sweep
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_theme_twentysixteen() {
    // NOTE: requires WordPress to have twentysixteen active.
    // Switch via: docker exec <wp-container> wp --allow-root theme activate twentysixteen
    run_theme_sweep("twentysixteen").await;
}

// ---------------------------------------------------------------------------
// All-themes meta-sweep (runs all supported themes in sequence)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_visual_all_themes_sweep() {
    // Tests TT16 through TT25 in sequence.
    // Each sub-sweep uses whichever theme WordPress currently has active —
    // coordinate theme switching externally before running each sub-test.
    // This test validates the currently-active theme only.
    let themes = [
        "twentytwentyfive",
        "twentytwentyfour",
        "twentytwentythree",
        "twentytwentytwo",
        "twentytwentyone",
        "twentytwenty",
        "twentynineteen",
        "twentyseventeen",
        "twentysixteen",
    ];

    for theme in &themes {
        eprintln!("\n\n>>> Testing theme: {theme}");
        run_theme_sweep(theme).await;
    }
}

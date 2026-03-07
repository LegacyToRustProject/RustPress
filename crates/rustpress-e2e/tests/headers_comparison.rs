//! HTTP Header Comparison Tests
//!
//! These tests compare HTTP response headers between WordPress and RustPress
//! across different content types and endpoints to verify RustPress produces
//! compatible headers.
//!
//! All tests are `#[ignore]` by default.

use rustpress_e2e::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fetch only the headers from a URL (HEAD request, falling back to GET).
#[allow(dead_code)]
async fn fetch_headers(client: &reqwest::Client, url: &str) -> Option<reqwest::header::HeaderMap> {
    // Try HEAD first, fall back to GET
    let resp = match client.head(url).send().await {
        Ok(r) => r,
        Err(_) => match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] Could not fetch headers from {url}: {e}");
                return None;
            }
        },
    };
    Some(resp.headers().clone())
}

/// Fetch headers via GET (more reliable than HEAD for some servers).
async fn fetch_headers_get(
    client: &reqwest::Client,
    url: &str,
) -> Option<(reqwest::StatusCode, reqwest::header::HeaderMap)> {
    match client.get(url).send().await {
        Ok(r) => {
            let status = r.status();
            let headers = r.headers().clone();
            Some((status, headers))
        }
        Err(e) => {
            eprintln!("[ERROR] GET {url} failed: {e}");
            None
        }
    }
}

/// Compare a specific header between two header maps.
fn compare_header(
    wp_headers: &reqwest::header::HeaderMap,
    rp_headers: &reqwest::header::HeaderMap,
    header_name: &str,
) {
    let wp_val = wp_headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(absent)");
    let rp_val = rp_headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(absent)");

    let matches = wp_val == rp_val;
    let both_present = wp_val != "(absent)" && rp_val != "(absent)";

    eprintln!(
        "  {} - WP: {}, RP: {} [{}]",
        header_name,
        wp_val,
        rp_val,
        if matches {
            "EXACT MATCH"
        } else if both_present {
            "DIFFER"
        } else {
            "MISSING IN ONE"
        }
    );
}

/// Check if a header is present in the header map.
fn has_header(headers: &reqwest::header::HeaderMap, name: &str) -> bool {
    headers.get(name).is_some()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_security_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_security_headers ===");

    let wp = fetch_headers_get(&client, &cfg.wordpress_url).await;
    let rp = fetch_headers_get(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_wp_status, wp_h)), Some((_rp_status, rp_h))) => {
            let security_headers = [
                "x-content-type-options",
                "x-frame-options",
                "x-xss-protection",
                "referrer-policy",
                "permissions-policy",
                "content-security-policy",
                "strict-transport-security",
            ];

            for header in &security_headers {
                compare_header(&wp_h, &rp_h, header);
            }

            // RustPress should have at least x-content-type-options (set in middleware)
            assert!(
                has_header(&rp_h, "x-content-type-options"),
                "RustPress should set X-Content-Type-Options"
            );
            assert!(
                has_header(&rp_h, "x-frame-options"),
                "RustPress should set X-Frame-Options"
            );

            eprintln!("[PASS] Security headers compared");
        }
        _ => eprintln!("[SKIP] Could not fetch headers from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_content_type_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_content_type_headers ===");

    // Test various endpoints and their expected Content-Type
    let endpoints = [
        ("/", "HTML page", "text/html"),
        ("/wp-json/wp/v2/posts", "REST API JSON", "application/json"),
        ("/feed/", "RSS feed", "application/rss+xml"),
        ("/sitemap.xml", "Sitemap XML", "application/xml"),
        ("/robots.txt", "robots.txt", "text/plain"),
    ];

    for (path, label, expected_contains) in &endpoints {
        let wp_url = format!("{}{}", cfg.wordpress_url, path);
        let rp_url = format!("{}{}", cfg.rustpress_url, path);

        let wp = fetch_headers_get(&client, &wp_url).await;
        let rp = fetch_headers_get(&client, &rp_url).await;

        eprintln!("\n  {label} ({path}):");

        match (wp, rp) {
            (Some((_wp_status, wp_h)), Some((_rp_status, rp_h))) => {
                let wp_ct = wp_h
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");
                let rp_ct = rp_h
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");

                eprintln!("    WP Content-Type:  {wp_ct}");
                eprintln!("    RP Content-Type:  {rp_ct}");
                eprintln!("    Expected contains: {expected_contains}");

                let rp_matches = rp_ct
                    .to_lowercase()
                    .contains(&expected_contains.to_lowercase());
                eprintln!(
                    "    RP matches expected: {} [{}]",
                    rp_matches,
                    if rp_matches { "OK" } else { "MISMATCH" }
                );
            }
            (None, Some(_)) => eprintln!("    [SKIP] WordPress endpoint not available"),
            (Some(_), None) => eprintln!("    [SKIP] RustPress endpoint not available"),
            (None, None) => eprintln!("    [SKIP] Neither endpoint available"),
        }
    }

    eprintln!("\n[PASS] Content-Type headers compared");
}

#[tokio::test]
#[ignore]
async fn test_cache_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_cache_headers ===");

    let endpoints = [
        ("/", "Homepage"),
        ("/wp-json/wp/v2/posts", "REST API posts"),
        ("/feed/", "RSS feed"),
    ];

    let cache_headers = [
        "cache-control",
        "expires",
        "etag",
        "last-modified",
        "vary",
        "pragma",
    ];

    for (path, label) in &endpoints {
        let wp_url = format!("{}{}", cfg.wordpress_url, path);
        let rp_url = format!("{}{}", cfg.rustpress_url, path);

        let wp = fetch_headers_get(&client, &wp_url).await;
        let rp = fetch_headers_get(&client, &rp_url).await;

        eprintln!("\n  {label} ({path}):");

        match (wp, rp) {
            (Some((_wp_status, wp_h)), Some((_rp_status, rp_h))) => {
                for header in &cache_headers {
                    let wp_val = wp_h
                        .get(*header)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("(absent)");
                    let rp_val = rp_h
                        .get(*header)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("(absent)");

                    if wp_val != "(absent)" || rp_val != "(absent)" {
                        eprintln!("    {header} - WP: {wp_val}, RP: {rp_val}");
                    }
                }
            }
            _ => eprintln!("    [SKIP] Could not fetch from one or both servers"),
        }
    }

    eprintln!("\n[PASS] Cache headers compared");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_wp_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_wp_headers ===");

    // WordPress REST API adds specific headers: X-WP-Total, X-WP-TotalPages
    let api_endpoints = [
        "/wp-json/wp/v2/posts",
        "/wp-json/wp/v2/categories",
        "/wp-json/wp/v2/tags",
        "/wp-json/wp/v2/pages",
        "/wp-json/wp/v2/comments",
        "/wp-json/wp/v2/media",
        "/wp-json/wp/v2/users",
    ];

    let wp_specific_headers = ["x-wp-total", "x-wp-totalpages"];

    for path in &api_endpoints {
        let wp = fetch_headers_get(&client, &format!("{}{}", cfg.wordpress_url, path)).await;
        let rp = fetch_headers_get(&client, &format!("{}{}", cfg.rustpress_url, path)).await;

        eprintln!("\n  {path}:");

        match (wp, rp) {
            (Some((_wp_status, wp_h)), Some((_rp_status, rp_h))) => {
                for header in &wp_specific_headers {
                    let wp_val = wp_h
                        .get(*header)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("(absent)");
                    let rp_val = rp_h
                        .get(*header)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("(absent)");

                    let both_present = wp_val != "(absent)" && rp_val != "(absent)";
                    eprintln!(
                        "    {} - WP: {}, RP: {} [{}]",
                        header,
                        wp_val,
                        rp_val,
                        if both_present {
                            "BOTH PRESENT"
                        } else {
                            "MISSING IN ONE"
                        }
                    );
                }

                // Also check Link header for pagination
                let wp_link = wp_h
                    .get("link")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");
                let rp_link = rp_h
                    .get("link")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");

                if wp_link != "(absent)" || rp_link != "(absent)" {
                    eprintln!(
                        "    link - WP: {}, RP: {}",
                        if wp_link.len() > 80 {
                            &wp_link[..80]
                        } else {
                            wp_link
                        },
                        if rp_link.len() > 80 {
                            &rp_link[..80]
                        } else {
                            rp_link
                        },
                    );
                }
            }
            _ => eprintln!("    [SKIP] Could not fetch from one or both servers"),
        }
    }

    eprintln!("\n[PASS] REST API WP-specific headers compared");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_expose_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_expose_headers ===");

    // WordPress exposes X-WP-Total and X-WP-TotalPages via
    // Access-Control-Expose-Headers so JavaScript clients can read them
    let wp = client
        .get(format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url))
        .header("Origin", "http://example.com")
        .send()
        .await;
    let rp = client
        .get(format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url))
        .header("Origin", "http://example.com")
        .send()
        .await;

    match (wp, rp) {
        (Ok(wp_resp), Ok(rp_resp)) => {
            let wp_expose = wp_resp
                .headers()
                .get("access-control-expose-headers")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");
            let rp_expose = rp_resp
                .headers()
                .get("access-control-expose-headers")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            eprintln!("  Access-Control-Expose-Headers:");
            eprintln!("    WP: {wp_expose}");
            eprintln!("    RP: {rp_expose}");

            // Check if X-WP-Total is exposed
            let wp_exposes_total = wp_expose.to_lowercase().contains("x-wp-total");
            let rp_exposes_total = rp_expose.to_lowercase().contains("x-wp-total");
            eprintln!(
                "  Exposes X-WP-Total - WP: {}, RP: {} [{}]",
                wp_exposes_total,
                rp_exposes_total,
                if wp_exposes_total == rp_exposes_total {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            eprintln!("[PASS] Expose headers compared");
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_cors_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_cors_headers ===");

    // CORS headers are most relevant on API endpoints
    let api_paths = ["/wp-json/", "/wp-json/wp/v2/posts", "/wp-json/wp/v2/users"];

    let cors_headers = [
        "access-control-allow-origin",
        "access-control-allow-methods",
        "access-control-allow-headers",
        "access-control-expose-headers",
        "access-control-allow-credentials",
    ];

    for path in &api_paths {
        let wp_url = format!("{}{}", cfg.wordpress_url, path);
        let rp_url = format!("{}{}", cfg.rustpress_url, path);

        // Send a request with an Origin header to trigger CORS
        let wp = client
            .get(&wp_url)
            .header("Origin", "http://example.com")
            .send()
            .await;
        let rp = client
            .get(&rp_url)
            .header("Origin", "http://example.com")
            .send()
            .await;

        eprintln!("\n  {path} :");

        match (wp, rp) {
            (Ok(wp_resp), Ok(rp_resp)) => {
                let wp_h = wp_resp.headers().clone();
                let rp_h = rp_resp.headers().clone();

                for header in &cors_headers {
                    let wp_val = wp_h
                        .get(*header)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("(absent)");
                    let rp_val = rp_h
                        .get(*header)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("(absent)");

                    if wp_val != "(absent)" || rp_val != "(absent)" {
                        eprintln!("    {header} - WP: {wp_val}, RP: {rp_val}");
                    }
                }
            }
            _ => eprintln!("    [SKIP] Could not fetch from one or both servers"),
        }

        // Also test OPTIONS preflight request
        let wp_opts = client
            .request(reqwest::Method::OPTIONS, &wp_url)
            .header("Origin", "http://example.com")
            .header("Access-Control-Request-Method", "POST")
            .send()
            .await;
        let rp_opts = client
            .request(reqwest::Method::OPTIONS, &rp_url)
            .header("Origin", "http://example.com")
            .header("Access-Control-Request-Method", "POST")
            .send()
            .await;

        eprintln!("  OPTIONS preflight:");
        match (wp_opts, rp_opts) {
            (Ok(wp_resp), Ok(rp_resp)) => {
                eprintln!(
                    "    Status - WP: {}, RP: {}",
                    wp_resp.status(),
                    rp_resp.status()
                );

                let wp_h = wp_resp.headers().clone();
                let rp_h = rp_resp.headers().clone();

                for header in &cors_headers {
                    let wp_val = wp_h.get(*header).and_then(|v| v.to_str().ok());
                    let rp_val = rp_h.get(*header).and_then(|v| v.to_str().ok());

                    if wp_val.is_some() || rp_val.is_some() {
                        eprintln!(
                            "    {} - WP: {}, RP: {}",
                            header,
                            wp_val.unwrap_or("(absent)"),
                            rp_val.unwrap_or("(absent)")
                        );
                    }
                }
            }
            _ => eprintln!("    [SKIP] OPTIONS preflight failed on one or both servers"),
        }
    }

    eprintln!("\n[PASS] CORS headers compared");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_error_json_content_type() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_error_json_content_type ===");

    // Request a non-existent post ID — both servers should return JSON even for errors
    let wp = fetch_headers_get(
        &client,
        &format!("{}/wp-json/wp/v2/posts/999999", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_headers_get(
        &client,
        &format!("{}/wp-json/wp/v2/posts/999999", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some((wp_status, wp_h)), Some((rp_status, rp_h))) => {
            eprintln!("  Status - WP: {wp_status}, RP: {rp_status}");
            compare_header(&wp_h, &rp_h, "content-type");

            let rp_ct = rp_h
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            let rp_is_json = rp_ct.to_lowercase().contains("application/json");
            eprintln!(
                "  RP error response is JSON: {} [{}]",
                rp_is_json,
                if rp_is_json { "OK" } else { "MISMATCH" }
            );

            assert!(
                rp_is_json,
                "RustPress REST API errors should return application/json content-type"
            );
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] REST API error JSON content-type verified");
}

#[tokio::test]
#[ignore]
async fn test_redirect_headers() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_redirect_headers ===");

    // Build a client that does NOT follow redirects
    let no_redirect_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to build no-redirect HTTP client");

    // GET /wp-admin (without trailing slash) should redirect to /wp-admin/
    let wp = no_redirect_client
        .get(format!("{}/wp-admin", cfg.wordpress_url))
        .send()
        .await;
    let rp = no_redirect_client
        .get(format!("{}/wp-admin", cfg.rustpress_url))
        .send()
        .await;

    match (wp, rp) {
        (Ok(wp_resp), Ok(rp_resp)) => {
            let wp_status = wp_resp.status();
            let rp_status = rp_resp.status();

            eprintln!("  Status - WP: {wp_status}, RP: {rp_status}");

            let wp_location = wp_resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");
            let rp_location = rp_resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            eprintln!("  Location - WP: {wp_location}, RP: {rp_location}");

            let rp_is_redirect = rp_status.as_u16() == 301 || rp_status.as_u16() == 302;
            eprintln!(
                "  RP is redirect: {} [{}]",
                rp_is_redirect,
                if rp_is_redirect { "OK" } else { "MISMATCH" }
            );
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] Redirect headers compared");
}

#[tokio::test]
#[ignore]
async fn test_login_page_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_login_page_headers ===");

    let wp = fetch_headers_get(&client, &format!("{}/wp-login.php", cfg.wordpress_url)).await;
    let rp = fetch_headers_get(&client, &format!("{}/wp-login.php", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((_wp_status, wp_h)), Some((_rp_status, rp_h))) => {
            compare_header(&wp_h, &rp_h, "content-type");
            compare_header(&wp_h, &rp_h, "cache-control");
            compare_header(&wp_h, &rp_h, "expires");
            compare_header(&wp_h, &rp_h, "pragma");

            // Login page should serve HTML
            let rp_ct = rp_h
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");
            assert!(
                rp_ct.to_lowercase().contains("text/html"),
                "Login page should return text/html content-type"
            );

            // Login page should discourage caching
            let rp_cc = rp_h
                .get("cache-control")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let has_no_cache = rp_cc.to_lowercase().contains("no-cache")
                || rp_cc.to_lowercase().contains("no-store")
                || rp_cc.to_lowercase().contains("private");
            eprintln!(
                "  RP cache-control discourages caching: {} [{}]",
                has_no_cache,
                if has_no_cache { "OK" } else { "WARN" }
            );
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] Login page headers compared");
}

#[tokio::test]
#[ignore]
async fn test_rss_content_type() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rss_content_type ===");

    let wp = fetch_headers_get(&client, &format!("{}/feed/", cfg.wordpress_url)).await;
    let rp = fetch_headers_get(&client, &format!("{}/feed/", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_h)), Some((rp_status, rp_h))) => {
            eprintln!("  Status - WP: {wp_status}, RP: {rp_status}");
            compare_header(&wp_h, &rp_h, "content-type");

            let rp_ct = rp_h
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            let rp_is_rss =
                rp_ct.to_lowercase().contains("rss+xml") || rp_ct.to_lowercase().contains("xml");
            eprintln!(
                "  RP content-type contains rss+xml or xml: {} [{}]",
                rp_is_rss,
                if rp_is_rss { "OK" } else { "MISMATCH" }
            );

            assert!(
                rp_is_rss,
                "RSS feed should have content-type containing rss+xml or xml"
            );
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] RSS content-type verified");
}

#[tokio::test]
#[ignore]
async fn test_robots_content_type() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_robots_content_type ===");

    let wp = fetch_headers_get(&client, &format!("{}/robots.txt", cfg.wordpress_url)).await;
    let rp = fetch_headers_get(&client, &format!("{}/robots.txt", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_h)), Some((rp_status, rp_h))) => {
            eprintln!("  Status - WP: {wp_status}, RP: {rp_status}");
            compare_header(&wp_h, &rp_h, "content-type");

            let rp_ct = rp_h
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            let rp_is_plain = rp_ct.to_lowercase().contains("text/plain");
            eprintln!(
                "  RP content-type is text/plain: {} [{}]",
                rp_is_plain,
                if rp_is_plain { "OK" } else { "MISMATCH" }
            );

            assert!(
                rp_is_plain,
                "robots.txt should have content-type text/plain"
            );
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] robots.txt content-type verified");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_json_content_type() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_json_content_type ===");

    let api_endpoints = [
        "/wp-json/",
        "/wp-json/wp/v2/posts",
        "/wp-json/wp/v2/categories",
        "/wp-json/wp/v2/tags",
        "/wp-json/wp/v2/pages",
        "/wp-json/wp/v2/users",
    ];

    for path in &api_endpoints {
        let rp = fetch_headers_get(&client, &format!("{}{}", cfg.rustpress_url, path)).await;

        eprintln!("\n  {path}:");

        match rp {
            Some((rp_status, rp_h)) => {
                let rp_ct = rp_h
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");

                eprintln!("    Status: {rp_status}");
                eprintln!("    Content-Type: {rp_ct}");

                let is_json = rp_ct.to_lowercase().contains("application/json");
                eprintln!(
                    "    Is JSON: {} [{}]",
                    is_json,
                    if is_json { "OK" } else { "MISMATCH" }
                );

                assert!(
                    is_json,
                    "REST API endpoint {path} should return application/json, got: {rp_ct}"
                );
            }
            None => eprintln!("    [SKIP] RustPress endpoint not available"),
        }
    }

    eprintln!("\n[PASS] All REST API endpoints return application/json");
}

#[tokio::test]
#[ignore]
async fn test_powered_by_header() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_powered_by_header ===");

    let wp = fetch_headers_get(&client, &cfg.wordpress_url).await;
    let rp = fetch_headers_get(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_wp_status, wp_h)), Some((_rp_status, rp_h))) => {
            let wp_powered = wp_h
                .get("x-powered-by")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");
            let rp_powered = rp_h
                .get("x-powered-by")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            eprintln!("  X-Powered-By - WP: {wp_powered}, RP: {rp_powered}");

            // WordPress typically has "PHP/x.x.x", RustPress may omit or customize
            let wp_has = has_header(&wp_h, "x-powered-by");
            let rp_has = has_header(&rp_h, "x-powered-by");
            eprintln!("  Header present - WP: {wp_has}, RP: {rp_has}");
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] X-Powered-By header compared");
}

#[tokio::test]
#[ignore]
async fn test_link_header_api_discovery() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_link_header_api_discovery ===");

    // WordPress homepage should include a Link header for REST API discovery:
    //   Link: <http://example.com/wp-json/>; rel="https://api.w.org/"
    let wp = fetch_headers_get(&client, &cfg.wordpress_url).await;
    let rp = fetch_headers_get(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_wp_status, wp_h)), Some((_rp_status, rp_h))) => {
            let wp_link = wp_h
                .get("link")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");
            let rp_link = rp_h
                .get("link")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            eprintln!("  Link header - WP: {wp_link}");
            eprintln!("  Link header - RP: {rp_link}");

            let wp_has_api_discovery = wp_link.contains("https://api.w.org/");
            let rp_has_api_discovery = rp_link.contains("https://api.w.org/");

            eprintln!(
                "  Contains api.w.org discovery - WP: {}, RP: {} [{}]",
                wp_has_api_discovery,
                rp_has_api_discovery,
                if wp_has_api_discovery == rp_has_api_discovery {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Also check the link points to /wp-json/
            let rp_points_to_wpjson = rp_link.contains("wp-json");
            eprintln!(
                "  RP Link points to wp-json: {} [{}]",
                rp_points_to_wpjson,
                if rp_points_to_wpjson { "OK" } else { "MISSING" }
            );
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] Link header API discovery compared");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_allow_header() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_allow_header ===");

    let client = build_http_client();

    // OPTIONS request on /wp-json/wp/v2/posts should include an Allow header
    let wp = client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
        )
        .send()
        .await;
    let rp = client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
        )
        .send()
        .await;

    match (wp, rp) {
        (Ok(wp_resp), Ok(rp_resp)) => {
            let wp_allow = wp_resp
                .headers()
                .get("allow")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");
            let rp_allow = rp_resp
                .headers()
                .get("allow")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            eprintln!("  Allow header - WP: {wp_allow}");
            eprintln!("  Allow header - RP: {rp_allow}");

            // Check that GET is listed in the Allow header
            let rp_allows_get = rp_allow.to_uppercase().contains("GET");
            eprintln!(
                "  RP Allow includes GET: {} [{}]",
                rp_allows_get,
                if rp_allows_get { "OK" } else { "MISSING" }
            );

            // Check that POST is listed
            let rp_allows_post = rp_allow.to_uppercase().contains("POST");
            eprintln!(
                "  RP Allow includes POST: {} [{}]",
                rp_allows_post,
                if rp_allows_post { "OK" } else { "MISSING" }
            );

            eprintln!(
                "  Status - WP: {}, RP: {}",
                wp_resp.status(),
                rp_resp.status()
            );
        }
        _ => eprintln!("[SKIP] Could not send OPTIONS to one or both servers"),
    }

    eprintln!("[PASS] REST API Allow header compared");
}

#[tokio::test]
#[ignore]
async fn test_sitemap_content_type() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_sitemap_content_type ===");

    let wp = fetch_headers_get(&client, &format!("{}/sitemap.xml", cfg.wordpress_url)).await;
    let rp = fetch_headers_get(&client, &format!("{}/sitemap.xml", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_h)), Some((rp_status, rp_h))) => {
            eprintln!("  Status - WP: {wp_status}, RP: {rp_status}");
            compare_header(&wp_h, &rp_h, "content-type");

            let rp_ct = rp_h
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            let rp_is_xml = rp_ct.to_lowercase().contains("application/xml")
                || rp_ct.to_lowercase().contains("text/xml");
            eprintln!(
                "  RP content-type is XML: {} [{}]",
                rp_is_xml,
                if rp_is_xml { "OK" } else { "MISMATCH" }
            );

            assert!(
                rp_is_xml,
                "sitemap.xml should return application/xml or text/xml content-type, got: {rp_ct}"
            );
        }
        _ => eprintln!("[SKIP] Could not fetch from one or both servers"),
    }

    eprintln!("[PASS] sitemap.xml content-type verified");
}

#[tokio::test]
#[ignore]
async fn test_static_assets_cache_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_static_assets_cache_headers ===");

    // RustPress serves static assets from /static/ (CSS, JS)
    let static_paths = ["/static/style.css", "/static/js/admin.js"];

    for path in &static_paths {
        let rp = fetch_headers_get(&client, &format!("{}{}", cfg.rustpress_url, path)).await;

        eprintln!("\n  {path}:");

        match rp {
            Some((rp_status, rp_h)) => {
                eprintln!("    Status: {rp_status}");

                let rp_cc = rp_h
                    .get("cache-control")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");
                let rp_ct = rp_h
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");

                eprintln!("    Content-Type: {rp_ct}");
                eprintln!("    Cache-Control: {rp_cc}");

                let has_cache_control = has_header(&rp_h, "cache-control");
                eprintln!(
                    "    Has cache-control header: {} [{}]",
                    has_cache_control,
                    if has_cache_control { "OK" } else { "MISSING" }
                );

                // Check for ETag or Last-Modified too
                let has_etag = has_header(&rp_h, "etag");
                let has_last_mod = has_header(&rp_h, "last-modified");
                eprintln!("    Has ETag: {has_etag}, Has Last-Modified: {has_last_mod}");
            }
            None => eprintln!("    [SKIP] RustPress static asset not available"),
        }
    }

    eprintln!("\n[PASS] Static assets cache headers checked");
}

#[tokio::test]
#[ignore]
async fn test_xmlrpc_content_type() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_xmlrpc_content_type ===");

    // POST a minimal XML-RPC request to /xmlrpc.php
    let xmlrpc_body = r#"<?xml version="1.0"?><methodCall><methodName>system.listMethods</methodName><params></params></methodCall>"#;

    let wp = client
        .post(format!("{}/xmlrpc.php", cfg.wordpress_url))
        .header("content-type", "text/xml")
        .body(xmlrpc_body)
        .send()
        .await;
    let rp = client
        .post(format!("{}/xmlrpc.php", cfg.rustpress_url))
        .header("content-type", "text/xml")
        .body(xmlrpc_body)
        .send()
        .await;

    match (wp, rp) {
        (Ok(wp_resp), Ok(rp_resp)) => {
            let wp_status = wp_resp.status();
            let rp_status = rp_resp.status();
            let wp_ct = wp_resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)")
                .to_string();
            let rp_ct = rp_resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)")
                .to_string();

            eprintln!("  Status - WP: {wp_status}, RP: {rp_status}");
            eprintln!("  Content-Type - WP: {wp_ct}, RP: {rp_ct}");

            let rp_is_xml = rp_ct.to_lowercase().contains("text/xml")
                || rp_ct.to_lowercase().contains("application/xml");
            eprintln!(
                "  RP content-type is XML: {} [{}]",
                rp_is_xml,
                if rp_is_xml { "OK" } else { "MISMATCH" }
            );
        }
        (_, Ok(rp_resp)) => {
            let rp_ct = rp_resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");
            eprintln!("  [SKIP] WordPress xmlrpc not available");
            eprintln!("  RP Status: {}, Content-Type: {}", rp_resp.status(), rp_ct);
        }
        (Ok(_), _) => eprintln!("[SKIP] RustPress xmlrpc not available"),
        _ => eprintln!("[SKIP] Could not reach either server's xmlrpc.php"),
    }

    eprintln!("[PASS] XML-RPC content-type compared");
}

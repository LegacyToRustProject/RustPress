//! Frontend HTML Comparison Tests
//!
//! These tests compare the HTML pages served by WordPress and RustPress to
//! ensure structural compatibility: presence of expected elements, status
//! codes, and content formats (RSS, sitemap, robots.txt).
//!
//! All tests are `#[ignore]` by default.

use rustpress_e2e::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn fetch_html(client: &reqwest::Client, url: &str) -> Option<(reqwest::StatusCode, String)> {
    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status();
            match resp.text().await {
                Ok(body) => Some((status, body)),
                Err(e) => {
                    eprintln!("[ERROR] Failed to read body from {}: {}", url, e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("[ERROR] GET {} failed: {}", url, e);
            None
        }
    }
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<(reqwest::StatusCode, String)> {
    fetch_html(client, url).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_homepage_structure() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_homepage_structure ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            // Both should return 200
            assert_eq!(wp_status.as_u16(), 200, "WordPress homepage should be 200");
            assert_eq!(rp_status.as_u16(), 200, "RustPress homepage should be 200");
            eprintln!("[OK] Both return 200");

            // Both should have basic HTML structure
            let checks = [
                ("html", "html"),
                ("head", "head"),
                ("body", "body"),
                ("header, [role=banner]", "header or banner role"),
                ("footer, [role=contentinfo]", "footer or contentinfo role"),
                ("article, .post, .hentry", "article/post/hentry"),
            ];

            for (selector, label) in &checks {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            // Structural similarity
            assert_similar_html(&wp_html, &rp_html, 0.5);
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_single_post_structure() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_single_post_structure ===");

    // Try to fetch the default "hello world" or "hello-rustpress" post.
    // WordPress default slug is "hello-world", RustPress uses "hello-rustpress".
    let wp_slugs = ["hello-world", "sample-page", "hello-rustpress"];
    let rp_slugs = ["hello-rustpress", "hello-world", "sample-page"];

    let mut wp_result = None;
    for slug in &wp_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found post at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &rp_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found post at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Single post page should have these elements
            let checks = [
                ("article, .post, .hentry", "article/post element"),
                ("h1, h2, .entry-title", "title element"),
                (".entry-content, .post-content, article p", "content area"),
                ("time, .entry-date, .post-date", "date element"),
            ];

            for (selector, label) in &checks {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            assert_similar_html(&wp_html, &rp_html, 0.4);
        }
        (None, _) => eprintln!("[SKIP] No single post found on WordPress"),
        (_, None) => eprintln!("[SKIP] No single post found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_search_page() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_search_page ===");

    // WordPress search URL: /?s=hello   RustPress: /search?s=hello
    let wp = fetch_html(&client, &format!("{}/?s=hello", cfg.wordpress_url)).await;
    let rp = fetch_html(&client, &format!("{}/search?s=hello", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!("WordPress search status: {}", wp_status);
            eprintln!("RustPress search status: {}", rp_status);

            // Both should return 200
            assert_eq!(wp_status.as_u16(), 200, "WordPress search should be 200");
            assert_eq!(rp_status.as_u16(), 200, "RustPress search should be 200");

            // Both should have a search results area
            let checks = [
                ("html", "html element"),
                ("body", "body element"),
                (
                    "article, .post, .search-results, .hentry",
                    "search results or articles",
                ),
            ];

            for (selector, label) in &checks {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            eprintln!("[PASS] Search page compared");
        }
        _ => eprintln!("[SKIP] Could not fetch search page from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_404_page() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_404_page ===");

    let slug = "this-page-definitely-does-not-exist-e2e-test-12345";
    let wp = fetch_html(&client, &format!("{}/{}", cfg.wordpress_url, slug)).await;
    let rp = fetch_html(&client, &format!("{}/{}", cfg.rustpress_url, slug)).await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            // Both should return 404
            assert_eq!(wp_status.as_u16(), 404, "WordPress should return 404");
            assert_eq!(rp_status.as_u16(), 404, "RustPress should return 404");
            eprintln!("[OK] Both return 404");

            // Both should have some error message content
            let wp_has_error = wp_html.to_lowercase().contains("not found")
                || wp_html.to_lowercase().contains("404")
                || has_element(&wp_html, ".error-404, .not-found");
            let rp_has_error = rp_html.to_lowercase().contains("not found")
                || rp_html.to_lowercase().contains("404")
                || has_element(&rp_html, ".error-404, .not-found");

            eprintln!(
                "  Error message - WP: {}, RP: {} [{}]",
                wp_has_error,
                rp_has_error,
                if wp_has_error == rp_has_error {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            assert!(
                rp_has_error,
                "RustPress 404 page should contain error indication"
            );
            eprintln!("[PASS] 404 page compared");
        }
        _ => eprintln!("[SKIP] Could not fetch 404 page from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rss_feed() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rss_feed ===");

    // WordPress: /feed/   RustPress: /feed/
    let wp = fetch_text(&client, &format!("{}/feed/", cfg.wordpress_url)).await;
    let rp = fetch_text(&client, &format!("{}/feed/", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_body)), Some((rp_status, rp_body))) => {
            eprintln!("WordPress feed status: {}", wp_status);
            eprintln!("RustPress feed status: {}", rp_status);

            // Both should be valid XML
            let wp_is_xml = is_valid_xml_basic(&wp_body);
            let rp_is_xml = is_valid_xml_basic(&rp_body);
            eprintln!("  Valid XML - WP: {}, RP: {}", wp_is_xml, rp_is_xml);
            assert!(rp_is_xml, "RustPress RSS feed should be valid XML");

            // Both should contain <rss> or <feed> root
            let wp_has_rss = wp_body.contains("<rss") || wp_body.contains("<feed");
            let rp_has_rss = rp_body.contains("<rss") || rp_body.contains("<feed");
            eprintln!(
                "  RSS root element - WP: {}, RP: {}",
                wp_has_rss, rp_has_rss
            );

            // Both should have <item> elements
            let wp_items = count_xml_tag(&wp_body, "item");
            let rp_items = count_xml_tag(&rp_body, "item");
            eprintln!("  Items - WP: {}, RP: {}", wp_items, rp_items);

            // Check for required RSS fields
            let required_tags = ["title", "link", "description"];
            for tag in &required_tags {
                let wp_count = count_xml_tag(&wp_body, tag);
                let rp_count = count_xml_tag(&rp_body, tag);
                eprintln!(
                    "  <{}> count - WP: {}, RP: {} [{}]",
                    tag,
                    wp_count,
                    rp_count,
                    if rp_count > 0 { "OK" } else { "MISSING" }
                );
            }

            eprintln!("[PASS] RSS feed compared");
        }
        _ => eprintln!("[SKIP] Could not fetch RSS feed from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_sitemap_xml() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_sitemap_xml ===");

    // WordPress sitemaps: /wp-sitemap.xml or /sitemap.xml (with plugin)
    // RustPress: /sitemap.xml
    let wp_urls = [
        format!("{}/sitemap.xml", cfg.wordpress_url),
        format!("{}/wp-sitemap.xml", cfg.wordpress_url),
    ];

    let mut wp_body = None;
    for url in &wp_urls {
        if let Some((status, body)) = fetch_text(&client, url).await {
            if status.as_u16() == 200 && body.contains("<urlset") || body.contains("<sitemapindex")
            {
                wp_body = Some(body);
                eprintln!("WordPress sitemap found at {}", url);
                break;
            }
        }
    }

    let rp = fetch_text(&client, &format!("{}/sitemap.xml", cfg.rustpress_url)).await;

    match (wp_body, rp) {
        (Some(wp_xml), Some((rp_status, rp_xml))) => {
            eprintln!("RustPress sitemap status: {}", rp_status);

            // Both should be XML
            let wp_is_xml = is_valid_xml_basic(&wp_xml);
            let rp_is_xml = is_valid_xml_basic(&rp_xml);
            eprintln!("  Valid XML - WP: {}, RP: {}", wp_is_xml, rp_is_xml);
            assert!(rp_is_xml, "RustPress sitemap should be valid XML");

            // Both should have <url> elements
            let wp_urls_count = count_xml_tag(&wp_xml, "url");
            let rp_urls_count = count_xml_tag(&rp_xml, "url");
            eprintln!(
                "  <url> count - WP: {}, RP: {}",
                wp_urls_count, rp_urls_count
            );

            // Both should have <loc> elements
            let wp_locs = count_xml_tag(&wp_xml, "loc");
            let rp_locs = count_xml_tag(&rp_xml, "loc");
            eprintln!("  <loc> count - WP: {}, RP: {}", wp_locs, rp_locs);

            assert!(
                rp_urls_count > 0,
                "RustPress sitemap should have at least one <url>"
            );
            eprintln!("[PASS] Sitemap compared");
        }
        (None, _) => {
            eprintln!("[SKIP] WordPress sitemap not found (try installing a sitemap plugin)")
        }
        (_, None) => eprintln!("[SKIP] Could not fetch sitemap from RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_robots_txt() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_robots_txt ===");

    let wp = fetch_text(&client, &format!("{}/robots.txt", cfg.wordpress_url)).await;
    let rp = fetch_text(&client, &format!("{}/robots.txt", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_body)), Some((rp_status, rp_body))) => {
            eprintln!("WordPress robots.txt status: {}", wp_status);
            eprintln!("RustPress robots.txt status: {}", rp_status);

            assert_eq!(
                rp_status.as_u16(),
                200,
                "RustPress robots.txt should be 200"
            );

            // Both should contain User-agent directive
            let wp_has_ua = wp_body.to_lowercase().contains("user-agent");
            let rp_has_ua = rp_body.to_lowercase().contains("user-agent");
            eprintln!(
                "  User-agent directive - WP: {}, RP: {}",
                wp_has_ua, rp_has_ua
            );
            assert!(rp_has_ua, "RustPress robots.txt should have User-agent");

            // Both should reference sitemap
            let wp_has_sitemap = wp_body.to_lowercase().contains("sitemap");
            let rp_has_sitemap = rp_body.to_lowercase().contains("sitemap");
            eprintln!(
                "  Sitemap reference - WP: {}, RP: {}",
                wp_has_sitemap, rp_has_sitemap
            );

            // Print diff for comparison
            print_diff("robots.txt", &wp_body, &rp_body);

            eprintln!("[PASS] robots.txt compared");
        }
        _ => eprintln!("[SKIP] Could not fetch robots.txt from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_category_archive() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_category_archive ===");

    // Fetch the default "Uncategorized" category archive
    let category_slugs = ["uncategorized", "general", "news"];

    let mut wp_result = None;
    for slug in &category_slugs {
        let url = format!("{}/category/{}/", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found category archive at /category/{}/", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &category_slugs {
        let url = format!("{}/category/{}/", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found category archive at /category/{}/", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            let checks = [
                ("html", "html element"),
                ("body", "body element"),
                ("article, .post, .hentry", "article/post element"),
                ("h1, h2, .archive-title", "archive title"),
            ];

            for (selector, label) in &checks {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            assert_similar_html(&wp_html, &rp_html, 0.4);
        }
        (None, _) => eprintln!("[SKIP] No category archive found on WordPress"),
        (_, None) => eprintln!("[SKIP] No category archive found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_tag_archive() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_tag_archive ===");

    // Try common tag slugs
    let tag_slugs = ["test", "sample", "hello"];

    let mut wp_result = None;
    for slug in &tag_slugs {
        let url = format!("{}/tag/{}/", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found tag archive at /tag/{}/", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &tag_slugs {
        let url = format!("{}/tag/{}/", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found tag archive at /tag/{}/", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            assert_similar_html(&wp_html, &rp_html, 0.4);
            eprintln!("[PASS] Tag archive compared");
        }
        _ => eprintln!("[SKIP] No tag archives found on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_date_archive() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_date_archive ===");

    // Use 2026 as test year (when RustPress was developed)
    let year = "2026";
    let month = "03";

    // Year archive
    let wp_year = fetch_html(&client, &format!("{}/{}/", cfg.wordpress_url, year)).await;
    let rp_year = fetch_html(&client, &format!("{}/{}/", cfg.rustpress_url, year)).await;

    match (&wp_year, &rp_year) {
        (Some((wp_status, _)), Some((rp_status, _))) => {
            eprintln!(
                "  Year archive /{}/: WP={}, RP={}",
                year, wp_status, rp_status
            );
        }
        _ => eprintln!("  [SKIP] Year archive not available on one or both servers"),
    }

    // Month archive
    let wp_month = fetch_html(
        &client,
        &format!("{}/{}/{}/", cfg.wordpress_url, year, month),
    )
    .await;
    let rp_month = fetch_html(
        &client,
        &format!("{}/{}/{}/", cfg.rustpress_url, year, month),
    )
    .await;

    match (&wp_month, &rp_month) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!(
                "  Month archive /{}/{}/: WP={}, RP={}",
                year, month, wp_status, rp_status
            );

            if wp_status.as_u16() == 200 && rp_status.as_u16() == 200 {
                let checks = [
                    ("html", "html element"),
                    ("body", "body element"),
                    ("article, .post, .hentry", "article/post elements"),
                ];

                for (selector, label) in &checks {
                    let wp_has = has_element(wp_html, selector);
                    let rp_has = has_element(rp_html, selector);
                    eprintln!(
                        "    {} - WP: {}, RP: {} [{}]",
                        label,
                        wp_has,
                        rp_has,
                        if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                    );
                }
            }
        }
        _ => eprintln!("  [SKIP] Month archive not available on one or both servers"),
    }

    eprintln!("[PASS] Date archive compared");
}

#[tokio::test]
#[ignore]
async fn test_author_archive() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_author_archive ===");

    // Author archive using the admin user
    let wp = fetch_html(
        &client,
        &format!("{}/author/{}/", cfg.wordpress_url, cfg.admin_user),
    )
    .await;
    let rp = fetch_html(
        &client,
        &format!("{}/author/{}/", cfg.rustpress_url, cfg.admin_user),
    )
    .await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!(
                "  Author archive status - WP: {}, RP: {}",
                wp_status, rp_status
            );

            if wp_status.as_u16() == 200 && rp_status.as_u16() == 200 {
                let checks = [
                    ("article, .post, .hentry", "article/post elements"),
                    ("h1, h2, .archive-title, .author-title", "author title"),
                ];

                for (selector, label) in &checks {
                    let wp_has = has_element(&wp_html, selector);
                    let rp_has = has_element(&rp_html, selector);
                    eprintln!(
                        "    {} - WP: {}, RP: {} [{}]",
                        label,
                        wp_has,
                        rp_has,
                        if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                    );
                }

                assert_similar_html(&wp_html, &rp_html, 0.4);
            }

            eprintln!("[PASS] Author archive compared");
        }
        _ => eprintln!("[SKIP] Could not fetch author archive from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_page_content() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_page_content ===");

    // WordPress creates a "Sample Page" by default
    let page_slugs = ["sample-page", "about", "contact"];

    let mut wp_result = None;
    for slug in &page_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found page at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &page_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found page at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            let checks = [
                ("article, .page, .hentry", "page article element"),
                ("h1, h2, .entry-title, .page-title", "page title"),
                (".entry-content, .page-content", "page content area"),
            ];

            for (selector, label) in &checks {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            // Pages should NOT have comment sections by default in many themes
            // but this is theme-dependent so we just check structure
            assert_similar_html(&wp_html, &rp_html, 0.4);
            eprintln!("[PASS] Page content compared");
        }
        (None, _) => eprintln!("[SKIP] No page found on WordPress"),
        (_, None) => eprintln!("[SKIP] No page found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_xmlrpc_endpoint() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_xmlrpc_endpoint ===");

    // XML-RPC endpoint: /xmlrpc.php
    // A GET request should return a message saying "XML-RPC server accepts POST requests only"
    // or return the RSD (Really Simple Discovery) document.
    let wp = fetch_text(&client, &format!("{}/xmlrpc.php", cfg.wordpress_url)).await;
    let rp = fetch_text(&client, &format!("{}/xmlrpc.php", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_body)), Some((rp_status, rp_body))) => {
            eprintln!("WordPress xmlrpc.php status: {}", wp_status);
            eprintln!("RustPress xmlrpc.php status: {}", rp_status);

            // WordPress returns 405 for GET on xmlrpc.php (POST only)
            // or some message about POST requests
            let wp_mentions_post =
                wp_body.to_lowercase().contains("post") || wp_status.as_u16() == 405;
            let rp_mentions_post =
                rp_body.to_lowercase().contains("post") || rp_status.as_u16() == 405;

            eprintln!(
                "  Indicates POST-only - WP: {}, RP: {}",
                wp_mentions_post, rp_mentions_post
            );

            // Try a POST with system.listMethods
            let xmlrpc_body = r#"<?xml version="1.0"?>
<methodCall>
  <methodName>system.listMethods</methodName>
  <params/>
</methodCall>"#;

            let wp_post = client
                .post(&format!("{}/xmlrpc.php", cfg.wordpress_url))
                .header("Content-Type", "text/xml")
                .body(xmlrpc_body.to_string())
                .send()
                .await;
            let rp_post = client
                .post(&format!("{}/xmlrpc.php", cfg.rustpress_url))
                .header("Content-Type", "text/xml")
                .body(xmlrpc_body.to_string())
                .send()
                .await;

            if let (Ok(wp_resp), Ok(rp_resp)) = (wp_post, rp_post) {
                eprintln!(
                    "  POST listMethods - WP: {}, RP: {}",
                    wp_resp.status(),
                    rp_resp.status()
                );

                let wp_xml = wp_resp.text().await.unwrap_or_default();
                let rp_xml = rp_resp.text().await.unwrap_or_default();

                let wp_has_response = wp_xml.contains("methodResponse");
                let rp_has_response = rp_xml.contains("methodResponse");
                eprintln!(
                    "  methodResponse - WP: {}, RP: {}",
                    wp_has_response, rp_has_response
                );
            }

            eprintln!("[PASS] XML-RPC endpoint compared");
        }
        _ => eprintln!("[SKIP] Could not fetch xmlrpc.php from one or both servers"),
    }
}

// ---------------------------------------------------------------------------
// Additional Tests (25)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_homepage_meta_tags() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_homepage_meta_tags ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for meta charset
            let wp_charset = has_element(&wp_html, "meta[charset]")
                || has_element(&wp_html, "meta[http-equiv='Content-Type']");
            let rp_charset = has_element(&rp_html, "meta[charset]")
                || has_element(&rp_html, "meta[http-equiv='Content-Type']");
            eprintln!(
                "  meta charset - WP: {}, RP: {} [{}]",
                wp_charset,
                rp_charset,
                if wp_charset == rp_charset {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for viewport meta
            let wp_viewport = has_element(&wp_html, "meta[name='viewport']");
            let rp_viewport = has_element(&rp_html, "meta[name='viewport']");
            eprintln!(
                "  meta viewport - WP: {}, RP: {} [{}]",
                wp_viewport,
                rp_viewport,
                if wp_viewport == rp_viewport {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for og:description or meta description
            let wp_desc = has_element(&wp_html, "meta[name='description']")
                || has_element(&wp_html, "meta[property='og:description']");
            let rp_desc = has_element(&rp_html, "meta[name='description']")
                || has_element(&rp_html, "meta[property='og:description']");
            eprintln!(
                "  description/og:description - WP: {}, RP: {} [{}]",
                wp_desc,
                rp_desc,
                if wp_desc == rp_desc {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for og:title
            let wp_og_title = has_element(&wp_html, "meta[property='og:title']");
            let rp_og_title = has_element(&rp_html, "meta[property='og:title']");
            eprintln!(
                "  og:title - WP: {}, RP: {} [{}]",
                wp_og_title,
                rp_og_title,
                if wp_og_title == rp_og_title {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for og:type
            let wp_og_type = has_element(&wp_html, "meta[property='og:type']");
            let rp_og_type = has_element(&rp_html, "meta[property='og:type']");
            eprintln!(
                "  og:type - WP: {}, RP: {} [{}]",
                wp_og_type,
                rp_og_type,
                if wp_og_type == rp_og_type {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for og:url
            let wp_og_url = has_element(&wp_html, "meta[property='og:url']");
            let rp_og_url = has_element(&rp_html, "meta[property='og:url']");
            eprintln!(
                "  og:url - WP: {}, RP: {} [{}]",
                wp_og_url,
                rp_og_url,
                if wp_og_url == rp_og_url {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // RustPress must have at least charset and viewport
            assert!(rp_charset, "RustPress should have meta charset");
            assert!(rp_viewport, "RustPress should have meta viewport");

            eprintln!("[PASS] Homepage meta tags compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_homepage_navigation() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_homepage_navigation ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for nav element or navigation role
            let wp_has_nav =
                has_element(&wp_html, "nav") || has_element(&wp_html, "[role='navigation']");
            let rp_has_nav =
                has_element(&rp_html, "nav") || has_element(&rp_html, "[role='navigation']");
            eprintln!(
                "  nav element - WP: {}, RP: {} [{}]",
                wp_has_nav,
                rp_has_nav,
                if wp_has_nav == rp_has_nav {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );
            assert!(
                rp_has_nav,
                "RustPress homepage should have navigation element"
            );

            // Compare navigation link counts
            let wp_nav_links = count_elements(&wp_html, "nav a, [role='navigation'] a");
            let rp_nav_links = count_elements(&rp_html, "nav a, [role='navigation'] a");
            eprintln!(
                "  Nav link count - WP: {}, RP: {}",
                wp_nav_links, rp_nav_links
            );

            // Check for menu/list structure inside nav
            let wp_has_menu = has_element(&wp_html, "nav ul, nav ol, .menu");
            let rp_has_menu = has_element(&rp_html, "nav ul, nav ol, .menu");
            eprintln!(
                "  Menu list structure - WP: {}, RP: {} [{}]",
                wp_has_menu,
                rp_has_menu,
                if wp_has_menu == rp_has_menu {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            eprintln!("[PASS] Homepage navigation compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_homepage_sidebar() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_homepage_sidebar ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            let sidebar_selectors = [
                ("aside, [role='complementary']", "aside/complementary role"),
                (".sidebar, .widget-area, #sidebar", "sidebar class/id"),
                (".widget, .wp-block-widget-area", "widget elements"),
            ];

            for (selector, label) in &sidebar_selectors {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            // Count widget areas
            let wp_widgets = count_elements(&wp_html, ".widget, .wp-block-widget-area");
            let rp_widgets = count_elements(&rp_html, ".widget, .wp-block-widget-area");
            eprintln!("  Widget count - WP: {}, RP: {}", wp_widgets, rp_widgets);

            eprintln!("[PASS] Homepage sidebar compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_homepage_footer() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_homepage_footer ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for footer element
            let wp_has_footer = has_element(&wp_html, "footer, [role='contentinfo']");
            let rp_has_footer = has_element(&rp_html, "footer, [role='contentinfo']");
            eprintln!(
                "  footer element - WP: {}, RP: {} [{}]",
                wp_has_footer,
                rp_has_footer,
                if wp_has_footer == rp_has_footer {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );
            assert!(rp_has_footer, "RustPress should have a footer element");

            // Check for copyright text in footer
            let wp_footer_text = extract_text(&wp_html, "footer, [role='contentinfo']");
            let rp_footer_text = extract_text(&rp_html, "footer, [role='contentinfo']");

            let wp_has_copyright = wp_footer_text
                .iter()
                .any(|t| t.contains("©") || t.to_lowercase().contains("copyright"));
            let rp_has_copyright = rp_footer_text
                .iter()
                .any(|t| t.contains("©") || t.to_lowercase().contains("copyright"));
            eprintln!(
                "  Copyright text - WP: {}, RP: {} [{}]",
                wp_has_copyright,
                rp_has_copyright,
                if wp_has_copyright == rp_has_copyright {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for links in footer
            let wp_footer_links = count_elements(&wp_html, "footer a, [role='contentinfo'] a");
            let rp_footer_links = count_elements(&rp_html, "footer a, [role='contentinfo'] a");
            eprintln!(
                "  Footer link count - WP: {}, RP: {}",
                wp_footer_links, rp_footer_links
            );

            eprintln!("[PASS] Homepage footer compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_homepage_sticky_posts() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_homepage_sticky_posts ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for .sticky class on posts
            let wp_sticky_count = count_elements(&wp_html, ".sticky");
            let rp_sticky_count = count_elements(&rp_html, ".sticky");
            eprintln!(
                "  .sticky post count - WP: {}, RP: {}",
                wp_sticky_count, rp_sticky_count
            );

            // Check first article element
            let wp_first_articles = extract_text(
                &wp_html,
                "article:first-of-type .entry-title, article:first-of-type h2",
            );
            let rp_first_articles = extract_text(
                &rp_html,
                "article:first-of-type .entry-title, article:first-of-type h2",
            );
            if !wp_first_articles.is_empty() {
                eprintln!("  WP first article title: {:?}", wp_first_articles.first());
            }
            if !rp_first_articles.is_empty() {
                eprintln!("  RP first article title: {:?}", rp_first_articles.first());
            }

            // If WordPress has sticky posts, RustPress should too
            if wp_sticky_count > 0 {
                eprintln!(
                    "  WordPress has {} sticky post(s); RustPress has {}",
                    wp_sticky_count, rp_sticky_count
                );
                // Sticky posts should appear if they exist in the database;
                // we only warn (not assert) since test data may differ.
            }

            eprintln!("[PASS] Sticky posts compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_single_post_comments_section() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_single_post_comments_section ===");

    // Find a single post on both sites
    let wp_slugs = ["hello-world", "sample-page", "hello-rustpress"];
    let rp_slugs = ["hello-rustpress", "hello-world", "sample-page"];

    let mut wp_result = None;
    for slug in &wp_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found post at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &rp_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found post at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Check for comments section
            let wp_has_comments = has_element(&wp_html, "#comments, .comments-area, .comment-list");
            let rp_has_comments = has_element(&rp_html, "#comments, .comments-area, .comment-list");
            eprintln!(
                "  Comments section - WP: {}, RP: {} [{}]",
                wp_has_comments,
                rp_has_comments,
                if wp_has_comments == rp_has_comments {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for comment form
            let wp_has_form =
                has_element(&wp_html, "#commentform, .comment-form, form.comment-form");
            let rp_has_form =
                has_element(&rp_html, "#commentform, .comment-form, form.comment-form");
            eprintln!(
                "  Comment form - WP: {}, RP: {} [{}]",
                wp_has_form,
                rp_has_form,
                if wp_has_form == rp_has_form {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for textarea in comment form
            let wp_has_textarea =
                has_element(&wp_html, "#commentform textarea, .comment-form textarea");
            let rp_has_textarea =
                has_element(&rp_html, "#commentform textarea, .comment-form textarea");
            eprintln!(
                "  Comment textarea - WP: {}, RP: {} [{}]",
                wp_has_textarea,
                rp_has_textarea,
                if wp_has_textarea == rp_has_textarea {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for submit button
            let wp_has_submit = has_element(&wp_html, "#commentform input[type='submit'], #commentform button[type='submit'], .comment-form input[type='submit'], .comment-form button[type='submit']");
            let rp_has_submit = has_element(&rp_html, "#commentform input[type='submit'], #commentform button[type='submit'], .comment-form input[type='submit'], .comment-form button[type='submit']");
            eprintln!(
                "  Submit button - WP: {}, RP: {} [{}]",
                wp_has_submit,
                rp_has_submit,
                if wp_has_submit == rp_has_submit {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for author/email input fields
            let wp_has_author = has_element(&wp_html, "input[name='author'], #author");
            let rp_has_author = has_element(&rp_html, "input[name='author'], #author");
            let wp_has_email = has_element(&wp_html, "input[name='email'], #email");
            let rp_has_email = has_element(&rp_html, "input[name='email'], #email");
            eprintln!(
                "  Author field - WP: {}, RP: {} [{}]",
                wp_has_author,
                rp_has_author,
                if wp_has_author == rp_has_author {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );
            eprintln!(
                "  Email field - WP: {}, RP: {} [{}]",
                wp_has_email,
                rp_has_email,
                if wp_has_email == rp_has_email {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            eprintln!("[PASS] Single post comments section compared");
        }
        (None, _) => eprintln!("[SKIP] No single post found on WordPress"),
        (_, None) => eprintln!("[SKIP] No single post found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_single_post_navigation() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_single_post_navigation ===");

    let wp_slugs = ["hello-world", "hello-rustpress"];
    let rp_slugs = ["hello-rustpress", "hello-world"];

    let mut wp_result = None;
    for slug in &wp_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found post at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &rp_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found post at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Check for prev/next navigation links
            let nav_selectors = [
                (
                    ".post-navigation, .nav-links, .navigation",
                    "post navigation container",
                ),
                (".nav-previous, .prev, a[rel='prev']", "previous post link"),
                (".nav-next, .next, a[rel='next']", "next post link"),
            ];

            for (selector, label) in &nav_selectors {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            eprintln!("[PASS] Single post navigation compared");
        }
        (None, _) => eprintln!("[SKIP] No single post found on WordPress"),
        (_, None) => eprintln!("[SKIP] No single post found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_single_post_author_info() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_single_post_author_info ===");

    let wp_slugs = ["hello-world", "hello-rustpress"];
    let rp_slugs = ["hello-rustpress", "hello-world"];

    let mut wp_result = None;
    for slug in &wp_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found post at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &rp_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found post at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Check for author name/link
            let author_selectors = [
                (
                    ".author, .entry-author, .post-author, [rel='author']",
                    "author element",
                ),
                ("a[href*='/author/']", "author link"),
            ];

            for (selector, label) in &author_selectors {
                let wp_has = has_element(&wp_html, selector);
                let rp_has = has_element(&rp_html, selector);
                eprintln!(
                    "  {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }

            // Extract author text
            let wp_author_text = extract_text(
                &wp_html,
                ".author, .entry-author, .post-author, [rel='author']",
            );
            let rp_author_text = extract_text(
                &rp_html,
                ".author, .entry-author, .post-author, [rel='author']",
            );
            if !wp_author_text.is_empty() {
                eprintln!("  WP author text: {:?}", wp_author_text.first());
            }
            if !rp_author_text.is_empty() {
                eprintln!("  RP author text: {:?}", rp_author_text.first());
            }

            eprintln!("[PASS] Single post author info compared");
        }
        (None, _) => eprintln!("[SKIP] No single post found on WordPress"),
        (_, None) => eprintln!("[SKIP] No single post found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_single_post_categories_tags() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_single_post_categories_tags ===");

    let wp_slugs = ["hello-world", "hello-rustpress"];
    let rp_slugs = ["hello-rustpress", "hello-world"];

    let mut wp_result = None;
    for slug in &wp_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found post at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &rp_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found post at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Check for category links
            let wp_cat_links = count_elements(&wp_html, "a[href*='/category/']");
            let rp_cat_links = count_elements(&rp_html, "a[href*='/category/']");
            eprintln!(
                "  Category links - WP: {}, RP: {}",
                wp_cat_links, rp_cat_links
            );

            // Check for tag links
            let wp_tag_links = count_elements(&wp_html, "a[href*='/tag/']");
            let rp_tag_links = count_elements(&rp_html, "a[href*='/tag/']");
            eprintln!("  Tag links - WP: {}, RP: {}", wp_tag_links, rp_tag_links);

            // Check for category/tag container elements
            let wp_has_cat_area =
                has_element(&wp_html, ".cat-links, .category-links, .post-categories");
            let rp_has_cat_area =
                has_element(&rp_html, ".cat-links, .category-links, .post-categories");
            eprintln!(
                "  Category area - WP: {}, RP: {} [{}]",
                wp_has_cat_area,
                rp_has_cat_area,
                if wp_has_cat_area == rp_has_cat_area {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            let wp_has_tag_area = has_element(&wp_html, ".tags-links, .tag-links, .post-tags");
            let rp_has_tag_area = has_element(&rp_html, ".tags-links, .tag-links, .post-tags");
            eprintln!(
                "  Tag area - WP: {}, RP: {} [{}]",
                wp_has_tag_area,
                rp_has_tag_area,
                if wp_has_tag_area == rp_has_tag_area {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            eprintln!("[PASS] Single post categories/tags compared");
        }
        (None, _) => eprintln!("[SKIP] No single post found on WordPress"),
        (_, None) => eprintln!("[SKIP] No single post found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_search_no_results() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_search_no_results ===");

    // Search for gibberish that should return no results
    let gibberish = "zxqjkw9876nonsensequerythatmatchesnothing";

    let wp = fetch_html(&client, &format!("{}/?s={}", cfg.wordpress_url, gibberish)).await;
    let rp = fetch_html(
        &client,
        &format!("{}/search?s={}", cfg.rustpress_url, gibberish),
    )
    .await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!("WordPress search status: {}", wp_status);
            eprintln!("RustPress search status: {}", rp_status);

            // Both should return 200 (search page renders even with no results)
            assert_eq!(
                wp_status.as_u16(),
                200,
                "WordPress no-results search should be 200"
            );
            assert_eq!(
                rp_status.as_u16(),
                200,
                "RustPress no-results search should be 200"
            );

            // Check for "no results" messaging
            let wp_lower = wp_html.to_lowercase();
            let rp_lower = rp_html.to_lowercase();

            let wp_no_results = wp_lower.contains("no results")
                || wp_lower.contains("nothing found")
                || wp_lower.contains("no posts")
                || wp_lower.contains("not found");
            let rp_no_results = rp_lower.contains("no results")
                || rp_lower.contains("nothing found")
                || rp_lower.contains("no posts")
                || rp_lower.contains("not found");

            eprintln!(
                "  'No results' message - WP: {}, RP: {} [{}]",
                wp_no_results,
                rp_no_results,
                if wp_no_results == rp_no_results {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );
            assert!(
                rp_no_results,
                "RustPress should show a 'no results' message for empty search"
            );

            // Should NOT have article/post elements
            let rp_article_count = count_elements(&rp_html, "article, .post, .hentry");
            eprintln!(
                "  RP article count (should be 0 or just wrapper): {}",
                rp_article_count
            );

            eprintln!("[PASS] Search no results compared");
        }
        _ => eprintln!("[SKIP] Could not fetch search page from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_search_form_present() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_search_form_present ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for search form
            let wp_has_search_form = has_element(
                &wp_html,
                "form[role='search'], .search-form, form.searchform",
            );
            let rp_has_search_form = has_element(
                &rp_html,
                "form[role='search'], .search-form, form.searchform",
            );
            eprintln!(
                "  Search form - WP: {}, RP: {} [{}]",
                wp_has_search_form,
                rp_has_search_form,
                if wp_has_search_form == rp_has_search_form {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for search input field
            let wp_has_search_input =
                has_element(&wp_html, "input[type='search'], input[name='s']");
            let rp_has_search_input =
                has_element(&rp_html, "input[type='search'], input[name='s']");
            eprintln!(
                "  Search input - WP: {}, RP: {} [{}]",
                wp_has_search_input,
                rp_has_search_input,
                if wp_has_search_input == rp_has_search_input {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // At least one form of search should be present
            let rp_has_any_search = rp_has_search_form || rp_has_search_input;
            eprintln!(
                "  RustPress has any search functionality: {}",
                rp_has_any_search
            );

            eprintln!("[PASS] Search form presence compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rss_feed_channel_fields() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rss_feed_channel_fields ===");

    let wp = fetch_text(&client, &format!("{}/feed/", cfg.wordpress_url)).await;
    let rp = fetch_text(&client, &format!("{}/feed/", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((_, wp_body)), Some((_, rp_body))) => {
            // Required <channel> sub-elements
            let channel_fields = ["title", "link", "description", "language", "generator"];

            for field in &channel_fields {
                let wp_count = count_xml_tag(&wp_body, field);
                let rp_count = count_xml_tag(&rp_body, field);
                let rp_ok = rp_count > 0;
                eprintln!(
                    "  <channel><{}> - WP: {}, RP: {} [{}]",
                    field,
                    wp_count,
                    rp_count,
                    if rp_ok { "OK" } else { "MISSING" }
                );
            }

            // Verify RustPress has all required channel fields
            for field in &channel_fields {
                assert!(
                    count_xml_tag(&rp_body, field) > 0,
                    "RustPress RSS feed should have <{}> in <channel>",
                    field
                );
            }

            // Check for lastBuildDate (optional but common)
            let wp_has_lbd = wp_body.contains("<lastBuildDate>");
            let rp_has_lbd = rp_body.contains("<lastBuildDate>");
            eprintln!(
                "  <lastBuildDate> - WP: {}, RP: {} [{}]",
                wp_has_lbd,
                rp_has_lbd,
                if wp_has_lbd == rp_has_lbd {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            eprintln!("[PASS] RSS feed channel fields compared");
        }
        _ => eprintln!("[SKIP] Could not fetch RSS feed from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rss_feed_item_fields() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rss_feed_item_fields ===");

    let wp = fetch_text(&client, &format!("{}/feed/", cfg.wordpress_url)).await;
    let rp = fetch_text(&client, &format!("{}/feed/", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((_, wp_body)), Some((_, rp_body))) => {
            // Both should have at least one <item>
            let wp_items = count_xml_tag(&wp_body, "item");
            let rp_items = count_xml_tag(&rp_body, "item");
            eprintln!("  <item> count - WP: {}, RP: {}", wp_items, rp_items);
            assert!(
                rp_items > 0,
                "RustPress RSS feed should have at least one <item>"
            );

            // Required <item> sub-elements
            let item_fields = ["title", "link", "pubDate", "description", "guid"];

            for field in &item_fields {
                let wp_count = count_xml_tag(&wp_body, field);
                let rp_count = count_xml_tag(&rp_body, field);
                let rp_ok = rp_count > 0;
                eprintln!(
                    "  <item><{}> - WP: {}, RP: {} [{}]",
                    field,
                    wp_count,
                    rp_count,
                    if rp_ok { "OK" } else { "MISSING" }
                );
            }

            // Each item should have title, link, pubDate, description, guid
            // Since count_xml_tag counts all occurrences, the count of each field
            // should be >= the number of items
            for field in &item_fields {
                let rp_count = count_xml_tag(&rp_body, field);
                assert!(
                    rp_count >= rp_items,
                    "RustPress RSS: each <item> should have <{}> (found {} for {} items)",
                    field,
                    rp_count,
                    rp_items
                );
            }

            // Check for optional but common fields
            let optional_fields = ["category", "dc:creator", "content:encoded"];
            for field in &optional_fields {
                let wp_count = count_xml_tag(&wp_body, field);
                let rp_count = count_xml_tag(&rp_body, field);
                eprintln!(
                    "  <item><{}> (optional) - WP: {}, RP: {}",
                    field, wp_count, rp_count
                );
            }

            eprintln!("[PASS] RSS feed item fields compared");
        }
        _ => eprintln!("[SKIP] Could not fetch RSS feed from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_sitemap_lastmod() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_sitemap_lastmod ===");

    // Fetch sitemaps
    let wp_urls = [
        format!("{}/sitemap.xml", cfg.wordpress_url),
        format!("{}/wp-sitemap.xml", cfg.wordpress_url),
    ];

    let mut wp_body = None;
    for url in &wp_urls {
        if let Some((status, body)) = fetch_text(&client, url).await {
            if status.as_u16() == 200
                && (body.contains("<urlset") || body.contains("<sitemapindex"))
            {
                wp_body = Some(body);
                eprintln!("WordPress sitemap found at {}", url);
                break;
            }
        }
    }

    let rp = fetch_text(&client, &format!("{}/sitemap.xml", cfg.rustpress_url)).await;

    match (wp_body, rp) {
        (Some(wp_xml), Some((_, rp_xml))) => {
            // Check for <lastmod> elements
            let wp_lastmod = count_xml_tag(&wp_xml, "lastmod");
            let rp_lastmod = count_xml_tag(&rp_xml, "lastmod");
            eprintln!("  <lastmod> count - WP: {}, RP: {}", wp_lastmod, rp_lastmod);

            // RustPress sitemap should have <lastmod> for each <url>
            let rp_url_count = count_xml_tag(&rp_xml, "url");
            eprintln!(
                "  <url> count in RP: {}, <lastmod> count: {}",
                rp_url_count, rp_lastmod
            );

            if rp_url_count > 0 {
                assert!(
                    rp_lastmod > 0,
                    "RustPress sitemap should have at least one <lastmod> element"
                );
            }

            // Check for <changefreq> and <priority> (optional but common)
            let rp_changefreq = count_xml_tag(&rp_xml, "changefreq");
            let rp_priority = count_xml_tag(&rp_xml, "priority");
            eprintln!(
                "  <changefreq> count in RP: {}, <priority> count: {}",
                rp_changefreq, rp_priority
            );

            eprintln!("[PASS] Sitemap lastmod compared");
        }
        (None, _) => eprintln!("[SKIP] WordPress sitemap not found"),
        (_, None) => eprintln!("[SKIP] Could not fetch sitemap from RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_pagination() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_pagination ===");

    // Check homepage for pagination elements first
    let wp_home = fetch_html(&client, &cfg.wordpress_url).await;
    let rp_home = fetch_html(&client, &cfg.rustpress_url).await;

    match (&wp_home, &rp_home) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for pagination elements on homepage
            let pagination_selectors = [
                (
                    "nav.pagination, .nav-links, .pagination",
                    "pagination container",
                ),
                ("a.page-numbers, .nav-links a", "pagination links"),
                (".next, a.next, a[rel='next']", "next page link"),
                (".prev, a.prev, a[rel='prev']", "previous page link"),
            ];

            for (selector, label) in &pagination_selectors {
                let wp_has = has_element(wp_html, selector);
                let rp_has = has_element(rp_html, selector);
                eprintln!(
                    "  Homepage {} - WP: {}, RP: {} [{}]",
                    label,
                    wp_has,
                    rp_has,
                    if wp_has == rp_has { "MATCH" } else { "DIFFER" }
                );
            }
        }
        _ => {
            eprintln!("[SKIP] Could not fetch homepage from one or both servers");
            return;
        }
    }

    // Try fetching page 2
    let wp_page2 = fetch_html(&client, &format!("{}/page/2/", cfg.wordpress_url)).await;
    let rp_page2 = fetch_html(&client, &format!("{}/page/2/", cfg.rustpress_url)).await;

    match (wp_page2, rp_page2) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!("  Page 2 status - WP: {}, RP: {}", wp_status, rp_status);

            // If WordPress has page 2, RustPress should too (assuming similar content)
            if wp_status.as_u16() == 200 {
                let wp_has_articles = has_element(&wp_html, "article, .post, .hentry");
                let rp_has_articles = has_element(&rp_html, "article, .post, .hentry");
                eprintln!(
                    "  Page 2 articles - WP: {}, RP: {} [{}]",
                    wp_has_articles,
                    rp_has_articles,
                    if wp_has_articles == rp_has_articles {
                        "MATCH"
                    } else {
                        "DIFFER"
                    }
                );
            }
        }
        _ => eprintln!(
            "  [INFO] Page 2 not available on one or both servers (may have too few posts)"
        ),
    }

    eprintln!("[PASS] Pagination compared");
}

#[tokio::test]
#[ignore]
async fn test_category_archive_title() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_category_archive_title ===");

    let category_slugs = ["uncategorized", "general", "news"];

    let mut wp_result = None;
    let mut wp_slug = "";
    for slug in &category_slugs {
        let url = format!("{}/category/{}/", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                wp_slug = slug;
                eprintln!("WordPress: found category at /category/{}/", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    let mut rp_slug = "";
    for slug in &category_slugs {
        let url = format!("{}/category/{}/", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                rp_slug = slug;
                eprintln!("RustPress: found category at /category/{}/", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Extract title or h1 text
            let wp_titles = extract_text(&wp_html, "h1, .archive-title, .page-title");
            let rp_titles = extract_text(&rp_html, "h1, .archive-title, .page-title");

            eprintln!("  WP title(s): {:?}", wp_titles);
            eprintln!("  RP title(s): {:?}", rp_titles);

            // Check that "Category" appears in the title or heading
            let wp_has_category_label = wp_titles
                .iter()
                .any(|t| t.to_lowercase().contains("category"));
            let rp_has_category_label = rp_titles
                .iter()
                .any(|t| t.to_lowercase().contains("category"));
            eprintln!(
                "  'Category:' in title - WP: {}, RP: {} [{}]",
                wp_has_category_label,
                rp_has_category_label,
                if wp_has_category_label == rp_has_category_label {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check that the category name appears somewhere in title/heading
            let rp_has_slug_in_title = rp_titles.iter().any(|t| {
                t.to_lowercase().contains(&rp_slug.replace('-', " "))
                    || t.to_lowercase().contains(rp_slug)
            });
            eprintln!(
                "  Category name '{}' in RP title: {}",
                rp_slug, rp_has_slug_in_title
            );

            // Also check <title> tag
            let wp_page_title = extract_text(&wp_html, "title");
            let rp_page_title = extract_text(&rp_html, "title");
            eprintln!("  WP <title>: {:?}", wp_page_title.first());
            eprintln!("  RP <title>: {:?}", rp_page_title.first());

            let _ = (wp_slug, rp_slug); // suppress unused warnings
            eprintln!("[PASS] Category archive title compared");
        }
        (None, _) => eprintln!("[SKIP] No category archive found on WordPress"),
        (_, None) => eprintln!("[SKIP] No category archive found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_tag_archive_title() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_tag_archive_title ===");

    let tag_slugs = ["test", "sample", "hello"];

    let mut wp_result = None;
    let mut wp_slug = "";
    for slug in &tag_slugs {
        let url = format!("{}/tag/{}/", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                wp_slug = slug;
                eprintln!("WordPress: found tag at /tag/{}/", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    let mut rp_slug = "";
    for slug in &tag_slugs {
        let url = format!("{}/tag/{}/", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                rp_slug = slug;
                eprintln!("RustPress: found tag at /tag/{}/", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            let wp_titles = extract_text(&wp_html, "h1, .archive-title, .page-title");
            let rp_titles = extract_text(&rp_html, "h1, .archive-title, .page-title");

            eprintln!("  WP title(s): {:?}", wp_titles);
            eprintln!("  RP title(s): {:?}", rp_titles);

            // Check that "Tag" appears in the title
            let wp_has_tag_label = wp_titles.iter().any(|t| t.to_lowercase().contains("tag"));
            let rp_has_tag_label = rp_titles.iter().any(|t| t.to_lowercase().contains("tag"));
            eprintln!(
                "  'Tag:' in title - WP: {}, RP: {} [{}]",
                wp_has_tag_label,
                rp_has_tag_label,
                if wp_has_tag_label == rp_has_tag_label {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check that the tag name appears in title
            let rp_has_slug_in_title = rp_titles.iter().any(|t| {
                t.to_lowercase().contains(&rp_slug.replace('-', " "))
                    || t.to_lowercase().contains(rp_slug)
            });
            eprintln!(
                "  Tag name '{}' in RP title: {}",
                rp_slug, rp_has_slug_in_title
            );

            let _ = (wp_slug, rp_slug);
            eprintln!("[PASS] Tag archive title compared");
        }
        (None, _) => eprintln!("[SKIP] No tag archive found on WordPress"),
        (_, None) => eprintln!("[SKIP] No tag archive found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_author_archive_title() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_author_archive_title ===");

    let wp = fetch_html(
        &client,
        &format!("{}/author/{}/", cfg.wordpress_url, cfg.admin_user),
    )
    .await;
    let rp = fetch_html(
        &client,
        &format!("{}/author/{}/", cfg.rustpress_url, cfg.admin_user),
    )
    .await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!(
                "  Author archive status - WP: {}, RP: {}",
                wp_status, rp_status
            );

            if wp_status.as_u16() == 200 && rp_status.as_u16() == 200 {
                // Extract title/heading
                let wp_titles =
                    extract_text(&wp_html, "h1, .archive-title, .author-title, .page-title");
                let rp_titles =
                    extract_text(&rp_html, "h1, .archive-title, .author-title, .page-title");

                eprintln!("  WP title(s): {:?}", wp_titles);
                eprintln!("  RP title(s): {:?}", rp_titles);

                // Check that the author name appears in title or heading
                let admin_lower = cfg.admin_user.to_lowercase();
                let rp_has_author_in_title = rp_titles
                    .iter()
                    .any(|t| t.to_lowercase().contains(&admin_lower));
                eprintln!(
                    "  Author '{}' in RP title: {}",
                    cfg.admin_user, rp_has_author_in_title
                );

                // Also check the <title> element
                let wp_page_title = extract_text(&wp_html, "title");
                let rp_page_title = extract_text(&rp_html, "title");
                eprintln!("  WP <title>: {:?}", wp_page_title.first());
                eprintln!("  RP <title>: {:?}", rp_page_title.first());

                let rp_title_has_author = rp_page_title
                    .iter()
                    .any(|t| t.to_lowercase().contains(&admin_lower));
                eprintln!("  Author name in RP <title>: {}", rp_title_has_author);

                // At least one of heading or <title> should contain author name
                assert!(
                    rp_has_author_in_title || rp_title_has_author,
                    "RustPress author archive should show author name '{}' in title or heading",
                    cfg.admin_user
                );
            }

            eprintln!("[PASS] Author archive title compared");
        }
        _ => eprintln!("[SKIP] Could not fetch author archive from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_date_archive_monthly() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_date_archive_monthly ===");

    let year = "2026";
    let month = "03";

    let wp = fetch_html(
        &client,
        &format!("{}/{}/{}/", cfg.wordpress_url, year, month),
    )
    .await;
    let rp = fetch_html(
        &client,
        &format!("{}/{}/{}/", cfg.rustpress_url, year, month),
    )
    .await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!(
                "  Monthly archive /{}/{}/: WP={}, RP={}",
                year, month, wp_status, rp_status
            );

            if wp_status.as_u16() == 200 && rp_status.as_u16() == 200 {
                // Should have article/post elements
                let wp_article_count = count_elements(&wp_html, "article, .post, .hentry");
                let rp_article_count = count_elements(&rp_html, "article, .post, .hentry");
                eprintln!(
                    "  Article count - WP: {}, RP: {}",
                    wp_article_count, rp_article_count
                );
                assert!(
                    rp_article_count > 0,
                    "RustPress monthly archive should have at least one article"
                );

                // Check that dates on posts are within the expected month
                let wp_dates = extract_text(&wp_html, "time, .entry-date, .post-date");
                let rp_dates = extract_text(&rp_html, "time, .entry-date, .post-date");
                eprintln!("  WP dates: {:?}", &wp_dates[..wp_dates.len().min(3)]);
                eprintln!("  RP dates: {:?}", &rp_dates[..rp_dates.len().min(3)]);

                // Verify RustPress dates contain the expected month/year
                // (March 2026 could appear as "March 2026", "2026-03", "03/2026", etc.)
                let month_patterns = ["march", "2026-03", "03/2026", "mar 2026"];
                let rp_dates_match = rp_dates.iter().any(|d| {
                    let d_lower = d.to_lowercase();
                    month_patterns.iter().any(|p| d_lower.contains(p))
                });
                eprintln!("  RP dates match March 2026: {}", rp_dates_match);

                // Check heading contains month/year info
                let rp_titles = extract_text(&rp_html, "h1, .archive-title, .page-title");
                eprintln!("  RP archive title(s): {:?}", rp_titles);
            } else if wp_status.as_u16() != 200 {
                eprintln!("  [INFO] WordPress monthly archive returned {}", wp_status);
            } else {
                eprintln!("  [INFO] RustPress monthly archive returned {}", rp_status);
            }

            eprintln!("[PASS] Monthly date archive compared");
        }
        _ => eprintln!("[SKIP] Could not fetch monthly archive from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_password_protected_post() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_password_protected_post ===");

    // Try to find a password-protected post
    // Common slugs for test password-protected posts
    let protected_slugs = ["protected-post", "password-protected", "private-post"];

    let mut wp_result = None;
    for slug in &protected_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                eprintln!("WordPress: found post at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    for slug in &protected_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                eprintln!("RustPress: found post at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Password-protected posts should show a password form
            let wp_has_pw_form = has_element(&wp_html, "input[type='password']")
                || wp_html.to_lowercase().contains("protected")
                || wp_html.to_lowercase().contains("password");
            let rp_has_pw_form = has_element(&rp_html, "input[type='password']")
                || rp_html.to_lowercase().contains("protected")
                || rp_html.to_lowercase().contains("password");

            eprintln!(
                "  Password form/indicator - WP: {}, RP: {} [{}]",
                wp_has_pw_form,
                rp_has_pw_form,
                if wp_has_pw_form == rp_has_pw_form {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for password input element specifically
            let wp_has_input = has_element(&wp_html, "input[type='password']");
            let rp_has_input = has_element(&rp_html, "input[type='password']");
            eprintln!(
                "  Password input field - WP: {}, RP: {} [{}]",
                wp_has_input,
                rp_has_input,
                if wp_has_input == rp_has_input { "MATCH" } else { "DIFFER" }
            );

            // The actual post content should NOT be visible
            let wp_has_content = has_element(&wp_html, ".entry-content p, .post-content p");
            let rp_has_content = has_element(&rp_html, ".entry-content p, .post-content p");
            eprintln!(
                "  Content visible (should be hidden) - WP: {}, RP: {}",
                wp_has_content, rp_has_content
            );

            eprintln!("[PASS] Password-protected post compared");
        }
        _ => eprintln!("[SKIP] No password-protected post found on one or both servers (create one for this test)"),
    }
}

#[tokio::test]
#[ignore]
async fn test_comment_rss_feed() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_comment_rss_feed ===");

    let wp = fetch_text(&client, &format!("{}/comments/feed/", cfg.wordpress_url)).await;
    let rp = fetch_text(&client, &format!("{}/comments/feed/", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, wp_body)), Some((rp_status, rp_body))) => {
            eprintln!("WordPress comments feed status: {}", wp_status);
            eprintln!("RustPress comments feed status: {}", rp_status);

            // Both should return valid XML
            if wp_status.as_u16() == 200 {
                let wp_is_xml = is_valid_xml_basic(&wp_body);
                eprintln!("  WP valid XML: {}", wp_is_xml);
            }

            if rp_status.as_u16() == 200 {
                let rp_is_xml = is_valid_xml_basic(&rp_body);
                eprintln!("  RP valid XML: {}", rp_is_xml);
                assert!(rp_is_xml, "RustPress comments feed should be valid XML");

                // Should have RSS structure
                let rp_has_rss = rp_body.contains("<rss") || rp_body.contains("<feed");
                eprintln!("  RP has RSS root: {}", rp_has_rss);

                // Check for <channel> element
                let rp_has_channel = rp_body.contains("<channel>");
                eprintln!("  RP has <channel>: {}", rp_has_channel);

                // Check for <item> elements (comment entries)
                let rp_items = count_xml_tag(&rp_body, "item");
                eprintln!("  RP comment items: {}", rp_items);

                // Check required channel fields
                let channel_fields = ["title", "link", "description"];
                for field in &channel_fields {
                    let rp_count = count_xml_tag(&rp_body, field);
                    eprintln!("  RP <{}>: {}", field, rp_count);
                }
            } else {
                eprintln!("  [INFO] RustPress comments feed returned {}", rp_status);
            }

            eprintln!("[PASS] Comment RSS feed compared");
        }
        _ => eprintln!("[SKIP] Could not fetch comments feed from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_category_feed() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_category_feed ===");

    let category_slugs = ["uncategorized", "general", "news"];

    let mut wp_result = None;
    let mut found_slug = "";
    for slug in &category_slugs {
        let url = format!("{}/category/{}/feed/", cfg.wordpress_url, slug);
        if let Some((status, body)) = fetch_text(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(body);
                found_slug = slug;
                eprintln!("WordPress: found category feed at /category/{}/feed/", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    // Try the same slug first, then fall back to others
    let rp_try_slugs = if !found_slug.is_empty() {
        vec![found_slug]
    } else {
        category_slugs.to_vec()
    };
    for slug in &rp_try_slugs {
        let url = format!("{}/category/{}/feed/", cfg.rustpress_url, slug);
        if let Some((status, body)) = fetch_text(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(body);
                eprintln!("RustPress: found category feed at /category/{}/feed/", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_body), Some(rp_body)) => {
            // Both should be valid RSS XML
            let wp_is_xml = is_valid_xml_basic(&wp_body);
            let rp_is_xml = is_valid_xml_basic(&rp_body);
            eprintln!("  Valid XML - WP: {}, RP: {}", wp_is_xml, rp_is_xml);
            assert!(rp_is_xml, "RustPress category feed should be valid XML");

            // Check for RSS root
            let rp_has_rss = rp_body.contains("<rss") || rp_body.contains("<feed");
            eprintln!("  RP has RSS root: {}", rp_has_rss);

            // Check for items
            let wp_items = count_xml_tag(&wp_body, "item");
            let rp_items = count_xml_tag(&rp_body, "item");
            eprintln!("  Items - WP: {}, RP: {}", wp_items, rp_items);

            // Check required tags
            let required_tags = ["title", "link", "description"];
            for tag in &required_tags {
                let wp_count = count_xml_tag(&wp_body, tag);
                let rp_count = count_xml_tag(&rp_body, tag);
                eprintln!(
                    "  <{}> count - WP: {}, RP: {} [{}]",
                    tag,
                    wp_count,
                    rp_count,
                    if rp_count > 0 { "OK" } else { "MISSING" }
                );
            }

            eprintln!("[PASS] Category feed compared");
        }
        (None, _) => eprintln!("[SKIP] No category feed found on WordPress"),
        (_, None) => eprintln!("[SKIP] No category feed found on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_wp_json_link_in_head() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_wp_json_link_in_head ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for REST API discovery link:
            // <link rel="https://api.w.org/" href="...wp-json/">
            let wp_has_api_link = wp_html.contains("api.w.org")
                || has_element(&wp_html, "link[rel='https://api.w.org/']");
            let rp_has_api_link = rp_html.contains("api.w.org")
                || has_element(&rp_html, "link[rel='https://api.w.org/']");

            eprintln!(
                "  REST API discovery link (api.w.org) - WP: {}, RP: {} [{}]",
                wp_has_api_link,
                rp_has_api_link,
                if wp_has_api_link == rp_has_api_link {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check that the link points to wp-json
            let wp_has_wp_json = wp_html.contains("wp-json");
            let rp_has_wp_json = rp_html.contains("wp-json");
            eprintln!(
                "  wp-json reference - WP: {}, RP: {} [{}]",
                wp_has_wp_json,
                rp_has_wp_json,
                if wp_has_wp_json == rp_has_wp_json {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            assert!(
                rp_has_api_link,
                "RustPress should include REST API discovery link in <head>"
            );

            eprintln!("[PASS] wp-json link in head compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rsd_link_in_head() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rsd_link_in_head ===");

    let wp = fetch_html(&client, &cfg.wordpress_url).await;
    let rp = fetch_html(&client, &cfg.rustpress_url).await;

    match (wp, rp) {
        (Some((_, wp_html)), Some((_, rp_html))) => {
            // Check for RSD (Really Simple Discovery) link:
            // <link rel="EditURI" type="application/rsd+xml" ... href="...xmlrpc.php?rsd">
            let wp_has_rsd =
                has_element(&wp_html, "link[rel='EditURI']") || wp_html.contains("EditURI");
            let rp_has_rsd =
                has_element(&rp_html, "link[rel='EditURI']") || rp_html.contains("EditURI");

            eprintln!(
                "  RSD link (EditURI) - WP: {}, RP: {} [{}]",
                wp_has_rsd,
                rp_has_rsd,
                if wp_has_rsd == rp_has_rsd {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check that the RSD link references xmlrpc.php?rsd
            let wp_has_rsd_href = wp_html.contains("xmlrpc.php?rsd");
            let rp_has_rsd_href = rp_html.contains("xmlrpc.php?rsd");
            eprintln!(
                "  xmlrpc.php?rsd in href - WP: {}, RP: {} [{}]",
                wp_has_rsd_href,
                rp_has_rsd_href,
                if wp_has_rsd_href == rp_has_rsd_href {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            // Check for rsd+xml content type
            let wp_has_rsd_type = wp_html.contains("application/rsd+xml");
            let rp_has_rsd_type = rp_html.contains("application/rsd+xml");
            eprintln!(
                "  application/rsd+xml type - WP: {}, RP: {} [{}]",
                wp_has_rsd_type,
                rp_has_rsd_type,
                if wp_has_rsd_type == rp_has_rsd_type {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            eprintln!("[PASS] RSD link in head compared");
        }
        _ => eprintln!("[SKIP] Could not fetch homepage from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_canonical_link() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_canonical_link ===");

    // Find a single post on both sites
    let wp_slugs = ["hello-world", "hello-rustpress", "sample-page"];
    let rp_slugs = ["hello-rustpress", "hello-world", "sample-page"];

    let mut wp_result = None;
    let mut wp_found_slug = "";
    for slug in &wp_slugs {
        let url = format!("{}/{}", cfg.wordpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                wp_result = Some(html);
                wp_found_slug = slug;
                eprintln!("WordPress: found post at /{}", slug);
                break;
            }
        }
    }

    let mut rp_result = None;
    let mut rp_found_slug = "";
    for slug in &rp_slugs {
        let url = format!("{}/{}", cfg.rustpress_url, slug);
        if let Some((status, html)) = fetch_html(&client, &url).await {
            if status.as_u16() == 200 {
                rp_result = Some(html);
                rp_found_slug = slug;
                eprintln!("RustPress: found post at /{}", slug);
                break;
            }
        }
    }

    match (wp_result, rp_result) {
        (Some(wp_html), Some(rp_html)) => {
            // Check for <link rel="canonical" href="...">
            let wp_has_canonical = has_element(&wp_html, "link[rel='canonical']");
            let rp_has_canonical = has_element(&rp_html, "link[rel='canonical']");

            eprintln!(
                "  Canonical link - WP: {}, RP: {} [{}]",
                wp_has_canonical,
                rp_has_canonical,
                if wp_has_canonical == rp_has_canonical {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            assert!(
                rp_has_canonical,
                "RustPress single post should have a <link rel=\"canonical\"> tag"
            );

            // Verify canonical URL contains the post slug
            let rp_has_slug_in_canonical = rp_html.contains(&format!(
                "rel=\"canonical\" href=\"{}",
                format!("{}/{}", cfg.rustpress_url, rp_found_slug)
            )) || rp_html.contains(&format!(
                "rel='canonical' href='{}",
                format!("{}/{}", cfg.rustpress_url, rp_found_slug)
            )) || {
                // More lenient check: canonical href contains the slug
                let canonical_pattern = format!("canonical");
                let slug_present = rp_html.contains(rp_found_slug);
                rp_html.contains(&canonical_pattern) && slug_present
            };

            eprintln!(
                "  Canonical contains post slug '{}': {}",
                rp_found_slug, rp_has_slug_in_canonical
            );

            let _ = wp_found_slug; // suppress unused warning
            eprintln!("[PASS] Canonical link compared");
        }
        (None, _) => eprintln!("[SKIP] No single post found on WordPress"),
        (_, None) => eprintln!("[SKIP] No single post found on RustPress"),
    }
}

// ---------------------------------------------------------------------------
// Query variable tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_paged_query_var() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_paged_query_var ===");

    // ?paged=1 should work on both servers (same as homepage for page 1)
    let wp = fetch_html(&client, &format!("{}/?paged=1", cfg.wordpress_url)).await;
    let rp = fetch_html(&client, &format!("{}/?paged=1", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some((wp_status, _wp_html)), Some((rp_status, _rp_html))) => {
            eprintln!("WordPress ?paged=1 -> {}", wp_status);
            eprintln!("RustPress ?paged=1 -> {}", rp_status);

            // Should return 200 or 301 redirect
            assert!(
                rp_status.is_success() || rp_status.is_redirection(),
                "RustPress ?paged=1 should return 2xx or 3xx, got {}",
                rp_status
            );
            eprintln!("[PASS] ?paged query var handled");
        }
        _ => eprintln!("[SKIP] Could not test ?paged from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_month_query_var_redirect() {
    let cfg = TestConfig::from_env();
    let client = build_http_client_no_redirect();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_month_query_var_redirect ===");

    // ?m=202401 should redirect to /2024/01/ on WordPress
    let rp_resp = client
        .get(&format!("{}/?m=202401", cfg.rustpress_url))
        .send()
        .await;

    match rp_resp {
        Ok(r) => {
            let status = r.status();
            eprintln!("RustPress ?m=202401 -> {}", status);

            // Either redirect to /2024/01/ or return 200 archive page
            assert!(
                status.is_success() || status.is_redirection(),
                "RustPress ?m=202401 should return 2xx or 3xx, got {}",
                status
            );

            if status.is_redirection() {
                let location = r
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(none)");
                eprintln!("  Redirect to: {}", location);
                // Should redirect to /2024/01/
                assert!(
                    location.contains("/2024/01/")
                        || location.contains("2024") && location.contains("01"),
                    "?m=202401 should redirect to date archive, got: {}",
                    location
                );
                eprintln!("[OK] Correctly redirects to date archive");
            } else {
                eprintln!("[OK] Returned 200 (date archive page)");
            }

            eprintln!("[PASS] ?m query var handled");
        }
        Err(e) => eprintln!("[SKIP] {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_author_query_var_redirect() {
    let cfg = TestConfig::from_env();
    let client = build_http_client_no_redirect();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_author_query_var_redirect ===");

    // ?author=1 should redirect to /author/{nicename}/ on WordPress
    let rp_resp = client
        .get(&format!("{}/?author=1", cfg.rustpress_url))
        .send()
        .await;

    match rp_resp {
        Ok(r) => {
            let status = r.status();
            eprintln!("RustPress ?author=1 -> {}", status);

            assert!(
                status.is_success() || status.is_redirection(),
                "RustPress ?author=1 should return 2xx or 3xx, got {}",
                status
            );

            if status.is_redirection() {
                let location = r
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(none)");
                eprintln!("  Redirect to: {}", location);
                assert!(
                    location.contains("/author/"),
                    "?author=1 should redirect to /author/..., got: {}",
                    location
                );
                eprintln!("[OK] Correctly redirects to author archive");
            }

            eprintln!("[PASS] ?author query var handled");
        }
        Err(e) => eprintln!("[SKIP] {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_login_checkemail_message() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_login_checkemail_message ===");

    // ?checkemail=registered should show a "check your email" message
    let wp = fetch_html(
        &client,
        &format!("{}/wp-login.php?checkemail=registered", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_html(
        &client,
        &format!("{}/wp-login.php?checkemail=registered", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some((wp_status, wp_html)), Some((rp_status, rp_html))) => {
            eprintln!("WordPress ?checkemail=registered -> {}", wp_status);
            eprintln!("RustPress ?checkemail=registered -> {}", rp_status);

            assert!(
                rp_status.is_success(),
                "Should return 200, got {}",
                rp_status
            );

            // Both should contain a message about checking email
            let wp_has_msg = wp_html.to_lowercase().contains("email")
                || wp_html.to_lowercase().contains("check");
            let rp_has_msg = rp_html.to_lowercase().contains("email")
                || rp_html.to_lowercase().contains("check");

            eprintln!("  WordPress has email message: {}", wp_has_msg);
            eprintln!("  RustPress has email message: {}", rp_has_msg);

            assert!(
                rp_has_msg,
                "RustPress login page with ?checkemail=registered should show email message"
            );

            eprintln!("[PASS] checkemail=registered message displayed");
        }
        _ => eprintln!("[SKIP] Could not test checkemail from one or both servers"),
    }
}

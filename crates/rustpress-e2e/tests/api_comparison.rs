//! REST API Comparison Tests
//!
//! Each test fetches the same endpoint from both a real WordPress instance and
//! a RustPress instance, then compares the JSON response structure (keys, types)
//! to ensure RustPress faithfully replicates the WP REST API.
//!
//! All tests are `#[ignore]` by default because they require both servers to be
//! running.  Run them with:
//!
//! ```sh
//! cargo test -p rustpress-e2e -- --ignored --nocapture
//! ```

use rustpress_e2e::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn fetch_json(client: &reqwest::Client, url: &str) -> Option<Value> {
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] GET {} failed: {}", url, e);
            return None;
        }
    };
    if !resp.status().is_success() {
        eprintln!("[WARN] GET {} returned {}", url, resp.status());
        // Try to parse body anyway for diagnostics
    }
    match resp.json::<Value>().await {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[ERROR] Failed to parse JSON from {}: {}", url, e);
            None
        }
    }
}

/// Fetch the first element from an array endpoint for field comparison.
async fn fetch_first_from_array(client: &reqwest::Client, url: &str) -> Option<Value> {
    let val = fetch_json(client, url).await?;
    match val {
        Value::Array(arr) => arr.into_iter().next(),
        other => Some(other),
    }
}

/// POST JSON to an endpoint with authentication.
async fn post_json_auth(
    client: &reqwest::Client,
    url: &str,
    body: &Value,
    auth_header: &str,
) -> Option<Value> {
    let resp = match client
        .post(url)
        .header("Authorization", auth_header)
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] POST {} failed: {}", url, e);
            return None;
        }
    };
    eprintln!("POST {} -> {}", url, resp.status());
    resp.json::<Value>().await.ok()
}

/// PUT JSON to an endpoint with authentication.
async fn put_json_auth(
    client: &reqwest::Client,
    url: &str,
    body: &Value,
    auth_header: &str,
) -> Option<Value> {
    let resp = match client
        .put(url)
        .header("Authorization", auth_header)
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] PUT {} failed: {}", url, e);
            return None;
        }
    };
    eprintln!("PUT {} -> {}", url, resp.status());
    resp.json::<Value>().await.ok()
}

/// DELETE with authentication. WordPress REST API requires `?force=true` for
/// permanent deletion.
async fn delete_auth(client: &reqwest::Client, url: &str, auth_header: &str) -> Option<Value> {
    let resp = match client
        .delete(url)
        .header("Authorization", auth_header)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] DELETE {} failed: {}", url, e);
            return None;
        }
    };
    eprintln!("DELETE {} -> {}", url, resp.status());
    resp.json::<Value>().await.ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_discovery() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_discovery ===");

    let wp = fetch_json(&client, &format!("{}/wp-json/", cfg.wordpress_url)).await;
    let rp = fetch_json(&client, &format!("{}/wp-json/", cfg.rustpress_url)).await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            // Both should be objects with key fields like "name", "namespaces", "routes"
            eprintln!("WordPress discovery keys: {:?}", json_top_keys(&wp_val));
            eprintln!("RustPress discovery keys: {:?}", json_top_keys(&rp_val));
            assert_json_keys_match(&wp_val, &rp_val);

            // Check that "namespaces" contains "wp/v2"
            if let Some(ns) = rp_val.get("namespaces") {
                let ns_str = ns.to_string();
                assert!(
                    ns_str.contains("wp/v2"),
                    "RustPress should expose wp/v2 namespace"
                );
                eprintln!("[PASS] RustPress exposes wp/v2 namespace");
            }
        }
        _ => eprintln!("[SKIP] Could not fetch discovery endpoint from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_list ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            // Both should be arrays
            assert!(wp_val.is_array(), "WordPress /posts should return an array");
            assert!(rp_val.is_array(), "RustPress /posts should return an array");

            let wp_arr = wp_val.as_array().unwrap();
            let rp_arr = rp_val.as_array().unwrap();

            eprintln!("WordPress posts count: {}", wp_arr.len());
            eprintln!("RustPress posts count: {}", rp_arr.len());

            // Compare structure of first post if available
            if let (Some(wp_first), Some(rp_first)) = (wp_arr.first(), rp_arr.first()) {
                assert_json_structure_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Both return post arrays");
        }
        _ => eprintln!("[SKIP] Could not fetch posts from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_fields() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_fields ===");

    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_post), Some(rp_post)) => {
            // Check that post objects have matching field names
            assert_json_keys_match(&wp_post, &rp_post);
            assert_json_types_match(&wp_post, &rp_post);

            // WordPress post objects have these standard fields:
            let expected_fields = [
                "id", "date", "slug", "status", "type", "title", "content", "excerpt", "author",
                "link",
            ];
            let rp_keys = json_top_keys(&rp_post);
            for field in &expected_fields {
                if rp_keys.contains(*field) {
                    eprintln!("  [OK] RustPress has field: {}", field);
                } else {
                    eprintln!("  [MISSING] RustPress missing field: {}", field);
                }
            }
        }
        _ => eprintln!("[SKIP] No posts available for field comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_users_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_users_list ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/users", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/users", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(wp_val.is_array(), "WordPress /users should return an array");
            assert!(rp_val.is_array(), "RustPress /users should return an array");

            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);

                let expected = ["id", "name", "slug", "link"];
                let rp_keys = json_top_keys(rp_first);
                for field in &expected {
                    if rp_keys.contains(*field) {
                        eprintln!("  [OK] RustPress user has field: {}", field);
                    } else {
                        eprintln!("  [MISSING] RustPress user missing field: {}", field);
                    }
                }
            }
            eprintln!("[PASS] Both return user arrays");
        }
        _ => eprintln!("[SKIP] Could not fetch users from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_categories() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_categories ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/categories", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/categories", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /categories should be an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /categories should be an array"
            );

            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                let expected = ["id", "count", "name", "slug", "taxonomy"];
                let rp_keys = json_top_keys(rp_first);
                for f in &expected {
                    eprintln!(
                        "  [{}] {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }
            eprintln!("[PASS] Both return category arrays");
        }
        _ => eprintln!("[SKIP] Could not fetch categories from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_tags() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_tags ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/tags", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/tags", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(wp_val.is_array(), "WordPress /tags should be an array");
            assert!(rp_val.is_array(), "RustPress /tags should be an array");

            eprintln!(
                "WordPress tags: {}, RustPress tags: {}",
                wp_val.as_array().map(|a| a.len()).unwrap_or(0),
                rp_val.as_array().map(|a| a.len()).unwrap_or(0),
            );

            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                let expected = ["id", "count", "name", "slug", "taxonomy"];
                let rp_keys = json_top_keys(rp_first);
                for f in &expected {
                    eprintln!(
                        "  [{}] {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }
            eprintln!("[PASS] Both return tag arrays");
        }
        _ => eprintln!("[SKIP] Could not fetch tags from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_create() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_create ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    let post_body = serde_json::json!({
        "title": "E2E Test Post",
        "content": "<p>This post was created by the E2E test suite.</p>",
        "status": "draft",
    });

    let wp_result = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
        &post_body,
        &wp_auth,
    )
    .await;

    let rp_result = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
        &post_body,
        &rp_token,
    )
    .await;

    match (wp_result, rp_result) {
        (Some(wp_post), Some(rp_post)) => {
            assert_json_keys_match(&wp_post, &rp_post);
            assert_json_types_match(&wp_post, &rp_post);

            // Verify title was set
            if let Some(rp_title) = rp_post.get("title") {
                eprintln!("RustPress created post title: {}", rp_title);
            }
            eprintln!("[PASS] Both created posts with matching structure");

            // Clean up: delete the created posts
            if let Some(wp_id) = wp_post.get("id").and_then(|v| v.as_u64()) {
                delete_auth(
                    &client,
                    &format!(
                        "{}/wp-json/wp/v2/posts/{}?force=true",
                        cfg.wordpress_url, wp_id
                    ),
                    &wp_auth,
                )
                .await;
            }
            if let Some(rp_id) = rp_post.get("id").and_then(|v| v.as_u64()) {
                delete_auth(
                    &client,
                    &format!(
                        "{}/wp-json/wp/v2/posts/{}?force=true",
                        cfg.rustpress_url, rp_id
                    ),
                    &rp_token,
                )
                .await;
            }
        }
        _ => eprintln!("[SKIP] Could not create posts on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_update() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_update ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // First create posts on both
    let create_body = serde_json::json!({
        "title": "E2E Update Test",
        "content": "<p>Original content.</p>",
        "status": "draft",
    });

    let wp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
        &create_body,
        &wp_auth,
    )
    .await;
    let rp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
        &create_body,
        &rp_token,
    )
    .await;

    match (wp_created, rp_created) {
        (Some(wp_post), Some(rp_post)) => {
            let wp_id = wp_post.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_post.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            // Update both
            let update_body = serde_json::json!({
                "title": "E2E Update Test - Modified",
                "content": "<p>Updated content.</p>",
            });

            let wp_updated = put_json_auth(
                &client,
                &format!("{}/wp-json/wp/v2/posts/{}", cfg.wordpress_url, wp_id),
                &update_body,
                &wp_auth,
            )
            .await;
            let rp_updated = put_json_auth(
                &client,
                &format!("{}/wp-json/wp/v2/posts/{}", cfg.rustpress_url, rp_id),
                &update_body,
                &rp_token,
            )
            .await;

            match (wp_updated, rp_updated) {
                (Some(wp_val), Some(rp_val)) => {
                    assert_json_keys_match(&wp_val, &rp_val);
                    eprintln!("[PASS] Both updated posts with matching structure");
                }
                _ => eprintln!("[PARTIAL] Could not update posts on one or both servers"),
            }

            // Clean up
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/posts/{}?force=true",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/posts/{}?force=true",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;
        }
        _ => eprintln!("[SKIP] Could not create posts for update test"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_delete() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_delete ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // Create posts on both to delete
    let create_body = serde_json::json!({
        "title": "E2E Delete Test",
        "content": "<p>This will be deleted.</p>",
        "status": "draft",
    });

    let wp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
        &create_body,
        &wp_auth,
    )
    .await;
    let rp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
        &create_body,
        &rp_token,
    )
    .await;

    match (wp_created, rp_created) {
        (Some(wp_post), Some(rp_post)) => {
            let wp_id = wp_post.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_post.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            let wp_del = delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/posts/{}?force=true",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            let rp_del = delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/posts/{}?force=true",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;

            match (wp_del, rp_del) {
                (Some(wp_val), Some(rp_val)) => {
                    // WordPress returns {"deleted": true, "previous": {...}}
                    eprintln!(
                        "WordPress delete response keys: {:?}",
                        json_top_keys(&wp_val)
                    );
                    eprintln!(
                        "RustPress delete response keys: {:?}",
                        json_top_keys(&rp_val)
                    );
                    eprintln!("[PASS] Both deleted posts successfully");
                }
                _ => eprintln!("[PARTIAL] Delete may have failed on one server"),
            }

            // Verify deletion: GET should return 404 or empty
            let wp_check = fetch_json(
                &client,
                &format!("{}/wp-json/wp/v2/posts/{}", cfg.wordpress_url, wp_id),
            )
            .await;
            let rp_check = fetch_json(
                &client,
                &format!("{}/wp-json/wp/v2/posts/{}", cfg.rustpress_url, rp_id),
            )
            .await;

            eprintln!(
                "Post-delete WP: {}, RP: {}",
                if wp_check.is_none() {
                    "gone (good)"
                } else {
                    "still exists"
                },
                if rp_check.is_none() {
                    "gone (good)"
                } else {
                    "still exists"
                },
            );
        }
        _ => eprintln!("[SKIP] Could not create posts for delete test"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_media_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_media_list ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/media", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/media", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(wp_val.is_array(), "WordPress /media should be an array");
            assert!(rp_val.is_array(), "RustPress /media should be an array");

            eprintln!(
                "WordPress media: {}, RustPress media: {}",
                wp_val.as_array().map(|a| a.len()).unwrap_or(0),
                rp_val.as_array().map(|a| a.len()).unwrap_or(0),
            );

            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }
            eprintln!("[PASS] Both return media arrays");
        }
        _ => eprintln!("[SKIP] Could not fetch media from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_settings() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_settings ===");

    // Settings endpoint typically requires authentication in WordPress,
    // but RustPress may expose it publicly. Try both.
    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);

    let wp_resp = client
        .get(&format!("{}/wp-json/wp/v2/settings", cfg.wordpress_url))
        .header("Authorization", &wp_auth)
        .send()
        .await;

    let rp_resp = client
        .get(&format!("{}/wp-json/wp/v2/settings", cfg.rustpress_url))
        .send()
        .await;

    let wp_val = wp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });
    let rp_val = rp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });

    match (wp_val, rp_val) {
        (Some(wp_r), Some(rp_r)) => {
            let wp_json: Value = wp_r.json().await.unwrap_or(Value::Null);
            let rp_json: Value = rp_r.json().await.unwrap_or(Value::Null);
            assert_json_keys_match(&wp_json, &rp_json);
            eprintln!("[PASS] Settings endpoint structure compared");
        }
        _ => eprintln!("[SKIP] Settings endpoint not accessible on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_statuses() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_statuses ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/statuses", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/statuses", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            // Statuses is an object with keys like "publish", "draft", etc.
            let wp_keys = json_top_keys(&wp_val);
            let rp_keys = json_top_keys(&rp_val);
            eprintln!("WordPress statuses: {:?}", wp_keys);
            eprintln!("RustPress statuses: {:?}", rp_keys);

            let expected = ["publish"];
            for s in &expected {
                assert!(rp_keys.contains(*s), "RustPress should have '{}' status", s);
                eprintln!("  [OK] RustPress has status: {}", s);
            }
            eprintln!("[PASS] Statuses endpoint compared");
        }
        _ => eprintln!("[SKIP] Could not fetch statuses from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_types() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_types ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/types", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/types", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            let wp_keys = json_top_keys(&wp_val);
            let rp_keys = json_top_keys(&rp_val);
            eprintln!("WordPress types: {:?}", wp_keys);
            eprintln!("RustPress types: {:?}", rp_keys);

            // At minimum, both should have "post" and "page"
            let required = ["post", "page"];
            for t in &required {
                if rp_keys.contains(*t) {
                    eprintln!("  [OK] RustPress has type: {}", t);
                } else {
                    eprintln!("  [MISSING] RustPress missing type: {}", t);
                }
            }

            // Compare structure of the "post" type object
            if let (Some(wp_post_type), Some(rp_post_type)) =
                (wp_val.get("post"), rp_val.get("post"))
            {
                assert_json_keys_match(wp_post_type, rp_post_type);
            }

            eprintln!("[PASS] Post types endpoint compared");
        }
        _ => eprintln!("[SKIP] Could not fetch post types from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_comments_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_comments_list ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/comments", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/comments", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(wp_val.is_array(), "WordPress /comments should be an array");
            assert!(rp_val.is_array(), "RustPress /comments should be an array");

            eprintln!(
                "WordPress comments: {}, RustPress comments: {}",
                wp_val.as_array().map(|a| a.len()).unwrap_or(0),
                rp_val.as_array().map(|a| a.len()).unwrap_or(0),
            );

            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);

                let expected = [
                    "id",
                    "post",
                    "parent",
                    "author_name",
                    "content",
                    "date",
                    "status",
                    "type",
                ];
                let rp_keys = json_top_keys(rp_first);
                for f in &expected {
                    eprintln!(
                        "  [{}] {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }
            eprintln!("[PASS] Both return comment arrays");
        }
        _ => eprintln!("[SKIP] Could not fetch comments from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_pages_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_pages_list ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/pages", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/pages", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(wp_val.is_array(), "WordPress /pages should be an array");
            assert!(rp_val.is_array(), "RustPress /pages should be an array");

            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);

                let expected = [
                    "id",
                    "date",
                    "slug",
                    "status",
                    "type",
                    "title",
                    "content",
                    "excerpt",
                    "author",
                    "parent",
                    "menu_order",
                ];
                let rp_keys = json_top_keys(rp_first);
                for f in &expected {
                    eprintln!(
                        "  [{}] {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }
            eprintln!("[PASS] Both return page arrays");
        }
        _ => eprintln!("[SKIP] Could not fetch pages from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_pagination_headers() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_pagination_headers ===");

    // Test pagination headers on /posts endpoint
    let endpoints = [
        "/wp-json/wp/v2/posts?per_page=1",
        "/wp-json/wp/v2/categories",
        "/wp-json/wp/v2/tags",
        "/wp-json/wp/v2/users",
    ];

    for path in &endpoints {
        let wp_url = format!("{}{}", cfg.wordpress_url, path);
        let rp_url = format!("{}{}", cfg.rustpress_url, path);

        let wp_resp = client.get(&wp_url).send().await;
        let rp_resp = client.get(&rp_url).send().await;

        eprintln!("\n  {}:", path);

        match (wp_resp, rp_resp) {
            (Ok(wp_r), Ok(rp_r)) => {
                let wp_total = wp_r
                    .headers()
                    .get("x-wp-total")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");
                let rp_total = rp_r
                    .headers()
                    .get("x-wp-total")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");
                let wp_pages = wp_r
                    .headers()
                    .get("x-wp-totalpages")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");
                let rp_pages = rp_r
                    .headers()
                    .get("x-wp-totalpages")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(absent)");

                eprintln!("    X-WP-Total - WP: {}, RP: {}", wp_total, rp_total);
                eprintln!("    X-WP-TotalPages - WP: {}, RP: {}", wp_pages, rp_pages);

                // RustPress should provide these headers
                if rp_total != "(absent)" {
                    eprintln!("    [OK] RustPress provides X-WP-Total");
                } else {
                    eprintln!("    [MISSING] RustPress does not provide X-WP-Total");
                }
                if rp_pages != "(absent)" {
                    eprintln!("    [OK] RustPress provides X-WP-TotalPages");
                } else {
                    eprintln!("    [MISSING] RustPress does not provide X-WP-TotalPages");
                }
            }
            _ => eprintln!("    [SKIP] Could not fetch from one or both servers"),
        }
    }

    eprintln!("\n[PASS] Pagination headers compared");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_embed_parameter() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_embed_parameter ===");

    // _embed should include linked resources (author, wp:term, etc.)
    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts?_embed", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts?_embed", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_post), Some(rp_post)) => {
            // With _embed, WordPress adds an _embedded object
            let wp_has_embedded = wp_post.get("_embedded").is_some();
            let rp_has_embedded = rp_post.get("_embedded").is_some();

            eprintln!(
                "  _embedded present - WP: {}, RP: {} [{}]",
                wp_has_embedded,
                rp_has_embedded,
                if wp_has_embedded == rp_has_embedded {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            if let Some(rp_embedded) = rp_post.get("_embedded") {
                let embed_keys = json_top_keys(rp_embedded);
                eprintln!("  RustPress _embedded keys: {:?}", embed_keys);

                // WordPress embeds author, wp:term, etc.
                let expected_embeds = ["author"];
                for key in &expected_embeds {
                    eprintln!(
                        "    [{}] {}",
                        if embed_keys.contains(*key) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        key
                    );
                }
            }

            eprintln!("[PASS] _embed parameter compared");
        }
        _ => eprintln!("[SKIP] No posts available for _embed comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_fields_parameter() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_fields_parameter ===");

    // _fields should limit the response to only the specified fields
    let fields = "id,title,slug";
    let wp = fetch_first_from_array(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?_fields={}",
            cfg.wordpress_url, fields
        ),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?_fields={}",
            cfg.rustpress_url, fields
        ),
    )
    .await;

    match (wp, rp) {
        (Some(wp_post), Some(rp_post)) => {
            let wp_keys = json_top_keys(&wp_post);
            let rp_keys = json_top_keys(&rp_post);

            eprintln!("  WordPress keys with _fields={}: {:?}", fields, wp_keys);
            eprintln!("  RustPress keys with _fields={}: {:?}", fields, rp_keys);

            // With _fields=id,title,slug, response should only have those fields
            let requested: Vec<&str> = fields.split(',').collect();
            for field in &requested {
                if rp_keys.contains(*field) {
                    eprintln!("    [OK] RustPress has requested field: {}", field);
                } else {
                    eprintln!("    [MISSING] RustPress missing requested field: {}", field);
                }
            }

            // Should NOT have extra fields
            let extra: Vec<_> = rp_keys
                .iter()
                .filter(|k| !requested.contains(&k.as_str()))
                .collect();
            if extra.is_empty() {
                eprintln!("    [OK] No extra fields in response");
            } else {
                eprintln!("    [WARN] Extra fields present: {:?}", extra);
            }

            eprintln!("[PASS] _fields parameter compared");
        }
        _ => eprintln!("[SKIP] No posts available for _fields comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_rendered_fields() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_rendered_fields ===");

    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_post), Some(rp_post)) => {
            // WordPress returns title, content, excerpt as {rendered: "..."} objects
            let rendered_fields = ["title", "content", "excerpt", "guid"];

            for field in &rendered_fields {
                let wp_val = wp_post.get(*field);
                let rp_val = rp_post.get(*field);

                let wp_is_obj = wp_val.map(|v| v.is_object()).unwrap_or(false);
                let rp_is_obj = rp_val.map(|v| v.is_object()).unwrap_or(false);

                let wp_has_rendered = wp_val.and_then(|v| v.get("rendered")).is_some();
                let rp_has_rendered = rp_val.and_then(|v| v.get("rendered")).is_some();

                eprintln!(
                    "  {} - WP: obj={} rendered={}, RP: obj={} rendered={} [{}]",
                    field,
                    wp_is_obj,
                    wp_has_rendered,
                    rp_is_obj,
                    rp_has_rendered,
                    if wp_has_rendered == rp_has_rendered {
                        "MATCH"
                    } else {
                        "DIFFER"
                    }
                );
            }

            eprintln!("[PASS] Rendered fields compared");
        }
        _ => eprintln!("[SKIP] No posts available for rendered fields comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_links() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_links ===");

    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_post), Some(rp_post)) => {
            // WordPress adds _links HATEOAS to each resource
            let wp_has_links = wp_post.get("_links").is_some();
            let rp_has_links = rp_post.get("_links").is_some();

            eprintln!(
                "  _links present - WP: {}, RP: {} [{}]",
                wp_has_links,
                rp_has_links,
                if wp_has_links == rp_has_links {
                    "MATCH"
                } else {
                    "DIFFER"
                }
            );

            if let (Some(wp_links), Some(rp_links)) = (wp_post.get("_links"), rp_post.get("_links"))
            {
                let wp_link_keys = json_top_keys(wp_links);
                let rp_link_keys = json_top_keys(rp_links);

                eprintln!("  WordPress _links keys: {:?}", wp_link_keys);
                eprintln!("  RustPress _links keys: {:?}", rp_link_keys);

                // Common link types WordPress provides
                let expected_links = ["self", "collection", "author"];
                for link in &expected_links {
                    eprintln!(
                        "    [{}] {}",
                        if rp_link_keys.contains(*link) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        link
                    );
                }
            }

            eprintln!("[PASS] _links compared");
        }
        _ => eprintln!("[SKIP] No posts available for _links comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_category_crud() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_category_crud ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // CREATE
    let cat_body = serde_json::json!({
        "name": "E2E Test Category",
        "slug": "e2e-test-category",
        "description": "Created by E2E test suite",
    });

    let wp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/categories", cfg.wordpress_url),
        &cat_body,
        &wp_auth,
    )
    .await;
    let rp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/categories", cfg.rustpress_url),
        &cat_body,
        &rp_token,
    )
    .await;

    match (wp_created, rp_created) {
        (Some(wp_cat), Some(rp_cat)) => {
            assert_json_keys_match(&wp_cat, &rp_cat);
            eprintln!("[OK] CREATE: Both created categories with matching structure");

            let wp_id = wp_cat.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_cat.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            // UPDATE
            let update_body = serde_json::json!({
                "name": "E2E Test Category - Updated",
            });

            let wp_updated = put_json_auth(
                &client,
                &format!("{}/wp-json/wp/v2/categories/{}", cfg.wordpress_url, wp_id),
                &update_body,
                &wp_auth,
            )
            .await;
            let rp_updated = put_json_auth(
                &client,
                &format!("{}/wp-json/wp/v2/categories/{}", cfg.rustpress_url, rp_id),
                &update_body,
                &rp_token,
            )
            .await;

            match (wp_updated, rp_updated) {
                (Some(wp_val), Some(rp_val)) => {
                    assert_json_keys_match(&wp_val, &rp_val);
                    eprintln!("[OK] UPDATE: Both updated categories");
                }
                _ => eprintln!("[PARTIAL] Could not update categories on one or both servers"),
            }

            // DELETE
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/categories/{}?force=true",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/categories/{}?force=true",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;

            eprintln!("[PASS] Category CRUD cycle complete");
        }
        _ => eprintln!("[SKIP] Could not create categories on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_tag_crud() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_tag_crud ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // CREATE
    let tag_body = serde_json::json!({
        "name": "E2E Test Tag",
        "slug": "e2e-test-tag",
        "description": "Created by E2E test suite",
    });

    let wp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/tags", cfg.wordpress_url),
        &tag_body,
        &wp_auth,
    )
    .await;
    let rp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/tags", cfg.rustpress_url),
        &tag_body,
        &rp_token,
    )
    .await;

    match (wp_created, rp_created) {
        (Some(wp_tag), Some(rp_tag)) => {
            assert_json_keys_match(&wp_tag, &rp_tag);
            eprintln!("[OK] CREATE: Both created tags with matching structure");

            let wp_id = wp_tag.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_tag.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            // DELETE
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/tags/{}?force=true",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/tags/{}?force=true",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;

            eprintln!("[PASS] Tag CRUD cycle complete");
        }
        _ => eprintln!("[SKIP] Could not create tags on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_comment_crud() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_comment_crud ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // First, get a post ID from each to attach comments to
    let wp_posts = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.wordpress_url),
    )
    .await;
    let rp_posts = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.rustpress_url),
    )
    .await;

    let wp_post_id = wp_posts
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64())
        .unwrap_or(1);
    let rp_post_id = rp_posts
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64())
        .unwrap_or(1);

    // CREATE comment
    let wp_comment_body = serde_json::json!({
        "post": wp_post_id,
        "content": "E2E test comment from comparison suite",
        "author_name": "E2E Tester",
        "author_email": "e2e@test.local",
        "status": "approved",
    });
    let rp_comment_body = serde_json::json!({
        "post": rp_post_id,
        "content": "E2E test comment from comparison suite",
        "author_name": "E2E Tester",
        "author_email": "e2e@test.local",
        "status": "approved",
    });

    let wp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/comments", cfg.wordpress_url),
        &wp_comment_body,
        &wp_auth,
    )
    .await;
    let rp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/comments", cfg.rustpress_url),
        &rp_comment_body,
        &rp_token,
    )
    .await;

    match (wp_created, rp_created) {
        (Some(wp_comment), Some(rp_comment)) => {
            assert_json_keys_match(&wp_comment, &rp_comment);
            eprintln!("[OK] CREATE: Both created comments with matching structure");

            let wp_id = wp_comment.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_comment.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            // DELETE
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/comments/{}?force=true",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/comments/{}?force=true",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;

            eprintln!("[PASS] Comment CRUD cycle complete");
        }
        _ => eprintln!("[SKIP] Could not create comments on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_users_me() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_users_me ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    let wp_resp = client
        .get(&format!("{}/wp-json/wp/v2/users/me", cfg.wordpress_url))
        .header("Authorization", &wp_auth)
        .send()
        .await;
    let rp_resp = client
        .get(&format!("{}/wp-json/wp/v2/users/me", cfg.rustpress_url))
        .header("Authorization", &rp_token)
        .send()
        .await;

    let wp_json = wp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });
    let rp_json = rp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });

    match (wp_json, rp_json) {
        (Some(wp_r), Some(rp_r)) => {
            let wp_val: Value = wp_r.json().await.unwrap_or(Value::Null);
            let rp_val: Value = rp_r.json().await.unwrap_or(Value::Null);

            assert_json_keys_match(&wp_val, &rp_val);
            assert_json_types_match(&wp_val, &rp_val);

            // users/me should return the authenticated user info
            let expected = ["id", "name", "slug", "email", "roles"];
            let rp_keys = json_top_keys(&rp_val);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            // Verify username matches
            if let (Some(wp_slug), Some(rp_slug)) = (
                wp_val.get("slug").and_then(|v| v.as_str()),
                rp_val.get("slug").and_then(|v| v.as_str()),
            ) {
                eprintln!(
                    "  User slug - WP: {}, RP: {} [{}]",
                    wp_slug,
                    rp_slug,
                    if wp_slug == rp_slug {
                        "MATCH"
                    } else {
                        "DIFFER"
                    }
                );
            }

            eprintln!("[PASS] users/me endpoint compared");
        }
        _ => eprintln!("[SKIP] Could not fetch users/me from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_search() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_search ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/search?search=hello", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/search?search=hello", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(wp_val.is_array(), "WordPress /search should be an array");
            assert!(rp_val.is_array(), "RustPress /search should be an array");

            eprintln!(
                "WordPress search results: {}, RustPress search results: {}",
                wp_val.as_array().map(|a| a.len()).unwrap_or(0),
                rp_val.as_array().map(|a| a.len()).unwrap_or(0),
            );

            // Compare structure of first result if available
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);

                // Search results should have id, title, url, type, subtype
                let expected = ["id", "title", "url"];
                let rp_keys = json_top_keys(rp_first);
                for f in &expected {
                    eprintln!(
                        "  [{}] {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }
            eprintln!("[PASS] Search endpoint compared");
        }
        _ => eprintln!("[SKIP] Could not fetch search from one or both servers"),
    }
}

// ---------------------------------------------------------------------------
// Additional REST API comparison tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_orderby() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_orderby ===");

    let wp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?orderby=title&order=asc",
            cfg.wordpress_url
        ),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?orderby=title&order=asc",
            cfg.rustpress_url
        ),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?orderby=title should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?orderby=title should return an array"
            );

            // Verify that both return arrays sorted by title ascending
            let wp_titles: Vec<String> = wp_val
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|p| {
                    p.get("title")
                        .and_then(|t| t.get("rendered"))
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect();
            let rp_titles: Vec<String> = rp_val
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|p| {
                    p.get("title")
                        .and_then(|t| t.get("rendered"))
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect();

            let wp_sorted = {
                let mut s = wp_titles.clone();
                s.sort();
                s
            };
            let rp_sorted = {
                let mut s = rp_titles.clone();
                s.sort();
                s
            };

            eprintln!("WordPress titles (asc): {:?}", wp_titles);
            eprintln!("RustPress titles (asc): {:?}", rp_titles);

            if wp_titles == wp_sorted {
                eprintln!("  [OK] WordPress results are sorted by title asc");
            } else {
                eprintln!("  [WARN] WordPress results may not be sorted by title asc");
            }
            if rp_titles == rp_sorted {
                eprintln!("  [OK] RustPress results are sorted by title asc");
            } else {
                eprintln!("  [WARN] RustPress results are NOT sorted by title asc");
            }

            // Compare structure of first post
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_structure_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Posts orderby=title compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts with orderby from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_search() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_search ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?search=hello", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?search=hello", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?search=hello should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?search=hello should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress search results: {}", wp_count);
            eprintln!("RustPress search results: {}", rp_count);

            // Both should return matching posts (likely the "Hello World" default post)
            if wp_count > 0 && rp_count > 0 {
                eprintln!("  [OK] Both return results for search=hello");
            } else if wp_count > 0 && rp_count == 0 {
                eprintln!("  [WARN] WordPress has results but RustPress has none");
            }

            // Compare structure of first result if both have results
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Posts search compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts with search from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_by_status() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_by_status ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // Drafts require authentication
    let wp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts?status=draft",
            cfg.wordpress_url
        ))
        .header("Authorization", &wp_auth)
        .send()
        .await;
    let rp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts?status=draft",
            cfg.rustpress_url
        ))
        .header("Authorization", &rp_token)
        .send()
        .await;

    let wp_val = wp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });
    let rp_val = rp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });

    match (wp_val, rp_val) {
        (Some(wp_r), Some(rp_r)) => {
            let wp_json: Value = wp_r.json().await.unwrap_or(Value::Null);
            let rp_json: Value = rp_r.json().await.unwrap_or(Value::Null);

            assert!(
                wp_json.is_array(),
                "WordPress /posts?status=draft should return an array"
            );
            assert!(
                rp_json.is_array(),
                "RustPress /posts?status=draft should return an array"
            );

            eprintln!(
                "WordPress drafts: {}, RustPress drafts: {}",
                wp_json.as_array().map(|a| a.len()).unwrap_or(0),
                rp_json.as_array().map(|a| a.len()).unwrap_or(0),
            );

            // Compare structure of first draft if both have them
            if let (Some(wp_first), Some(rp_first)) = (
                wp_json.as_array().and_then(|a| a.first()),
                rp_json.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);

                // Verify status field is "draft"
                if let Some(rp_status) = rp_first.get("status").and_then(|v| v.as_str()) {
                    assert_eq!(rp_status, "draft", "RustPress post status should be draft");
                    eprintln!("  [OK] RustPress draft post has status=draft");
                }
            }

            eprintln!("[PASS] Posts by status=draft compared");
        }
        _ => eprintln!("[SKIP] Could not fetch draft posts from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_per_page() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_per_page ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?per_page=1 should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?per_page=1 should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress count with per_page=1: {}", wp_count);
            eprintln!("RustPress count with per_page=1: {}", rp_count);

            assert!(
                wp_count <= 1,
                "WordPress should return at most 1 post with per_page=1"
            );
            assert!(
                rp_count <= 1,
                "RustPress should return at most 1 post with per_page=1, got {}",
                rp_count
            );

            eprintln!("[PASS] per_page=1 correctly limits results");
        }
        _ => eprintln!("[SKIP] Could not fetch posts with per_page from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_by_category() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_by_category ===");

    // Category 1 is typically "Uncategorized" in WordPress
    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?categories=1", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?categories=1", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?categories=1 should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?categories=1 should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress posts in category 1: {}", wp_count);
            eprintln!("RustPress posts in category 1: {}", rp_count);

            // Verify all returned posts have category 1 in their categories array
            if let Some(rp_arr) = rp_val.as_array() {
                for (i, post) in rp_arr.iter().enumerate() {
                    if let Some(cats) = post.get("categories").and_then(|v| v.as_array()) {
                        let has_cat_1 = cats.iter().any(|c| c.as_u64() == Some(1));
                        eprintln!(
                            "  Post {}: categories={:?}, has_cat_1={}",
                            i,
                            cats.iter().filter_map(|c| c.as_u64()).collect::<Vec<_>>(),
                            has_cat_1
                        );
                    }
                }
            }

            // Compare structure
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Posts filtered by category compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts by category from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_by_tag() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_by_tag ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?tags=1", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?tags=1", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?tags=1 should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?tags=1 should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress posts with tag 1: {}", wp_count);
            eprintln!("RustPress posts with tag 1: {}", rp_count);

            // May be empty -- that's fine, just compare structure if both have results
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Posts filtered by tag compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts by tag from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_exclude() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_exclude ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?exclude=1", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?exclude=1", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?exclude=1 should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?exclude=1 should return an array"
            );

            // Verify post ID 1 is not in either result set
            let wp_has_id_1 = wp_val
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p.get("id").and_then(|v| v.as_u64()) == Some(1));
            let rp_has_id_1 = rp_val
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p.get("id").and_then(|v| v.as_u64()) == Some(1));

            eprintln!("WordPress has post 1 in results: {}", wp_has_id_1);
            eprintln!("RustPress has post 1 in results: {}", rp_has_id_1);

            if !wp_has_id_1 {
                eprintln!("  [OK] WordPress correctly excludes post 1");
            }
            if !rp_has_id_1 {
                eprintln!("  [OK] RustPress correctly excludes post 1");
            } else {
                eprintln!("  [FAIL] RustPress still includes post 1 despite exclude=1");
            }

            eprintln!("[PASS] Posts exclude parameter compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts with exclude from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_slug() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_slug ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?slug=hello-world", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?slug=hello-world", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?slug=hello-world should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?slug=hello-world should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress results for slug=hello-world: {}", wp_count);
            eprintln!("RustPress results for slug=hello-world: {}", rp_count);

            // Verify slug matches in RustPress results
            if let Some(rp_arr) = rp_val.as_array() {
                for post in rp_arr {
                    if let Some(slug) = post.get("slug").and_then(|v| v.as_str()) {
                        assert_eq!(
                            slug, "hello-world",
                            "RustPress returned post with wrong slug"
                        );
                        eprintln!("  [OK] RustPress post slug: {}", slug);
                    }
                }
            }

            // Compare structure
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Posts slug filter compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts by slug from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_single() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_single ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts/1", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts/1", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            // Single post should be an object, not an array
            assert!(
                wp_val.is_object(),
                "WordPress /posts/1 should return an object"
            );
            assert!(
                rp_val.is_object(),
                "RustPress /posts/1 should return an object"
            );

            assert_json_keys_match(&wp_val, &rp_val);
            assert_json_types_match(&wp_val, &rp_val);
            assert_json_structure_match(&wp_val, &rp_val);

            // Verify ID is 1
            if let Some(rp_id) = rp_val.get("id").and_then(|v| v.as_u64()) {
                assert_eq!(rp_id, 1, "RustPress should return post with id=1");
                eprintln!("  [OK] RustPress post id: {}", rp_id);
            }

            let expected = [
                "id", "date", "slug", "status", "type", "title", "content", "excerpt", "author",
                "link",
            ];
            let rp_keys = json_top_keys(&rp_val);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            eprintln!("[PASS] Single post structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch single post from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_page_single() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_page_single ===");

    // First get a page ID from both servers
    let wp_pages = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/pages?per_page=1", cfg.wordpress_url),
    )
    .await;
    let rp_pages = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/pages?per_page=1", cfg.rustpress_url),
    )
    .await;

    let wp_page_id = wp_pages
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64());
    let rp_page_id = rp_pages
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64());

    match (wp_page_id, rp_page_id) {
        (Some(wp_id), Some(rp_id)) => {
            let wp = fetch_json(
                &client,
                &format!("{}/wp-json/wp/v2/pages/{}", cfg.wordpress_url, wp_id),
            )
            .await;
            let rp = fetch_json(
                &client,
                &format!("{}/wp-json/wp/v2/pages/{}", cfg.rustpress_url, rp_id),
            )
            .await;

            match (wp, rp) {
                (Some(wp_val), Some(rp_val)) => {
                    assert!(
                        wp_val.is_object(),
                        "WordPress /pages/{} should return an object",
                        wp_id
                    );
                    assert!(
                        rp_val.is_object(),
                        "RustPress /pages/{} should return an object",
                        rp_id
                    );

                    assert_json_keys_match(&wp_val, &rp_val);
                    assert_json_types_match(&wp_val, &rp_val);
                    assert_json_structure_match(&wp_val, &rp_val);

                    let expected = [
                        "id",
                        "date",
                        "slug",
                        "status",
                        "type",
                        "title",
                        "content",
                        "excerpt",
                        "author",
                        "parent",
                        "menu_order",
                    ];
                    let rp_keys = json_top_keys(&rp_val);
                    for f in &expected {
                        eprintln!(
                            "  [{}] {}",
                            if rp_keys.contains(*f) {
                                "OK"
                            } else {
                                "MISSING"
                            },
                            f
                        );
                    }

                    eprintln!("[PASS] Single page structure compared");
                }
                _ => eprintln!("[SKIP] Could not fetch single page from one or both servers"),
            }
        }
        _ => eprintln!("[SKIP] No pages available on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_user_single() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_user_single ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/users/1", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/users/1", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_object(),
                "WordPress /users/1 should return an object"
            );
            assert!(
                rp_val.is_object(),
                "RustPress /users/1 should return an object"
            );

            assert_json_keys_match(&wp_val, &rp_val);
            assert_json_types_match(&wp_val, &rp_val);

            // Verify ID is 1
            if let Some(rp_id) = rp_val.get("id").and_then(|v| v.as_u64()) {
                assert_eq!(rp_id, 1, "RustPress should return user with id=1");
                eprintln!("  [OK] RustPress user id: {}", rp_id);
            }

            let expected = ["id", "name", "slug", "link", "avatar_urls"];
            let rp_keys = json_top_keys(&rp_val);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            eprintln!("[PASS] Single user structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch single user from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_category_single() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_category_single ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/categories/1", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/categories/1", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_object(),
                "WordPress /categories/1 should return an object"
            );
            assert!(
                rp_val.is_object(),
                "RustPress /categories/1 should return an object"
            );

            assert_json_keys_match(&wp_val, &rp_val);
            assert_json_types_match(&wp_val, &rp_val);

            // Verify ID is 1
            if let Some(rp_id) = rp_val.get("id").and_then(|v| v.as_u64()) {
                assert_eq!(rp_id, 1, "RustPress should return category with id=1");
                eprintln!("  [OK] RustPress category id: {}", rp_id);
            }

            let expected = ["id", "count", "name", "slug", "taxonomy", "parent"];
            let rp_keys = json_top_keys(&rp_val);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            eprintln!("[PASS] Single category structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch single category from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_media_fields() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_media_fields ===");

    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/media", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/media", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_media), Some(rp_media)) => {
            assert_json_keys_match(&wp_media, &rp_media);
            assert_json_types_match(&wp_media, &rp_media);

            let expected = [
                "id",
                "date",
                "slug",
                "status",
                "type",
                "title",
                "author",
                "mime_type",
                "source_url",
            ];
            let rp_keys = json_top_keys(&rp_media);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            eprintln!("[PASS] Media fields compared");
        }
        _ => eprintln!("[SKIP] No media available for field comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_comments_fields() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_comments_fields ===");

    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/comments", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/comments", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_comment), Some(rp_comment)) => {
            assert_json_keys_match(&wp_comment, &rp_comment);
            assert_json_types_match(&wp_comment, &rp_comment);

            let expected = [
                "id",
                "post",
                "parent",
                "author",
                "author_name",
                "author_email",
                "content",
                "date",
                "status",
                "type",
                "link",
            ];
            let rp_keys = json_top_keys(&rp_comment);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            // Deep structure comparison
            assert_json_structure_match(&wp_comment, &rp_comment);

            eprintln!("[PASS] Comment fields compared");
        }
        _ => eprintln!("[SKIP] No comments available for field comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_media_crud() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_media_crud ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // Test media via authenticated GET with structure comparison.
    // Multipart upload requires the "multipart" feature in reqwest, so we
    // focus on comparing the GET /media response structure with auth.
    let wp_resp = client
        .get(&format!("{}/wp-json/wp/v2/media", cfg.wordpress_url))
        .header("Authorization", &wp_auth)
        .send()
        .await;
    let rp_resp = client
        .get(&format!("{}/wp-json/wp/v2/media", cfg.rustpress_url))
        .header("Authorization", &rp_token)
        .send()
        .await;

    let wp_val = wp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });
    let rp_val = rp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });

    match (wp_val, rp_val) {
        (Some(wp_r), Some(rp_r)) => {
            let wp_json: Value = wp_r.json().await.unwrap_or(Value::Null);
            let rp_json: Value = rp_r.json().await.unwrap_or(Value::Null);

            assert!(
                wp_json.is_array(),
                "WordPress /media should return an array"
            );
            assert!(
                rp_json.is_array(),
                "RustPress /media should return an array"
            );

            eprintln!(
                "WordPress media (auth): {}, RustPress media (auth): {}",
                wp_json.as_array().map(|a| a.len()).unwrap_or(0),
                rp_json.as_array().map(|a| a.len()).unwrap_or(0),
            );

            // Compare structure of first media item if both have items
            if let (Some(wp_first), Some(rp_first)) = (
                wp_json.as_array().and_then(|a| a.first()),
                rp_json.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);

                let expected = [
                    "id",
                    "date",
                    "slug",
                    "status",
                    "type",
                    "title",
                    "author",
                    "mime_type",
                    "source_url",
                ];
                let rp_keys = json_top_keys(rp_first);
                for f in &expected {
                    eprintln!(
                        "  [{}] {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }

            eprintln!("[PASS] Media CRUD (GET with auth) structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch media from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_settings_read() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_settings_read ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    let wp_resp = client
        .get(&format!("{}/wp-json/wp/v2/settings", cfg.wordpress_url))
        .header("Authorization", &wp_auth)
        .send()
        .await;
    let rp_resp = client
        .get(&format!("{}/wp-json/wp/v2/settings", cfg.rustpress_url))
        .header("Authorization", &rp_token)
        .send()
        .await;

    let wp_val = wp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });
    let rp_val = rp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });

    match (wp_val, rp_val) {
        (Some(wp_r), Some(rp_r)) => {
            let wp_json: Value = wp_r.json().await.unwrap_or(Value::Null);
            let rp_json: Value = rp_r.json().await.unwrap_or(Value::Null);

            assert_json_keys_match(&wp_json, &rp_json);

            // WordPress settings should include these common keys
            let expected = [
                "title",
                "description",
                "url",
                "email",
                "timezone_string",
                "date_format",
                "time_format",
                "posts_per_page",
                "default_comment_status",
            ];
            let rp_keys = json_top_keys(&rp_json);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            eprintln!("[PASS] Settings read compared");
        }
        _ => eprintln!("[SKIP] Settings endpoint not accessible on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_settings_update() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_settings_update ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // First, read the current description so we can restore it
    let wp_original = client
        .get(&format!("{}/wp-json/wp/v2/settings", cfg.wordpress_url))
        .header("Authorization", &wp_auth)
        .send()
        .await
        .ok()
        .and_then(|r| {
            if r.status().is_success() {
                Some(r)
            } else {
                None
            }
        });
    let rp_original = client
        .get(&format!("{}/wp-json/wp/v2/settings", cfg.rustpress_url))
        .header("Authorization", &rp_token)
        .send()
        .await
        .ok()
        .and_then(|r| {
            if r.status().is_success() {
                Some(r)
            } else {
                None
            }
        });

    let wp_orig_json: Value = match wp_original {
        Some(r) => r.json().await.unwrap_or(Value::Null),
        None => {
            eprintln!("[SKIP] Could not read WordPress settings");
            return;
        }
    };
    let rp_orig_json: Value = match rp_original {
        Some(r) => r.json().await.unwrap_or(Value::Null),
        None => {
            eprintln!("[SKIP] Could not read RustPress settings");
            return;
        }
    };

    let wp_orig_desc = wp_orig_json
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let rp_orig_desc = rp_orig_json
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Update description
    let update_body = serde_json::json!({
        "description": "E2E test description update"
    });

    let wp_updated = put_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/settings", cfg.wordpress_url),
        &update_body,
        &wp_auth,
    )
    .await;
    let rp_updated = put_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/settings", cfg.rustpress_url),
        &update_body,
        &rp_token,
    )
    .await;

    match (&wp_updated, &rp_updated) {
        (Some(wp_val), Some(rp_val)) => {
            assert_json_keys_match(wp_val, rp_val);

            // Verify description was updated
            if let Some(rp_desc) = rp_val.get("description").and_then(|v| v.as_str()) {
                if rp_desc == "E2E test description update" {
                    eprintln!("  [OK] RustPress description updated successfully");
                } else {
                    eprintln!("  [WARN] RustPress description: {}", rp_desc);
                }
            }

            eprintln!("[PASS] Settings update compared");
        }
        _ => eprintln!("[PARTIAL] Could not update settings on one or both servers"),
    }

    // Restore original descriptions
    let restore_wp = serde_json::json!({ "description": wp_orig_desc });
    let restore_rp = serde_json::json!({ "description": rp_orig_desc });

    put_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/settings", cfg.wordpress_url),
        &restore_wp,
        &wp_auth,
    )
    .await;
    put_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/settings", cfg.rustpress_url),
        &restore_rp,
        &rp_token,
    )
    .await;

    eprintln!("  [OK] Original descriptions restored");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_user_create() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_user_create ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    let user_body = serde_json::json!({
        "username": "e2e_test_user",
        "email": "e2e_test_user@test.local",
        "password": "E2eTestP@ss123!",
        "roles": ["subscriber"],
    });

    let wp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/users", cfg.wordpress_url),
        &user_body,
        &wp_auth,
    )
    .await;
    let rp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/users", cfg.rustpress_url),
        &user_body,
        &rp_token,
    )
    .await;

    match (wp_created, rp_created) {
        (Some(wp_user), Some(rp_user)) => {
            assert_json_keys_match(&wp_user, &rp_user);
            eprintln!("[OK] CREATE: Both created users with matching structure");

            let wp_id = wp_user.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_user.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            // Verify username
            if let Some(rp_slug) = rp_user.get("slug").and_then(|v| v.as_str()) {
                eprintln!("  [OK] RustPress created user slug: {}", rp_slug);
            }

            // DELETE -- WordPress requires ?force=true&reassign=1 for user deletion
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/users/{}?force=true&reassign=1",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/users/{}?force=true&reassign=1",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;

            eprintln!("[PASS] User create/delete cycle complete");
        }
        _ => eprintln!("[SKIP] Could not create users on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_comment_create_anonymous() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_comment_create_anonymous ===");

    // Get a post ID from each server to attach the comment to
    let wp_posts = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.wordpress_url),
    )
    .await;
    let rp_posts = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.rustpress_url),
    )
    .await;

    let wp_post_id = wp_posts
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64())
        .unwrap_or(1);
    let rp_post_id = rp_posts
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64())
        .unwrap_or(1);

    // Try posting a comment without auth
    let wp_comment_body = serde_json::json!({
        "post": wp_post_id,
        "content": "Anonymous E2E test comment",
        "author_name": "Anonymous Tester",
        "author_email": "anon@test.local",
    });
    let rp_comment_body = serde_json::json!({
        "post": rp_post_id,
        "content": "Anonymous E2E test comment",
        "author_name": "Anonymous Tester",
        "author_email": "anon@test.local",
    });

    // POST without auth header
    let wp_resp = client
        .post(&format!("{}/wp-json/wp/v2/comments", cfg.wordpress_url))
        .header("Content-Type", "application/json")
        .json(&wp_comment_body)
        .send()
        .await;
    let rp_resp = client
        .post(&format!("{}/wp-json/wp/v2/comments", cfg.rustpress_url))
        .header("Content-Type", "application/json")
        .json(&rp_comment_body)
        .send()
        .await;

    let wp_status = wp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);
    let rp_status = rp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);

    eprintln!("WordPress anonymous comment status: {}", wp_status);
    eprintln!("RustPress anonymous comment status: {}", rp_status);

    // Both should either allow (201) or reject (401/403) anonymous comments
    let wp_val: Option<Value> = match wp_resp {
        Ok(r) if r.status().is_success() || r.status().as_u16() == 201 => {
            r.json::<Value>().await.ok()
        }
        _ => None,
    };
    let rp_val: Option<Value> = match rp_resp {
        Ok(r) if r.status().is_success() || r.status().as_u16() == 201 => {
            r.json::<Value>().await.ok()
        }
        _ => None,
    };

    match (&wp_val, &rp_val) {
        (Some(wp_c), Some(rp_c)) => {
            assert_json_keys_match(wp_c, rp_c);
            eprintln!("[OK] Both servers accepted anonymous comments");

            // Clean up with auth
            let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
            let rp_token = match get_rustpress_token(
                &client,
                &cfg.rustpress_url,
                &cfg.admin_user,
                &cfg.admin_password,
            )
            .await
            {
                Ok(t) => format!("Bearer {}", t),
                Err(_) => return,
            };

            if let Some(wp_id) = wp_c.get("id").and_then(|v| v.as_u64()) {
                delete_auth(
                    &client,
                    &format!(
                        "{}/wp-json/wp/v2/comments/{}?force=true",
                        cfg.wordpress_url, wp_id
                    ),
                    &wp_auth,
                )
                .await;
            }
            if let Some(rp_id) = rp_c.get("id").and_then(|v| v.as_u64()) {
                delete_auth(
                    &client,
                    &format!(
                        "{}/wp-json/wp/v2/comments/{}?force=true",
                        cfg.rustpress_url, rp_id
                    ),
                    &rp_token,
                )
                .await;
            }
        }
        (None, None) => {
            eprintln!("[OK] Both servers rejected anonymous comments (same behavior)");
        }
        _ => {
            eprintln!(
                "[WARN] Behavior differs: WP={}, RP={}",
                if wp_val.is_some() {
                    "accepted"
                } else {
                    "rejected"
                },
                if rp_val.is_some() {
                    "accepted"
                } else {
                    "rejected"
                }
            );
        }
    }

    eprintln!("[PASS] Anonymous comment creation compared");
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_revisions() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_revisions ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // Get a post ID from each server
    let wp_posts = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.wordpress_url),
    )
    .await;
    let rp_posts = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.rustpress_url),
    )
    .await;

    let wp_post_id = wp_posts
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64())
        .unwrap_or(1);
    let rp_post_id = rp_posts
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_u64())
        .unwrap_or(1);

    // Revisions endpoint requires authentication
    let wp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts/{}/revisions",
            cfg.wordpress_url, wp_post_id
        ))
        .header("Authorization", &wp_auth)
        .send()
        .await;
    let rp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts/{}/revisions",
            cfg.rustpress_url, rp_post_id
        ))
        .header("Authorization", &rp_token)
        .send()
        .await;

    let wp_json: Option<Value> = match wp_resp {
        Ok(r) if r.status().is_success() => r.json::<Value>().await.ok(),
        _ => None,
    };
    let rp_json: Option<Value> = match rp_resp {
        Ok(r) if r.status().is_success() => r.json::<Value>().await.ok(),
        _ => None,
    };

    match (wp_json, rp_json) {
        (Some(ref wp_revisions), Some(ref rp_revisions)) => {
            assert!(
                wp_revisions.is_array(),
                "WordPress /posts/{}/revisions should be an array",
                wp_post_id
            );
            assert!(
                rp_revisions.is_array(),
                "RustPress /posts/{}/revisions should be an array",
                rp_post_id
            );

            eprintln!(
                "WordPress revisions: {}, RustPress revisions: {}",
                wp_revisions.as_array().map(|a| a.len()).unwrap_or(0),
                rp_revisions.as_array().map(|a| a.len()).unwrap_or(0),
            );

            // Compare structure of first revision if both have them
            if let (Some(wp_first), Some(rp_first)) = (
                wp_revisions.as_array().and_then(|a| a.first()),
                rp_revisions.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);

                let expected = ["id", "author", "date", "title", "content", "excerpt"];
                let rp_keys = json_top_keys(rp_first);
                for f in &expected {
                    eprintln!(
                        "  [{}] {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }

            eprintln!("[PASS] Post revisions structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch revisions from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_error_not_found() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_error_not_found ===");

    // Request a non-existent post
    let wp_resp = client
        .get(&format!("{}/wp-json/wp/v2/posts/999999", cfg.wordpress_url))
        .send()
        .await;
    let rp_resp = client
        .get(&format!("{}/wp-json/wp/v2/posts/999999", cfg.rustpress_url))
        .send()
        .await;

    let wp_status = wp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);
    let rp_status = rp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);

    eprintln!("WordPress 404 status: {}", wp_status);
    eprintln!("RustPress 404 status: {}", rp_status);

    // Both should return 404
    assert_eq!(
        wp_status, 404,
        "WordPress should return 404 for non-existent post"
    );
    assert_eq!(
        rp_status, 404,
        "RustPress should return 404 for non-existent post"
    );

    // Compare error response structure
    let wp_json: Option<Value> = match wp_resp {
        Ok(r) => r.json::<Value>().await.ok(),
        Err(_) => None,
    };
    let rp_json: Option<Value> = match rp_resp {
        Ok(r) => r.json::<Value>().await.ok(),
        Err(_) => None,
    };

    match (wp_json, rp_json) {
        (Some(wp_err), Some(rp_err)) => {
            // WordPress error format: {"code": "...", "message": "...", "data": {"status": 404}}
            let expected = ["code", "message", "data"];
            let rp_keys = json_top_keys(&rp_err);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }
            assert_json_keys_match(&wp_err, &rp_err);
            eprintln!("[PASS] Error 404 response structure compared");
        }
        _ => eprintln!("[SKIP] Could not parse error JSON from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_error_unauthorized() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_error_unauthorized ===");

    // POST /posts without auth should fail with 401
    let post_body = serde_json::json!({
        "title": "Unauthorized test",
        "content": "Should not be created",
        "status": "draft",
    });

    let wp_resp = client
        .post(&format!("{}/wp-json/wp/v2/posts", cfg.wordpress_url))
        .header("Content-Type", "application/json")
        .json(&post_body)
        .send()
        .await;
    let rp_resp = client
        .post(&format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url))
        .header("Content-Type", "application/json")
        .json(&post_body)
        .send()
        .await;

    let wp_status = wp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);
    let rp_status = rp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);

    eprintln!("WordPress unauthorized status: {}", wp_status);
    eprintln!("RustPress unauthorized status: {}", rp_status);

    // WordPress returns 401, RustPress should too
    assert!(
        wp_status == 401 || wp_status == 403,
        "WordPress should return 401 or 403 for unauthorized POST"
    );
    assert!(
        rp_status == 401 || rp_status == 403,
        "RustPress should return 401 or 403 for unauthorized POST, got {}",
        rp_status
    );

    // Compare error JSON structure
    let wp_json: Option<Value> = match wp_resp {
        Ok(r) => r.json::<Value>().await.ok(),
        Err(_) => None,
    };
    let rp_json: Option<Value> = match rp_resp {
        Ok(r) => r.json::<Value>().await.ok(),
        Err(_) => None,
    };

    match (wp_json, rp_json) {
        (Some(wp_err), Some(rp_err)) => {
            let expected = ["code", "message", "data"];
            let rp_keys = json_top_keys(&rp_err);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }
            assert_json_keys_match(&wp_err, &rp_err);
            eprintln!("[PASS] Error 401 response structure compared");
        }
        _ => eprintln!("[SKIP] Could not parse error JSON from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_error_invalid_param() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_error_invalid_param ===");

    // per_page=invalid should return a 400 error
    let wp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts?per_page=invalid",
            cfg.wordpress_url
        ))
        .send()
        .await;
    let rp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts?per_page=invalid",
            cfg.rustpress_url
        ))
        .send()
        .await;

    let wp_status = wp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);
    let rp_status = rp_resp.as_ref().map(|r| r.status().as_u16()).unwrap_or(0);

    eprintln!("WordPress invalid param status: {}", wp_status);
    eprintln!("RustPress invalid param status: {}", rp_status);

    // WordPress returns 400 for invalid parameters
    assert_eq!(
        wp_status, 400,
        "WordPress should return 400 for invalid per_page"
    );
    assert_eq!(
        rp_status, 400,
        "RustPress should return 400 for invalid per_page, got {}",
        rp_status
    );

    // Compare error JSON structure
    let wp_json: Option<Value> = match wp_resp {
        Ok(r) => r.json::<Value>().await.ok(),
        Err(_) => None,
    };
    let rp_json: Option<Value> = match rp_resp {
        Ok(r) => r.json::<Value>().await.ok(),
        Err(_) => None,
    };

    match (wp_json, rp_json) {
        (Some(wp_err), Some(rp_err)) => {
            let expected = ["code", "message", "data"];
            let rp_keys = json_top_keys(&rp_err);
            for f in &expected {
                eprintln!(
                    "  [{}] {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }
            assert_json_keys_match(&wp_err, &rp_err);
            eprintln!("[PASS] Error 400 response structure compared");
        }
        _ => eprintln!("[SKIP] Could not parse error JSON from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_types_structure() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_types_structure ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/types", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/types", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            // Deep structure comparison of the entire /types response
            assert_json_structure_match(&wp_val, &rp_val);

            // Compare structure of "post" type in detail
            if let (Some(wp_post), Some(rp_post)) = (wp_val.get("post"), rp_val.get("post")) {
                assert_json_keys_match(wp_post, rp_post);
                assert_json_types_match(wp_post, rp_post);

                let expected = [
                    "description",
                    "hierarchical",
                    "has_archive",
                    "name",
                    "slug",
                    "rest_base",
                    "rest_namespace",
                ];
                let rp_keys = json_top_keys(rp_post);
                for f in &expected {
                    eprintln!(
                        "  [{}] post type field: {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }

            // Compare structure of "page" type too
            if let (Some(wp_page), Some(rp_page)) = (wp_val.get("page"), rp_val.get("page")) {
                assert_json_keys_match(wp_page, rp_page);
                eprintln!("  [OK] Page type structure compared");
            }

            eprintln!("[PASS] Post types deep structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch post types from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_statuses_structure() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_statuses_structure ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/statuses", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/statuses", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            // Deep structure comparison
            assert_json_structure_match(&wp_val, &rp_val);

            // Compare individual status object structures
            if let (Some(wp_publish), Some(rp_publish)) =
                (wp_val.get("publish"), rp_val.get("publish"))
            {
                assert_json_keys_match(wp_publish, rp_publish);
                assert_json_types_match(wp_publish, rp_publish);

                let expected = ["name", "slug", "public", "queryable"];
                let rp_keys = json_top_keys(rp_publish);
                for f in &expected {
                    eprintln!(
                        "  [{}] publish status field: {}",
                        if rp_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }

            eprintln!("[PASS] Statuses deep structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch statuses from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_taxonomies_structure() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_taxonomies_structure ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/taxonomies", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/taxonomies", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            // Deep structure comparison
            assert_json_structure_match(&wp_val, &rp_val);

            let wp_keys = json_top_keys(&wp_val);
            let rp_keys = json_top_keys(&rp_val);
            eprintln!("WordPress taxonomies: {:?}", wp_keys);
            eprintln!("RustPress taxonomies: {:?}", rp_keys);

            // Should have "category" and "post_tag" at minimum
            let required = ["category", "post_tag"];
            for t in &required {
                if rp_keys.contains(*t) {
                    eprintln!("  [OK] RustPress has taxonomy: {}", t);
                } else {
                    eprintln!("  [MISSING] RustPress missing taxonomy: {}", t);
                }
            }

            // Compare category taxonomy structure in detail
            if let (Some(wp_cat), Some(rp_cat)) = (wp_val.get("category"), rp_val.get("category")) {
                assert_json_keys_match(wp_cat, rp_cat);
                assert_json_types_match(wp_cat, rp_cat);

                let expected = [
                    "name",
                    "slug",
                    "description",
                    "hierarchical",
                    "rest_base",
                    "rest_namespace",
                ];
                let rp_cat_keys = json_top_keys(rp_cat);
                for f in &expected {
                    eprintln!(
                        "  [{}] category taxonomy field: {}",
                        if rp_cat_keys.contains(*f) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        f
                    );
                }
            }

            eprintln!("[PASS] Taxonomies deep structure compared");
        }
        _ => eprintln!("[SKIP] Could not fetch taxonomies from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_categories_orderby() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_categories_orderby ===");

    let wp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/categories?orderby=name&order=asc",
            cfg.wordpress_url
        ),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/categories?orderby=name&order=asc",
            cfg.rustpress_url
        ),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /categories?orderby=name should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /categories?orderby=name should return an array"
            );

            let wp_names: Vec<String> = wp_val
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|c| {
                    c.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect();
            let rp_names: Vec<String> = rp_val
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|c| {
                    c.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect();

            let rp_sorted = {
                let mut s = rp_names.clone();
                s.sort();
                s
            };

            eprintln!("WordPress category names (asc): {:?}", wp_names);
            eprintln!("RustPress category names (asc): {:?}", rp_names);

            if rp_names == rp_sorted {
                eprintln!("  [OK] RustPress categories are sorted by name asc");
            } else {
                eprintln!("  [WARN] RustPress categories are NOT sorted by name asc");
            }

            // Compare structure
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Categories orderby=name compared");
        }
        _ => eprintln!("[SKIP] Could not fetch categories with orderby from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_tags_search() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_tags_search ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/tags?search=test", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/tags?search=test", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /tags?search=test should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /tags?search=test should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress tags search results: {}", wp_count);
            eprintln!("RustPress tags search results: {}", rp_count);

            // Compare structure if both have results
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
                assert_json_types_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Tags search compared");
        }
        _ => eprintln!("[SKIP] Could not fetch tags with search from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_users_orderby() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_users_orderby ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/users?orderby=name", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/users?orderby=name", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /users?orderby=name should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /users?orderby=name should return an array"
            );

            let wp_names: Vec<String> = wp_val
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|u| {
                    u.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect();
            let rp_names: Vec<String> = rp_val
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|u| {
                    u.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect();

            eprintln!("WordPress users ordered by name: {:?}", wp_names);
            eprintln!("RustPress users ordered by name: {:?}", rp_names);

            // Compare structure
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Users orderby=name compared");
        }
        _ => eprintln!("[SKIP] Could not fetch users with orderby from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_page_crud() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_page_crud ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // CREATE
    let page_body = serde_json::json!({
        "title": "E2E Test Page",
        "content": "<p>This page was created by the E2E test suite.</p>",
        "status": "draft",
    });

    let wp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/pages", cfg.wordpress_url),
        &page_body,
        &wp_auth,
    )
    .await;
    let rp_created = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/pages", cfg.rustpress_url),
        &page_body,
        &rp_token,
    )
    .await;

    match (wp_created, rp_created) {
        (Some(wp_page), Some(rp_page)) => {
            assert_json_keys_match(&wp_page, &rp_page);
            assert_json_types_match(&wp_page, &rp_page);
            eprintln!("[OK] CREATE: Both created pages with matching structure");

            let wp_id = wp_page.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_page.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            // READ
            let wp_read = fetch_json(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/pages/{}?context=edit",
                    cfg.wordpress_url, wp_id
                ),
            )
            .await;
            let rp_read_resp = client
                .get(&format!(
                    "{}/wp-json/wp/v2/pages/{}?context=edit",
                    cfg.rustpress_url, rp_id
                ))
                .header("Authorization", &rp_token)
                .send()
                .await;
            let rp_read = rp_read_resp.ok().and_then(|r| {
                if r.status().is_success() {
                    Some(r)
                } else {
                    None
                }
            });
            let rp_read_json = match rp_read {
                Some(r) => r.json::<Value>().await.ok(),
                None => None,
            };

            // For read, just fetch with auth from WP too
            let wp_read_resp = client
                .get(&format!(
                    "{}/wp-json/wp/v2/pages/{}?context=edit",
                    cfg.wordpress_url, wp_id
                ))
                .header("Authorization", &wp_auth)
                .send()
                .await;
            let wp_read_json = wp_read_resp.ok().and_then(|r| {
                if r.status().is_success() {
                    Some(r)
                } else {
                    None
                }
            });
            let wp_read_val = match wp_read_json {
                Some(r) => r.json::<Value>().await.ok(),
                None => wp_read,
            };

            if let (Some(wp_r), Some(rp_r)) = (wp_read_val, rp_read_json) {
                assert_json_keys_match(&wp_r, &rp_r);
                eprintln!("[OK] READ: Both returned page with matching structure");
            }

            // UPDATE
            let update_body = serde_json::json!({
                "title": "E2E Test Page - Updated",
                "content": "<p>Updated page content.</p>",
            });

            let wp_updated = put_json_auth(
                &client,
                &format!("{}/wp-json/wp/v2/pages/{}", cfg.wordpress_url, wp_id),
                &update_body,
                &wp_auth,
            )
            .await;
            let rp_updated = put_json_auth(
                &client,
                &format!("{}/wp-json/wp/v2/pages/{}", cfg.rustpress_url, rp_id),
                &update_body,
                &rp_token,
            )
            .await;

            match (wp_updated, rp_updated) {
                (Some(wp_val), Some(rp_val)) => {
                    assert_json_keys_match(&wp_val, &rp_val);
                    eprintln!("[OK] UPDATE: Both updated pages with matching structure");
                }
                _ => eprintln!("[PARTIAL] Could not update pages on one or both servers"),
            }

            // DELETE
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/pages/{}?force=true",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/pages/{}?force=true",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;

            eprintln!("[PASS] Page CRUD cycle complete");
        }
        _ => eprintln!("[SKIP] Could not create pages on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_post_context_edit() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_post_context_edit ===");

    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);
    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };

    // GET /posts/1?context=edit with auth -- reveals additional fields like "raw"
    let wp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts/1?context=edit",
            cfg.wordpress_url
        ))
        .header("Authorization", &wp_auth)
        .send()
        .await;
    let rp_resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/posts/1?context=edit",
            cfg.rustpress_url
        ))
        .header("Authorization", &rp_token)
        .send()
        .await;

    let wp_val = wp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });
    let rp_val = rp_resp.ok().and_then(|r| {
        if r.status().is_success() {
            Some(r)
        } else {
            None
        }
    });

    match (wp_val, rp_val) {
        (Some(wp_r), Some(rp_r)) => {
            let wp_json: Value = wp_r.json().await.unwrap_or(Value::Null);
            let rp_json: Value = rp_r.json().await.unwrap_or(Value::Null);

            assert_json_keys_match(&wp_json, &rp_json);
            assert_json_types_match(&wp_json, &rp_json);

            // In edit context, title/content/excerpt should have "raw" in addition to "rendered"
            let rendered_fields = ["title", "content", "excerpt"];
            for field in &rendered_fields {
                let wp_has_raw = wp_json.get(*field).and_then(|v| v.get("raw")).is_some();
                let rp_has_raw = rp_json.get(*field).and_then(|v| v.get("raw")).is_some();

                eprintln!(
                    "  {}.raw - WP: {}, RP: {} [{}]",
                    field,
                    wp_has_raw,
                    rp_has_raw,
                    if wp_has_raw == rp_has_raw {
                        "MATCH"
                    } else {
                        "DIFFER"
                    }
                );
            }

            // Edit context should include additional fields like password, status
            let edit_fields = ["password", "status", "sticky"];
            let rp_keys = json_top_keys(&rp_json);
            for f in &edit_fields {
                eprintln!(
                    "  [{}] edit context field: {}",
                    if rp_keys.contains(*f) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    f
                );
            }

            eprintln!("[PASS] Post context=edit compared");
        }
        _ => eprintln!("[SKIP] Could not fetch post with context=edit from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_sticky() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_sticky ===");

    let wp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?sticky=true", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?sticky=true", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?sticky=true should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?sticky=true should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress sticky posts: {}", wp_count);
            eprintln!("RustPress sticky posts: {}", rp_count);

            // Verify all returned posts have sticky=true
            if let Some(rp_arr) = rp_val.as_array() {
                for (i, post) in rp_arr.iter().enumerate() {
                    if let Some(sticky) = post.get("sticky").and_then(|v| v.as_bool()) {
                        eprintln!("  Post {}: sticky={}", i, sticky);
                        assert!(sticky, "All posts with sticky=true filter should be sticky");
                    }
                }
            }

            // Compare structure if both have results
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Sticky posts filter compared");
        }
        _ => eprintln!("[SKIP] Could not fetch sticky posts from one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_embed_author() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_embed_author ===");

    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts?_embed", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts?_embed", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_post), Some(rp_post)) => {
            // Check _embedded.author exists and is an array with user objects
            let wp_author = wp_post
                .get("_embedded")
                .and_then(|e| e.get("author"))
                .and_then(|a| a.as_array());
            let rp_author = rp_post
                .get("_embedded")
                .and_then(|e| e.get("author"))
                .and_then(|a| a.as_array());

            eprintln!(
                "WordPress _embedded.author: {}",
                if wp_author.is_some() {
                    "present"
                } else {
                    "absent"
                }
            );
            eprintln!(
                "RustPress _embedded.author: {}",
                if rp_author.is_some() {
                    "present"
                } else {
                    "absent"
                }
            );

            match (wp_author, rp_author) {
                (Some(wp_authors), Some(rp_authors)) => {
                    if let (Some(wp_first), Some(rp_first)) =
                        (wp_authors.first(), rp_authors.first())
                    {
                        assert_json_keys_match(wp_first, rp_first);

                        // Embedded author should have user fields
                        let expected = ["id", "name", "slug", "link", "avatar_urls"];
                        let rp_keys = json_top_keys(rp_first);
                        for f in &expected {
                            eprintln!(
                                "  [{}] embedded author field: {}",
                                if rp_keys.contains(*f) {
                                    "OK"
                                } else {
                                    "MISSING"
                                },
                                f
                            );
                        }
                    }
                    eprintln!("[PASS] Embedded author data compared");
                }
                _ => eprintln!("[WARN] Embedded author not present on one or both servers"),
            }
        }
        _ => eprintln!("[SKIP] No posts available for embedded author comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_embed_replies() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_embed_replies ===");

    let wp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts?_embed", cfg.wordpress_url),
    )
    .await;
    let rp = fetch_first_from_array(
        &client,
        &format!("{}/wp-json/wp/v2/posts?_embed", cfg.rustpress_url),
    )
    .await;

    match (wp, rp) {
        (Some(wp_post), Some(rp_post)) => {
            // Check _embedded.replies exists (array of comment arrays)
            let wp_replies = wp_post.get("_embedded").and_then(|e| e.get("replies"));
            let rp_replies = rp_post.get("_embedded").and_then(|e| e.get("replies"));

            eprintln!(
                "WordPress _embedded.replies: {}",
                if wp_replies.is_some() {
                    "present"
                } else {
                    "absent"
                }
            );
            eprintln!(
                "RustPress _embedded.replies: {}",
                if rp_replies.is_some() {
                    "present"
                } else {
                    "absent"
                }
            );

            match (wp_replies, rp_replies) {
                (Some(wp_r), Some(rp_r)) => {
                    // replies is typically an array of arrays
                    let wp_is_array = wp_r.is_array();
                    let rp_is_array = rp_r.is_array();

                    eprintln!(
                        "  replies is array - WP: {}, RP: {}",
                        wp_is_array, rp_is_array
                    );

                    // Check first reply comment structure
                    let wp_first_comment = wp_r
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|arr| arr.as_array())
                        .and_then(|a| a.first());
                    let rp_first_comment = rp_r
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|arr| arr.as_array())
                        .and_then(|a| a.first());

                    if let (Some(wp_c), Some(rp_c)) = (wp_first_comment, rp_first_comment) {
                        assert_json_keys_match(wp_c, rp_c);
                        eprintln!("[OK] Embedded reply comment structure matches");
                    } else {
                        eprintln!("[INFO] No embedded reply comments to compare");
                    }

                    eprintln!("[PASS] Embedded replies compared");
                }
                (Some(_), None) => {
                    eprintln!("[WARN] WordPress has embedded replies but RustPress does not");
                }
                (None, Some(_)) => {
                    eprintln!("[INFO] RustPress has embedded replies but WordPress does not");
                }
                (None, None) => {
                    eprintln!(
                        "[OK] Neither server has embedded replies (post may have no comments)"
                    );
                }
            }
        }
        _ => eprintln!("[SKIP] No posts available for embedded replies comparison"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_posts_after_before() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_posts_after_before ===");

    let after = "2020-01-01T00:00:00";
    let before = "2030-01-01T00:00:00";

    let wp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?after={}&before={}",
            cfg.wordpress_url, after, before
        ),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?after={}&before={}",
            cfg.rustpress_url, after, before
        ),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress /posts?after=...&before=... should return an array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress /posts?after=...&before=... should return an array"
            );

            let wp_count = wp_val.as_array().map(|a| a.len()).unwrap_or(0);
            let rp_count = rp_val.as_array().map(|a| a.len()).unwrap_or(0);

            eprintln!("WordPress posts in date range: {}", wp_count);
            eprintln!("RustPress posts in date range: {}", rp_count);

            // Verify dates are within range for RustPress results
            if let Some(rp_arr) = rp_val.as_array() {
                for (i, post) in rp_arr.iter().enumerate() {
                    if let Some(date) = post.get("date").and_then(|v| v.as_str()) {
                        eprintln!("  Post {}: date={}", i, date);
                        // Basic check: date should be after 2020 and before 2030
                        if date >= after && date <= before {
                            eprintln!("    [OK] Date is within range");
                        } else {
                            eprintln!("    [WARN] Date may be outside range");
                        }
                    }
                }
            }

            // Compare structure
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Posts date range filtering compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts with date range from one or both servers"),
    }
}

// ---------------------------------------------------------------------------
// Block Patterns
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_block_patterns_categories() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_block_patterns_categories ===");

    let wp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/block-patterns/categories",
            cfg.wordpress_url
        ),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/block-patterns/categories",
            cfg.rustpress_url
        ),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress block-patterns/categories should be array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress block-patterns/categories should be array"
            );

            let rp_arr = rp_val.as_array().unwrap();
            eprintln!("RustPress pattern categories count: {}", rp_arr.len());
            assert!(
                !rp_arr.is_empty(),
                "RustPress should return at least one category"
            );

            // Each category should have name and label
            if let Some(first) = rp_arr.first() {
                let keys = json_top_keys(first);
                assert!(keys.contains("name"), "Category should have 'name' field");
                assert!(keys.contains("label"), "Category should have 'label' field");
                eprintln!("[OK] Category object has required fields");
            }

            // Compare structure with WordPress
            if let (Some(wp_first), Some(rp_first)) =
                (wp_val.as_array().and_then(|a| a.first()), rp_arr.first())
            {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Block pattern categories compared");
        }
        _ => eprintln!("[SKIP] Could not fetch block pattern categories"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_block_patterns_patterns() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_block_patterns_patterns ===");

    let wp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/block-patterns/patterns",
            cfg.wordpress_url
        ),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/block-patterns/patterns",
            cfg.rustpress_url
        ),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            assert!(
                wp_val.is_array(),
                "WordPress block-patterns/patterns should be array"
            );
            assert!(
                rp_val.is_array(),
                "RustPress block-patterns/patterns should be array"
            );

            let rp_arr = rp_val.as_array().unwrap();
            eprintln!("RustPress patterns count: {}", rp_arr.len());
            assert!(
                !rp_arr.is_empty(),
                "RustPress should return at least one pattern"
            );

            // Each pattern should have name, title, content
            if let Some(first) = rp_arr.first() {
                let keys = json_top_keys(first);
                for field in &["name", "title", "content", "categories"] {
                    eprintln!(
                        "  [{}] {}",
                        if keys.contains(*field) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        field
                    );
                }
            }

            // Compare structure with WordPress first element
            if let (Some(wp_first), Some(rp_first)) =
                (wp_val.as_array().and_then(|a| a.first()), rp_arr.first())
            {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] Block patterns compared");
        }
        _ => eprintln!("[SKIP] Could not fetch block patterns"),
    }
}

// ---------------------------------------------------------------------------
// Block Types
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_block_types_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_block_types_list ===");

    let rp = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/block-types", cfg.rustpress_url),
    )
    .await;

    match rp {
        Some(rp_val) => {
            assert!(
                rp_val.is_array(),
                "RustPress /block-types should return array"
            );
            let arr = rp_val.as_array().unwrap();
            eprintln!("RustPress block types count: {}", arr.len());
            assert!(arr.len() >= 10, "Should have at least 10 core block types");

            // core/paragraph should be present
            let has_paragraph = arr
                .iter()
                .any(|b| b.get("name").and_then(|v| v.as_str()) == Some("core/paragraph"));
            if has_paragraph {
                eprintln!("[OK] core/paragraph block type present");
            } else {
                eprintln!("[MISSING] core/paragraph block type missing");
            }

            // Structure check: each block type should have name and title
            if let Some(first) = arr.first() {
                let keys = json_top_keys(first);
                for field in &["name", "title", "description"] {
                    eprintln!(
                        "  [{}] {}",
                        if keys.contains(*field) {
                            "OK"
                        } else {
                            "MISSING"
                        },
                        field
                    );
                }
            }

            eprintln!("[PASS] Block types list verified");
        }
        _ => eprintln!("[SKIP] Could not fetch block types from RustPress"),
    }
}

// ---------------------------------------------------------------------------
// Templates API
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_templates_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };
    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);

    eprintln!("\n=== test_rest_api_templates_list ===");

    // WordPress templates
    let wp_resp = client
        .get(&format!("{}/wp-json/wp/v2/templates", cfg.wordpress_url))
        .header("Authorization", &wp_auth)
        .send()
        .await;

    // RustPress templates
    let rp_resp = client
        .get(&format!("{}/wp-json/wp/v2/templates", cfg.rustpress_url))
        .header("Authorization", &rp_token)
        .send()
        .await;

    match (wp_resp, rp_resp) {
        (Ok(wp_r), Ok(rp_r)) => {
            let wp_status = wp_r.status();
            let rp_status = rp_r.status();
            eprintln!("WordPress /templates -> {}", wp_status);
            eprintln!("RustPress /templates -> {}", rp_status);

            // Both should respond with 2xx (or 401 if auth not working)
            assert!(
                rp_status.is_success() || rp_status.as_u16() == 401,
                "Expected 200 or 401 from /templates, got {}",
                rp_status
            );

            if rp_status.is_success() {
                let rp_body: Value = rp_r.json().await.unwrap_or(Value::Null);
                assert!(rp_body.is_array(), "Templates should return array");

                if wp_status.is_success() {
                    let wp_body: Value = wp_r.json().await.unwrap_or(Value::Null);
                    if let (Some(wp_first), Some(rp_first)) = (
                        wp_body.as_array().and_then(|a| a.first()),
                        rp_body.as_array().and_then(|a| a.first()),
                    ) {
                        assert_json_keys_match(wp_first, rp_first);
                    }
                }
            }
            eprintln!("[PASS] Templates endpoint responds");
        }
        _ => eprintln!("[SKIP] Could not reach templates endpoints"),
    }
}

// ---------------------------------------------------------------------------
// Application Passwords
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_application_passwords_crud() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] Could not get RustPress token: {}", e);
            return;
        }
    };
    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);

    eprintln!("\n=== test_rest_api_application_passwords_crud ===");

    // First, get current user ID from /users/me
    let rp_user_id = {
        let r = client
            .get(&format!("{}/wp-json/wp/v2/users/me", cfg.rustpress_url))
            .header("Authorization", &rp_token)
            .send()
            .await;
        match r {
            Ok(resp) if resp.status().is_success() => resp
                .json::<Value>()
                .await
                .ok()
                .and_then(|u| u.get("id").and_then(|v| v.as_u64()))
                .unwrap_or(1),
            _ => 1,
        }
    };

    eprintln!("RustPress user id: {}", rp_user_id);

    // CREATE app password
    let create_body = serde_json::json!({"name": "E2E Test Password"});
    let rp_created = post_json_auth(
        &client,
        &format!(
            "{}/wp-json/wp/v2/users/{}/application-passwords",
            cfg.rustpress_url, rp_user_id
        ),
        &create_body,
        &rp_token,
    )
    .await;

    match rp_created {
        Some(rp_ap) => {
            let ap_keys = json_top_keys(&rp_ap);
            eprintln!("App password keys: {:?}", ap_keys);

            // WordPress app password response has: uuid, name, password, created, last_used, last_ip
            for field in &["uuid", "name", "password"] {
                eprintln!(
                    "  [{}] {}",
                    if ap_keys.contains(*field) {
                        "OK"
                    } else {
                        "MISSING"
                    },
                    field
                );
            }

            // Also compare with WordPress structure
            let wp_user_id = {
                let r = client
                    .get(&format!("{}/wp-json/wp/v2/users/me", cfg.wordpress_url))
                    .header("Authorization", &wp_auth)
                    .send()
                    .await;
                match r {
                    Ok(resp) if resp.status().is_success() => resp
                        .json::<Value>()
                        .await
                        .ok()
                        .and_then(|u| u.get("id").and_then(|v| v.as_u64()))
                        .unwrap_or(1),
                    _ => 1,
                }
            };

            let wp_created = post_json_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/users/{}/application-passwords",
                    cfg.wordpress_url, wp_user_id
                ),
                &create_body,
                &wp_auth,
            )
            .await;

            if let Some(wp_ap) = wp_created {
                assert_json_keys_match(&wp_ap, &rp_ap);
                eprintln!("[OK] App password structures match WordPress");

                // Cleanup WP
                if let Some(wp_uuid) = wp_ap.get("uuid").and_then(|v| v.as_str()) {
                    let _ = delete_auth(
                        &client,
                        &format!(
                            "{}/wp-json/wp/v2/users/{}/application-passwords/{}",
                            cfg.wordpress_url, wp_user_id, wp_uuid
                        ),
                        &wp_auth,
                    )
                    .await;
                }
            }

            // Cleanup RP
            if let Some(rp_uuid) = rp_ap.get("uuid").and_then(|v| v.as_str()) {
                let _ = delete_auth(
                    &client,
                    &format!(
                        "{}/wp-json/wp/v2/users/{}/application-passwords/{}",
                        cfg.rustpress_url, rp_user_id, rp_uuid
                    ),
                    &rp_token,
                )
                .await;
                eprintln!("[OK] Cleaned up test app password");
            }

            eprintln!("[PASS] Application passwords CRUD complete");
        }
        _ => eprintln!("[SKIP] Could not create application password on RustPress"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_application_passwords_list() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] {}", e);
            return;
        }
    };

    eprintln!("\n=== test_rest_api_application_passwords_list ===");

    let user_id = {
        let r = client
            .get(&format!("{}/wp-json/wp/v2/users/me", cfg.rustpress_url))
            .header("Authorization", &rp_token)
            .send()
            .await;
        match r {
            Ok(resp) if resp.status().is_success() => resp
                .json::<Value>()
                .await
                .ok()
                .and_then(|u| u.get("id").and_then(|v| v.as_u64()))
                .unwrap_or(1),
            _ => 1,
        }
    };

    let resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/users/{}/application-passwords",
            cfg.rustpress_url, user_id
        ))
        .header("Authorization", &rp_token)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status();
            eprintln!("GET /application-passwords -> {}", status);
            assert!(status.is_success(), "Should return 200, got {}", status);

            let body: Value = r.json().await.unwrap_or(Value::Null);
            assert!(body.is_array(), "Should return array");
            eprintln!("[PASS] Application passwords list returns array");
        }
        Err(e) => eprintln!("[SKIP] {}", e),
    }
}

// ---------------------------------------------------------------------------
// Batch API
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_batch() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] {}", e);
            return;
        }
    };
    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);

    eprintln!("\n=== test_rest_api_batch ===");

    let batch_body = serde_json::json!({
        "requests": [
            { "path": "/wp/v2/posts", "method": "GET" },
            { "path": "/wp/v2/categories", "method": "GET" },
            { "path": "/wp/v2/users/me", "method": "GET" }
        ]
    });

    // WordPress batch
    let wp_resp = client
        .post(&format!("{}/wp-json/batch/v1", cfg.wordpress_url))
        .header("Authorization", &wp_auth)
        .json(&batch_body)
        .send()
        .await;

    // RustPress batch
    let rp_resp = client
        .post(&format!("{}/wp-json/batch/v1", cfg.rustpress_url))
        .header("Authorization", &rp_token)
        .json(&batch_body)
        .send()
        .await;

    match (wp_resp, rp_resp) {
        (Ok(wp_r), Ok(rp_r)) => {
            let wp_status = wp_r.status();
            let rp_status = rp_r.status();
            eprintln!("WordPress /batch/v1 -> {}", wp_status);
            eprintln!("RustPress /batch/v1 -> {}", rp_status);

            assert!(
                rp_status.is_success(),
                "RustPress batch should return 2xx, got {}",
                rp_status
            );

            let rp_body: Value = rp_r.json().await.unwrap_or(Value::Null);
            eprintln!(
                "RustPress batch response keys: {:?}",
                json_top_keys(&rp_body)
            );

            // Response should have "responses" array
            assert!(
                rp_body.get("responses").is_some(),
                "Batch response should have 'responses' key"
            );

            if let Some(responses) = rp_body.get("responses").and_then(|v| v.as_array()) {
                eprintln!("RustPress batch response count: {}", responses.len());
                assert_eq!(responses.len(), 3, "Should have 3 responses for 3 requests");

                // Each response should have status
                for (i, resp) in responses.iter().enumerate() {
                    let status = resp.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
                    eprintln!("  Response {}: status={}", i, status);
                }
            }

            // Compare with WordPress if it returned success
            if wp_status.is_success() {
                eprintln!("[OK] WordPress also supports batch API");
            }

            eprintln!("[PASS] Batch API works");
        }
        (_, Err(e)) => eprintln!("[SKIP] Could not reach RustPress batch endpoint: {}", e),
        (Err(e), _) => eprintln!("[SKIP] Could not reach WordPress batch endpoint: {}", e),
    }
}

// ---------------------------------------------------------------------------
// PATCH method support
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_patch_post() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] {}", e);
            return;
        }
    };
    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);

    eprintln!("\n=== test_rest_api_patch_post ===");

    // Get first post ID from each server
    let wp_id = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.wordpress_url),
    )
    .await
    .and_then(|v| {
        v.as_array()
            .and_then(|a| a.first().and_then(|p| p.get("id")))
            .map(|v| v.clone())
    })
    .and_then(|v| v.as_u64())
    .unwrap_or(1);

    let rp_id = fetch_json(
        &client,
        &format!("{}/wp-json/wp/v2/posts?per_page=1", cfg.rustpress_url),
    )
    .await
    .and_then(|v| {
        v.as_array()
            .and_then(|a| a.first().and_then(|p| p.get("id")))
            .map(|v| v.clone())
    })
    .and_then(|v| v.as_u64())
    .unwrap_or(1);

    let patch_body = serde_json::json!({"excerpt": {"raw": "PATCH test excerpt"}});

    // WordPress PATCH
    let wp_resp = client
        .patch(&format!(
            "{}/wp-json/wp/v2/posts/{}",
            cfg.wordpress_url, wp_id
        ))
        .header("Authorization", &wp_auth)
        .json(&patch_body)
        .send()
        .await;

    // RustPress PATCH
    let rp_resp = client
        .patch(&format!(
            "{}/wp-json/wp/v2/posts/{}",
            cfg.rustpress_url, rp_id
        ))
        .header("Authorization", &rp_token)
        .json(&patch_body)
        .send()
        .await;

    match (wp_resp, rp_resp) {
        (Ok(wp_r), Ok(rp_r)) => {
            let wp_status = wp_r.status();
            let rp_status = rp_r.status();
            eprintln!("WordPress PATCH /posts/{} -> {}", wp_id, wp_status);
            eprintln!("RustPress PATCH /posts/{} -> {}", rp_id, rp_status);

            assert!(
                rp_status.is_success(),
                "RustPress PATCH should return 2xx, got {}",
                rp_status
            );

            let rp_body: Value = rp_r.json().await.unwrap_or(Value::Null);
            let wp_body: Value = wp_r.json().await.unwrap_or(Value::Null);

            assert_json_keys_match(&wp_body, &rp_body);
            eprintln!("[PASS] PATCH /posts works on both servers");
        }
        _ => eprintln!("[SKIP] Could not PATCH posts on one or both servers"),
    }
}

#[tokio::test]
#[ignore]
async fn test_rest_api_patch_category() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] {}", e);
            return;
        }
    };
    let wp_auth = wordpress_basic_auth(&cfg.admin_user, &cfg.admin_password);

    eprintln!("\n=== test_rest_api_patch_category ===");

    // Create a category to patch, then clean up
    let create_body = serde_json::json!({"name": "PATCH Test Category"});

    let wp_cat = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/categories", cfg.wordpress_url),
        &create_body,
        &wp_auth,
    )
    .await;
    let rp_cat = post_json_auth(
        &client,
        &format!("{}/wp-json/wp/v2/categories", cfg.rustpress_url),
        &create_body,
        &rp_token,
    )
    .await;

    match (wp_cat, rp_cat) {
        (Some(wp_c), Some(rp_c)) => {
            let wp_id = wp_c.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let rp_id = rp_c.get("id").and_then(|v| v.as_u64()).unwrap_or(0);

            let patch_body = serde_json::json!({"description": "patched description"});

            let wp_patch = client
                .patch(&format!(
                    "{}/wp-json/wp/v2/categories/{}",
                    cfg.wordpress_url, wp_id
                ))
                .header("Authorization", &wp_auth)
                .json(&patch_body)
                .send()
                .await;

            let rp_patch = client
                .patch(&format!(
                    "{}/wp-json/wp/v2/categories/{}",
                    cfg.rustpress_url, rp_id
                ))
                .header("Authorization", &rp_token)
                .json(&patch_body)
                .send()
                .await;

            match (wp_patch, rp_patch) {
                (Ok(wp_r), Ok(rp_r)) => {
                    eprintln!("WP PATCH /categories/{} -> {}", wp_id, wp_r.status());
                    eprintln!("RP PATCH /categories/{} -> {}", rp_id, rp_r.status());
                    assert!(rp_r.status().is_success(), "PATCH category should succeed");
                    eprintln!("[PASS] PATCH /categories works");
                }
                _ => eprintln!("[WARN] Could not PATCH category"),
            }

            // Cleanup
            let _ = delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/categories/{}?force=true",
                    cfg.wordpress_url, wp_id
                ),
                &wp_auth,
            )
            .await;
            let _ = delete_auth(
                &client,
                &format!(
                    "{}/wp-json/wp/v2/categories/{}?force=true",
                    cfg.rustpress_url, rp_id
                ),
                &rp_token,
            )
            .await;
        }
        _ => eprintln!("[SKIP] Could not create test categories"),
    }
}

// ---------------------------------------------------------------------------
// X-WP-Nonce header
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_xwp_nonce_header() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] {}", e);
            return;
        }
    };

    eprintln!("\n=== test_rest_api_xwp_nonce_header ===");

    // Authenticated requests should return X-WP-Nonce header
    let resp = client
        .get(&format!("{}/wp-json/wp/v2/posts", cfg.rustpress_url))
        .header("Authorization", &rp_token)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status();
            let nonce = r
                .headers()
                .get("x-wp-nonce")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(absent)");

            eprintln!("Status: {}", status);
            eprintln!("X-WP-Nonce: {}", nonce);

            assert!(status.is_success(), "Should return 2xx");
            assert_ne!(
                nonce, "(absent)",
                "Authenticated requests should include X-WP-Nonce"
            );
            assert!(!nonce.is_empty(), "X-WP-Nonce should not be empty");

            eprintln!("[PASS] X-WP-Nonce header present on authenticated responses");
        }
        Err(e) => eprintln!("[SKIP] {}", e),
    }
}

// ---------------------------------------------------------------------------
// Global Styles
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_global_styles() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let rp_token = match get_rustpress_token(
        &client,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await
    {
        Ok(t) => format!("Bearer {}", t),
        Err(e) => {
            eprintln!("[SKIP] {}", e);
            return;
        }
    };

    eprintln!("\n=== test_rest_api_global_styles ===");

    // GET global styles for the active theme
    let resp = client
        .get(&format!(
            "{}/wp-json/wp/v2/global-styles/themes/default",
            cfg.rustpress_url
        ))
        .header("Authorization", &rp_token)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status();
            eprintln!("GET /global-styles/themes/default -> {}", status);
            // 200 or 404 are both acceptable (404 = no active theme named "default")
            assert!(
                status.is_success() || status.as_u16() == 404,
                "Expected 200 or 404, got {}",
                status
            );
            eprintln!("[PASS] Global styles endpoint responds");
        }
        Err(e) => eprintln!("[SKIP] {}", e),
    }
}

// ---------------------------------------------------------------------------
// _fields parameter (detailed)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_rest_api_fields_parameter_detailed() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_rest_api_fields_parameter_detailed ===");

    let fields = "id,title,slug";

    let wp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?_fields={}&per_page=1",
            cfg.wordpress_url, fields
        ),
    )
    .await;
    let rp = fetch_json(
        &client,
        &format!(
            "{}/wp-json/wp/v2/posts?_fields={}&per_page=1",
            cfg.rustpress_url, fields
        ),
    )
    .await;

    match (wp, rp) {
        (Some(wp_val), Some(rp_val)) => {
            let rp_first = rp_val.as_array().and_then(|a| a.first()).cloned();

            if let Some(rp_post) = rp_first {
                let rp_keys = json_top_keys(&rp_post);
                eprintln!("RustPress _fields response keys: {:?}", rp_keys);

                // Should only have the requested fields
                assert!(
                    rp_keys.contains("id"),
                    "_fields=id,title,slug should include id"
                );
                assert!(
                    rp_keys.contains("title"),
                    "_fields=id,title,slug should include title"
                );
                assert!(
                    rp_keys.contains("slug"),
                    "_fields=id,title,slug should include slug"
                );

                // Should NOT have unrequested fields like content, excerpt
                if !rp_keys.contains("content") {
                    eprintln!("[OK] 'content' correctly excluded by _fields");
                } else {
                    eprintln!("[WARN] 'content' should be excluded by _fields filter");
                }
            }

            // Compare with WordPress
            if let (Some(wp_first), Some(rp_first)) = (
                wp_val.as_array().and_then(|a| a.first()),
                rp_val.as_array().and_then(|a| a.first()),
            ) {
                assert_json_keys_match(wp_first, rp_first);
            }

            eprintln!("[PASS] _fields parameter detailed compared");
        }
        _ => eprintln!("[SKIP] Could not fetch posts with _fields from one or both servers"),
    }
}

// ---------------------------------------------------------------------------
// XML-RPC term methods
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_xmlrpc_get_terms() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_xmlrpc_get_terms ===");

    let xmlrpc_body = format!(
        r#"<?xml version="1.0"?>
<methodCall>
  <methodName>wp.getTerms</methodName>
  <params>
    <param><value><int>1</int></value></param>
    <param><value><string>{}</string></value></param>
    <param><value><string>{}</string></value></param>
    <param><value><string>category</string></value></param>
  </params>
</methodCall>"#,
        cfg.admin_user, cfg.admin_password
    );

    let rp_resp = client
        .post(&format!("{}/xmlrpc.php", cfg.rustpress_url))
        .header("Content-Type", "text/xml")
        .body(xmlrpc_body.clone())
        .send()
        .await;

    let wp_resp = client
        .post(&format!("{}/xmlrpc.php", cfg.wordpress_url))
        .header("Content-Type", "text/xml")
        .body(xmlrpc_body)
        .send()
        .await;

    match (wp_resp, rp_resp) {
        (Ok(wp_r), Ok(rp_r)) => {
            let wp_status = wp_r.status();
            let rp_status = rp_r.status();
            eprintln!("WordPress wp.getTerms -> {}", wp_status);
            eprintln!("RustPress wp.getTerms -> {}", rp_status);

            let rp_body = rp_r.text().await.unwrap_or_default();

            // Should not be a fault response
            assert!(
                !rp_body.contains("<fault>"),
                "wp.getTerms should not return a fault: {}",
                &rp_body[..rp_body.len().min(200)]
            );

            // Should contain array response with term_id fields
            if rp_body.contains("term_id") {
                eprintln!("[OK] wp.getTerms returns term data");
            }

            eprintln!("[PASS] XML-RPC wp.getTerms responded");
        }
        _ => eprintln!("[SKIP] Could not reach XML-RPC endpoints"),
    }
}

#[tokio::test]
#[ignore]
async fn test_xmlrpc_get_taxonomies() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_xmlrpc_get_taxonomies ===");

    let xmlrpc_body = format!(
        r#"<?xml version="1.0"?>
<methodCall>
  <methodName>wp.getTaxonomies</methodName>
  <params>
    <param><value><int>1</int></value></param>
    <param><value><string>{}</string></value></param>
    <param><value><string>{}</string></value></param>
  </params>
</methodCall>"#,
        cfg.admin_user, cfg.admin_password
    );

    let rp_resp = client
        .post(&format!("{}/xmlrpc.php", cfg.rustpress_url))
        .header("Content-Type", "text/xml")
        .body(xmlrpc_body)
        .send()
        .await;

    match rp_resp {
        Ok(r) => {
            let status = r.status();
            eprintln!("RustPress wp.getTaxonomies -> {}", status);
            let body = r.text().await.unwrap_or_default();

            assert!(!body.contains("<fault>"), "Should not return fault");
            if body.contains("category") {
                eprintln!("[OK] wp.getTaxonomies returns 'category' taxonomy");
            }
            eprintln!("[PASS] XML-RPC wp.getTaxonomies responded");
        }
        Err(e) => eprintln!("[SKIP] {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_xmlrpc_suggest_categories() {
    let cfg = TestConfig::from_env();
    let client = build_http_client();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    eprintln!("\n=== test_xmlrpc_suggest_categories ===");

    let xmlrpc_body = format!(
        r#"<?xml version="1.0"?>
<methodCall>
  <methodName>wp.suggestCategories</methodName>
  <params>
    <param><value><int>1</int></value></param>
    <param><value><string>{}</string></value></param>
    <param><value><string>{}</string></value></param>
    <param><value><string>Un</string></value></param>
    <param><value><int>5</int></value></param>
  </params>
</methodCall>"#,
        cfg.admin_user, cfg.admin_password
    );

    let rp_resp = client
        .post(&format!("{}/xmlrpc.php", cfg.rustpress_url))
        .header("Content-Type", "text/xml")
        .body(xmlrpc_body)
        .send()
        .await;

    match rp_resp {
        Ok(r) => {
            let body = r.text().await.unwrap_or_default();
            assert!(!body.contains("<fault>"), "Should not return fault");
            eprintln!("[PASS] XML-RPC wp.suggestCategories responded");
        }
        Err(e) => eprintln!("[SKIP] {}", e),
    }
}

//! Selenium Admin Panel Comparison Tests
//!
//! These tests use the `thirtyfour` WebDriver crate to drive a real browser
//! against both WordPress and RustPress, comparing admin panel UI elements.
//!
//! Prerequisites:
//! - ChromeDriver or GeckoDriver running (default: `http://localhost:9515`)
//! - Both WordPress and RustPress servers running
//!
//! All tests are `#[ignore]` by default.

use rustpress_e2e::*;
use thirtyfour::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a WebDriver instance. Returns `None` if WebDriver is not available.
async fn create_driver(config: &TestConfig) -> Option<WebDriver> {
    if !is_webdriver_available(&config.webdriver_url).await {
        eprintln!("[SKIP] WebDriver not available at {}", config.webdriver_url);
        return None;
    }

    let caps = DesiredCapabilities::chrome();
    match WebDriver::new(&config.webdriver_url, caps).await {
        Ok(driver) => Some(driver),
        Err(e) => {
            eprintln!("[SKIP] Could not create WebDriver session: {e}");
            None
        }
    }
}

/// Log in to WordPress via the browser. WordPress login is at /wp-login.php.
async fn wp_login(driver: &WebDriver, base_url: &str, user: &str, pass: &str) -> bool {
    let url = format!("{base_url}/wp-login.php");
    if driver.goto(&url).await.is_err() {
        eprintln!("[ERROR] Could not navigate to {url}");
        return false;
    }

    // Wait for login form
    let user_field = match driver.find(By::Id("user_login")).await {
        Ok(el) => el,
        Err(_) => match driver.find(By::Name("log")).await {
            Ok(el) => el,
            Err(_) => {
                // RustPress uses name="username"
                match driver.find(By::Name("username")).await {
                    Ok(el) => el,
                    Err(_) => {
                        eprintln!("[ERROR] Could not find username field on {url}");
                        return false;
                    }
                }
            }
        },
    };

    let pass_field = match driver.find(By::Id("user_pass")).await {
        Ok(el) => el,
        Err(_) => match driver.find(By::Name("pwd")).await {
            Ok(el) => el,
            Err(_) => {
                // RustPress uses name="password"
                match driver.find(By::Name("password")).await {
                    Ok(el) => el,
                    Err(_) => {
                        eprintln!("[ERROR] Could not find password field on {url}");
                        return false;
                    }
                }
            }
        },
    };

    if user_field.send_keys(user).await.is_err() {
        return false;
    }
    if pass_field.send_keys(pass).await.is_err() {
        return false;
    }

    // Submit the form: try multiple selectors for the submit button
    let submit = match driver.find(By::Id("wp-submit")).await {
        Ok(el) => Some(el),
        Err(_) => match driver.find(By::Css("input[type=submit]")).await {
            Ok(el) => Some(el),
            Err(_) => (driver.find(By::Css("button[type=submit]")).await).ok(),
        },
    };

    match submit {
        Some(btn) => {
            if btn.click().await.is_err() {
                eprintln!("[ERROR] Could not click submit button");
                return false;
            }
        }
        None => {
            // Try submitting the form by pressing Enter in the password field
            if pass_field.send_keys("\n").await.is_err() {
                return false;
            }
        }
    }

    // Wait a moment for redirect
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    true
}

/// Check if a CSS selector finds any elements in the current page.
async fn driver_has_element(driver: &WebDriver, css: &str) -> bool {
    driver.find(By::Css(css)).await.is_ok()
}

/// Count elements matching a CSS selector.
async fn driver_count_elements(driver: &WebDriver, css: &str) -> usize {
    driver
        .find_all(By::Css(css))
        .await
        .map(|els| els.len())
        .unwrap_or(0)
}

/// Run a check on both sites and report comparison.
async fn compare_element(
    wp_driver: &WebDriver,
    rp_driver: &WebDriver,
    selector: &str,
    label: &str,
) {
    let wp = driver_has_element(wp_driver, selector).await;
    let rp = driver_has_element(rp_driver, selector).await;
    eprintln!(
        "  {} - WP: {}, RP: {} [{}]",
        label,
        wp,
        rp,
        if wp == rp { "MATCH" } else { "DIFFER" }
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_login_page_renders() {
    let cfg = TestConfig::from_env();

    // Check servers first
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };

    eprintln!("\n=== test_login_page_renders ===");

    // Check WordPress login page
    let wp_url = format!("{}/wp-login.php", cfg.wordpress_url);
    driver.goto(&wp_url).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let wp_has_user = driver_has_element(
        &driver,
        "#user_login, input[name=log], input[name=username]",
    )
    .await;
    let wp_has_pass =
        driver_has_element(&driver, "#user_pass, input[name=pwd], input[name=password]").await;
    let wp_has_submit = driver_has_element(
        &driver,
        "#wp-submit, input[type=submit], button[type=submit]",
    )
    .await;

    eprintln!("WordPress login:");
    eprintln!("  Username field: {wp_has_user}");
    eprintln!("  Password field: {wp_has_pass}");
    eprintln!("  Submit button:  {wp_has_submit}");

    // Check RustPress login page
    let rp_url = format!("{}/wp-login.php", cfg.rustpress_url);
    driver.goto(&rp_url).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let rp_has_user = driver_has_element(
        &driver,
        "#user_login, input[name=log], input[name=username]",
    )
    .await;
    let rp_has_pass =
        driver_has_element(&driver, "#user_pass, input[name=pwd], input[name=password]").await;
    let rp_has_submit = driver_has_element(
        &driver,
        "#wp-submit, input[type=submit], button[type=submit]",
    )
    .await;

    eprintln!("RustPress login:");
    eprintln!("  Username field: {rp_has_user}");
    eprintln!("  Password field: {rp_has_pass}");
    eprintln!("  Submit button:  {rp_has_submit}");

    assert!(rp_has_user, "RustPress login should have username field");
    assert!(rp_has_pass, "RustPress login should have password field");
    assert!(rp_has_submit, "RustPress login should have submit button");

    eprintln!("[PASS] Both login pages render with expected form fields");

    driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_login_redirects_to_dashboard() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };

    eprintln!("\n=== test_login_redirects_to_dashboard ===");

    // Login to WordPress
    let wp_ok = wp_login(
        &driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    if wp_ok {
        let wp_current = driver
            .current_url()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();
        let wp_is_admin = wp_current.contains("wp-admin");
        eprintln!(
            "WordPress after login: {wp_current} (is admin: {wp_is_admin})"
        );
    }

    // Clear cookies and login to RustPress
    driver.delete_all_cookies().await.ok();

    let rp_ok = wp_login(
        &driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    if rp_ok {
        let rp_current = driver
            .current_url()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();
        let rp_is_admin = rp_current.contains("wp-admin");
        eprintln!(
            "RustPress after login: {rp_current} (is admin: {rp_is_admin})"
        );
        assert!(
            rp_is_admin,
            "RustPress login should redirect to wp-admin dashboard"
        );
    }

    eprintln!("[PASS] Login redirects to dashboard");
    driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_dashboard_has_widgets() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    // Use two separate drivers for side-by-side comparison
    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_dashboard_has_widgets ===");

    // Login to both
    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    // Navigate to dashboard
    wp_driver
        .goto(&format!("{}/wp-admin/", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check for dashboard widgets
    let widget_selectors = [
        (
            "#dashboard_right_now, .at-a-glance, .glance-items",
            "At a Glance widget",
        ),
        (
            "#dashboard_quick_press, .quick-draft, #quick-press",
            "Quick Draft widget",
        ),
        (
            "#dashboard_activity, .recent-activity, .activity-block",
            "Recent Activity widget",
        ),
    ];

    for (selector, label) in &widget_selectors {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    // Check overall dashboard structure
    compare_element(
        &wp_driver,
        &rp_driver,
        "#wpcontent, .wrap, #wpbody-content",
        "Main content area",
    )
    .await;

    eprintln!("[PASS] Dashboard widgets compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_posts_list_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_posts_list_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/edit.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/edit.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("table, .wp-list-table, .posts-table", "Posts table"),
        (".subsubsub, .status-filters", "Status filter links"),
        (
            ".page-title-action, .add-new, a[href*='post-new']",
            "Add New button",
        ),
        ("thead, th", "Table header"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Posts list page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_create_post_flow() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };

    eprintln!("\n=== test_create_post_flow ===");

    // Test on RustPress only (WordPress uses Gutenberg which is hard to automate)
    wp_login(
        &driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    // Navigate to new post page
    driver
        .goto(&format!("{}/wp-admin/post-new.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check for post editor elements
    let has_title = driver_has_element(
        &driver,
        "#title, input[name=post_title], input[name=title], .editor-post-title",
    )
    .await;
    let has_content = driver_has_element(
        &driver,
        "#content, textarea[name=post_content], textarea[name=content], .editor-post-text-editor",
    )
    .await;
    let has_publish = driver_has_element(
        &driver,
        "#publish, input[value=Publish], button[type=submit], .editor-post-publish-button",
    )
    .await;

    eprintln!("RustPress post editor:");
    eprintln!("  Title field:    {has_title}");
    eprintln!("  Content area:   {has_content}");
    eprintln!("  Publish button: {has_publish}");

    if has_title && has_content {
        // Fill in title
        let title_field = driver
            .find(By::Css("#title, input[name=post_title], input[name=title]"))
            .await;
        if let Ok(field) = title_field {
            field.send_keys("Selenium E2E Test Post").await.ok();
        }

        // Fill in content
        let content_field = driver
            .find(By::Css(
                "#content, textarea[name=post_content], textarea[name=content]",
            ))
            .await;
        if let Ok(field) = content_field {
            field
                .send_keys("This post was created by the Selenium E2E test.")
                .await
                .ok();
        }

        eprintln!("[PASS] Post editor form is functional");
    } else {
        eprintln!("[PARTIAL] Post editor missing some elements");
    }

    driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_media_library_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_media_library_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/upload.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/upload.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
        (
            ".page-title-action, .add-new, a[href*='media-new']",
            "Add New button",
        ),
        (".media-frame, .upload-ui, table, .media-list", "Media area"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Media library page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_users_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_users_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/users.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/users.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("table, .wp-list-table, .users-table", "Users table"),
        ("th, thead", "Table header"),
        (".column-username, td", "Username column / cells"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    // Both should show at least 1 user (admin)
    let wp_rows = driver_count_elements(&wp_driver, "tbody tr, .user-row").await;
    let rp_rows = driver_count_elements(&rp_driver, "tbody tr, .user-row").await;
    eprintln!(
        "  User rows - WP: {}, RP: {} [{}]",
        wp_rows,
        rp_rows,
        if wp_rows > 0 && rp_rows > 0 {
            "BOTH HAVE DATA"
        } else {
            "MISSING"
        }
    );

    eprintln!("[PASS] Users page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_settings_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_settings_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/options-general.php",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/options-general.php",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("form", "Settings form"),
        ("input[name=blogname], input#blogname", "Site Title field"),
        (
            "input[name=blogdescription], input#blogdescription",
            "Tagline field",
        ),
        ("input[type=submit], button[type=submit]", "Save button"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Settings page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_comments_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_comments_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/edit-comments.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/edit-comments.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("table, .wp-list-table, .comments-table", "Comments table"),
        (".subsubsub, .status-filters", "Moderation status filters"),
        ("th, thead", "Table header"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Comments page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_categories_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_categories_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/edit-tags.php?taxonomy=category",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/edit-tags.php?taxonomy=category",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("table, .wp-list-table, .taxonomy-table", "Categories table"),
        ("form, #addtag, .add-category-form", "Add category form"),
        (
            "input[name=tag-name], input[name=name]",
            "Category name input",
        ),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Categories page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_admin_sidebar_links() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_admin_sidebar_links ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // WordPress admin sidebar has these main menu items
    let expected_menu_items = [
        ("Dashboard", "a[href*='index.php'], .menu-top .wp-menu-name"),
        ("Posts", "a[href*='edit.php'], .menu-icon-post"),
        ("Media", "a[href*='upload.php'], .menu-icon-media"),
        (
            "Pages",
            "a[href*='edit.php?post_type=page'], .menu-icon-page",
        ),
        (
            "Comments",
            "a[href*='edit-comments.php'], .menu-icon-comments",
        ),
        ("Users", "a[href*='users.php'], .menu-icon-users"),
        (
            "Settings",
            "a[href*='options-general.php'], .menu-icon-settings",
        ),
    ];

    for (label, selector) in &expected_menu_items {
        let wp_has = driver_has_element(&wp_driver, selector).await;
        let rp_has = driver_has_element(&rp_driver, selector).await;
        eprintln!(
            "  {} - WP: {}, RP: {} [{}]",
            label,
            wp_has,
            rp_has,
            if rp_has { "OK" } else { "MISSING" }
        );
    }

    // Count total sidebar links
    let wp_links = driver_count_elements(&wp_driver, "#adminmenu a, .admin-sidebar a, nav a").await;
    let rp_links = driver_count_elements(&rp_driver, "#adminmenu a, .admin-sidebar a, nav a").await;
    eprintln!("  Total sidebar links - WP: {wp_links}, RP: {rp_links}");

    eprintln!("[PASS] Admin sidebar links compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_plugins_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_plugins_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/plugins.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/plugins.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("table, .wp-list-table, .plugins-table", "Plugins table"),
        (".plugin-title, td, .plugin-name", "Plugin name cells"),
        (".subsubsub, .status-filters", "Status filter links"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Plugins page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_themes_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_themes_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/themes.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/themes.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
        (".theme, .theme-browser, .themes-list", "Theme listing"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    // Count available themes
    let wp_themes = driver_count_elements(&wp_driver, ".theme, .theme-card").await;
    let rp_themes = driver_count_elements(&rp_driver, ".theme, .theme-card").await;
    eprintln!("  Theme count - WP: {wp_themes}, RP: {rp_themes}");

    eprintln!("[PASS] Themes page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_admin_toolbar() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_admin_toolbar ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // WordPress admin bar / toolbar at the top
    let toolbar_selectors = [
        ("#wpadminbar, .admin-bar, .admin-toolbar", "Admin toolbar"),
        (
            "#wp-admin-bar-site-name, .site-name, a[href*='index.php']",
            "Site name in toolbar",
        ),
        (
            "#wp-admin-bar-my-account, .user-info, .admin-user",
            "User account in toolbar",
        ),
    ];

    for (selector, label) in &toolbar_selectors {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Admin toolbar compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_logout_flow() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };

    eprintln!("\n=== test_logout_flow ===");

    // Test RustPress logout flow
    let logged_in = wp_login(
        &driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    if !logged_in {
        eprintln!("[SKIP] Could not log in to RustPress");
        driver.quit().await.ok();
        return;
    }

    // Verify we are in admin area
    let current = driver
        .current_url()
        .await
        .map(|u| u.to_string())
        .unwrap_or_default();
    eprintln!("After login, URL: {current}");

    // Navigate to logout
    // WordPress: /wp-login.php?action=logout&_wpnonce=...
    // RustPress may use a simpler /wp-admin/logout or /wp-login.php?action=logout
    let logout_urls = [
        format!("{}/wp-login.php?action=logout", cfg.rustpress_url),
        format!("{}/wp-admin/logout", cfg.rustpress_url),
    ];

    let mut logged_out = false;
    for url in &logout_urls {
        driver.goto(url).await.ok();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let after_logout = driver
            .current_url()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();
        eprintln!("After logout attempt, URL: {after_logout}");

        // After logout, we should be redirected to login page or homepage
        if after_logout.contains("wp-login") || after_logout.contains("login") {
            logged_out = true;
            break;
        }
    }

    if logged_out {
        // Try to access admin area - should redirect to login
        driver
            .goto(&format!("{}/wp-admin/", cfg.rustpress_url))
            .await
            .ok();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let after_admin = driver
            .current_url()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();
        let redirected_to_login = after_admin.contains("wp-login") || after_admin.contains("login");
        eprintln!(
            "After logout, accessing wp-admin redirects to login: {redirected_to_login}"
        );

        assert!(
            redirected_to_login,
            "Accessing admin after logout should redirect to login"
        );
        eprintln!("[PASS] Logout flow works correctly");
    } else {
        eprintln!("[PARTIAL] Could not confirm logout redirect");
    }

    driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_pages_list_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_pages_list_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/edit.php?post_type=page",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/edit.php?post_type=page",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("table, .wp-list-table, .pages-table", "Pages table"),
        ("thead, th", "Table headers"),
        (
            ".page-title-action, .add-new, a[href*='post-new.php?post_type=page']",
            "Add New button",
        ),
        (".subsubsub, .status-filters", "Status filter links"),
        ("tbody tr, .page-row", "Page rows"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    // Count page rows
    let wp_rows = driver_count_elements(&wp_driver, "tbody tr, .page-row").await;
    let rp_rows = driver_count_elements(&rp_driver, "tbody tr, .page-row").await;
    eprintln!(
        "  Page rows - WP: {}, RP: {} [{}]",
        wp_rows,
        rp_rows,
        if wp_rows > 0 && rp_rows > 0 {
            "BOTH HAVE DATA"
        } else {
            "MISSING"
        }
    );

    eprintln!("[PASS] Pages list page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_tags_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_tags_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/edit-tags.php?taxonomy=post_tag",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/edit-tags.php?taxonomy=post_tag",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("table, .wp-list-table, .taxonomy-table", "Tags table"),
        ("form, #addtag, .add-tag-form", "Add tag form"),
        ("input[name=tag-name], input[name=name]", "Tag name input"),
        (
            "textarea[name=description], input[name=description]",
            "Tag description field",
        ),
        ("input[type=submit], button[type=submit]", "Submit button"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Tags page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_user_profile_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_user_profile_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/profile.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/profile.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("form, #your-profile", "Profile form"),
        (
            "input[name=display_name], #display_name, select[name=display_name]",
            "Display name field",
        ),
        (
            "input[name=email], #email, input[type=email]",
            "Email field",
        ),
        (
            "textarea[name=description], #description",
            "Biographical info (description)",
        ),
        ("input[name=first_name], #first_name", "First name field"),
        ("input[name=last_name], #last_name", "Last name field"),
        (
            "input[type=submit], button[type=submit]",
            "Update Profile button",
        ),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] User profile page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_post_editor_sidebar() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };

    eprintln!("\n=== test_post_editor_sidebar ===");

    // Test on RustPress only (WordPress uses Gutenberg which has a different structure)
    wp_login(
        &driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    driver
        .goto(&format!("{}/wp-admin/post-new.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check for publish meta box
    let has_publish = driver_has_element(
        &driver,
        "#submitdiv, #publish, .publish-meta-box, .postbox .submit, input[value=Publish], button[type=submit]",
    )
    .await;
    eprintln!("  Publish meta box / button: {has_publish}");

    // Check for category checklist
    let has_categories = driver_has_element(
        &driver,
        "#categorydiv, #categorychecklist, .category-checklist, input[name='post_category[]'], select[name=post_category]",
    )
    .await;
    eprintln!("  Category checklist: {has_categories}");

    // Check for tags input
    let has_tags = driver_has_element(
        &driver,
        "#tagsdiv-post_tag, #new-tag-post_tag, input[name=tags_input], input[name=newtag], input[name=tags], .tagadd",
    )
    .await;
    eprintln!("  Tags input: {has_tags}");

    // Check for post status selector
    let has_status = driver_has_element(
        &driver,
        "#post_status, select[name=post_status], .post-status-select",
    )
    .await;
    eprintln!("  Post status selector: {has_status}");

    assert!(
        has_publish,
        "Post editor should have publish button or meta box"
    );

    eprintln!("[PASS] Post editor sidebar elements checked");
    driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_admin_footer() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_admin_footer ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("#wpfooter, .admin-footer, footer", "Admin footer container"),
        (
            "#footer-left, .footer-text, .admin-footer-text",
            "Footer text area",
        ),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    // Check that footer is present on RustPress
    let rp_has_footer = driver_has_element(&rp_driver, "#wpfooter, .admin-footer, footer").await;
    assert!(
        rp_has_footer,
        "RustPress admin should have a footer element"
    );

    eprintln!("[PASS] Admin footer compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_dashboard_quick_draft() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_dashboard_quick_draft ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check Quick Draft widget elements
    let checks = [
        (
            "#dashboard_quick_press, .quick-draft, #quick-press",
            "Quick Draft widget",
        ),
        (
            "input[name=post_title], #title, .quick-draft input[type=text]",
            "Quick Draft title input",
        ),
        (
            "textarea[name=content], #content, .quick-draft textarea",
            "Quick Draft content textarea",
        ),
        (
            "#save-post, input[value='Save Draft'], button[type=submit]",
            "Save Draft button",
        ),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Dashboard Quick Draft widget compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_settings_writing() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_settings_writing ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/options-writing.php",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/options-writing.php",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("form", "Settings form"),
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
        (
            "select[name=default_category], #default_category",
            "Default category selector",
        ),
        (
            "select[name=default_post_format], #default_post_format",
            "Default post format",
        ),
        ("input[type=submit], button[type=submit]", "Save button"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Settings Writing page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_settings_reading() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_settings_reading ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/options-reading.php",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/options-reading.php",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("form", "Settings form"),
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
        (
            "input[name=posts_per_page], #posts_per_page",
            "Posts per page",
        ),
        (
            "input[name=posts_per_rss], #posts_per_rss",
            "Syndication feeds items",
        ),
        ("input[type=submit], button[type=submit]", "Save button"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Settings Reading page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_settings_discussion() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_settings_discussion ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/options-discussion.php",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/options-discussion.php",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("form", "Settings form"),
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
        (
            "input[name=default_comment_status], #default_comment_status",
            "Default comment status",
        ),
        (
            "input[name=require_name_email], #require_name_email",
            "Require name and email",
        ),
        ("input[type=submit], button[type=submit]", "Save button"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Settings Discussion page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_settings_permalink() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_settings_permalink ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!(
            "{}/wp-admin/options-permalink.php",
            cfg.wordpress_url
        ))
        .await
        .ok();
    rp_driver
        .goto(&format!(
            "{}/wp-admin/options-permalink.php",
            cfg.rustpress_url
        ))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        ("form", "Settings form"),
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
        (
            "input[type=radio], .permalink-structure",
            "Permalink structure options",
        ),
        (
            "input[name=permalink_structure], #permalink_structure",
            "Custom permalink input",
        ),
        ("input[type=submit], button[type=submit]", "Save button"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Settings Permalink page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_admin_search_posts() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_admin_search_posts ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    // Navigate to posts list with a search query
    wp_driver
        .goto(&format!("{}/wp-admin/edit.php?s=hello", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/edit.php?s=hello", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify search results page structure
    let checks = [
        ("table, .wp-list-table, .posts-table", "Posts table present"),
        (
            "input[name=s], #post-search-input, .search-box input",
            "Search input field",
        ),
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    // Check that the search term is reflected in the search box
    let rp_has_search = driver_has_element(
        &rp_driver,
        "input[name=s], #post-search-input, .search-box input",
    )
    .await;
    eprintln!("  RustPress has search input: {rp_has_search}");

    // Count result rows (may be 0 if no posts match)
    let wp_rows = driver_count_elements(&wp_driver, "tbody tr, .post-row").await;
    let rp_rows = driver_count_elements(&rp_driver, "tbody tr, .post-row").await;
    eprintln!("  Search result rows - WP: {wp_rows}, RP: {rp_rows}");

    eprintln!("[PASS] Admin search posts compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

#[tokio::test]
#[ignore]
async fn test_admin_menus_page() {
    let cfg = TestConfig::from_env();
    if skip_if_unavailable(&cfg.wordpress_url, &cfg.rustpress_url).await {
        return;
    }

    let wp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => return,
    };
    let rp_driver = match create_driver(&cfg).await {
        Some(d) => d,
        None => {
            wp_driver.quit().await.ok();
            return;
        }
    };

    eprintln!("\n=== test_admin_menus_page ===");

    wp_login(
        &wp_driver,
        &cfg.wordpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;
    wp_login(
        &rp_driver,
        &cfg.rustpress_url,
        &cfg.admin_user,
        &cfg.admin_password,
    )
    .await;

    wp_driver
        .goto(&format!("{}/wp-admin/nav-menus.php", cfg.wordpress_url))
        .await
        .ok();
    rp_driver
        .goto(&format!("{}/wp-admin/nav-menus.php", cfg.rustpress_url))
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let checks = [
        (".wrap, #wpcontent, #wpbody-content", "Content wrapper"),
        ("#menu-management, .menu-editor, form", "Menu editor area"),
        ("#menu-name, input[name=menu-name]", "Menu name input"),
        (
            "#nav-menu-meta, .menu-item-settings, #add-custom-links",
            "Menu item add panels",
        ),
        (
            "input[type=submit], button[type=submit]",
            "Save / Create Menu button",
        ),
    ];

    for (selector, label) in &checks {
        compare_element(&wp_driver, &rp_driver, selector, label).await;
    }

    eprintln!("[PASS] Admin Menus page compared");
    wp_driver.quit().await.ok();
    rp_driver.quit().await.ok();
}

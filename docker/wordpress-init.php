<?php
/**
 * RustPress E2E Init — must-use plugin
 *
 * Automatically installs WordPress and creates test data when the site
 * is first accessed. Runs only once (checks for existing posts).
 *
 * Admin credentials:  admin / password
 */

if ( ! defined( 'ABSPATH' ) ) {
    exit;
}

add_action( 'init', function () {
    // Only run setup once (check if admin user already exists)
    if ( username_exists( 'admin' ) ) {
        return;
    }

    // ── Install WordPress ──────────────────────────────────────────────────
    require_once ABSPATH . 'wp-admin/includes/upgrade.php';

    $site_url = 'http://localhost:8081';

    wp_install(
        'RustPress E2E',     // blog title
        'admin',             // user login
        'admin@localhost',   // user email
        true,                // is_public
        '',                  // deprecated
        'password',          // user password
        get_locale()
    );

    update_option( 'siteurl', $site_url );
    update_option( 'home',    $site_url );
    update_option( 'blogname', 'RustPress E2E Test Site' );
    update_option( 'blogdescription', 'Comparison test site for RustPress' );

    // ── Sample posts ──────────────────────────────────────────────────────
    wp_insert_post( [
        'post_title'   => 'Hello World',
        'post_content' => '<p>Welcome to WordPress. This is your first post. Edit or delete it, then start writing!</p>',
        'post_status'  => 'publish',
        'post_type'    => 'post',
        'post_name'    => 'hello-world',
    ] );

    wp_insert_post( [
        'post_title'   => 'Sample Page',
        'post_content' => '<p>This is an example page.</p>',
        'post_status'  => 'publish',
        'post_type'    => 'page',
        'post_name'    => 'sample-page',
    ] );

    // Sticky post
    $sticky_id = wp_insert_post( [
        'post_title'   => 'Sticky Post',
        'post_content' => '<p>This post is sticky.</p>',
        'post_status'  => 'publish',
        'post_type'    => 'post',
        'post_name'    => 'sticky-post',
    ] );
    stick_post( $sticky_id );

    // Password-protected post
    wp_insert_post( [
        'post_title'        => 'Protected Post',
        'post_content'      => '<p>Secret content.</p>',
        'post_status'       => 'publish',
        'post_type'         => 'post',
        'post_name'         => 'protected-post',
        'post_password'     => 'secret',
    ] );

    // ── Categories & Tags ─────────────────────────────────────────────────
    wp_create_term( 'Uncategorized', 'category' );
    wp_create_term( 'Technology',    'category' );
    wp_create_term( 'rust',          'post_tag' );
    wp_create_term( 'wordpress',     'post_tag' );

    // ── Application Passwords (requires WP 5.6+) ──────────────────────────
    // Enable Application Passwords for non-HTTPS
    add_filter( 'wp_is_application_passwords_available', '__return_true' );

    // ── Enable REST API for all ───────────────────────────────────────────
    update_option( 'default_comment_status', 'open' );
    update_option( 'permalink_structure',    '/%postname%/' );

    flush_rewrite_rules();
} );

// Always allow Application Passwords over HTTP (needed for E2E on localhost)
add_filter( 'wp_is_application_passwords_available', '__return_true' );
add_filter( 'application_password_is_api_request', '__return_true' );

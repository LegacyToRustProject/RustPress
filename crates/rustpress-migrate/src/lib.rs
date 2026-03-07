pub mod analyze;
pub mod rewrites;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement};
use tracing::info;

/// WordPress database migration and schema management.
///
/// Handles creating the WordPress-compatible database schema
/// and running migrations for RustPress-specific extensions.
/// Create all WordPress core tables if they don't exist.
/// Compatible with WordPress 6.9 database schema.
pub async fn create_wp_tables(db: &DatabaseConnection) -> Result<(), DbErr> {
    let tables = vec![
        create_posts_table(),
        create_postmeta_table(),
        create_users_table(),
        create_usermeta_table(),
        create_options_table(),
        create_comments_table(),
        create_commentmeta_table(),
        create_terms_table(),
        create_term_taxonomy_table(),
        create_term_relationships_table(),
        create_links_table(),
        create_termmeta_table(),
    ];

    for (name, sql) in tables {
        info!(table = name, "creating table if not exists");
        db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
            .await?;
    }

    info!("all WordPress tables created");
    Ok(())
}

/// Insert default WordPress options.
pub async fn insert_default_options(
    db: &DatabaseConnection,
    site_url: &str,
    site_name: &str,
) -> Result<(), DbErr> {
    let defaults = vec![
        ("siteurl", site_url),
        ("home", site_url),
        ("blogname", site_name),
        ("blogdescription", "Just another RustPress site"),
        ("users_can_register", "0"),
        ("admin_email", "admin@example.com"),
        ("posts_per_page", "10"),
        ("date_format", "F j, Y"),
        ("time_format", "g:i a"),
        ("permalink_structure", "/%postname%/"),
        ("default_comment_status", "open"),
        ("default_ping_status", "open"),
        ("timezone_string", "UTC"),
        ("template", "twentyrust"),
        ("stylesheet", "twentyrust"),
        ("db_version", "58975"),
    ];

    for (name, value) in defaults {
        let sql = format!(
            "INSERT IGNORE INTO wp_options (option_name, option_value, autoload) VALUES ('{}', '{}', 'yes')",
            name, value
        );
        db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
            .await?;
    }

    info!("default options inserted");
    Ok(())
}

/// Create the default admin user.
pub async fn create_default_admin(
    db: &DatabaseConnection,
    password_hash: &str,
) -> Result<(), DbErr> {
    let sql = format!(
        "INSERT IGNORE INTO wp_users (user_login, user_pass, user_nicename, user_email, user_url, user_registered, user_activation_key, user_status, display_name) VALUES ('admin', '{}', 'admin', 'admin@example.com', '', NOW(), '', 0, 'Admin')",
        password_hash
    );

    db.execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
        .await?;

    info!("default admin user created");
    Ok(())
}

fn create_posts_table() -> (&'static str, String) {
    (
        "wp_posts",
        "CREATE TABLE IF NOT EXISTS wp_posts (
        ID bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        post_author bigint(20) unsigned NOT NULL DEFAULT 0,
        post_date datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        post_date_gmt datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        post_content longtext NOT NULL,
        post_title text NOT NULL,
        post_excerpt text NOT NULL,
        post_status varchar(20) NOT NULL DEFAULT 'publish',
        comment_status varchar(20) NOT NULL DEFAULT 'open',
        ping_status varchar(20) NOT NULL DEFAULT 'open',
        post_password varchar(255) NOT NULL DEFAULT '',
        post_name varchar(200) NOT NULL DEFAULT '',
        to_ping text NOT NULL,
        pinged text NOT NULL,
        post_modified datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        post_modified_gmt datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        post_content_filtered longtext NOT NULL,
        post_parent bigint(20) unsigned NOT NULL DEFAULT 0,
        guid varchar(255) NOT NULL DEFAULT '',
        menu_order int(11) NOT NULL DEFAULT 0,
        post_type varchar(20) NOT NULL DEFAULT 'post',
        post_mime_type varchar(100) NOT NULL DEFAULT '',
        comment_count bigint(20) NOT NULL DEFAULT 0,
        PRIMARY KEY (ID),
        KEY post_name (post_name(191)),
        KEY type_status_date (post_type, post_status, post_date, ID),
        KEY post_parent (post_parent),
        KEY post_author (post_author)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_postmeta_table() -> (&'static str, String) {
    (
        "wp_postmeta",
        "CREATE TABLE IF NOT EXISTS wp_postmeta (
        meta_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        post_id bigint(20) unsigned NOT NULL DEFAULT 0,
        meta_key varchar(255) DEFAULT NULL,
        meta_value longtext,
        PRIMARY KEY (meta_id),
        KEY post_id (post_id),
        KEY meta_key (meta_key(191))
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_users_table() -> (&'static str, String) {
    (
        "wp_users",
        "CREATE TABLE IF NOT EXISTS wp_users (
        ID bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        user_login varchar(60) NOT NULL DEFAULT '',
        user_pass varchar(255) NOT NULL DEFAULT '',
        user_nicename varchar(50) NOT NULL DEFAULT '',
        user_email varchar(100) NOT NULL DEFAULT '',
        user_url varchar(100) NOT NULL DEFAULT '',
        user_registered datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        user_activation_key varchar(255) NOT NULL DEFAULT '',
        user_status int(11) NOT NULL DEFAULT 0,
        display_name varchar(250) NOT NULL DEFAULT '',
        PRIMARY KEY (ID),
        KEY user_login_key (user_login),
        KEY user_nicename (user_nicename),
        KEY user_email (user_email)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_usermeta_table() -> (&'static str, String) {
    (
        "wp_usermeta",
        "CREATE TABLE IF NOT EXISTS wp_usermeta (
        umeta_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        user_id bigint(20) unsigned NOT NULL DEFAULT 0,
        meta_key varchar(255) DEFAULT NULL,
        meta_value longtext,
        PRIMARY KEY (umeta_id),
        KEY user_id (user_id),
        KEY meta_key (meta_key(191))
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_options_table() -> (&'static str, String) {
    (
        "wp_options",
        "CREATE TABLE IF NOT EXISTS wp_options (
        option_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        option_name varchar(191) NOT NULL DEFAULT '',
        option_value longtext NOT NULL,
        autoload varchar(20) NOT NULL DEFAULT 'yes',
        PRIMARY KEY (option_id),
        UNIQUE KEY option_name (option_name),
        KEY autoload (autoload)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_comments_table() -> (&'static str, String) {
    (
        "wp_comments",
        "CREATE TABLE IF NOT EXISTS wp_comments (
        comment_ID bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        comment_post_ID bigint(20) unsigned NOT NULL DEFAULT 0,
        comment_author text NOT NULL,
        comment_author_email varchar(100) NOT NULL DEFAULT '',
        comment_author_url varchar(200) NOT NULL DEFAULT '',
        comment_author_IP varchar(100) NOT NULL DEFAULT '',
        comment_date datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        comment_date_gmt datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        comment_content text NOT NULL,
        comment_karma int(11) NOT NULL DEFAULT 0,
        comment_approved varchar(20) NOT NULL DEFAULT '1',
        comment_agent varchar(255) NOT NULL DEFAULT '',
        comment_type varchar(20) NOT NULL DEFAULT 'comment',
        comment_parent bigint(20) unsigned NOT NULL DEFAULT 0,
        user_id bigint(20) unsigned NOT NULL DEFAULT 0,
        PRIMARY KEY (comment_ID),
        KEY comment_post_ID (comment_post_ID),
        KEY comment_approved_date_gmt (comment_approved, comment_date_gmt),
        KEY comment_date_gmt (comment_date_gmt),
        KEY comment_parent (comment_parent),
        KEY comment_author_email (comment_author_email(10))
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_commentmeta_table() -> (&'static str, String) {
    (
        "wp_commentmeta",
        "CREATE TABLE IF NOT EXISTS wp_commentmeta (
        meta_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        comment_id bigint(20) unsigned NOT NULL DEFAULT 0,
        meta_key varchar(255) DEFAULT NULL,
        meta_value longtext,
        PRIMARY KEY (meta_id),
        KEY comment_id (comment_id),
        KEY meta_key (meta_key(191))
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_terms_table() -> (&'static str, String) {
    (
        "wp_terms",
        "CREATE TABLE IF NOT EXISTS wp_terms (
        term_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        name varchar(200) NOT NULL DEFAULT '',
        slug varchar(200) NOT NULL DEFAULT '',
        term_group bigint(10) NOT NULL DEFAULT 0,
        PRIMARY KEY (term_id),
        KEY slug (slug(191)),
        KEY name (name(191))
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_term_taxonomy_table() -> (&'static str, String) {
    (
        "wp_term_taxonomy",
        "CREATE TABLE IF NOT EXISTS wp_term_taxonomy (
        term_taxonomy_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        term_id bigint(20) unsigned NOT NULL DEFAULT 0,
        taxonomy varchar(32) NOT NULL DEFAULT '',
        description longtext NOT NULL,
        parent bigint(20) unsigned NOT NULL DEFAULT 0,
        count bigint(20) NOT NULL DEFAULT 0,
        PRIMARY KEY (term_taxonomy_id),
        UNIQUE KEY term_id_taxonomy (term_id, taxonomy),
        KEY taxonomy (taxonomy)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_term_relationships_table() -> (&'static str, String) {
    (
        "wp_term_relationships",
        "CREATE TABLE IF NOT EXISTS wp_term_relationships (
        object_id bigint(20) unsigned NOT NULL DEFAULT 0,
        term_taxonomy_id bigint(20) unsigned NOT NULL DEFAULT 0,
        term_order int(11) NOT NULL DEFAULT 0,
        PRIMARY KEY (object_id, term_taxonomy_id),
        KEY term_taxonomy_id (term_taxonomy_id)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_links_table() -> (&'static str, String) {
    (
        "wp_links",
        "CREATE TABLE IF NOT EXISTS wp_links (
        link_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        link_url varchar(255) NOT NULL DEFAULT '',
        link_name varchar(255) NOT NULL DEFAULT '',
        link_image varchar(255) NOT NULL DEFAULT '',
        link_target varchar(25) NOT NULL DEFAULT '',
        link_description varchar(255) NOT NULL DEFAULT '',
        link_visible varchar(20) NOT NULL DEFAULT 'Y',
        link_owner bigint(20) unsigned NOT NULL DEFAULT 1,
        link_rating int(11) NOT NULL DEFAULT 0,
        link_updated datetime NOT NULL DEFAULT '0000-00-00 00:00:00',
        link_rel varchar(255) NOT NULL DEFAULT '',
        link_notes mediumtext NOT NULL,
        link_rss varchar(255) NOT NULL DEFAULT '',
        PRIMARY KEY (link_id),
        KEY link_visible (link_visible)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

fn create_termmeta_table() -> (&'static str, String) {
    (
        "wp_termmeta",
        "CREATE TABLE IF NOT EXISTS wp_termmeta (
        meta_id bigint(20) unsigned NOT NULL AUTO_INCREMENT,
        term_id bigint(20) unsigned NOT NULL DEFAULT 0,
        meta_key varchar(255) DEFAULT NULL,
        meta_value longtext,
        PRIMARY KEY (meta_id),
        KEY term_id (term_id),
        KEY meta_key (meta_key(191))
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_520_ci"
            .to_string(),
    )
}

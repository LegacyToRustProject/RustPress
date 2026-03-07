//! Table prefix management for WordPress multisite.
//!
//! In a multisite installation, each site has its own set of tables with a
//! numeric prefix (e.g., `wp_2_posts` for blog_id 2). The main site (blog_id 1)
//! uses the base prefix without a number (e.g., `wp_posts`).
//!
//! Some tables are shared globally across all sites in the network.

/// The default WordPress table prefix.
const DEFAULT_PREFIX: &str = "wp";

/// Returns the full table name for a given blog and base table.
///
/// For the main site (blog_id 1), returns "wp_posts".
/// For other sites, returns "wp_2_posts", "wp_3_posts", etc.
///
/// # Examples
///
/// ```
/// use rustpress_multisite::tables::table_name;
///
/// assert_eq!(table_name(1, "posts"), "wp_posts");
/// assert_eq!(table_name(2, "posts"), "wp_2_posts");
/// assert_eq!(table_name(10, "options"), "wp_10_options");
/// ```
pub fn table_name(blog_id: u64, base_table: &str) -> String {
    if is_main_site(blog_id) {
        format!("{}_{}", DEFAULT_PREFIX, base_table)
    } else {
        format!("{}_{}_{}", DEFAULT_PREFIX, blog_id, base_table)
    }
}

/// Returns the full table name with a custom prefix.
///
/// # Examples
///
/// ```
/// use rustpress_multisite::tables::table_name_with_prefix;
///
/// assert_eq!(table_name_with_prefix("myprefix", 1, "posts"), "myprefix_posts");
/// assert_eq!(table_name_with_prefix("myprefix", 3, "posts"), "myprefix_3_posts");
/// ```
pub fn table_name_with_prefix(prefix: &str, blog_id: u64, base_table: &str) -> String {
    if is_main_site(blog_id) {
        format!("{}_{}", prefix, base_table)
    } else {
        format!("{}_{}_{}", prefix, blog_id, base_table)
    }
}

/// Returns a list of global tables shared across all sites in the network.
///
/// These tables are not prefixed with a blog ID and exist once per network.
/// They correspond to the tables listed in WordPress's `$wpdb->tables('global')`.
pub fn global_tables() -> Vec<&'static str> {
    vec![
        "wp_users",
        "wp_usermeta",
        "wp_blogs",
        "wp_blog_versions",
        "wp_site",
        "wp_sitemeta",
        "wp_signups",
        "wp_registration_log",
    ]
}

/// Returns a list of per-site table base names (without prefix).
///
/// In a multisite installation, each site gets its own copy of these tables,
/// prefixed with `wp_N_` where N is the blog ID (except for blog_id 1).
pub fn per_site_tables() -> Vec<&'static str> {
    vec![
        "posts",
        "postmeta",
        "comments",
        "commentmeta",
        "options",
        "terms",
        "term_taxonomy",
        "term_relationships",
        "termmeta",
        "links",
    ]
}

/// Check if the given blog_id is the main site.
///
/// The main site always has blog_id 1 in WordPress multisite.
pub fn is_main_site(blog_id: u64) -> bool {
    blog_id == 1
}

/// Get all table names for a specific blog.
///
/// Returns the fully qualified table names for all per-site tables.
pub fn all_tables_for_blog(blog_id: u64) -> Vec<String> {
    per_site_tables()
        .iter()
        .map(|base| table_name(blog_id, base))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_site_table_names() {
        assert_eq!(table_name(1, "posts"), "wp_posts");
        assert_eq!(table_name(1, "options"), "wp_options");
        assert_eq!(table_name(1, "comments"), "wp_comments");
    }

    #[test]
    fn test_sub_site_table_names() {
        assert_eq!(table_name(2, "posts"), "wp_2_posts");
        assert_eq!(table_name(3, "options"), "wp_3_options");
        assert_eq!(table_name(100, "comments"), "wp_100_comments");
    }

    #[test]
    fn test_is_main_site() {
        assert!(is_main_site(1));
        assert!(!is_main_site(2));
        assert!(!is_main_site(0));
        assert!(!is_main_site(999));
    }

    #[test]
    fn test_global_tables_contains_expected() {
        let globals = global_tables();
        assert!(globals.contains(&"wp_users"));
        assert!(globals.contains(&"wp_usermeta"));
        assert!(globals.contains(&"wp_blogs"));
        assert!(globals.contains(&"wp_site"));
        assert!(globals.contains(&"wp_sitemeta"));
    }

    #[test]
    fn test_per_site_tables_contains_expected() {
        let per_site = per_site_tables();
        assert!(per_site.contains(&"posts"));
        assert!(per_site.contains(&"postmeta"));
        assert!(per_site.contains(&"comments"));
        assert!(per_site.contains(&"options"));
        assert!(per_site.contains(&"terms"));
    }

    #[test]
    fn test_all_tables_for_blog() {
        let tables = all_tables_for_blog(2);
        assert!(tables.contains(&"wp_2_posts".to_string()));
        assert!(tables.contains(&"wp_2_options".to_string()));
        assert_eq!(tables.len(), per_site_tables().len());
    }

    #[test]
    fn test_custom_prefix() {
        assert_eq!(table_name_with_prefix("myapp", 1, "posts"), "myapp_posts");
        assert_eq!(table_name_with_prefix("myapp", 5, "posts"), "myapp_5_posts");
    }
}

//! WordPress-style widget system for RustPress.
//!
//! Defines widget types (RecentPosts, Categories, Archives, Search, Text,
//! CustomHTML, Meta, RecentComments), widget areas (sidebars), and
//! persistence via `wp_options` as JSON under the key `widget_config`.

use rustpress_db::options::OptionsManager;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

use rustpress_db::entities::{wp_comments, wp_posts, wp_term_taxonomy, wp_terms};

/// All supported widget types, mirroring WordPress core widgets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum WidgetType {
    RecentPosts {
        title: String,
        /// How many posts to show (default 5).
        count: u32,
    },
    Categories {
        title: String,
        /// "list" or "dropdown"
        display: String,
    },
    Archives {
        title: String,
    },
    Search {
        title: String,
    },
    Text {
        title: String,
        content: String,
    },
    CustomHTML {
        title: String,
        content: String,
    },
    Meta {
        title: String,
    },
    RecentComments {
        title: String,
        count: u32,
    },
}

impl WidgetType {
    /// Return a human-readable label for the widget type.
    pub fn type_name(&self) -> &str {
        match self {
            WidgetType::RecentPosts { .. } => "RecentPosts",
            WidgetType::Categories { .. } => "Categories",
            WidgetType::Archives { .. } => "Archives",
            WidgetType::Search { .. } => "Search",
            WidgetType::Text { .. } => "Text",
            WidgetType::CustomHTML { .. } => "CustomHTML",
            WidgetType::Meta { .. } => "Meta",
            WidgetType::RecentComments { .. } => "RecentComments",
        }
    }

    /// Return the user-facing title configured for this widget.
    pub fn title(&self) -> &str {
        match self {
            WidgetType::RecentPosts { title, .. }
            | WidgetType::Categories { title, .. }
            | WidgetType::Archives { title, .. }
            | WidgetType::Search { title, .. }
            | WidgetType::Text { title, .. }
            | WidgetType::CustomHTML { title, .. }
            | WidgetType::Meta { title, .. }
            | WidgetType::RecentComments { title, .. } => title,
        }
    }
}

/// A single widget instance placed in a widget area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetInstance {
    /// Unique ID within the configuration (e.g. "recentposts-1").
    pub id: String,
    pub widget: WidgetType,
}

/// The full widget configuration: which widgets live in which areas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    /// Map of area ID -> list of widget instances.
    pub areas: std::collections::HashMap<String, Vec<WidgetInstance>>,
}

/// Metadata about a widget area (sidebar).
pub struct WidgetAreaInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

/// All registered widget areas.
pub const WIDGET_AREAS: &[WidgetAreaInfo] = &[
    WidgetAreaInfo {
        id: "sidebar-1",
        name: "Primary Sidebar",
        description: "Main sidebar that appears on posts and pages.",
    },
    WidgetAreaInfo {
        id: "footer-1",
        name: "Footer Column 1",
        description: "First footer widget area.",
    },
    WidgetAreaInfo {
        id: "footer-2",
        name: "Footer Column 2",
        description: "Second footer widget area.",
    },
];

/// Available widget type descriptors shown in the "add widget" UI.
pub struct AvailableWidget {
    pub type_key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

pub const AVAILABLE_WIDGETS: &[AvailableWidget] = &[
    AvailableWidget {
        type_key: "RecentPosts",
        label: "Recent Posts",
        description: "Your site's most recent posts.",
    },
    AvailableWidget {
        type_key: "Categories",
        label: "Categories",
        description: "A list or dropdown of categories.",
    },
    AvailableWidget {
        type_key: "Archives",
        label: "Archives",
        description: "A monthly archive of your site's posts.",
    },
    AvailableWidget {
        type_key: "Search",
        label: "Search",
        description: "A search form for your site.",
    },
    AvailableWidget {
        type_key: "Text",
        label: "Text",
        description: "Arbitrary text or HTML.",
    },
    AvailableWidget {
        type_key: "CustomHTML",
        label: "Custom HTML",
        description: "Arbitrary HTML code.",
    },
    AvailableWidget {
        type_key: "Meta",
        label: "Meta",
        description: "Login, logout, feed and RustPress links.",
    },
    AvailableWidget {
        type_key: "RecentComments",
        label: "Recent Comments",
        description: "Your site's most recent comments.",
    },
];

/// The wp_options key storing the JSON widget configuration.
pub const WIDGET_CONFIG_KEY: &str = "widget_config";

impl WidgetConfig {
    /// Return the default widget configuration (similar to WordPress default).
    pub fn default_config() -> Self {
        let mut areas = std::collections::HashMap::new();

        areas.insert(
            "sidebar-1".to_string(),
            vec![
                WidgetInstance {
                    id: "search-1".to_string(),
                    widget: WidgetType::Search {
                        title: "Search".to_string(),
                    },
                },
                WidgetInstance {
                    id: "recentposts-1".to_string(),
                    widget: WidgetType::RecentPosts {
                        title: "Recent Posts".to_string(),
                        count: 5,
                    },
                },
                WidgetInstance {
                    id: "categories-1".to_string(),
                    widget: WidgetType::Categories {
                        title: "Categories".to_string(),
                        display: "list".to_string(),
                    },
                },
            ],
        );
        areas.insert("footer-1".to_string(), vec![]);
        areas.insert("footer-2".to_string(), vec![]);

        WidgetConfig { areas }
    }

    /// Ensure all registered areas exist in the config (for forward-compat).
    pub fn ensure_areas(&mut self) {
        for area in WIDGET_AREAS {
            self.areas
                .entry(area.id.to_string())
                .or_insert_with(Vec::new);
        }
    }
}

/// Load widget configuration from `wp_options`. Falls back to defaults if
/// the option does not exist or cannot be parsed.
pub async fn load_widget_config(options: &OptionsManager) -> WidgetConfig {
    match options.get_option(WIDGET_CONFIG_KEY).await {
        Ok(Some(json_str)) => match serde_json::from_str::<WidgetConfig>(&json_str) {
            Ok(mut cfg) => {
                cfg.ensure_areas();
                cfg
            }
            Err(e) => {
                tracing::warn!("Failed to parse widget_config JSON, using defaults: {}", e);
                WidgetConfig::default_config()
            }
        },
        _ => WidgetConfig::default_config(),
    }
}

/// Save widget configuration to `wp_options` as JSON.
pub async fn save_widget_config(
    options: &OptionsManager,
    config: &WidgetConfig,
) -> Result<(), sea_orm::DbErr> {
    let json_str = serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string());
    options.update_option(WIDGET_CONFIG_KEY, &json_str).await
}

// ---------------------------------------------------------------------------
// Frontend HTML rendering
// ---------------------------------------------------------------------------

/// Render all widgets in a given area to HTML.
pub async fn render_widget_area(
    config: &WidgetConfig,
    area_id: &str,
    db: &DatabaseConnection,
    site_url: &str,
) -> String {
    let widgets = match config.areas.get(area_id) {
        Some(w) => w,
        None => return String::new(),
    };

    if widgets.is_empty() {
        return String::new();
    }

    let mut html = String::new();
    for inst in widgets {
        let widget_html = render_single_widget(&inst.widget, db, site_url).await;
        if !widget_html.is_empty() {
            html.push_str(&format!(
                "<div class=\"widget widget-{}\" id=\"{}\">\n{}\n</div>\n",
                inst.widget.type_name().to_lowercase(),
                inst.id,
                widget_html
            ));
        }
    }
    html
}

async fn render_single_widget(
    widget: &WidgetType,
    db: &DatabaseConnection,
    site_url: &str,
) -> String {
    match widget {
        WidgetType::RecentPosts { title, count } => {
            render_recent_posts(title, *count, db, site_url).await
        }
        WidgetType::Categories { title, display } => {
            render_categories(title, display, db).await
        }
        WidgetType::Archives { title } => render_archives(title, db).await,
        WidgetType::Search { title } => render_search(title),
        WidgetType::Text { title, content } => render_text(title, content),
        WidgetType::CustomHTML { title, content } => render_custom_html(title, content),
        WidgetType::Meta { title } => render_meta(title, site_url),
        WidgetType::RecentComments { title, count } => {
            render_recent_comments(title, *count, db).await
        }
    }
}

// -- Individual widget renderers --

async fn render_recent_posts(
    title: &str,
    count: u32,
    db: &DatabaseConnection,
    _site_url: &str,
) -> String {
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_desc(wp_posts::Column::PostDate)
        .limit(count as u64)
        .all(db)
        .await
        .unwrap_or_default();

    let display_title = if title.is_empty() {
        "Recent Posts"
    } else {
        title
    };

    let mut html = format!("<h3 class=\"widget-title\">{}</h3>\n<ul>\n", escape_html(display_title));
    for p in &posts {
        html.push_str(&format!(
            "  <li><a href=\"/{}\">{}</a></li>\n",
            escape_html(&p.post_name),
            escape_html(&p.post_title)
        ));
    }
    html.push_str("</ul>");
    html
}

async fn render_categories(
    title: &str,
    display: &str,
    db: &DatabaseConnection,
) -> String {
    let tt_records = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .all(db)
        .await
        .unwrap_or_default();

    let term_ids: Vec<u64> = tt_records.iter().map(|tt| tt.term_id).collect();
    if term_ids.is_empty() {
        return String::new();
    }

    let terms = wp_terms::Entity::find()
        .filter(wp_terms::Column::TermId.is_in(term_ids))
        .order_by_asc(wp_terms::Column::Name)
        .all(db)
        .await
        .unwrap_or_default();

    // Build a count map from term_taxonomy
    let count_map: std::collections::HashMap<u64, i64> = tt_records
        .iter()
        .map(|tt| (tt.term_id, tt.count))
        .collect();

    let display_title = if title.is_empty() {
        "Categories"
    } else {
        title
    };

    let mut html = format!("<h3 class=\"widget-title\">{}</h3>\n", escape_html(display_title));

    if display == "dropdown" {
        html.push_str("<form>\n<select onchange=\"if(this.value)location.href=this.value\">\n");
        html.push_str("<option value=\"\">Select Category</option>\n");
        for t in &terms {
            let cnt = count_map.get(&t.term_id).copied().unwrap_or(0);
            html.push_str(&format!(
                "<option value=\"/category/{}\">{} ({})</option>\n",
                escape_html(&t.slug),
                escape_html(&t.name),
                cnt
            ));
        }
        html.push_str("</select>\n</form>");
    } else {
        html.push_str("<ul>\n");
        for t in &terms {
            let cnt = count_map.get(&t.term_id).copied().unwrap_or(0);
            html.push_str(&format!(
                "  <li><a href=\"/category/{}\">{}</a> ({})</li>\n",
                escape_html(&t.slug),
                escape_html(&t.name),
                cnt
            ));
        }
        html.push_str("</ul>");
    }

    html
}

async fn render_archives(title: &str, db: &DatabaseConnection) -> String {
    // Get distinct year-month combos from published posts
    use sea_orm::{ConnectionTrait, Statement};
    let sql = "SELECT DATE_FORMAT(post_date, '%Y-%m') AS ym, COUNT(*) AS cnt FROM wp_posts WHERE post_type='post' AND post_status='publish' GROUP BY ym ORDER BY ym DESC LIMIT 12";

    let display_title = if title.is_empty() {
        "Archives"
    } else {
        title
    };

    let mut html = format!("<h3 class=\"widget-title\">{}</h3>\n<ul>\n", escape_html(display_title));

    if let Ok(rows) = db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::MySql,
            sql.to_string(),
        ))
        .await
    {
        for row in &rows {
            let ym: String = row.try_get("", "ym").unwrap_or_default();
            let cnt: i64 = row.try_get("", "cnt").unwrap_or(0);
            if !ym.is_empty() {
                // Parse year-month for display
                let parts: Vec<&str> = ym.split('-').collect();
                let label = if parts.len() == 2 {
                    let month_name = match parts[1] {
                        "01" => "January",
                        "02" => "February",
                        "03" => "March",
                        "04" => "April",
                        "05" => "May",
                        "06" => "June",
                        "07" => "July",
                        "08" => "August",
                        "09" => "September",
                        "10" => "October",
                        "11" => "November",
                        "12" => "December",
                        _ => parts[1],
                    };
                    format!("{} {}", month_name, parts[0])
                } else {
                    ym.clone()
                };
                // Archive URL: /archives/YYYY/MM (simple version)
                html.push_str(&format!(
                    "  <li><a href=\"/archives/{}\">{}</a> ({})</li>\n",
                    ym.replace('-', "/"),
                    escape_html(&label),
                    cnt
                ));
            }
        }
    }

    html.push_str("</ul>");
    html
}

fn render_search(title: &str) -> String {
    let display_title = if title.is_empty() {
        "Search"
    } else {
        title
    };

    format!(
        r#"<h3 class="widget-title">{}</h3>
<form role="search" method="get" class="search-form" action="/search">
  <label>
    <span class="screen-reader-text">Search for:</span>
    <input type="search" class="search-field" placeholder="Search&hellip;" name="s" />
  </label>
  <button type="submit" class="search-submit">Search</button>
</form>"#,
        escape_html(display_title)
    )
}

fn render_text(title: &str, content: &str) -> String {
    let mut html = String::new();
    if !title.is_empty() {
        html.push_str(&format!(
            "<h3 class=\"widget-title\">{}</h3>\n",
            escape_html(title)
        ));
    }
    html.push_str(&format!("<div class=\"textwidget\">{}</div>", escape_html(content)));
    html
}

fn render_custom_html(title: &str, content: &str) -> String {
    let mut html = String::new();
    if !title.is_empty() {
        html.push_str(&format!(
            "<h3 class=\"widget-title\">{}</h3>\n",
            escape_html(title)
        ));
    }
    // CustomHTML: output raw HTML (not escaped)
    html.push_str(&format!("<div class=\"custom-html-widget\">{}</div>", content));
    html
}

fn render_meta(title: &str, _site_url: &str) -> String {
    let display_title = if title.is_empty() { "Meta" } else { title };
    format!(
        r#"<h3 class="widget-title">{title}</h3>
<ul>
  <li><a href="/wp-login.php">Log in</a></li>
  <li><a href="/feed/">Entries RSS</a></li>
  <li><a href="/feed/">Comments RSS</a></li>
  <li><a href="https://github.com/rustpress/rustpress">RustPress</a></li>
</ul>"#,
        title = escape_html(display_title)
    )
}

async fn render_recent_comments(
    title: &str,
    count: u32,
    db: &DatabaseConnection,
) -> String {
    let comments = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .filter(wp_comments::Column::CommentType.eq("comment"))
        .order_by_desc(wp_comments::Column::CommentDate)
        .limit(count as u64)
        .all(db)
        .await
        .unwrap_or_default();

    let display_title = if title.is_empty() {
        "Recent Comments"
    } else {
        title
    };

    let mut html = format!("<h3 class=\"widget-title\">{}</h3>\n<ul>\n", escape_html(display_title));

    for c in &comments {
        // Try to load the post title for context
        let post_title = if let Ok(Some(post)) =
            wp_posts::Entity::find_by_id(c.comment_post_id)
                .one(db)
                .await
        {
            post.post_title
        } else {
            format!("Post #{}", c.comment_post_id)
        };

        let excerpt = if c.comment_content.len() > 50 {
            format!("{}...", &c.comment_content[..50])
        } else {
            c.comment_content.clone()
        };

        html.push_str(&format!(
            "  <li>{} on <a href=\"/{}#comment-{}\">{}</a>: {}</li>\n",
            escape_html(&c.comment_author),
            c.comment_post_id,
            c.comment_id,
            escape_html(&post_title),
            escape_html(&excerpt)
        ));
    }

    html.push_str("</ul>");
    html
}

/// Minimal HTML escaping for widget output.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

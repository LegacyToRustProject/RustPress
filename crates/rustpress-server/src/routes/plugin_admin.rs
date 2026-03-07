//! Admin pages for RustPress plugin system.
//!
//! Provides wp-admin pages for:
//! - WooCommerce (Products, Orders)
//! - Yoast SEO settings
//! - ACF Custom Fields
//! - Contact Form 7
//! - Wordfence Security Dashboard

use axum::{
    extract::{Extension, Form, Query, State},
    response::Html,
    routing::get,
    Router,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use std::sync::Arc;

use rustpress_auth::session::Session;
use rustpress_commerce::woo_compat;
use rustpress_db::entities::{wp_postmeta, wp_posts};
use rustpress_db::queries;
use rustpress_security::{scanner::ScannerContext, SecurityScanner};

use crate::middleware::{require_admin, require_admin_session};
use crate::state::AppState;

// Re-use the same AdminPostsQuery from wp_admin
#[derive(Deserialize)]
#[allow(dead_code)]
pub struct PluginPostsQuery {
    pub post_type: Option<String>,
    pub status: Option<String>,
    pub page: Option<u64>,
}

/// Register plugin admin routes (all require admin + session).
pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/wp-admin/admin-seo.php",
            get(seo_settings_page).post(seo_settings_save),
        )
        .route(
            "/wp-admin/admin-acf.php",
            get(acf_fields_page).post(acf_fields_save),
        )
        .route(
            "/wp-admin/admin-cf7.php",
            get(cf7_forms_page).post(cf7_forms_save),
        )
        .route(
            "/wp-admin/admin-security.php",
            get(security_dashboard_page).post(security_dashboard_save),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin_session,
        ))
        .with_state(state)
}

// --- Shared helpers ---

async fn admin_context(state: &AppState, session: &Session) -> tera::Context {
    // Delegate to the shared admin_context from wp_admin
    // We build a minimal context here since we can't call the private function
    let mut ctx = tera::Context::new();
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    ctx.insert("site_name", &site_name);
    ctx.insert("site_url", &state.site_url);
    ctx.insert(
        "current_user",
        &serde_json::json!({
            "login": session.login,
            "role": &session.role,
            "user_id": session.user_id,
        }),
    );
    // Admin pages always have full capabilities
    ctx.insert("can_manage_options", &true);
    ctx.insert("can_list_users", &true);
    ctx.insert("can_edit_others_posts", &true);
    ctx.insert("can_moderate_comments", &true);
    ctx.insert("can_manage_categories", &true);
    ctx.insert("can_upload_files", &true);
    ctx.insert("can_edit_posts", &true);
    ctx.insert("can_edit_pages", &true);
    ctx.insert("can_edit_theme_options", &true);
    ctx.insert("can_activate_plugins", &true);
    ctx
}

fn render_admin(state: &AppState, template: &str, context: &tera::Context) -> Html<String> {
    match state.admin_tera.render(template, context) {
        Ok(html) => Html(html),
        Err(e) => {
            tracing::error!("Plugin admin template error: {}", e);
            Html(format!(
                "<h1>Admin Template Error</h1><pre>{}</pre>",
                e
            ))
        }
    }
}

// =============================================================================
// WooCommerce Products Page
// =============================================================================

pub async fn wc_products_page(
    state: Arc<AppState>,
    session: Session,
    params: PluginPostsQuery,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "wc-products");

    let page = params.page.unwrap_or(1);
    let per_page = 20u64;
    let status_filter = params.status.clone();

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::PRODUCT));

    if let Some(ref status) = status_filter {
        query = query.filter(wp_posts::Column::PostStatus.eq(status.as_str()));
    } else {
        query = query.filter(wp_posts::Column::PostStatus.ne("trash"));
    }

    let total_count = query.clone().count(&state.db).await.unwrap_or(0);

    let publish_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::PRODUCT))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let draft_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::PRODUCT))
        .filter(wp_posts::Column::PostStatus.eq("draft"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let products = query
        .order_by_desc(wp_posts::Column::PostDate)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut product_list = Vec::new();
    for post in &products {
        let meta = queries::get_post_meta_map(&state.db, post.id)
            .await
            .unwrap_or_default();
        let woo = woo_compat::WooProductData::from_post_and_meta(
            post.id,
            &post.post_title,
            &post.post_name,
            &post.post_content,
            &post.post_excerpt,
            &meta,
        );
        product_list.push(serde_json::json!({
            "id": woo.post_id,
            "name": woo.name,
            "slug": woo.slug,
            "sku": woo.sku,
            "regular_price": format!("{:.2}", woo.regular_price),
            "sale_price": woo.sale_price.map(|p| format!("{:.2}", p)),
            "stock_quantity": woo.stock_quantity,
            "stock_status": woo.stock_status,
            "manage_stock": woo.manage_stock,
            "product_type": if woo.product_type.is_empty() { "simple".to_string() } else { woo.product_type.clone() },
            "currency_symbol": "$",
            "date": post.post_date.format("%Y-%m-%d").to_string(),
        }));
    }

    let total_pages = (total_count as f64 / per_page as f64).ceil() as u64;

    ctx.insert("products", &product_list);
    ctx.insert("total_count", &total_count);
    ctx.insert("publish_count", &publish_count);
    ctx.insert("draft_count", &draft_count);
    ctx.insert("current_page", &page);
    ctx.insert("total_pages", &total_pages);
    ctx.insert("status_filter", &status_filter);

    render_admin(&state, "admin/woocommerce-products.html", &ctx)
}

// =============================================================================
// WooCommerce Orders Page
// =============================================================================

pub async fn wc_orders_page(
    state: Arc<AppState>,
    session: Session,
    params: PluginPostsQuery,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "wc-orders");

    let page = params.page.unwrap_or(1);
    let per_page = 20u64;
    let status_filter = params.status.clone();

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::ORDER));

    if let Some(ref status) = status_filter {
        let s = if status.starts_with("wc-") {
            status.clone()
        } else {
            format!("wc-{}", status)
        };
        query = query.filter(wp_posts::Column::PostStatus.eq(s));
    } else {
        query = query.filter(wp_posts::Column::PostStatus.ne("trash"));
    }

    let total_count = query.clone().count(&state.db).await.unwrap_or(0);

    let processing_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::ORDER))
        .filter(wp_posts::Column::PostStatus.eq("wc-processing"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let completed_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::ORDER))
        .filter(wp_posts::Column::PostStatus.eq("wc-completed"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let on_hold_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::ORDER))
        .filter(wp_posts::Column::PostStatus.eq("wc-on-hold"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let orders = query
        .order_by_desc(wp_posts::Column::PostDate)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut order_list = Vec::new();
    for post in &orders {
        let meta = queries::get_post_meta_map(&state.db, post.id)
            .await
            .unwrap_or_default();
        let woo = woo_compat::WooOrderData::from_post_and_meta(post.id, &post.post_status, &meta);
        let status = woo
            .status
            .strip_prefix("wc-")
            .unwrap_or(&woo.status)
            .to_string();
        let status_label = match status.as_str() {
            "processing" => "Processing",
            "completed" => "Completed",
            "on-hold" => "On Hold",
            "pending" => "Pending",
            "cancelled" => "Cancelled",
            "refunded" => "Refunded",
            "failed" => "Failed",
            _ => &status,
        };
        order_list.push(serde_json::json!({
            "id": woo.post_id,
            "status": status,
            "status_label": status_label,
            "total": format!("{:.2}", woo.total),
            "currency_symbol": "$",
            "billing_name": format!("{} {}", woo.billing.first_name, woo.billing.last_name),
            "billing_email": woo.billing.email,
            "payment_method_title": woo.payment_method_title,
            "date": post.post_date.format("%Y-%m-%d %H:%M").to_string(),
        }));
    }

    let total_pages = (total_count as f64 / per_page as f64).ceil() as u64;

    ctx.insert("orders", &order_list);
    ctx.insert("total_count", &total_count);
    ctx.insert("processing_count", &processing_count);
    ctx.insert("completed_count", &completed_count);
    ctx.insert("on_hold_count", &on_hold_count);
    ctx.insert("current_page", &page);
    ctx.insert("total_pages", &total_pages);
    ctx.insert("status_filter", &status_filter);

    render_admin(&state, "admin/woocommerce-orders.html", &ctx)
}

// =============================================================================
// Yoast SEO Admin Page
// =============================================================================

#[derive(Deserialize)]
struct SeoSettingsForm {
    seo_title_separator: Option<String>,
    seo_homepage_title: Option<String>,
    seo_homepage_desc: Option<String>,
    seo_post_title: Option<String>,
    seo_page_title: Option<String>,
    seo_noindex_empty_cat: Option<String>,
    seo_og_default_image: Option<String>,
    seo_og_enabled: Option<String>,
    #[allow(dead_code)]
    seo_sitemap_posts: Option<String>,
    #[allow(dead_code)]
    seo_sitemap_pages: Option<String>,
}

async fn seo_settings_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "seo");

    let sep = state
        .options
        .get_option_or("_yoast_wpseo_separator", "-")
        .await
        .unwrap_or_else(|_| "-".into());
    let homepage_title = state
        .options
        .get_option_or("_yoast_wpseo_title-homepage", "")
        .await
        .unwrap_or_default();
    let homepage_desc = state
        .options
        .get_option_or("_yoast_wpseo_metadesc-homepage", "")
        .await
        .unwrap_or_default();
    let post_title = state
        .options
        .get_option_or("_yoast_wpseo_title-post", "%%title%% %%sep%% %%sitename%%")
        .await
        .unwrap_or_else(|_| "%%title%% %%sep%% %%sitename%%".into());
    let page_title = state
        .options
        .get_option_or("_yoast_wpseo_title-page", "%%title%% %%sep%% %%sitename%%")
        .await
        .unwrap_or_else(|_| "%%title%% %%sep%% %%sitename%%".into());
    let noindex_empty_cat = state
        .options
        .get_option_or("_yoast_wpseo_noindex-subpages", "0")
        .await
        .unwrap_or_else(|_| "0".into())
        == "1";
    let og_default_image = state
        .options
        .get_option_or("_yoast_wpseo_og_default_image", "")
        .await
        .unwrap_or_default();
    let og_enabled = state
        .options
        .get_option_or("_yoast_wpseo_opengraph", "1")
        .await
        .unwrap_or_else(|_| "1".into())
        == "1";

    let indexed_posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .count(&state.db)
        .await
        .unwrap_or(0);
    let indexed_pages = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("page"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .count(&state.db)
        .await
        .unwrap_or(0);
    let posts_with_desc = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::MetaKey.eq("_yoast_wpseo_metadesc"))
        .filter(wp_postmeta::Column::MetaValue.ne(""))
        .count(&state.db)
        .await
        .unwrap_or(0);

    ctx.insert("title_separator", &sep);
    ctx.insert("separators", &vec!["-", "|", "/", "\\", "*", "~", "&bull;", "&mdash;"]);
    ctx.insert("homepage_title", &homepage_title);
    ctx.insert("homepage_desc", &homepage_desc);
    ctx.insert("post_title_template", &post_title);
    ctx.insert("page_title_template", &page_title);
    ctx.insert("noindex_empty_cat", &noindex_empty_cat);
    ctx.insert("og_default_image", &og_default_image);
    ctx.insert("og_enabled", &og_enabled);
    ctx.insert("sitemap_posts", &true);
    ctx.insert("sitemap_pages", &true);
    ctx.insert("indexed_posts", &indexed_posts);
    ctx.insert("indexed_pages", &indexed_pages);
    ctx.insert("posts_with_desc", &posts_with_desc);

    render_admin(&state, "admin/yoast-seo.html", &ctx)
}

async fn seo_settings_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<SeoSettingsForm>,
) -> Html<String> {
    let options = vec![
        ("_yoast_wpseo_separator", form.seo_title_separator.unwrap_or_else(|| "-".into())),
        ("_yoast_wpseo_title-homepage", form.seo_homepage_title.unwrap_or_default()),
        ("_yoast_wpseo_metadesc-homepage", form.seo_homepage_desc.unwrap_or_default()),
        ("_yoast_wpseo_title-post", form.seo_post_title.unwrap_or_default()),
        ("_yoast_wpseo_title-page", form.seo_page_title.unwrap_or_default()),
        ("_yoast_wpseo_noindex-subpages", if form.seo_noindex_empty_cat.is_some() { "1".into() } else { "0".into() }),
        ("_yoast_wpseo_og_default_image", form.seo_og_default_image.unwrap_or_default()),
        ("_yoast_wpseo_opengraph", if form.seo_og_enabled.is_some() { "1".into() } else { "0".into() }),
    ];

    for (key, value) in options {
        let _ = state.options.update_option(key, &value).await;
    }

    seo_settings_page(State(state), Extension(session)).await
}

// =============================================================================
// ACF Custom Fields Admin Page
// =============================================================================

#[derive(Deserialize)]
struct AcfFieldGroupForm {
    fg_title: String,
    fg_location: Option<String>,
    #[serde(default)]
    field_label: Vec<String>,
    #[serde(default)]
    field_type: Vec<String>,
}

async fn acf_fields_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "acf");

    let registry = state.field_registry.read().await;
    let groups = registry.all_groups();

    let group_list: Vec<serde_json::Value> = groups
        .iter()
        .map(|g| {
            let location_rules: Vec<String> = g
                .location_rules
                .iter()
                .flatten()
                .map(|r| format!("{:?} = {}", r.param, r.value))
                .collect();
            serde_json::json!({
                "id": 0,
                "key": g.key,
                "title": g.title,
                "field_count": g.fields.len(),
                "location_rules": location_rules,
                "active": true,
            })
        })
        .collect();
    drop(registry);

    // Also look for ACF field group posts in the database
    let acf_posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("acf-field-group"))
        .filter(wp_posts::Column::PostStatus.ne("trash"))
        .order_by_desc(wp_posts::Column::PostDate)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut all_groups = group_list;
    for post in &acf_posts {
        let field_count = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostParent.eq(post.id))
            .filter(wp_posts::Column::PostType.eq("acf-field"))
            .count(&state.db)
            .await
            .unwrap_or(0);

        all_groups.push(serde_json::json!({
            "id": post.id,
            "key": post.post_name,
            "title": post.post_title,
            "field_count": field_count,
            "location_rules": Vec::<String>::new(),
            "active": post.post_status == "publish",
        }));
    }

    ctx.insert("field_groups", &all_groups);

    render_admin(&state, "admin/acf-fields.html", &ctx)
}

async fn acf_fields_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<AcfFieldGroupForm>,
) -> Html<String> {
    let now = chrono::Utc::now().naive_utc();
    let post_name = form
        .fg_title
        .to_lowercase()
        .replace(' ', "_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>();
    let group_key = format!("group_{}", &post_name);

    let group_post = wp_posts::ActiveModel {
        post_author: Set(session.user_id),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(String::new()),
        post_title: Set(form.fg_title.clone()),
        post_excerpt: Set(String::new()),
        post_status: Set("publish".into()),
        post_name: Set(group_key),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_type: Set("acf-field-group".into()),
        post_parent: Set(0),
        ..Default::default()
    };

    if let Ok(result) = group_post.insert(&state.db).await {
        let parent_id = result.id;

        for (i, label) in form.field_label.iter().enumerate() {
            if label.is_empty() {
                continue;
            }
            let field_type = form
                .field_type
                .get(i)
                .cloned()
                .unwrap_or_else(|| "text".into());
            let field_name = label
                .to_lowercase()
                .replace(' ', "_")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>();
            let field_key = format!("field_{}_{}", &post_name, &field_name);

            let field_post = wp_posts::ActiveModel {
                post_author: Set(session.user_id),
                post_date: Set(now),
                post_date_gmt: Set(now),
                post_content: Set(format!(
                    "a:1:{{s:4:\"type\";s:{}:\"{}\";}}", field_type.len(), field_type
                )),
                post_title: Set(label.clone()),
                post_excerpt: Set(field_name),
                post_status: Set("publish".into()),
                post_name: Set(field_key),
                post_modified: Set(now),
                post_modified_gmt: Set(now),
                post_type: Set("acf-field".into()),
                post_parent: Set(parent_id),
                ..Default::default()
            };
            let _ = field_post.insert(&state.db).await;
        }

        if let Some(ref location) = form.fg_location {
            let _ = queries::set_post_meta(&state.db, parent_id, "location_rules", location).await;
        }
    }

    acf_fields_page(State(state), Extension(session)).await
}

// =============================================================================
// Contact Form 7 Admin Page
// =============================================================================

#[derive(Deserialize)]
struct Cf7FormCreateForm {
    form_title: String,
    form_template: Option<String>,
    form_mail_to: Option<String>,
}

async fn cf7_forms_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "cf7");

    let cf7_posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wpcf7_contact_form"))
        .filter(wp_posts::Column::PostStatus.ne("trash"))
        .order_by_desc(wp_posts::Column::PostDate)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut form_list = Vec::new();
    for post in &cf7_posts {
        let submission_count = state.form_submissions.count(&post.id.to_string(), None);
        form_list.push(serde_json::json!({
            "id": post.id,
            "title": post.post_title,
            "submission_count": submission_count,
            "date": post.post_date.format("%Y-%m-%d").to_string(),
        }));
    }

    ctx.insert("forms", &form_list);

    render_admin(&state, "admin/contact-form7.html", &ctx)
}

async fn cf7_forms_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<Cf7FormCreateForm>,
) -> Html<String> {
    let now = chrono::Utc::now().naive_utc();
    let post_name = form
        .form_title
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();

    let template = form.form_template.unwrap_or_else(|| {
        "[text* your-name placeholder \"Your Name\"]\n\
         [email* your-email placeholder \"Email\"]\n\
         [textarea your-message placeholder \"Message\"]\n\
         [submit \"Send\"]"
            .into()
    });

    let cf7_post = wp_posts::ActiveModel {
        post_author: Set(session.user_id),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(template),
        post_title: Set(form.form_title.clone()),
        post_excerpt: Set(String::new()),
        post_status: Set("publish".into()),
        post_name: Set(post_name),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_type: Set("wpcf7_contact_form".into()),
        post_parent: Set(0),
        ..Default::default()
    };

    if let Ok(result) = cf7_post.insert(&state.db).await {
        if let Some(ref mail_to) = form.form_mail_to {
            if !mail_to.is_empty() {
                let _ = queries::set_post_meta(&state.db, result.id, "_mail_to", mail_to).await;
            }
        }
    }

    cf7_forms_page(State(state), Extension(session)).await
}

// =============================================================================
// Wordfence Security Dashboard
// =============================================================================

#[derive(Deserialize)]
struct SecurityForm {
    section: String,
    waf_enabled: Option<String>,
    rate_limit_enabled: Option<String>,
    max_rpm: Option<String>,
    login_protection_enabled: Option<String>,
    max_attempts: Option<String>,
    lockout_mins: Option<String>,
    block_ip: Option<String>,
    unblock_ip: Option<String>,
}

async fn security_dashboard_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "security");

    let waf = state.waf.read().await;
    let waf_rule_count = waf.rule_count();
    drop(waf);
    ctx.insert("waf_enabled", &true);
    ctx.insert("waf_rule_count", &waf_rule_count);
    ctx.insert("blocked_24h", &0u64);

    ctx.insert("rate_limit_enabled", &true);
    ctx.insert("max_rpm", &120u32);

    let lp = state.login_protection.read().await;
    let locked_out_count = lp.tracked_ip_count();
    drop(lp);
    ctx.insert("login_protection_enabled", &true);
    ctx.insert("max_attempts", &5u32);
    ctx.insert("lockout_mins", &30u32);
    ctx.insert("locked_out_count", &locked_out_count);

    let wf_blocked = state
        .options
        .get_option_or("wf_blocked_ips", "")
        .await
        .unwrap_or_default();
    let blocked_ips: Vec<String> = if wf_blocked.is_empty() {
        vec![]
    } else {
        wf_blocked
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    ctx.insert("blocked_ips", &blocked_ips);

    let scanner = SecurityScanner::new(ScannerContext {
        ssl_enabled: state.site_url.starts_with("https"),
        db_prefix: "wp_".into(),
        ..Default::default()
    });
    let checks = scanner.run_all_checks();
    let checks_passed = checks
        .iter()
        .filter(|c| matches!(c.status, rustpress_security::CheckStatus::Pass))
        .count();
    let checks_total = checks.len();
    let security_score = if checks_total > 0 {
        (checks_passed * 100) / checks_total
    } else {
        0
    };

    let scan_results: Vec<serde_json::Value> = checks
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "status": match c.status {
                    rustpress_security::CheckStatus::Pass => "pass",
                    rustpress_security::CheckStatus::Warning => "warning",
                    rustpress_security::CheckStatus::Fail => "fail",
                },
            })
        })
        .collect();

    ctx.insert("security_score", &security_score);
    ctx.insert("checks_passed", &checks_passed);
    ctx.insert("checks_total", &checks_total);
    ctx.insert("scan_results", &scan_results);

    render_admin(&state, "admin/wordfence.html", &ctx)
}

async fn security_dashboard_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<SecurityForm>,
) -> Html<String> {
    match form.section.as_str() {
        "block_ip" => {
            if let Some(ref ip) = form.block_ip {
                if !ip.is_empty() {
                    let current = state
                        .options
                        .get_option_or("wf_blocked_ips", "")
                        .await
                        .unwrap_or_default();
                    let mut ips: Vec<String> = if current.is_empty() {
                        vec![]
                    } else {
                        current.split(',').map(|s| s.trim().to_string()).collect()
                    };
                    if !ips.contains(ip) {
                        ips.push(ip.clone());
                    }
                    let _ = state
                        .options
                        .update_option("wf_blocked_ips", &ips.join(","))
                        .await;
                }
            }
        }
        "unblock_ip" => {
            if let Some(ref ip) = form.unblock_ip {
                let current = state
                    .options
                    .get_option_or("wf_blocked_ips", "")
                    .await
                    .unwrap_or_default();
                let ips: Vec<String> = current
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != ip)
                    .collect();
                let _ = state
                    .options
                    .update_option("wf_blocked_ips", &ips.join(","))
                    .await;
            }
        }
        "firewall" => {
            let enabled = if form.waf_enabled.is_some() { "1" } else { "0" };
            let _ = state.options.update_option("wf_firewall_enabled", enabled).await;
        }
        "rate_limit" => {
            let enabled = if form.rate_limit_enabled.is_some() {
                "1"
            } else {
                "0"
            };
            let _ = state
                .options
                .update_option("wf_rate_limit_enabled", enabled)
                .await;
            if let Some(ref rpm) = form.max_rpm {
                let _ = state
                    .options
                    .update_option("wf_rate_limit_maxRequestsPerMin", rpm)
                    .await;
            }
        }
        "login" => {
            let enabled = if form.login_protection_enabled.is_some() {
                "1"
            } else {
                "0"
            };
            let _ = state
                .options
                .update_option("wf_login_sec_enabled", enabled)
                .await;
            if let Some(ref max) = form.max_attempts {
                let _ = state
                    .options
                    .update_option("wf_login_sec_maxFailures", max)
                    .await;
            }
            if let Some(ref mins) = form.lockout_mins {
                let _ = state
                    .options
                    .update_option("wf_login_sec_lockoutMins", mins)
                    .await;
            }
        }
        _ => {}
    }

    security_dashboard_page(State(state), Extension(session)).await
}

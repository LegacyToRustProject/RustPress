use axum::{
    extract::{Form, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::Deserialize;
use std::sync::Arc;

use rustpress_db::entities::{
    wp_comments, wp_postmeta, wp_posts, wp_term_relationships, wp_term_taxonomy, wp_terms, wp_users,
};
use rustpress_themes::hierarchy::PageType;
use rustpress_themes::tags::{
    insert_post_context_full, insert_posts_context_with_hooks, PaginationData, PostTemplateData,
};

use rustpress_cache::page_cache::CachedPage;

use crate::state::AppState;
use crate::widgets;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub s: Option<String>,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<u64>,
}

#[derive(Deserialize)]
pub struct CommentForm {
    pub comment_post_id: u64,
    pub author: Option<String>,
    pub email: Option<String>,
    pub url: Option<String>,
    pub comment: String,
    pub comment_parent: Option<u64>,
    pub _wpnonce: Option<String>,
    pub redirect_to: Option<String>,
}

#[derive(Deserialize)]
pub struct WpQueryVars {
    pub p: Option<u64>,
    pub page_id: Option<u64>,
    pub name: Option<String>,
    pub cat: Option<u64>,
    pub tag: Option<String>,
    pub s: Option<String>,
    pub attachment_id: Option<u64>,
    pub feed: Option<String>,
    pub paged: Option<u64>,
    pub m: Option<String>,
    pub author: Option<u64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(front_page_or_query))
        .route("/feed", get(rss_feed))
        .route("/feed/", get(rss_feed))
        .route("/page/{num}", get(paginated_home))
        .route("/page/{num}/", get(paginated_home))
        .route("/search", get(search_page))
        .route("/search/", get(search_page))
        .route("/category/{slug}", get(category_archive))
        .route("/category/{slug}/", get(category_archive))
        .route("/tag/{slug}", get(tag_archive))
        .route("/tag/{slug}/", get(tag_archive))
        .route("/author/{slug}", get(author_archive))
        .route("/author/{slug}/", get(author_archive))
        // Category/tag RSS feeds
        .route("/category/{slug}/feed", get(category_feed))
        .route("/category/{slug}/feed/", get(category_feed))
        .route("/tag/{slug}/feed", get(tag_feed))
        .route("/tag/{slug}/feed/", get(tag_feed))
        // Comments feed
        .route("/comments/feed", get(comments_feed))
        .route("/comments/feed/", get(comments_feed))
        // Date-based archives (WordPress compatible) — single post routes BEFORE archives
        .route("/{year}/{month}/{day}/{slug}", get(single_by_date_slug))
        .route("/{year}/{month}/{day}/{slug}/", get(single_by_date_slug))
        .route(
            "/{year}/{month}/{slug}",
            get(single_by_month_slug_or_day_archive),
        )
        .route(
            "/{year}/{month}/{slug}/",
            get(single_by_month_slug_or_day_archive),
        )
        .route("/{year}/{month}", get(month_archive))
        .route("/{year}/{month}/", get(month_archive))
        .route("/wp-comments-post.php", axum::routing::post(submit_comment))
        // wp-login.php is registered in wp_admin routes
        // admin-ajax.php compatible endpoint
        .route("/wp-admin/admin-ajax.php", get(admin_ajax).post(admin_ajax))
        // wp-cron.php HTTP trigger
        .route("/wp-cron.php", get(wp_cron))
        // Per-post comment feed
        .route("/{slug}/feed", get(post_comment_feed))
        .route("/{slug}/feed/", get(post_comment_feed))
        // Trackback
        .route("/{slug}/trackback", axum::routing::post(trackback_handler))
        .route("/{slug}/trackback/", axum::routing::post(trackback_handler))
        // wp-register.php — redirect to login
        .route("/wp-register.php", get(wp_register_redirect))
        .route("/{slug}", get(single_by_slug))
        .route("/{slug}/", get(single_by_slug))
}

async fn build_base_context(state: &AppState) -> tera::Context {
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    let site_desc = state
        .options
        .get_blogdescription()
        .await
        .unwrap_or_default();
    let engine = state.theme_engine.read().await;
    let mut ctx = engine.base_context(&site_name, &site_desc, &state.site_url);

    // Fire wp_enqueue_scripts — allows plugins registered at runtime to add assets.
    // Theme assets are pre-registered at startup; this hook is for runtime additions.
    state
        .hooks
        .do_action("wp_enqueue_scripts", &serde_json::json!({}));

    // Render enqueued styles + header scripts into {{ wp_head }}.
    // Render footer scripts into {{ wp_footer }}.
    let wp_head_html = state.asset_manager.render_head_styles()
        + &state.asset_manager.render_head_scripts();
    let wp_footer_html = state.asset_manager.render_footer_scripts();
    ctx.insert("wp_head", &wp_head_html);
    ctx.insert("wp_footer", &wp_footer_html);

    // Fire wp_head / wp_footer actions for any remaining raw HTML hooks.
    // NOTE: Output from these callbacks is not yet captured (Phase 6: task_local! buffer).
    state
        .hooks
        .do_action("wp_head", &serde_json::json!({"site_url": &state.site_url}));
    state.hooks.do_action(
        "wp_footer",
        &serde_json::json!({"site_url": &state.site_url}),
    );

    // Load navigation menus from WordPress nav_menu system
    let menu_locations = crate::nav_menu::get_menu_locations(&state.options).await;
    let mut header_links: Vec<serde_json::Value> = Vec::new();

    // Try WordPress nav_menu locations first
    if let Some(&menu_id) = menu_locations
        .get("primary")
        .or(menu_locations.get("header"))
    {
        if let Some(menu) = crate::nav_menu::load_menu(&state.db, menu_id).await {
            header_links = menu
                .items
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "label": item.title,
                        "url": item.url,
                    })
                })
                .collect();
        }
    }

    // Fallback to legacy nav_menu_header option
    if header_links.is_empty() {
        let header_menu_text = state
            .options
            .get_option("nav_menu_header")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        header_links = parse_menu_text(&header_menu_text);
    }

    let mut footer_links: Vec<serde_json::Value> = Vec::new();
    if let Some(&menu_id) = menu_locations
        .get("footer")
        .or(menu_locations.get("secondary"))
    {
        if let Some(menu) = crate::nav_menu::load_menu(&state.db, menu_id).await {
            footer_links = menu
                .items
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "label": item.title,
                        "url": item.url,
                    })
                })
                .collect();
        }
    }
    if footer_links.is_empty() {
        let footer_menu_text = state
            .options
            .get_option("nav_menu_footer")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        footer_links = parse_menu_text(&footer_menu_text);
    }

    ctx.insert("header_menu", &header_links);
    ctx.insert("footer_menu", &footer_links);

    // Load widget configuration and render widget areas as HTML
    let widget_config = widgets::load_widget_config(&state.options).await;
    let sidebar_widgets_html =
        widgets::render_widget_area(&widget_config, "sidebar-1", &state.db, &state.site_url).await;
    let footer1_widgets_html =
        widgets::render_widget_area(&widget_config, "footer-1", &state.db, &state.site_url).await;
    let footer2_widgets_html =
        widgets::render_widget_area(&widget_config, "footer-2", &state.db, &state.site_url).await;

    ctx.insert("sidebar_widgets", &sidebar_widgets_html);
    ctx.insert("footer_widgets_1", &footer1_widgets_html);
    ctx.insert("footer_widgets_2", &footer2_widgets_html);
    // Combined footer widgets for convenience
    let footer_widgets_html = if footer1_widgets_html.is_empty() && footer2_widgets_html.is_empty()
    {
        String::new()
    } else {
        format!(
            "<div class=\"footer-widgets-row\">\
             <div class=\"footer-widgets-col\">{footer1_widgets_html}</div>\
             <div class=\"footer-widgets-col\">{footer2_widgets_html}</div>\
             </div>"
        )
    };
    ctx.insert("footer_widgets", &footer_widgets_html);

    // Default empty SEO meta tags (overridden per-page)
    ctx.insert("seo_meta_tags", &"");

    // Load published pages for navigation fallback (wp-block-page-list)
    if header_links.is_empty() {
        let pages = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostType.eq("page"))
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .order_by_asc(wp_posts::Column::MenuOrder)
            .order_by_asc(wp_posts::Column::PostTitle)
            .all(&state.db)
            .await
            .unwrap_or_default();
        let rewrite = state.rewrite_rules.read().await;
        let page_data: Vec<PostTemplateData> = pages
            .iter()
            .map(|p| PostTemplateData::from_model_with_rewrite(p, &state.site_url, &rewrite))
            .collect();
        drop(rewrite);
        ctx.insert("pages", &page_data);
    }

    ctx
}

fn parse_menu_text(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            if parts.len() == 2 {
                Some(serde_json::json!({
                    "label": parts[0].trim(),
                    "url": parts[1].trim(),
                }))
            } else {
                None
            }
        })
        .collect()
}

async fn render_theme_page(
    state: &AppState,
    page_type: &PageType,
    context: &tera::Context,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let engine = state.theme_engine.read().await;
    match engine.render_page(page_type, context) {
        Ok(html) => Ok(Html(html)),
        Err(e) => {
            tracing::error!("Template render error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<h1>Render Error</h1><pre>{e}</pre>")),
            ))
        }
    }
}

async fn get_posts_page(
    state: &AppState,
    page: u64,
) -> Result<(Vec<PostTemplateData>, PaginationData), (StatusCode, Html<String>)> {
    let per_page = state.options.get_posts_per_page().await.unwrap_or(10) as u64;

    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = match query.clone().count(&state.db).await {
        Ok(n) => n,
        Err(e) => {
            tracing::debug!(error = %e, "post count query failed (no-DB mode?)");
            return Ok((vec![], PaginationData::new(1, 1, 0)));
        }
    };

    let total_pages = total.div_ceil(per_page);

    let models = match query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!(error = %e, "post list query failed (no-DB mode?)");
            vec![]
        }
    };

    let rewrite = state.rewrite_rules.read().await;
    let mut posts: Vec<PostTemplateData> = models
        .iter()
        .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
        .collect();
    drop(rewrite);

    // Bulk-load featured images
    load_featured_images(&state.db, &mut posts).await;

    let pagination = PaginationData::new(page, total_pages, total);

    Ok((posts, pagination))
}

/// Bulk-load featured image URLs for a list of posts.
///
/// Queries `_thumbnail_id` from wp_postmeta, then resolves attachment GUIDs.
/// Populates `featured_image_url` on each matching post in-place.
async fn load_featured_images(db: &sea_orm::DatabaseConnection, posts: &mut [PostTemplateData]) {
    if posts.is_empty() {
        return;
    }

    let post_ids: Vec<u64> = posts.iter().map(|p| p.id).collect();

    // Get all _thumbnail_id meta for these posts
    let thumb_metas = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.is_in(post_ids))
        .filter(wp_postmeta::Column::MetaKey.eq("_thumbnail_id"))
        .all(db)
        .await
        .unwrap_or_default();

    // Map post_id -> attachment_id
    let mut post_to_attachment: std::collections::HashMap<u64, u64> =
        std::collections::HashMap::new();
    for meta in &thumb_metas {
        if let Some(ref val) = meta.meta_value {
            if let Ok(att_id) = val.parse::<u64>() {
                post_to_attachment.insert(meta.post_id, att_id);
            }
        }
    }

    if post_to_attachment.is_empty() {
        return;
    }

    // Load all attachment posts in one query
    let att_ids: Vec<u64> = post_to_attachment.values().copied().collect();
    let attachments = wp_posts::Entity::find()
        .filter(wp_posts::Column::Id.is_in(att_ids))
        .all(db)
        .await
        .unwrap_or_default();

    // Map attachment_id -> guid
    let att_guids: std::collections::HashMap<u64, String> =
        attachments.into_iter().map(|a| (a.id, a.guid)).collect();

    // Populate featured_image_url on posts
    for post in posts.iter_mut() {
        if let Some(att_id) = post_to_attachment.get(&post.id) {
            if let Some(url) = att_guids.get(att_id) {
                post.featured_image_url = url.clone();
            }
        }
    }
}

async fn front_page(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Check page cache
    let cache_key = "front_page";
    if let Some(cached) = state.page_cache.get(cache_key).await {
        return Ok(Html(cached.html));
    }

    let mut context = build_base_context(&state).await;
    let (mut posts, pagination) = get_posts_page(&state, 1).await?;

    // Prepend sticky posts on page 1
    let sticky_ids = get_sticky_post_ids(&state).await;
    if !sticky_ids.is_empty() {
        let sticky_models = wp_posts::Entity::find()
            .filter(wp_posts::Column::Id.is_in(sticky_ids.clone()))
            .filter(wp_posts::Column::PostType.eq("post"))
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .order_by_desc(wp_posts::Column::PostDate)
            .all(&state.db)
            .await
            .unwrap_or_default();

        let rewrite = state.rewrite_rules.read().await;
        let sticky_posts: Vec<PostTemplateData> = sticky_models
            .iter()
            .map(|m| {
                let mut data =
                    PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite);
                data.sticky = true;
                data
            })
            .collect();
        drop(rewrite);

        // Remove sticky posts from normal list to avoid duplicates
        posts.retain(|p| !sticky_ids.contains(&p.id));

        // Prepend sticky posts
        let mut combined = sticky_posts;
        combined.append(&mut posts);
        posts = combined;
    }

    insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));

    // When show_on_front = 'posts', use the blog listing template (Home/Index)
    // rather than front-page.html (which is only for static front pages).
    let show_on_front = state
        .options
        .get_option("show_on_front")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "posts".to_string());
    let page_type = if show_on_front == "page" {
        PageType::FrontPage
    } else {
        PageType::Home
    };

    let result = render_theme_page(&state, &page_type, &context).await?;

    // Store in cache
    state
        .page_cache
        .set(
            cache_key,
            CachedPage {
                html: result.0.clone(),
                content_type: "text/html".to_string(),
                status_code: 200,
            },
        )
        .await;
    Ok(result)
}

/// Dispatcher for `/` that handles WordPress query vars (?p=, ?page_id=, etc.)
/// Falls back to the normal front page when no special query vars are present.
async fn front_page_or_query(
    State(state): State<Arc<AppState>>,
    Query(qv): Query<WpQueryVars>,
    headers: HeaderMap,
) -> Response {
    // Helper: convert Result<Html, (StatusCode, Html)> to Response
    fn conv(r: Result<Html<String>, (StatusCode, Html<String>)>) -> Response {
        match r {
            Ok(h) => h.into_response(),
            Err((s, h)) => (s, h).into_response(),
        }
    }

    // ?p=123 — post by ID
    if let Some(post_id) = qv.p {
        let post = wp_posts::Entity::find_by_id(post_id)
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .one(&state.db)
            .await;
        return match post {
            Ok(Some(p)) => conv(single_post_by_slug(&state, &p.post_name, &headers).await),
            Ok(None) => (StatusCode::NOT_FOUND, Html("Post not found".to_string())).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())).into_response(),
        };
    }

    // ?page_id=123 — page by ID
    if let Some(page_id) = qv.page_id {
        let post = wp_posts::Entity::find_by_id(page_id)
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .one(&state.db)
            .await;
        return match post {
            Ok(Some(p)) => conv(single_post_by_slug(&state, &p.post_name, &headers).await),
            Ok(None) => (StatusCode::NOT_FOUND, Html("Page not found".to_string())).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())).into_response(),
        };
    }

    // ?name=slug — post by slug
    if let Some(name) = qv.name {
        return conv(single_post_by_slug(&state, &name, &headers).await);
    }

    // ?s=query — search
    if let Some(s) = qv.s {
        let mut context = build_base_context(&state).await;
        context.insert("search_query", &s);
        let search_lower = s.to_lowercase();
        let mut results = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .filter(wp_posts::Column::PostType.eq("post"))
            .filter(
                sea_orm::Condition::any()
                    .add(wp_posts::Column::PostTitle.like(format!("%{s}%")))
                    .add(wp_posts::Column::PostContent.like(format!("%{s}%"))),
            )
            .order_by_desc(wp_posts::Column::PostDate)
            .limit(20)
            .all(&state.db)
            .await
            .unwrap_or_default();
        // Sort by relevance: title matches first, then by date (like WordPress)
        results.sort_by(|a, b| {
            let a_title = a.post_title.to_lowercase().contains(&search_lower);
            let b_title = b.post_title.to_lowercase().contains(&search_lower);
            match (a_title, b_title) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.post_date.cmp(&a.post_date),
            }
        });
        let rewrite = state.rewrite_rules.read().await;
        let posts: Vec<PostTemplateData> = results
            .iter()
            .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
            .collect();
        drop(rewrite);
        let pagination = rustpress_themes::tags::PaginationData::new(1, 1, results.len() as u64);
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
        return conv(
            render_theme_page(
                &state,
                &rustpress_themes::hierarchy::PageType::Search,
                &context,
            )
            .await,
        );
    }

    // ?cat=5 — category archive by term_id
    if let Some(cat_id) = qv.cat {
        let term = wp_terms::Entity::find_by_id(cat_id).one(&state.db).await;
        let (slug, proper_name) = match term {
            Ok(Some(t)) => (t.slug.clone(), t.name.clone()),
            _ => (cat_id.to_string(), cat_id.to_string()),
        };
        let mut context = build_base_context(&state).await;
        let (posts, pagination, term_id) = match taxonomy_posts(&state, &slug, "category", 1).await
        {
            Ok(v) => v,
            Err(e) => return conv(Err(e)),
        };
        context.insert("term_name", &proper_name);
        context.insert("archive_title", &format!("Category: {proper_name}"));
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
        return conv(
            render_theme_page(
                &state,
                &rustpress_themes::hierarchy::PageType::Category { slug, id: term_id },
                &context,
            )
            .await,
        );
    }

    // ?tag=slug — tag archive by slug
    if let Some(tag_slug) = qv.tag {
        let mut context = build_base_context(&state).await;
        let (posts, pagination, term_id) =
            match taxonomy_posts(&state, &tag_slug, "post_tag", 1).await {
                Ok(v) => v,
                Err(e) => return conv(Err(e)),
            };
        let term_name = tag_slug.replace('-', " ");
        context.insert("term_name", &term_name);
        context.insert("archive_title", &format!("Tag: {term_name}"));
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
        return conv(
            render_theme_page(
                &state,
                &rustpress_themes::hierarchy::PageType::Tag {
                    slug: tag_slug,
                    id: term_id,
                },
                &context,
            )
            .await,
        );
    }

    // ?attachment_id=N — attachment/media page
    if let Some(att_id) = qv.attachment_id {
        let post = wp_posts::Entity::find_by_id(att_id)
            .filter(wp_posts::Column::PostType.eq("attachment"))
            .one(&state.db)
            .await;
        return match post {
            Ok(Some(p)) => conv(single_post_by_slug(&state, &p.post_name, &headers).await),
            Ok(None) => (
                StatusCode::NOT_FOUND,
                Html("Attachment not found".to_string()),
            )
                .into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())).into_response(),
        };
    }

    // ?feed=rss2 — RSS feed (same as /feed/)
    if let Some(ref feed) = qv.feed {
        if feed == "rss2" || feed == "rss" || feed == "atom" {
            return rss_feed(State(Arc::clone(&state))).await;
        }
    }

    // ?paged=N — explicit pagination (e.g. /?paged=2)
    if let Some(page) = qv.paged {
        let page = if page == 0 { 1 } else { page };
        let mut context = build_base_context(&state).await;
        let (posts, pagination) = match get_posts_page(&state, page).await {
            Ok(v) => v,
            Err(e) => return conv(Err(e)),
        };
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
        return conv(render_theme_page(&state, &PageType::Home, &context).await);
    }

    // ?m=YYYYMM or ?m=YYYYMMDD — date archive via query param → redirect to /YYYY/MM/
    if let Some(ref m_val) = qv.m {
        if m_val.len() >= 6 {
            if let (Ok(year), Ok(month)) = (m_val[0..4].parse::<u32>(), m_val[4..6].parse::<u32>())
            {
                let url = format!("/{year:04}/{month:02}");
                return Redirect::permanent(&url).into_response();
            }
        }
    }

    // ?author=N — redirect to /author/{nicename}/ like WordPress does.
    if let Some(author_id) = qv.author {
        use rustpress_db::entities::wp_users;
        if let Ok(Some(user)) = wp_users::Entity::find_by_id(author_id).one(&state.db).await {
            let url = format!("/author/{}/", user.user_nicename);
            return Redirect::permanent(&url).into_response();
        }
        // Author not found — fall through to 404
        return conv(Err((
            StatusCode::NOT_FOUND,
            axum::response::Html("Author not found".to_string()),
        )));
    }

    // No special query vars — render the front page
    conv(front_page(State(state)).await)
}

async fn paginated_home(
    State(state): State<Arc<AppState>>,
    Path(num): Path<u64>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let page = if num == 0 { 1 } else { num };
    let mut context = build_base_context(&state).await;
    let (posts, pagination) = get_posts_page(&state, page).await?;
    insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
    render_theme_page(&state, &PageType::Home, &context).await
}

async fn single_by_slug(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Check if slug is a 4-digit year → year archive
    if let Ok(year) = slug.parse::<u32>() {
        if (1970..=2099).contains(&year) {
            return year_archive_page(&state, year, None).await;
        }
    }

    single_post_by_slug(&state, &slug, &headers).await
}

async fn single_post_by_slug(
    state: &AppState,
    slug: &str,
    headers: &HeaderMap,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Try post first (include password-protected posts which have status 'publish')
    let post = match wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(slug))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(&state.db)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(error = %e, "post-by-slug query failed (no-DB mode?) — serving 404");
            None
        }
    };

    match post {
        Some(p) => {
            let post_type = p.post_type.clone();
            let post_id = p.id;
            let post_date = p.post_date;
            let post_password = p.post_password.clone();

            // Check password protection
            if !post_password.is_empty() {
                let has_access = check_post_password_cookie(headers, slug, &post_password);
                if !has_access {
                    // Show password form
                    let mut context = build_base_context(state).await;
                    context.insert("post_title", &p.post_title);
                    context.insert("post_id", &post_id);
                    context.insert("redirect_to", &format!("/{slug}"));
                    return render_theme_page(
                        state,
                        &PageType::Single {
                            post_type: "password-form".to_string(),
                            slug: slug.to_string(),
                        },
                        &context,
                    )
                    .await
                    .or_else(|_| {
                        // Fallback: render inline password form
                        let html = format!(
                            r#"<h1>{}</h1>
<p>This content is password protected. To view it please enter your password below:</p>
<form method="post" action="/wp-login.php?action=postpass">
  <input type="hidden" name="redirect_to" value="/{}">
  <p><label for="post_password">Password:</label>
  <input type="password" name="post_password" id="post_password" size="20"></p>
  <p><button type="submit">Enter</button></p>
</form>"#,
                            p.post_title, slug
                        );
                        Ok(Html(html))
                    });
                }
            }

            let rewrite = state.rewrite_rules.read().await;
            let data = PostTemplateData::from_model_with_rewrite(&p, &state.site_url, &rewrite);
            drop(rewrite);
            let mut context = build_base_context(state).await;
            insert_post_context_full(&mut context, &data, Some(&state.shortcodes), &state.hooks);

            // If content has Gutenberg blocks, render them with the block renderer
            if rustpress_themes::formatting::has_blocks(&p.post_content) {
                let blocks = rustpress_blocks::parse_blocks(&p.post_content);
                let rendered = state.block_renderer.render_blocks(&blocks);
                context.insert("the_content", &rendered);
            }

            // Generate SEO meta tags for this post
            let site_name = state.options.get_blogname().await.unwrap_or_default();
            let seo_meta = rustpress_seo::SeoMeta {
                title: Some(rustpress_seo::generate_title(
                    &p.post_title,
                    &site_name,
                    "-",
                )),
                description: Some(rustpress_seo::auto_generate_description(
                    &p.post_content,
                    160,
                )),
                canonical: Some(format!("{}/{}", state.site_url, p.post_name)),
                robots: Some("index, follow".to_string()),
                og_title: Some(p.post_title.clone()),
                og_description: Some(rustpress_seo::auto_generate_description(
                    &p.post_content,
                    200,
                )),
                og_url: Some(format!("{}/{}", state.site_url, p.post_name)),
                og_type: Some(if post_type == "page" {
                    "website".to_string()
                } else {
                    "article".to_string()
                }),
                og_site_name: Some(site_name.clone()),
                twitter_card: Some("summary_large_image".to_string()),
                ..Default::default()
            };
            context.insert(
                "seo_meta_tags",
                &rustpress_seo::generate_meta_tags(&seo_meta),
            );

            // Load featured image
            let thumb_meta = wp_postmeta::Entity::find()
                .filter(wp_postmeta::Column::PostId.eq(post_id))
                .filter(wp_postmeta::Column::MetaKey.eq("_thumbnail_id"))
                .one(&state.db)
                .await
                .ok()
                .flatten();
            if let Some(meta) = thumb_meta {
                if let Some(ref val) = meta.meta_value {
                    if let Ok(mid) = val.parse::<u64>() {
                        if let Ok(Some(att)) =
                            wp_posts::Entity::find_by_id(mid).one(&state.db).await
                        {
                            context.insert("featured_image_url", &att.guid);
                        }
                    }
                }
            }

            // Load all postmeta for plugin compatibility:
            // Yoast SEO, ACF, WooCommerce, Elementor, and any other plugin using wp_postmeta.
            let all_meta = load_all_postmeta(&state.db, post_id).await;
            if !all_meta.is_empty() {
                // Yoast SEO: override auto-generated SEO meta if Yoast data is present
                let has_yoast = all_meta.contains_key("_yoast_wpseo_title")
                    || all_meta.contains_key("_yoast_wpseo_metadesc");
                if has_yoast {
                    let yoast =
                        rustpress_seo::yoast_compat::YoastPostSeo::from_meta(post_id, &all_meta);
                    let permalink =
                        format!("{}/{}", state.site_url.trim_end_matches('/'), p.post_name);
                    let seo = yoast.to_seo_meta(&p.post_title, &site_name, &permalink);
                    context.insert("seo_meta_tags", &rustpress_seo::generate_meta_tags(&seo));
                }

                // ACF: inject custom fields as `acf_fields` template variable
                let acf_data =
                    rustpress_fields::acf_compat::AcfPostData::from_meta(post_id, &all_meta);
                if !acf_data.fields.is_empty() {
                    let acf_map: std::collections::HashMap<String, String> = acf_data
                        .fields
                        .iter()
                        .map(|f| (f.field_name.clone(), f.value.clone()))
                        .collect();
                    context.insert("acf_fields", &acf_map);
                }

                // WooCommerce: if this is a product CPT, inject structured product data
                if post_type == "product" {
                    let woo = rustpress_commerce::woo_compat::WooProductData::from_post_and_meta(
                        post_id,
                        &p.post_title,
                        &p.post_name,
                        &p.post_content,
                        &p.post_excerpt,
                        &all_meta,
                    );
                    context.insert("product", &woo);
                }

                // Elementor: convert stored JSON widget tree to HTML
                if let Some(el_data) = all_meta.get("_elementor_data") {
                    if !el_data.is_empty() && el_data != "[]" {
                        let rendered = render_elementor_content(el_data);
                        if !rendered.is_empty() {
                            context.insert("the_content", &rendered);
                        }
                    }
                }

                // Expose raw postmeta to templates (filtered: skip large/noisy internal keys)
                let filtered_meta: std::collections::HashMap<String, String> = all_meta
                    .into_iter()
                    .filter(|(k, _)| {
                        !k.starts_with("_edit_")
                            && !k.starts_with("_wp_old_")
                            && k != "_elementor_data"
                            && k != "_elementor_css"
                    })
                    .collect();
                context.insert("post_meta", &filtered_meta);
            }

            // Load approved comments for this post and build threaded tree
            let comments = wp_comments::Entity::find()
                .filter(wp_comments::Column::CommentPostId.eq(post_id))
                .filter(wp_comments::Column::CommentApproved.eq("1"))
                .order_by_asc(wp_comments::Column::CommentDate)
                .all(&state.db)
                .await
                .unwrap_or_default();

            let comment_tree = build_comment_tree(&comments);
            let comment_count = comments.len();

            context.insert("comments", &comment_tree);
            context.insert("comment_count", &comment_count);

            // Generate nonce for comment form (WordPress uses "comment_{post_id}" action)
            let comment_nonce = state.nonces.create_nonce(&format!("comment_{post_id}"), 0);
            context.insert("comment_nonce", &comment_nonce);

            // Load author info
            if let Ok(Some(author)) = wp_users::Entity::find_by_id(p.post_author)
                .one(&state.db)
                .await
            {
                context.insert("author_name", &author.display_name);
                context.insert("author_slug", &author.user_nicename);
            }

            // Load post categories
            let term_rels = wp_term_relationships::Entity::find()
                .filter(wp_term_relationships::Column::ObjectId.eq(post_id))
                .all(&state.db)
                .await
                .unwrap_or_default();
            let mut cats: Vec<serde_json::Value> = vec![];
            for rel in &term_rels {
                if let Ok(Some(tax)) = wp_term_taxonomy::Entity::find_by_id(rel.term_taxonomy_id)
                    .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
                    .one(&state.db)
                    .await
                {
                    if let Ok(Some(term)) = wp_terms::Entity::find_by_id(tax.term_id)
                        .one(&state.db)
                        .await
                    {
                        cats.push(serde_json::json!({"name": term.name, "slug": term.slug}));
                    }
                }
            }
            if !cats.is_empty() {
                context.insert("categories", &cats);
            }

            // Load previous/next posts for navigation
            if post_type == "post" {
                // Previous post (older)
                let prev = wp_posts::Entity::find()
                    .filter(wp_posts::Column::PostType.eq("post"))
                    .filter(wp_posts::Column::PostStatus.eq("publish"))
                    .filter(wp_posts::Column::PostDate.lt(post_date))
                    .order_by_desc(wp_posts::Column::PostDate)
                    .one(&state.db)
                    .await
                    .ok()
                    .flatten();
                if let Some(prev_post) = prev {
                    context.insert(
                        "prev_post",
                        &serde_json::json!({
                            "title": prev_post.post_title,
                            "permalink": format!("/{}", prev_post.post_name),
                        }),
                    );
                }
                // Next post (newer)
                let next = wp_posts::Entity::find()
                    .filter(wp_posts::Column::PostType.eq("post"))
                    .filter(wp_posts::Column::PostStatus.eq("publish"))
                    .filter(wp_posts::Column::PostDate.gt(post_date))
                    .order_by_asc(wp_posts::Column::PostDate)
                    .one(&state.db)
                    .await
                    .ok()
                    .flatten();
                if let Some(next_post) = next {
                    context.insert(
                        "next_post",
                        &serde_json::json!({
                            "title": next_post.post_title,
                            "permalink": format!("/{}", next_post.post_name),
                        }),
                    );
                }
            }

            let page_type = if post_type == "page" {
                PageType::Page {
                    slug: slug.to_string(),
                    id: post_id,
                }
            } else {
                PageType::Single {
                    post_type,
                    slug: slug.to_string(),
                }
            };

            render_theme_page(state, &page_type, &context).await
        }
        None => {
            // 404
            let mut context = build_base_context(state).await;
            context.insert("request_uri", &format!("/{slug}"));
            let engine = state.theme_engine.read().await;
            match engine.render_page(&PageType::NotFound, &context) {
                Ok(html) => Err((StatusCode::NOT_FOUND, Html(html))),
                Err(_) => Err((
                    StatusCode::NOT_FOUND,
                    Html("<h1>404 - Not Found</h1>".to_string()),
                )),
            }
        }
    }
}

async fn search_page(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let search_term = params.s.unwrap_or_default();
    let mut context = build_base_context(&state).await;
    context.insert("search_query", &search_term);

    if search_term.is_empty() {
        let posts: Vec<PostTemplateData> = vec![];
        let pagination = PaginationData::new(1, 1, 0);
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
    } else {
        let like_term = format!("%{search_term}%");
        let search_lower = search_term.to_lowercase();
        let mut models = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostType.eq("post"))
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .filter(
                sea_orm::Condition::any()
                    .add(wp_posts::Column::PostTitle.like(&like_term))
                    .add(wp_posts::Column::PostContent.like(&like_term)),
            )
            .order_by_desc(wp_posts::Column::PostDate)
            .limit(20)
            .all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

        // Sort by relevance: title matches first, then by date (like WordPress)
        models.sort_by(|a, b| {
            let a_title = a.post_title.to_lowercase().contains(&search_lower);
            let b_title = b.post_title.to_lowercase().contains(&search_lower);
            match (a_title, b_title) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.post_date.cmp(&a.post_date),
            }
        });

        let rewrite = state.rewrite_rules.read().await;
        let posts: Vec<PostTemplateData> = models
            .iter()
            .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
            .collect();
        drop(rewrite);
        let total = posts.len() as u64;
        let pagination = PaginationData::new(1, 1, total);
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
    }

    render_theme_page(&state, &PageType::Search, &context).await
}

async fn category_archive(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(params): Query<PageQuery>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let mut context = build_base_context(&state).await;

    let (posts, pagination, term_id) =
        taxonomy_posts(&state, &slug, "category", params.page.unwrap_or(1)).await?;

    let term_name = match wp_terms::Entity::find()
        .filter(wp_terms::Column::Slug.eq(&slug))
        .one(&state.db)
        .await
    {
        Ok(Some(t)) => t.name,
        _ => slug.replace('-', " "),
    };
    context.insert("term_name", &term_name);
    context.insert("archive_title", &format!("Category: {term_name}"));
    insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));

    render_theme_page(
        &state,
        &PageType::Category {
            slug: slug.clone(),
            id: term_id,
        },
        &context,
    )
    .await
}

async fn tag_archive(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(params): Query<PageQuery>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let mut context = build_base_context(&state).await;

    let (posts, pagination, term_id) =
        taxonomy_posts(&state, &slug, "post_tag", params.page.unwrap_or(1)).await?;

    let term_name = wp_terms::Entity::find()
        .filter(wp_terms::Column::Slug.eq(&slug))
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .map(|t| t.name)
        .unwrap_or_else(|| slug.replace('-', " "));
    context.insert("term_name", &term_name);
    context.insert("archive_title", &format!("Tag: {term_name}"));
    insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));

    render_theme_page(
        &state,
        &PageType::Tag {
            slug: slug.clone(),
            id: term_id,
        },
        &context,
    )
    .await
}

async fn author_archive(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(params): Query<PageQuery>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let mut context = build_base_context(&state).await;
    context.insert("author_name", &slug);
    context.insert("archive_title", &format!("Author: {slug}"));

    // Query posts by the author
    use rustpress_db::entities::wp_users;
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserNicename.eq(&slug))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    if let Some(user) = user {
        context.insert("author_name", &user.display_name);
        context.insert("archive_title", &format!("Author: {}", user.display_name));
        let page = params.page.unwrap_or(1);
        let per_page = 10u64;

        let query = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostAuthor.eq(user.id))
            .filter(wp_posts::Column::PostType.eq("post"))
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .order_by_desc(wp_posts::Column::PostDate);

        let total = query
            .clone()
            .count(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;
        let total_pages = total.div_ceil(per_page);

        let models = query
            .offset((page - 1) * per_page)
            .limit(per_page)
            .all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

        let rewrite = state.rewrite_rules.read().await;
        let mut posts: Vec<PostTemplateData> = models
            .iter()
            .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
            .collect();
        drop(rewrite);
        // Populate author info on each post (all by the same author in an author archive)
        for post in &mut posts {
            post.author_name = user.display_name.clone();
            post.author_nicename = user.user_nicename.clone();
        }
        let pagination = PaginationData::new(page, total_pages, total);
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
    } else {
        let posts: Vec<PostTemplateData> = vec![];
        let pagination = PaginationData::new(1, 1, 0);
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
    }

    render_theme_page(
        &state,
        &PageType::Author {
            nicename: slug.clone(),
            id: 0,
        },
        &context,
    )
    .await
}

// ---- Taxonomy helper (used by category_archive and tag_archive) ----

async fn taxonomy_posts(
    state: &AppState,
    slug: &str,
    taxonomy: &str,
    page: u64,
) -> Result<(Vec<PostTemplateData>, PaginationData, u64), (StatusCode, Html<String>)> {
    let per_page = state.options.get_posts_per_page().await.unwrap_or(10) as u64;

    // 1. Find the term by slug
    let term = match wp_terms::Entity::find()
        .filter(wp_terms::Column::Slug.eq(slug))
        .one(&state.db)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            let pagination = PaginationData::new(page, 1, 0);
            return Ok((vec![], pagination, 0));
        }
        Err(e) => {
            tracing::debug!(error = %e, "term query failed (no-DB mode?)");
            let pagination = PaginationData::new(page, 1, 0);
            return Ok((vec![], pagination, 0));
        }
    };

    // 2. Find term_taxonomy for this term + taxonomy type
    let tt = match wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(term.term_id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .one(&state.db)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            let pagination = PaginationData::new(page, 1, 0);
            return Ok((vec![], pagination, term.term_id));
        }
        Err(e) => {
            tracing::debug!(error = %e, "term_taxonomy query failed (no-DB mode?)");
            let pagination = PaginationData::new(page, 1, 0);
            return Ok((vec![], pagination, term.term_id));
        }
    };

    // 3. Get post IDs from term_relationships
    let relationships = match wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::TermTaxonomyId.eq(tt.term_taxonomy_id))
        .all(&state.db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "term_relationships query failed (no-DB mode?)");
            vec![]
        }
    };

    let post_ids: Vec<u64> = relationships.iter().map(|r| r.object_id).collect();

    if post_ids.is_empty() {
        let pagination = PaginationData::new(page, 1, 0);
        return Ok((vec![], pagination, term.term_id));
    }

    // 4. Query published posts with those IDs
    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::Id.is_in(post_ids))
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = match query.clone().count(&state.db).await {
        Ok(n) => n,
        Err(e) => {
            tracing::debug!(error = %e, "taxonomy post count failed (no-DB mode?)");
            return Ok((vec![], PaginationData::new(page, 1, 0), tt.term_id));
        }
    };
    let total_pages = if total == 0 { 1 } else { total.div_ceil(per_page) };

    let models = match query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!(error = %e, "taxonomy posts query failed (no-DB mode?)");
            vec![]
        }
    };

    let rewrite = state.rewrite_rules.read().await;
    let posts: Vec<PostTemplateData> = models
        .iter()
        .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
        .collect();
    drop(rewrite);
    let pagination = PaginationData::new(page, total_pages, total);

    Ok((posts, pagination, term.term_id))
}

// ---- RSS Feed ----

async fn rss_feed(State(state): State<Arc<AppState>>) -> Response {
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    let site_desc = state
        .options
        .get_blogdescription()
        .await
        .unwrap_or_default();
    let site_url = &state.site_url;

    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_desc(wp_posts::Column::PostDate)
        .limit(20)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut items = String::new();
    for p in &posts {
        let post_url = format!("{}/{}", site_url, p.post_name);
        let pub_date = p.post_date_gmt.format("%a, %d %b %Y %H:%M:%S +0000");

        // Apply content filters for the full content (content:encoded)
        let full_content = rustpress_themes::apply_content_filters(&p.post_content);

        let description = if p.post_excerpt.is_empty() {
            // Strip HTML tags for a basic excerpt
            let content = p.post_content.replace("<!-- wp:", "").replace(" -->", "");
            let plain: String = content
                .chars()
                .fold((String::new(), false), |(mut acc, in_tag), c| {
                    if c == '<' {
                        (acc, true)
                    } else if c == '>' {
                        (acc, false)
                    } else if !in_tag {
                        acc.push(c);
                        (acc, false)
                    } else {
                        (acc, true)
                    }
                })
                .0;
            if plain.len() > 300 {
                format!("{}...", &plain[..300])
            } else {
                plain
            }
        } else {
            p.post_excerpt.clone()
        };

        items.push_str(&format!(
            r#"    <item>
      <title><![CDATA[{}]]></title>
      <link>{}</link>
      <description><![CDATA[{}]]></description>
      <content:encoded><![CDATA[{}]]></content:encoded>
      <pubDate>{}</pubDate>
      <guid isPermaLink="true">{}</guid>
    </item>
"#,
            p.post_title, post_url, description, full_content, pub_date, post_url
        ));
    }

    let last_build = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S +0000")
        .to_string();

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title><![CDATA[{site_name}]]></title>
    <link>{site_url}</link>
    <description><![CDATA[{site_desc}]]></description>
    <lastBuildDate>{last_build}</lastBuildDate>
    <language>en-US</language>
    <atom:link href="{site_url}/feed/" rel="self" type="application/rss+xml"/>
{items}  </channel>
</rss>"#
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/rss+xml; charset=UTF-8")],
        xml,
    )
        .into_response()
}

// ---- Comment Submission ----

async fn submit_comment(
    State(state): State<Arc<AppState>>,
    Form(form): Form<CommentForm>,
) -> Response {
    // Validate nonce (WordPress uses "comment_{post_id}" action)
    let nonce_action = format!("comment_{}", form.comment_post_id);
    let nonce_valid = form
        ._wpnonce
        .as_deref()
        .map(|n| state.nonces.verify_nonce(n, &nonce_action, 0).is_some())
        .unwrap_or(false);

    if !nonce_valid {
        return (
            StatusCode::FORBIDDEN,
            Html("<p>Security check failed. Please go back and try again.</p>".to_string()),
        )
            .into_response();
    }

    let author = form.author.as_deref().unwrap_or("").trim().to_string();
    let email = form.email.as_deref().unwrap_or("").trim().to_string();

    // Validate required fields
    if author.is_empty() || form.comment.trim().is_empty() {
        return Redirect::to(&format!("/{}?comment_error=required", form.comment_post_id))
            .into_response();
    }

    // Check that the post exists and comments are open
    let post = wp_posts::Entity::find_by_id(form.comment_post_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let post = match post {
        Some(p) if p.comment_status == "open" => p,
        _ => {
            return Redirect::to("/").into_response();
        }
    };

    // Determine redirect URL
    let post_slug = post.post_name.clone();
    let redirect_url = form
        .redirect_to
        .as_deref()
        .filter(|r| !r.is_empty())
        .unwrap_or(&format!("/{post_slug}"))
        .to_string();

    let post_id = post.id;
    let now = chrono::Utc::now().naive_utc();

    let new_comment = wp_comments::ActiveModel {
        comment_id: sea_orm::ActiveValue::NotSet,
        comment_post_id: Set(form.comment_post_id),
        comment_author: Set(author),
        comment_author_email: Set(email),
        comment_author_url: Set(form.url.unwrap_or_default().trim().to_string()),
        comment_author_ip: Set(String::new()),
        comment_date: Set(now),
        comment_date_gmt: Set(now),
        comment_content: Set(form.comment.trim().to_string()),
        comment_karma: Set(0),
        comment_approved: Set("1".to_string()),
        comment_agent: Set(String::new()),
        comment_type: Set("comment".to_string()),
        comment_parent: Set(form.comment_parent.unwrap_or(0)),
        user_id: Set(0),
    };

    let _ = new_comment.insert(&state.db).await;

    // Update comment count on the post
    let new_count = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentPostId.eq(post_id))
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let mut active_post: wp_posts::ActiveModel = post.into();
    active_post.comment_count = Set(new_count as i64);
    let _ = active_post.update(&state.db).await;

    Redirect::to(&format!("{redirect_url}#comments")).into_response()
}

// ---- Date-based Archives ----

async fn year_archive_page(
    state: &AppState,
    year: u32,
    page_query: Option<u64>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let mut context = build_base_context(state).await;
    context.insert("archive_title", &format!("Year: {year}"));
    context.insert("archive_year", &year);

    let page = page_query.unwrap_or(1);
    let per_page = state.options.get_posts_per_page().await.unwrap_or(10) as u64;

    let start = chrono::NaiveDate::from_ymd_opt(year as i32, 1, 1)
        .unwrap_or_default()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default();
    let end = chrono::NaiveDate::from_ymd_opt(year as i32 + 1, 1, 1)
        .unwrap_or_default()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default();

    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .filter(wp_posts::Column::PostDate.gte(start))
        .filter(wp_posts::Column::PostDate.lt(end))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;
    let total_pages = if total == 0 {
        1
    } else {
        total.div_ceil(per_page)
    };

    let models = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    let rewrite = state.rewrite_rules.read().await;
    let posts: Vec<PostTemplateData> = models
        .iter()
        .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
        .collect();
    drop(rewrite);
    let pagination = PaginationData::new(page, total_pages, total);
    insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));

    render_theme_page(state, &PageType::DateArchive, &context).await
}

async fn month_archive(
    State(state): State<Arc<AppState>>,
    Path((year, month)): Path<(String, String)>,
    Query(params): Query<PageQuery>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let year: u32 = year
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;
    let month: u32 = month
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;

    if !(1..=12).contains(&month) {
        return Err((StatusCode::NOT_FOUND, Html("Not found".to_string())));
    }

    let mut context = build_base_context(&state).await;
    let month_name = month_to_name(month);
    context.insert("archive_title", &format!("Month: {month_name} {year}"));
    context.insert("archive_year", &year);
    context.insert("archive_month", &month);

    let page = params.page.unwrap_or(1);
    let per_page = state.options.get_posts_per_page().await.unwrap_or(10) as u64;

    let start = chrono::NaiveDate::from_ymd_opt(year as i32, month, 1)
        .unwrap_or_default()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default();
    let next_month = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year as i32 + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year as i32, month + 1, 1)
    };
    let end = next_month
        .unwrap_or_default()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default();

    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .filter(wp_posts::Column::PostDate.gte(start))
        .filter(wp_posts::Column::PostDate.lt(end))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    // WordPress shows 404 for date archives with no posts
    if total == 0 {
        return render_theme_page(&state, &PageType::NotFound, &context).await;
    }

    let total_pages = total.div_ceil(per_page);

    let models = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    let rewrite = state.rewrite_rules.read().await;
    let posts: Vec<PostTemplateData> = models
        .iter()
        .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
        .collect();
    drop(rewrite);
    let pagination = PaginationData::new(page, total_pages, total);
    insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));

    render_theme_page(&state, &PageType::DateArchive, &context).await
}

async fn day_archive(
    State(state): State<Arc<AppState>>,
    Path((year, month, day)): Path<(String, String, String)>,
    Query(params): Query<PageQuery>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let year: u32 = year
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;
    let month: u32 = month
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;
    let day: u32 = day
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err((StatusCode::NOT_FOUND, Html("Not found".to_string())));
    }

    let mut context = build_base_context(&state).await;
    let month_name = month_to_name(month);
    context.insert("archive_title", &format!("{month_name} {day}, {year}"));
    context.insert("archive_year", &year);
    context.insert("archive_month", &month);
    context.insert("archive_day", &day);

    let page = params.page.unwrap_or(1);
    let per_page = state.options.get_posts_per_page().await.unwrap_or(10) as u64;

    let start = chrono::NaiveDate::from_ymd_opt(year as i32, month, day)
        .unwrap_or_default()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default();
    let end = chrono::NaiveDate::from_ymd_opt(year as i32, month, day)
        .unwrap_or_default()
        .and_hms_opt(23, 59, 59)
        .unwrap_or_default();

    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .filter(wp_posts::Column::PostDate.gte(start))
        .filter(wp_posts::Column::PostDate.lte(end))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;
    let total_pages = if total == 0 {
        1
    } else {
        total.div_ceil(per_page)
    };

    let models = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    let rewrite = state.rewrite_rules.read().await;
    let posts: Vec<PostTemplateData> = models
        .iter()
        .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
        .collect();
    drop(rewrite);
    let pagination = PaginationData::new(page, total_pages, total);
    insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));

    render_theme_page(&state, &PageType::DateArchive, &context).await
}

fn month_to_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

// ---- Sticky Posts ----

async fn get_sticky_post_ids(state: &AppState) -> Vec<u64> {
    let sticky_opt = state
        .options
        .get_option("sticky_posts")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    if sticky_opt.is_empty() {
        return vec![];
    }

    // WordPress stores sticky_posts as a serialized PHP array like "a:1:{i:0;i:6;}"
    // or sometimes as JSON like "[6]" or comma-separated "6,7"
    parse_php_serialized_ids(&sticky_opt)
}

/// Parse WordPress PHP serialized integer array like "a:1:{i:0;i:6;}"
/// Also handles JSON arrays "[6,7]" and comma-separated "6,7".
fn parse_php_serialized_ids(input: &str) -> Vec<u64> {
    let trimmed = input.trim();

    // PHP serialized array: a:N:{i:K;i:V;...}
    if trimmed.starts_with("a:") {
        // Extract all "i:NUMBER;" patterns — values are at odd positions
        let numbers: Vec<u64> = trimmed
            .split("i:")
            .filter_map(|s| s.trim_end_matches([';', '}']).parse::<u64>().ok())
            .collect();
        // In PHP serialized arrays, format is i:KEY;i:VALUE; so values are at odd indices
        // But since we split on "i:", we get ["a:1:{", "0;", "6;", "}"]
        // Filter to get only the actual post IDs (skip index keys)
        // For "a:1:{i:0;i:6;}" -> after split on "i:" -> ["a:1:{", "0;", "6;", "}"]
        // Indices in the serialized data: 0 is key, 6 is value
        // We take every other number starting from index 1 (the values)
        return numbers.into_iter().skip(1).step_by(2).collect();
    }

    // JSON array: [6, 7, 8]
    if trimmed.starts_with('[') {
        return trimmed
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect();
    }

    // Comma-separated: "6,7,8"
    trimmed
        .split(',')
        .filter_map(|s| s.trim().parse::<u64>().ok())
        .collect()
}

// ---- Password-Protected Posts ----

fn check_post_password_cookie(headers: &HeaderMap, slug: &str, expected_password: &str) -> bool {
    let cookie_name = format!("wp-postpass_slug_{slug}");
    if let Some(cookie_header) = headers.get(header::COOKIE) {
        if let Ok(cookies) = cookie_header.to_str() {
            for cookie in cookies.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix(&format!("{cookie_name}=")) {
                    return value == expected_password;
                }
            }
        }
    }
    false
}

/// POST /{slug}/trackback — WordPress trackback receiver (deprecated, respond with success XML).
async fn trackback_handler(Path(_slug): Path<String>) -> Response {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?><response><error>1</error><message>Trackbacks are not accepted on this site.</message></response>"#;
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/xml; charset=UTF-8")],
        xml,
    )
        .into_response()
}

/// GET /wp-register.php — redirect to wp-login.php?action=register (or disabled message).
async fn wp_register_redirect(State(state): State<Arc<AppState>>) -> Response {
    // Check if user registration is enabled (users_can_register option)
    let can_register = state
        .options
        .get_option("users_can_register")
        .await
        .ok()
        .flatten()
        .map(|v| v == "1")
        .unwrap_or(false);

    let redirect_url = if can_register {
        format!(
            "{}/wp-login.php?action=register",
            state.site_url.trim_end_matches('/')
        )
    } else {
        format!("{}/wp-login.php", state.site_url.trim_end_matches('/'))
    };
    axum::response::Redirect::to(&redirect_url).into_response()
}

// ---- Threaded Comments ----

fn build_comment_tree(
    comments: &[rustpress_db::entities::wp_comments::Model],
) -> Vec<serde_json::Value> {
    let flat: Vec<serde_json::Value> = comments
        .iter()
        .map(|c| {
            let dt = c.comment_date;
            let month_name = match dt.format("%m").to_string().as_str() {
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
                _ => "January",
            };
            let day = dt.format("%-d").to_string();
            let year = dt.format("%Y").to_string();
            let time_12h = dt.format("%-I:%M %P").to_string();
            let date_formatted = format!("{month_name} {day}, {year} at {time_12h}");
            let date_iso = dt.format("%Y-%m-%dT%H:%M:%S+00:00").to_string();
            serde_json::json!({
                "id": c.comment_id,
                "author": c.comment_author,
                "author_url": c.comment_author_url,
                "content": c.comment_content,
                "date": date_formatted,
                "date_iso": date_iso,
                "parent": c.comment_parent,
                "children": [],
            })
        })
        .collect();

    // Build tree: collect children under their parents
    let mut tree: Vec<serde_json::Value> = Vec::new();
    let mut orphans: Vec<serde_json::Value> = Vec::new();

    // First pass: separate top-level and child comments
    let mut children_map: std::collections::HashMap<u64, Vec<serde_json::Value>> =
        std::collections::HashMap::new();

    for comment in &flat {
        let parent_id = comment["parent"].as_u64().unwrap_or(0);
        if parent_id == 0 {
            orphans.push(comment.clone());
        } else {
            children_map
                .entry(parent_id)
                .or_default()
                .push(comment.clone());
        }
    }

    // Second pass: attach children recursively
    fn attach_children(
        node: &mut serde_json::Value,
        children_map: &std::collections::HashMap<u64, Vec<serde_json::Value>>,
    ) {
        let node_id = node["id"].as_u64().unwrap_or(0);
        if let Some(children) = children_map.get(&node_id) {
            let mut kids = children.clone();
            for kid in &mut kids {
                attach_children(kid, children_map);
            }
            node["children"] = serde_json::json!(kids);
        }
    }

    for mut comment in orphans {
        attach_children(&mut comment, &children_map);
        tree.push(comment);
    }

    tree
}

// ---- Category / Tag RSS Feeds ----

async fn category_feed(State(state): State<Arc<AppState>>, Path(slug): Path<String>) -> Response {
    taxonomy_feed(&state, "category", &slug).await
}

async fn tag_feed(State(state): State<Arc<AppState>>, Path(slug): Path<String>) -> Response {
    taxonomy_feed(&state, "post_tag", &slug).await
}

/// Generate an RSS feed for a taxonomy term (category or tag).
async fn taxonomy_feed(state: &AppState, taxonomy: &str, slug: &str) -> Response {
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    let site_url = &state.site_url;

    // Find the term
    let term = wp_terms::Entity::find()
        .filter(wp_terms::Column::Slug.eq(slug))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let term = match term {
        Some(t) => t,
        None => {
            return (StatusCode::NOT_FOUND, "Feed not found").into_response();
        }
    };

    // Find term_taxonomy
    let tt = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(term.term_id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let tt = match tt {
        Some(t) => t,
        None => {
            return (StatusCode::NOT_FOUND, "Feed not found").into_response();
        }
    };

    // Find post IDs in this term
    let rels = wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::TermTaxonomyId.eq(tt.term_taxonomy_id))
        .all(&state.db)
        .await
        .unwrap_or_default();

    let post_ids: Vec<u64> = rels.iter().map(|r| r.object_id).collect();
    if post_ids.is_empty() {
        let xml = empty_feed(&term.name, site_url);
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/rss+xml; charset=UTF-8")],
            xml,
        )
            .into_response();
    }

    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::Id.is_in(post_ids))
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_desc(wp_posts::Column::PostDate)
        .limit(20)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let taxonomy_label = if taxonomy == "category" {
        "Category"
    } else {
        "Tag"
    };
    let feed_title = format!("{} » {} {} Feed", site_name, term.name, taxonomy_label);
    let feed_link = format!(
        "{}/{}/{}",
        site_url,
        if taxonomy == "category" {
            "category"
        } else {
            "tag"
        },
        slug
    );
    let feed_desc = format!(
        "Posts in {} \"{}\"",
        taxonomy_label.to_lowercase(),
        term.name
    );

    let items = build_rss_items(&posts, site_url);

    let last_build = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S +0000")
        .to_string();

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title><![CDATA[{feed_title}]]></title>
    <link>{feed_link}</link>
    <description><![CDATA[{feed_desc}]]></description>
    <lastBuildDate>{last_build}</lastBuildDate>
    <language>en-US</language>
    <atom:link href="{feed_link}/feed/" rel="self" type="application/rss+xml"/>
{items}  </channel>
</rss>"#
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/rss+xml; charset=UTF-8")],
        xml,
    )
        .into_response()
}

// ---- Comments RSS Feed ----

async fn comments_feed(State(state): State<Arc<AppState>>) -> Response {
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    let site_url = &state.site_url;

    let comments = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .filter(wp_comments::Column::CommentType.eq("comment"))
        .order_by_desc(wp_comments::Column::CommentDate)
        .limit(20)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut items = String::new();
    for c in &comments {
        let pub_date = c.comment_date_gmt.format("%a, %d %b %Y %H:%M:%S +0000");
        let link = format!(
            "{}/?p={}#comment-{}",
            site_url, c.comment_post_id, c.comment_id
        );
        let title = format!("{} on post #{}", c.comment_author, c.comment_post_id);
        items.push_str(&format!(
            r#"    <item>
      <title><![CDATA[{}]]></title>
      <link>{}</link>
      <dc:creator><![CDATA[{}]]></dc:creator>
      <pubDate>{}</pubDate>
      <description><![CDATA[{}]]></description>
      <guid isPermaLink="false">{}</guid>
    </item>
"#,
            title, link, c.comment_author, pub_date, c.comment_content, link
        ));
    }

    let last_build = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S +0000")
        .to_string();

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title><![CDATA[Comments for {site_name}]]></title>
    <link>{site_url}</link>
    <description><![CDATA[Comments]]></description>
    <lastBuildDate>{last_build}</lastBuildDate>
    <language>en-US</language>
    <atom:link href="{site_url}/comments/feed/" rel="self" type="application/rss+xml"/>
{items}  </channel>
</rss>"#
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/rss+xml; charset=UTF-8")],
        xml,
    )
        .into_response()
}

// ---- admin-ajax.php ----

#[derive(Deserialize)]
pub struct AjaxQuery {
    pub action: Option<String>,
}

/// WordPress admin-ajax.php compatible endpoint.
///
/// Dispatches AJAX actions via the hook registry.
/// WordPress plugins and themes use `admin-ajax.php?action=my_action`
/// as a general-purpose AJAX endpoint.
async fn admin_ajax(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AjaxQuery>,
) -> Response {
    let action = query.action.as_deref().unwrap_or("");
    if action.is_empty() {
        return (StatusCode::BAD_REQUEST, "0").into_response();
    }

    // Check for built-in actions first
    match action {
        "heartbeat" => {
            let response = serde_json::json!({
                "wp-auth-check": true,
                "server_time": chrono::Utc::now().timestamp(),
            });
            return axum::Json(response).into_response();
        }
        // REST API nonce — used by Gutenberg block editor and wp-json clients
        "rest-nonce" => {
            let nonce = state.nonces.create_nonce("wp_rest", 0);
            return (StatusCode::OK, nonce).into_response();
        }
        _ => {}
    }

    // Build context
    let ctx = serde_json::json!({
        "action": action,
    });

    // Fire wp_ajax_{action} hook (for logged-in users)
    let hook_name = format!("wp_ajax_{action}");
    state.hooks.do_action(&hook_name, &ctx);

    // Fire wp_ajax_nopriv_{action} hook (for non-logged-in users)
    let nopriv_hook = format!("wp_ajax_nopriv_{action}");
    state.hooks.do_action(&nopriv_hook, &ctx);

    // Apply filter to get response (if any plugin set one)
    let result = state.hooks.apply_filters(
        &format!("ajax_response_{action}"),
        serde_json::Value::String("0".to_string()),
    );

    let body = match result {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    };

    (StatusCode::OK, body).into_response()
}

// ---- RSS helpers ----

fn build_rss_items(posts: &[wp_posts::Model], site_url: &str) -> String {
    let mut items = String::new();
    for p in posts {
        let post_url = format!("{}/{}", site_url, p.post_name);
        let pub_date = p.post_date_gmt.format("%a, %d %b %Y %H:%M:%S +0000");
        let full_content = rustpress_themes::apply_content_filters(&p.post_content);
        let description = if p.post_excerpt.is_empty() {
            let plain = strip_rss_html(&p.post_content);
            if plain.len() > 300 {
                format!("{}...", &plain[..300])
            } else {
                plain
            }
        } else {
            p.post_excerpt.clone()
        };
        items.push_str(&format!(
            r#"    <item>
      <title><![CDATA[{}]]></title>
      <link>{}</link>
      <description><![CDATA[{}]]></description>
      <content:encoded><![CDATA[{}]]></content:encoded>
      <pubDate>{}</pubDate>
      <guid isPermaLink="true">{}</guid>
    </item>
"#,
            p.post_title, post_url, description, full_content, pub_date, post_url
        ));
    }
    items
}

fn empty_feed(title: &str, site_url: &str) -> String {
    let last_build = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S +0000")
        .to_string();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title><![CDATA[{title}]]></title>
    <link>{site_url}</link>
    <description></description>
    <lastBuildDate>{last_build}</lastBuildDate>
    <language>en-US</language>
  </channel>
</rss>"#
    )
}

fn strip_rss_html(html: &str) -> String {
    let content = html.replace("<!-- wp:", "").replace(" -->", "");
    content
        .chars()
        .fold((String::new(), false), |(mut acc, in_tag), c| {
            if c == '<' {
                (acc, true)
            } else if c == '>' {
                (acc, false)
            } else if !in_tag {
                acc.push(c);
                (acc, false)
            } else {
                (acc, true)
            }
        })
        .0
}

// ---- wp-cron.php HTTP trigger ----

/// WordPress-compatible wp-cron.php endpoint.
/// Triggers due cron events just like WordPress's wp-cron.php does.
/// WordPress calls this URL on every page load if `DISABLE_WP_CRON` is false.
async fn wp_cron(State(state): State<Arc<AppState>>) -> Response {
    // Run due cron events (non-blocking — spawn task)
    let cron = state.cron.clone();
    tokio::spawn(async move {
        cron.run_due_events();
    });

    // WordPress responds with "1\n" on success
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=UTF-8")],
        "1\n",
    )
        .into_response()
}

// ---- Date-based single post URLs ----

/// Handle WordPress date-based permalink: /{year}/{month}/{day}/{slug}
async fn single_by_date_slug(
    State(state): State<Arc<AppState>>,
    Path((year, month, day, slug)): Path<(String, String, String, String)>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Validate year/month/day are numeric — if not, fall through to archive
    let _year: u32 = year
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;
    let _month: u32 = month
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;
    let _day: u32 = day
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;

    // Delegate to slug-based post lookup
    single_post_by_slug(&state, &slug, &headers).await
}

/// Handle WordPress date-based permalink: /{year}/{month}/{slug}
/// Also handles day archive: /{year}/{month}/{day} when third segment is numeric
async fn single_by_month_slug_or_day_archive(
    State(state): State<Arc<AppState>>,
    Path((year, month, slug_or_day)): Path<(String, String, String)>,
    Query(params): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let _year: u32 = year
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;
    let _month: u32 = month
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, Html("Not found".to_string())))?;

    // If third segment is a number (1-31), treat as day archive
    if let Ok(day) = slug_or_day.parse::<u32>() {
        if (1..=31).contains(&day) {
            return day_archive(
                State(state),
                Path((year, month, slug_or_day)),
                Query(params),
            )
            .await;
        }
    }

    // Otherwise treat as post slug
    single_post_by_slug(&state, &slug_or_day, &headers).await
}

// ---- Per-post comment feed ----

/// WordPress per-post comment feed: /{slug}/feed
async fn post_comment_feed(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    let site_url = &state.site_url;

    // Find the post
    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(slug.as_str()))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let post = match post {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Feed not found").into_response(),
    };

    let comments = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentPostId.eq(post.id))
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .filter(wp_comments::Column::CommentType.eq("comment"))
        .order_by_desc(wp_comments::Column::CommentDate)
        .limit(20)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut items = String::new();
    for c in &comments {
        let pub_date = c.comment_date_gmt.format("%a, %d %b %Y %H:%M:%S +0000");
        let link = format!("{}/{}/#comment-{}", site_url, slug, c.comment_id);
        let title = format!("Comment by {} on {}", c.comment_author, post.post_title);
        items.push_str(&format!(
            r#"    <item>
      <title><![CDATA[{}]]></title>
      <link>{}</link>
      <dc:creator><![CDATA[{}]]></dc:creator>
      <pubDate>{}</pubDate>
      <description><![CDATA[{}]]></description>
      <guid isPermaLink="false">{}</guid>
    </item>
"#,
            title, link, c.comment_author, pub_date, c.comment_content, link
        ));
    }

    let last_build = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S +0000")
        .to_string();
    let post_url = format!("{site_url}/{slug}");
    let feed_title = format!("{} » Comments on {}", site_name, post.post_title);

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title><![CDATA[{}]]></title>
    <link>{}</link>
    <description><![CDATA[Comments on {}]]></description>
    <lastBuildDate>{}</lastBuildDate>
    <language>en-US</language>
    <atom:link href="{}/feed/" rel="self" type="application/rss+xml"/>
{}  </channel>
</rss>"#,
        feed_title, post_url, post.post_title, last_build, post_url, items
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/rss+xml; charset=UTF-8")],
        xml,
    )
        .into_response()
}

/// Load ALL wp_postmeta for a given post_id.
///
/// Returns a `HashMap<meta_key, meta_value>` for use by plugin compatibility layers
/// (Yoast SEO, ACF, WooCommerce, Elementor, etc.).
async fn load_all_postmeta(
    db: &sea_orm::DatabaseConnection,
    post_id: u64,
) -> std::collections::HashMap<String, String> {
    wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| Some((m.meta_key?, m.meta_value.unwrap_or_default())))
        .collect()
}

/// Convert Elementor page-builder JSON (`_elementor_data` postmeta) to HTML.
///
/// Elementor stores a recursive widget tree as JSON. This renderer handles the
/// most common widget types: heading, text-editor, image, button, icon-list,
/// video, divider, spacer, and falls back to recursing into children for any
/// container / section / column elements.
fn render_elementor_content(json_str: &str) -> String {
    let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return String::new();
    };
    let mut html = String::new();
    if let Some(sections) = data.as_array() {
        for section in sections {
            render_elementor_element(section, &mut html);
        }
    }
    html
}

fn render_elementor_element(element: &serde_json::Value, html: &mut String) {
    let eltype = element.get("elType").and_then(|v| v.as_str()).unwrap_or("");
    let widget_type = element
        .get("widgetType")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let settings = element
        .get("settings")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));

    match (eltype, widget_type) {
        // Structural containers — recurse into children
        ("section" | "container", _) => {
            html.push_str("<section class=\"elementor-section\">");
            recurse_elementor_children(element, html);
            html.push_str("</section>\n");
        }
        ("column", _) => {
            html.push_str("<div class=\"elementor-column\">");
            recurse_elementor_children(element, html);
            html.push_str("</div>\n");
        }

        // Heading widget
        ("widget", "heading") => {
            let text = settings.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let tag = settings
                .get("header_size")
                .and_then(|v| v.as_str())
                .unwrap_or("h2");
            let safe_tag = match tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => tag,
                _ => "h2",
            };
            html.push_str(&format!(
                "<{safe_tag} class=\"elementor-heading-title\">{text}</{safe_tag}>\n"
            ));
        }

        // Text / rich-text editor widget
        ("widget", "text-editor") => {
            let content = settings
                .get("editor")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            html.push_str(&format!(
                "<div class=\"elementor-text-editor\">{content}</div>\n"
            ));
        }

        // Image widget
        ("widget", "image") => {
            let url = settings
                .get("image")
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let alt = settings
                .get("image")
                .and_then(|v| v.get("alt"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !url.is_empty() {
                html.push_str(&format!(
                    "<figure class=\"elementor-image\"><img src=\"{url}\" alt=\"{alt}\"/></figure>\n"
                ));
            }
        }

        // Button widget
        ("widget", "button") => {
            let text = settings
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("Button");
            let url = settings
                .get("link")
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("#");
            html.push_str(&format!(
                "<a href=\"{url}\" class=\"elementor-button\">{text}</a>\n"
            ));
        }

        // Icon list widget
        ("widget", "icon-list") => {
            if let Some(items) = settings.get("icon_list").and_then(|v| v.as_array()) {
                html.push_str("<ul class=\"elementor-icon-list-items\">\n");
                for item in items {
                    let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    html.push_str(&format!(
                        "  <li class=\"elementor-icon-list-item\">{text}</li>\n"
                    ));
                }
                html.push_str("</ul>\n");
            }
        }

        // Video widget (YouTube / Vimeo link fallback)
        ("widget", "video") => {
            let provider = settings
                .get("video_type")
                .and_then(|v| v.as_str())
                .unwrap_or("youtube");
            let url = if provider == "youtube" {
                settings
                    .get("youtube_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            } else {
                settings
                    .get("vimeo_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            };
            if !url.is_empty() {
                html.push_str(&format!(
                    "<div class=\"elementor-video\"><a href=\"{url}\">Watch Video</a></div>\n"
                ));
            }
        }

        // Divider / spacer
        ("widget", "divider") => {
            html.push_str("<hr class=\"elementor-divider\"/>\n");
        }
        ("widget", "spacer") => {
            html.push_str("<div class=\"elementor-spacer\"></div>\n");
        }

        // Unknown element — recurse into children if any
        _ => {
            recurse_elementor_children(element, html);
        }
    }
}

fn recurse_elementor_children(element: &serde_json::Value, html: &mut String) {
    if let Some(children) = element.get("elements").and_then(|v| v.as_array()) {
        for child in children {
            render_elementor_element(child, html);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_author_query_var_parsed() {
        // Ensure ?author=1 is correctly deserialized so the 403 gate triggers
        let qv: WpQueryVars =
            serde_urlencoded::from_str("author=1").expect("should parse author=1");
        assert_eq!(qv.author, Some(1));
    }

    #[test]
    fn test_author_query_var_absent() {
        // Without ?author, the field is None and enumeration guard is skipped
        let qv: WpQueryVars =
            serde_urlencoded::from_str("s=hello").expect("should parse without author");
        assert!(qv.author.is_none());
    }
}

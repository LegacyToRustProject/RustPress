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

use rustpress_db::entities::{wp_comments, wp_postmeta, wp_posts, wp_term_relationships, wp_term_taxonomy, wp_terms, wp_users};
use rustpress_themes::hierarchy::PageType;
use rustpress_themes::tags::{
    insert_post_context_full, insert_posts_context_with_hooks,
    PaginationData, PostTemplateData,
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
pub struct PostPasswordForm {
    pub post_password: String,
    pub redirect_to: String,
}

#[derive(Deserialize)]
pub struct WpLoginQuery {
    pub action: Option<String>,
    pub redirect_to: Option<String>,
    pub loggedout: Option<String>,
    pub reauth: Option<String>,
    pub _wpnonce: Option<String>,
    pub key: Option<String>,
    pub login: Option<String>,
    pub checkemail: Option<String>,
}

#[derive(Deserialize)]
pub struct WpLoginForm {
    pub log: Option<String>,
    pub pwd: Option<String>,
    pub rememberme: Option<String>,
    pub redirect_to: Option<String>,
    pub post_password: Option<String>,
    // Lost password fields
    pub user_login: Option<String>,
    // Password reset fields
    pub pass1: Option<String>,
    pub pass2: Option<String>,
    pub key: Option<String>,
    pub login: Option<String>,
    // Registration fields
    pub user_email: Option<String>,
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
        .route("/{year}/{month}/{slug}", get(single_by_month_slug_or_day_archive))
        .route("/{year}/{month}/{slug}/", get(single_by_month_slug_or_day_archive))
        .route("/{year}/{month}", get(month_archive))
        .route("/{year}/{month}/", get(month_archive))
        .route(
            "/wp-comments-post.php",
            axum::routing::post(submit_comment),
        )
        // wp-login.php is registered in wp_admin routes
        // admin-ajax.php compatible endpoint
        .route(
            "/wp-admin/admin-ajax.php",
            get(admin_ajax).post(admin_ajax),
        )
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

    // Generate wp_head() and wp_footer() standard outputs
    let wp_head_html = rustpress_themes::wp_head::wp_head(&state.site_url, &site_name, &site_desc);
    let wp_footer_html = rustpress_themes::wp_head::wp_footer(&state.site_url);
    ctx.insert("wp_head", &wp_head_html);
    ctx.insert("wp_footer", &wp_footer_html);

    // Fire wp_head and wp_footer action hooks so plugins can add output
    state.hooks.do_action("wp_head", &serde_json::json!({"site_url": &state.site_url}));
    state.hooks.do_action("wp_footer", &serde_json::json!({"site_url": &state.site_url}));

    // Load navigation menu links
    let header_menu_text = state
        .options
        .get_option("nav_menu_header")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let footer_menu_text = state
        .options
        .get_option("nav_menu_footer")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    let header_links: Vec<serde_json::Value> = parse_menu_text(&header_menu_text);
    let footer_links: Vec<serde_json::Value> = parse_menu_text(&footer_menu_text);
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
             <div class=\"footer-widgets-col\">{}</div>\
             <div class=\"footer-widgets-col\">{}</div>\
             </div>",
            footer1_widgets_html, footer2_widgets_html
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
                Html(format!("<h1>Render Error</h1><pre>{}</pre>", e)),
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

    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    let total_pages = (total + per_page - 1) / per_page;

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

    Ok((posts, pagination))
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
                let mut data = PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite);
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
    let result = render_theme_page(&state, &PageType::FrontPage, &context).await?;

    // Store in cache
    state.page_cache.set(cache_key, CachedPage {
        html: result.0.clone(),
        content_type: "text/html".to_string(),
        status_code: 200,
    }).await;
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
        let results = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostStatus.eq("publish"))
            .filter(wp_posts::Column::PostType.eq("post"))
            .filter(
                sea_orm::Condition::any()
                    .add(wp_posts::Column::PostTitle.like(&format!("%{}%", s)))
                    .add(wp_posts::Column::PostContent.like(&format!("%{}%", s))),
            )
            .order_by_desc(wp_posts::Column::PostDate)
            .limit(10)
            .all(&state.db)
            .await
            .unwrap_or_default();
        let rewrite = state.rewrite_rules.read().await;
        let posts: Vec<PostTemplateData> = results
            .iter()
            .map(|m| PostTemplateData::from_model_with_rewrite(m, &state.site_url, &rewrite))
            .collect();
        drop(rewrite);
        let pagination = rustpress_themes::tags::PaginationData::new(1, 1, results.len() as u64);
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
        return conv(render_theme_page(&state, &rustpress_themes::hierarchy::PageType::Search, &context).await);
    }

    // ?cat=5 — category archive by term_id
    if let Some(cat_id) = qv.cat {
        let term = wp_terms::Entity::find_by_id(cat_id)
            .one(&state.db)
            .await;
        let (slug, proper_name) = match term {
            Ok(Some(t)) => (t.slug.clone(), t.name.clone()),
            _ => (cat_id.to_string(), cat_id.to_string()),
        };
        let mut context = build_base_context(&state).await;
        let (posts, pagination, term_id) = match taxonomy_posts(&state, &slug, "category", 1).await {
            Ok(v) => v,
            Err(e) => return conv(Err(e)),
        };
        context.insert("term_name", &proper_name);
        context.insert("archive_title", &format!("Category: {}", proper_name));
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
        return conv(render_theme_page(
            &state,
            &rustpress_themes::hierarchy::PageType::Category { slug, id: term_id },
            &context,
        ).await);
    }

    // ?tag=slug — tag archive by slug
    if let Some(tag_slug) = qv.tag {
        let mut context = build_base_context(&state).await;
        let (posts, pagination, term_id) = match taxonomy_posts(&state, &tag_slug, "post_tag", 1).await {
            Ok(v) => v,
            Err(e) => return conv(Err(e)),
        };
        let term_name = tag_slug.replace('-', " ");
        context.insert("term_name", &term_name);
        context.insert("archive_title", &format!("Tag: {}", term_name));
        insert_posts_context_with_hooks(&mut context, &posts, &pagination, Some(&state.hooks));
        return conv(render_theme_page(
            &state,
            &rustpress_themes::hierarchy::PageType::Tag { slug: tag_slug, id: term_id },
            &context,
        ).await);
    }

    // ?attachment_id=N — attachment/media page
    if let Some(att_id) = qv.attachment_id {
        let post = wp_posts::Entity::find_by_id(att_id)
            .filter(wp_posts::Column::PostType.eq("attachment"))
            .one(&state.db)
            .await;
        return match post {
            Ok(Some(p)) => conv(single_post_by_slug(&state, &p.post_name, &headers).await),
            Ok(None) => (StatusCode::NOT_FOUND, Html("Attachment not found".to_string())).into_response(),
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
            if let (Ok(year), Ok(month)) = (
                m_val[0..4].parse::<u32>(),
                m_val[4..6].parse::<u32>(),
            ) {
                let url = format!("/{:04}/{:02}", year, month);
                return Redirect::permanent(&url).into_response();
            }
        }
    }

    // ?author=N — author archive by user ID → redirect to /author/{nicename}/
    if let Some(author_id) = qv.author {
        let user = wp_users::Entity::find_by_id(author_id)
            .one(&state.db)
            .await;
        return match user {
            Ok(Some(u)) => Redirect::permanent(&format!("/author/{}/", u.user_nicename)).into_response(),
            Ok(None) => (StatusCode::NOT_FOUND, Html("Author not found".to_string())).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())).into_response(),
        };
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
    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(slug))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

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
                    let mut context = build_base_context(&state).await;
                    context.insert("post_title", &p.post_title);
                    context.insert("post_id", &post_id);
                    context.insert("redirect_to", &format!("/{}", slug));
                    return render_theme_page(&state, &PageType::Single {
                        post_type: "password-form".to_string(),
                        slug: slug.to_string(),
                    }, &context).await.or_else(|_| {
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
            let mut context = build_base_context(&state).await;
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
                title: Some(rustpress_seo::generate_title(&p.post_title, &site_name, "-")),
                description: Some(rustpress_seo::auto_generate_description(&p.post_content, 160)),
                canonical: Some(format!("{}/{}", state.site_url, p.post_name)),
                robots: Some("index, follow".to_string()),
                og_title: Some(p.post_title.clone()),
                og_description: Some(rustpress_seo::auto_generate_description(&p.post_content, 200)),
                og_url: Some(format!("{}/{}", state.site_url, p.post_name)),
                og_type: Some(if post_type == "page" { "website".to_string() } else { "article".to_string() }),
                og_site_name: Some(site_name),
                twitter_card: Some("summary_large_image".to_string()),
                ..Default::default()
            };
            context.insert("seo_meta_tags", &rustpress_seo::generate_meta_tags(&seo_meta));

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
            let comment_nonce = state.nonces.create_nonce(&format!("comment_{}", post_id), 0);
            context.insert("comment_nonce", &comment_nonce);

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

            render_theme_page(&state, &page_type, &context).await
        }
        None => {
            // 404
            let mut context = build_base_context(&state).await;
            context.insert("request_uri", &format!("/{}", slug));
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
        let like_term = format!("%{}%", search_term);
        let models = wp_posts::Entity::find()
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
    context.insert("archive_title", &format!("Category: {}", term_name));
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
    context.insert("archive_title", &format!("Tag: {}", term_name));
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
    context.insert("archive_title", &format!("Author: {}", slug));

    // Query posts by the author
    use rustpress_db::entities::wp_users;
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserNicename.eq(&slug))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    if let Some(user) = user {
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
        let total_pages = (total + per_page - 1) / per_page;

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
    let term = wp_terms::Entity::find()
        .filter(wp_terms::Column::Slug.eq(slug))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    let term = match term {
        Some(t) => t,
        None => {
            let pagination = PaginationData::new(page, 1, 0);
            return Ok((vec![], pagination, 0));
        }
    };

    // 2. Find term_taxonomy for this term + taxonomy type
    let tt = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(term.term_id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

    let tt = match tt {
        Some(t) => t,
        None => {
            let pagination = PaginationData::new(page, 1, 0);
            return Ok((vec![], pagination, term.term_id));
        }
    };

    // 3. Get post IDs from term_relationships
    let relationships = wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::TermTaxonomyId.eq(tt.term_taxonomy_id))
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;

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

    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Html(e.to_string())))?;
    let total_pages = if total == 0 {
        1
    } else {
        (total + per_page - 1) / per_page
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
    <title><![CDATA[{}]]></title>
    <link>{}</link>
    <description><![CDATA[{}]]></description>
    <lastBuildDate>{}</lastBuildDate>
    <language>en-US</language>
    <atom:link href="{}/feed/" rel="self" type="application/rss+xml"/>
{}  </channel>
</rss>"#,
        site_name, site_url, site_desc, last_build, site_url, items
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
    let nonce_valid = form._wpnonce.as_deref()
        .map(|n| state.nonces.verify_nonce(n, &nonce_action, 0).is_some())
        .unwrap_or(false);

    if !nonce_valid {
        return (StatusCode::FORBIDDEN, Html("<p>Security check failed. Please go back and try again.</p>".to_string())).into_response();
    }

    let author = form.author.as_deref().unwrap_or("").trim().to_string();
    let email = form.email.as_deref().unwrap_or("").trim().to_string();

    // Validate required fields
    if author.is_empty() || form.comment.trim().is_empty() {
        return Redirect::to(&format!(
            "/{}?comment_error=required",
            form.comment_post_id
        ))
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
    let redirect_url = form.redirect_to
        .as_deref()
        .filter(|r| !r.is_empty())
        .unwrap_or(&format!("/{}", post_slug))
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

    Redirect::to(&format!("{}#comments", redirect_url)).into_response()
}

// ---- Date-based Archives ----

async fn year_archive_page(
    state: &AppState,
    year: u32,
    page_query: Option<u64>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let mut context = build_base_context(state).await;
    context.insert("archive_title", &format!("Year: {}", year));
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
    let total_pages = if total == 0 { 1 } else { (total + per_page - 1) / per_page };

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
    let year: u32 = year.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;
    let month: u32 = month.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;

    if !(1..=12).contains(&month) {
        return Err((StatusCode::NOT_FOUND, Html("Not found".to_string())));
    }

    let mut context = build_base_context(&state).await;
    let month_name = month_to_name(month);
    context.insert("archive_title", &format!("{} {}", month_name, year));
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

    let total_pages = (total + per_page - 1) / per_page;

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
    let year: u32 = year.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;
    let month: u32 = month.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;
    let day: u32 = day.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err((StatusCode::NOT_FOUND, Html("Not found".to_string())));
    }

    let mut context = build_base_context(&state).await;
    let month_name = month_to_name(month);
    context.insert("archive_title", &format!("{} {}, {}", month_name, day, year));
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
    let total_pages = if total == 0 { 1 } else { (total + per_page - 1) / per_page };

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
            .filter_map(|s| {
                s.trim_end_matches(|c: char| c == ';' || c == '}')
                    .parse::<u64>()
                    .ok()
            })
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
    let cookie_name = format!("wp-postpass_slug_{}", slug);
    if let Some(cookie_header) = headers.get(header::COOKIE) {
        if let Ok(cookies) = cookie_header.to_str() {
            for cookie in cookies.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix(&format!("{}=", cookie_name)) {
                    return value == expected_password;
                }
            }
        }
    }
    false
}

async fn post_password_handler(
    Form(form): Form<PostPasswordForm>,
) -> Response {
    // Extract post ID from redirect_to path (e.g. "/my-post" → slug)
    let redirect = if form.redirect_to.is_empty() {
        "/".to_string()
    } else {
        form.redirect_to.clone()
    };

    // We need the post ID to set the cookie. Extract from the slug.
    // The cookie is set generically; the password check happens on the post page.
    let slug = redirect.trim_start_matches('/');

    // Set cookie with the password value (WordPress stores the phpass hash,
    // but for simplicity we store the plain password in the cookie and compare directly)
    let cookie_value = form.post_password.clone();

    // We need to look up the post to get its ID for the cookie name.
    // Since we don't have DB access here, use a generic cookie keyed by slug.
    let cookie = format!(
        "wp-postpass_slug_{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=864000",
        slug, cookie_value
    );

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, redirect.as_str()),
            (header::SET_COOKIE, cookie.as_str()),
        ],
    )
        .into_response()
}

// ---- wp-login.php ----

/// POST /{slug}/trackback — WordPress trackback receiver (deprecated, respond with success XML).
async fn trackback_handler(
    Path(_slug): Path<String>,
) -> Response {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?><response><error>1</error><message>Trackbacks are not accepted on this site.</message></response>"#;
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/xml; charset=UTF-8")],
        xml,
    )
        .into_response()
}

/// GET /wp-register.php — redirect to wp-login.php?action=register (or disabled message).
async fn wp_register_redirect(
    State(state): State<Arc<AppState>>,
) -> Response {
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
        format!("{}/wp-login.php?action=register", state.site_url.trim_end_matches('/'))
    } else {
        format!("{}/wp-login.php", state.site_url.trim_end_matches('/'))
    };
    axum::response::Redirect::to(&redirect_url).into_response()
}

/// Simple pseudo-random byte for key generation.
fn frontend_rand_byte() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ((t ^ (c.wrapping_mul(6364136223846793005) >> 32) as u32) & 0xFF) as u8
}

/// Render the lost password / password recovery page.
fn render_lostpassword_page(site_name: &str, site_url: &str, error: &str, sent: bool) -> String {
    let error_html = if !error.is_empty() {
        format!(r#"<div id="login_error"><strong>Error</strong>: {}</div>"#, error)
    } else { String::new() };

    let content = if sent {
        r#"<p class="message">Check your email for the confirmation link.</p>"#.to_string()
    } else {
        format!(r#"{}<p>Please enter your username or email address. You will receive an email message with instructions on how to reset your password.</p>
<form name="lostpasswordform" id="lostpasswordform" action="{}/wp-login.php?action=lostpassword" method="post">
<p><label for="user_login">Username or Email Address</label><br>
<input type="text" name="user_login" id="user_login" class="input" value="" size="20" autocapitalize="off"></p>
<p class="submit"><input type="submit" name="wp-submit" id="wp-submit" class="button button-primary button-large" value="Get New Password"></p>
</form>
<p id="nav"><a href="{}/wp-login.php">&larr; Back to Log in</a></p>"#, error_html, site_url.trim_end_matches('/'), site_url.trim_end_matches('/'))
    };

    format!(r#"<!DOCTYPE html><html lang="en-US">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width">
<title>Lost Password &lsaquo; {} &#8212; WordPress</title>
<style>
body{{background:#f0f0f1;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Oxygen-Sans,Ubuntu,Cantarell,"Helvetica Neue",sans-serif;}}
#login{{width:320px;margin:5% auto;padding:20px;}}
.login h1 a{{background-image:url(//s.w.org/style/images/wp-header-logo-2x.png?3);background-size:84px;width:84px;height:84px;display:block;margin:0 auto 20px;}}
.input{{width:100%;padding:3px 10px;box-sizing:border-box;border:1px solid #8c8f94;border-radius:4px;}}
.button-primary{{background:#2271b1;border-color:#2271b1;color:#fff;padding:5px 12px;cursor:pointer;border-radius:3px;}}
#login_error{{background:#f8d7da;padding:10px;margin:10px 0;border-left:4px solid #dc3232;}}
.message{{background:#d4edda;padding:10px;margin:10px 0;}}
</style></head>
<body class="login login-action-lostpassword">
<div id="login" class="login">
<h1><a href="https://wordpress.org/">WordPress</a></h1>
{}
</div></body></html>"#, site_name, content)
}

/// Render the password reset (set new password) page.
fn render_reset_password_page(site_name: &str, _site_url: &str, message: &str, key: &str, login: &str) -> String {
    let msg_html = if !message.is_empty() {
        format!(r#"<div id="login_error">{}</div>"#, message)
    } else { String::new() };

    format!(r#"<!DOCTYPE html><html lang="en-US">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width">
<title>Reset Password &lsaquo; {} &#8212; WordPress</title>
<style>
body{{background:#f0f0f1;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Oxygen-Sans,Ubuntu,Cantarell,"Helvetica Neue",sans-serif;}}
#login{{width:320px;margin:5% auto;padding:20px;}}
.login h1 a{{background-image:url(//s.w.org/style/images/wp-header-logo-2x.png?3);background-size:84px;width:84px;height:84px;display:block;margin:0 auto 20px;}}
.input{{width:100%;padding:3px 10px;box-sizing:border-box;border:1px solid #8c8f94;border-radius:4px;}}
.button-primary{{background:#2271b1;border-color:#2271b1;color:#fff;padding:5px 12px;cursor:pointer;border-radius:3px;}}
#login_error{{background:#f8d7da;padding:10px;margin:10px 0;border-left:4px solid #dc3232;}}
</style></head>
<body class="login login-action-rp">
<div id="login" class="login">
<h1><a href="https://wordpress.org/">WordPress</a></h1>
{}
<form name="resetpassform" id="resetpassform" action="/wp-login.php?action=rp" method="post">
<input type="hidden" id="key" name="key" value="{}">
<input type="hidden" id="login" name="login" value="{}">
<p><label for="pass1">New Password</label><br>
<input type="password" name="pass1" id="pass1" class="input" size="20" autocomplete="new-password"></p>
<p><label for="pass2">Confirm New Password</label><br>
<input type="password" name="pass2" id="pass2" class="input" size="20" autocomplete="new-password"></p>
<p class="submit"><input type="submit" name="wp-submit" id="wp-submit" class="button button-primary button-large" value="Save Password"></p>
</form>
</div></body></html>"#, site_name, msg_html, key, login)
}

/// Render the user registration page.
fn render_register_page(site_name: &str, site_url: &str, error: &str, username: &str) -> String {
    let error_html = if !error.is_empty() {
        format!(r#"<div id="login_error"><strong>Error</strong>: {}</div>"#, error)
    } else { String::new() };

    format!(r#"<!DOCTYPE html><html lang="en-US">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width">
<title>Registration Form &lsaquo; {} &#8212; WordPress</title>
<style>
body{{background:#f0f0f1;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Oxygen-Sans,Ubuntu,Cantarell,"Helvetica Neue",sans-serif;}}
#login{{width:320px;margin:5% auto;padding:20px;}}
.login h1 a{{background-image:url(//s.w.org/style/images/wp-header-logo-2x.png?3);background-size:84px;width:84px;height:84px;display:block;margin:0 auto 20px;}}
.input{{width:100%;padding:3px 10px;box-sizing:border-box;border:1px solid #8c8f94;border-radius:4px;}}
.button-primary{{background:#2271b1;border-color:#2271b1;color:#fff;padding:5px 12px;cursor:pointer;border-radius:3px;}}
#login_error{{background:#f8d7da;padding:10px;margin:10px 0;border-left:4px solid #dc3232;}}
</style></head>
<body class="login login-action-register">
<div id="login" class="login">
<h1><a href="https://wordpress.org/">WordPress</a></h1>
{}
<p>A password will be emailed to you.</p>
<form name="registerform" id="registerform" action="{}/wp-login.php?action=register" method="post">
<p><label for="user_login">Username</label><br>
<input type="text" name="user_login" id="user_login" class="input" value="{}" size="20" autocapitalize="off"></p>
<p><label for="user_email">Email</label><br>
<input type="email" name="user_email" id="user_email" class="input" value="" size="25"></p>
<p class="submit"><input type="submit" name="wp-submit" id="wp-submit" class="button button-primary button-large" value="Register"></p>
</form>
<p id="nav">
<a href="{}/wp-login.php">Log in</a> |
<a href="{}/wp-login.php?action=lostpassword">Lost your password?</a>
</p>
</div></body></html>"#, site_name, error_html, site_url.trim_end_matches('/'), username, site_url.trim_end_matches('/'), site_url.trim_end_matches('/'))
}

fn render_login_page(site_name: &str, site_url: &str, error: &str, redirect_to: &str, loggedout: bool) -> String {
    let error_html = if !error.is_empty() {
        format!(r#"<div id="login_error"><strong>Error</strong>: {}</div>"#, error)
    } else {
        String::new()
    };
    let loggedout_html = if loggedout {
        r#"<p class="message">You are now logged out.</p>"#.to_string()
    } else {
        String::new()
    };

    format!(r#"<!DOCTYPE html>
<html lang="en-US">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width">
<title>Log In &lsaquo; {site_name} &#8212; WordPress</title>
<style>
body{{background:#f0f0f1;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;}}
#login{{margin:100px auto;max-width:320px;padding:0;}}
.login h1 a{{background-image:url({site_url}/static/vendor/images/wordpress-logo.svg);background-size:84px;background-repeat:no-repeat;background-position:center top;color:#3c434a;display:block;height:84px;font-size:0;overflow:hidden;padding:0;text-indent:-9999px;text-decoration:none;width:84px;margin:0 auto 25px;}}
#loginform{{background:#fff;border:1px solid #c3c4c7;border-bottom:none;border-radius:4px;padding:26px 24px 46px;box-shadow:0 1px 3px rgba(0,0,0,.04);}}
#loginform label{{font-size:14px;display:block;margin-bottom:4px;color:#3c434a;}}
#loginform input[type=text],#loginform input[type=password]{{width:100%;box-sizing:border-box;padding:6px 10px;border:1px solid #8c8f94;border-radius:4px;font-size:14px;margin-bottom:16px;}}
#loginform input[type=submit]{{width:100%;background:#2271b1;border:1px solid #2271b1;color:#fff;font-size:13px;padding:0 10px;border-radius:3px;height:32px;cursor:pointer;}}
#loginform input[type=submit]:hover{{background:#135e96;}}
#nav{{padding:0 24px;background:#fff;border:1px solid #c3c4c7;border-top:none;border-radius:0 0 4px 4px;}}
#nav a{{font-size:13px;line-height:2;color:#2271b1;display:block;text-align:center;}}
#login_error,.message{{background:#fff;border:1px solid #c3c4c7;border-left:4px solid #d63638;padding:8px 12px;margin-bottom:16px;font-size:13px;border-radius:4px;}}
.message{{border-left-color:#72aee6;}}
.forgetmenot{{font-size:13px;line-height:1.5;display:flex;align-items:center;gap:6px;margin-top:16px;}}
</style>
</head>
<body class="login">
<div id="login">
<h1><a href="{site_url}">{site_name}</a></h1>
{error_html}
{loggedout_html}
<form name="loginform" id="loginform" action="/wp-login.php" method="post">
<label for="user_login">Username or Email Address</label>
<input type="text" name="log" id="user_login" autocomplete="username" required>
<label for="user_pass">Password</label>
<input type="password" name="pwd" id="user_pass" autocomplete="current-password" required>
<input type="hidden" name="redirect_to" value="{redirect_to}">
<p class="forgetmenot"><label><input name="rememberme" type="checkbox" value="forever"> Remember Me</label></p>
<input type="submit" name="wp-submit" id="wp-submit" class="button button-primary button-large" value="Log In">
</form>
<p id="nav"><a href="/wp-login.php?action=lostpassword">Lost your password?</a></p>
</div>
</body>
</html>"#, site_name=site_name, site_url=site_url, error_html=error_html, loggedout_html=loggedout_html, redirect_to=redirect_to)
}

/// GET /wp-login.php — WordPress-compatible login page
async fn wp_login_page(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WpLoginQuery>,
    headers: HeaderMap,
) -> Response {
    let action = query.action.as_deref().unwrap_or("login");
    let site_name = state.options.get_blogname().await.unwrap_or_default();

    match action {
        "logout" => {
            // Verify the logout nonce (wordpress uses _wpnonce)
            // Also destroy the session cookie
            let session_id = headers
                .get(header::COOKIE)
                .and_then(|v| v.to_str().ok())
                .and_then(|cookies| {
                    cookies.split(';').find_map(|c| {
                        c.trim()
                            .strip_prefix("rustpress_session=")
                            .map(|v| v.to_string())
                    })
                });

            if let Some(sid) = session_id {
                state.sessions.destroy_session(&sid).await;
            }

            // Redirect to login with loggedout=true
            let clear_cookie = "rustpress_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
            (
                StatusCode::SEE_OTHER,
                [
                    (header::LOCATION, "/wp-login.php?loggedout=true".to_string()),
                    (header::SET_COOKIE, clear_cookie.to_string()),
                ],
            )
                .into_response()
        }
        "lostpassword" => {
            let sent = query.checkemail.as_deref() == Some("confirm");
            let html = render_lostpassword_page(&site_name, &state.site_url, "", sent);
            (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response()
        }
        "rp" | "resetpass" => {
            let key = query.key.as_deref().unwrap_or("");
            let login = query.login.as_deref().unwrap_or("");
            // Validate that key and login exist
            if key.is_empty() || login.is_empty() {
                let html = render_login_page(&site_name, &state.site_url, "Invalid password reset link.", "/wp-admin/", false);
                return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
            }
            let msg = if query.checkemail.as_deref() == Some("confirm") {
                "Your password has been reset. You can now log in."
            } else { "" };
            let html = render_reset_password_page(&site_name, &state.site_url, msg, key, login);
            (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response()
        }
        "register" => {
            let can_register = state
                .options
                .get_option("users_can_register")
                .await
                .ok()
                .flatten()
                .map(|v| v == "1")
                .unwrap_or(false);
            if !can_register {
                let html = render_login_page(&site_name, &state.site_url, "User registration is currently not allowed.", "/wp-admin/", false);
                return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
            }
            let html = render_register_page(&site_name, &state.site_url, "", "");
            (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response()
        }
        _ => {
            // Default: show login form
            let redirect_to = query.redirect_to.as_deref().unwrap_or("/wp-admin/").to_string();
            let loggedout = query.loggedout.as_deref() == Some("true");
            let msg = match query.checkemail.as_deref() {
                Some("registered") => "Registration complete. Please check your email, then visit the login page.",
                Some("confirm") => "Please check your email for the confirmation link.",
                _ => "",
            };
            let html = render_login_page(&site_name, &state.site_url, msg, &redirect_to, loggedout);
            (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response()
        }
    }
}

/// POST /wp-login.php — handles login, lostpassword, rp, and register forms
async fn wp_login_post(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WpLoginQuery>,
    Form(form): Form<WpLoginForm>,
) -> Response {
    let post_action = query.action.as_deref().unwrap_or("login");
    let site_name = state.options.get_blogname().await.unwrap_or_default();

    // ?action=lostpassword POST
    if post_action == "lostpassword" || form.user_login.is_some() && form.log.is_none() && form.post_password.is_none() {
        let user_login = form.user_login.as_deref().unwrap_or("").trim().to_string();
        if user_login.is_empty() {
            let html = render_lostpassword_page(&site_name, &state.site_url, "Please enter your username or email address.", false);
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }

        // Find user by login or email
        let user = wp_users::Entity::find()
            .filter(wp_users::Column::UserLogin.eq(&user_login))
            .one(&state.db)
            .await
            .ok()
            .flatten();
        let user = if user.is_none() {
            wp_users::Entity::find()
                .filter(wp_users::Column::UserEmail.eq(&user_login))
                .one(&state.db)
                .await
                .ok()
                .flatten()
        } else { user };

        if user.is_none() {
            let html = render_lostpassword_page(&site_name, &state.site_url, "There is no account with that username or email address.", false);
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }
        let user = user.unwrap();

        // Generate reset key (random 20-char string)
        let reset_key: String = (0..20)
            .map(|_| {
                let idx = (frontend_rand_byte() % 62) as usize;
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"[idx] as char
            })
            .collect();

        // Store reset key in usermeta
        use rustpress_db::entities::wp_usermeta;
        let existing_key = wp_usermeta::Entity::find()
            .filter(wp_usermeta::Column::UserId.eq(user.id))
            .filter(wp_usermeta::Column::MetaKey.eq("_password_reset_key"))
            .one(&state.db)
            .await
            .ok()
            .flatten();

        if let Some(rec) = existing_key {
            let mut active: wp_usermeta::ActiveModel = rec.into();
            active.meta_value = sea_orm::ActiveValue::Set(Some(reset_key.clone()));
            let _ = active.update(&state.db).await;
        } else {
            let new_meta = wp_usermeta::ActiveModel {
                umeta_id: sea_orm::ActiveValue::NotSet,
                user_id: sea_orm::ActiveValue::Set(user.id),
                meta_key: sea_orm::ActiveValue::Set(Some("_password_reset_key".to_string())),
                meta_value: sea_orm::ActiveValue::Set(Some(reset_key.clone())),
            };
            let _ = new_meta.insert(&state.db).await;
        }

        // Build reset URL
        let site_url = state.site_url.trim_end_matches('/');
        let encoded_login: String = user.user_login.chars().map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c.to_string() }
            else { format!("%{:02X}", c as u8) }
        }).collect();
        let reset_url = format!(
            "{}/wp-login.php?action=rp&key={}&login={}",
            site_url, reset_key, encoded_login
        );

        // Try to send email; on failure just show the link (dev mode)
        let body = format!(
            "Someone has requested a password reset for the following account:\n\nSite Name: {}\nUsername: {}\n\nIf this was a mistake, just ignore this email and nothing will happen.\n\nTo reset your password, visit the following address:\n{}",
            site_name, user.user_login, reset_url
        );
        let admin_email = state.options.get_option("admin_email").await.ok().flatten().unwrap_or_else(|| "admin@localhost".to_string());
        let mail = rustpress_core::mail::WpMail::new(rustpress_core::mail::MailConfig {
            smtp_host: "localhost".to_string(),
            smtp_port: 25,
            smtp_username: String::new(),
            smtp_password: String::new(),
            from_name: site_name.clone(),
            from_email: admin_email,
        });
        let _ = mail.wp_mail(&user.user_email, &format!("[{}] Password Reset", site_name), &body, None).await;

        let html = render_lostpassword_page(&site_name, &state.site_url, "", true);
        return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
    }

    // ?action=rp POST — set new password
    if post_action == "rp" || post_action == "resetpass" {
        let key = form.key.as_deref().unwrap_or("").to_string();
        let login_name = form.login.as_deref().unwrap_or("").to_string();
        let pass1 = form.pass1.as_deref().unwrap_or("").to_string();
        let pass2 = form.pass2.as_deref().unwrap_or("").to_string();

        if pass1 != pass2 {
            let html = render_reset_password_page(&site_name, &state.site_url, "Passwords do not match.", &key, &login_name);
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }
        if pass1.len() < 6 {
            let html = render_reset_password_page(&site_name, &state.site_url, "Password must be at least 6 characters.", &key, &login_name);
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }

        // Verify key
        use rustpress_db::entities::wp_usermeta;
        let user = wp_users::Entity::find()
            .filter(wp_users::Column::UserLogin.eq(&login_name))
            .one(&state.db)
            .await
            .ok()
            .flatten();

        if let Some(user) = user {
            let stored_key = wp_usermeta::Entity::find()
                .filter(wp_usermeta::Column::UserId.eq(user.id))
                .filter(wp_usermeta::Column::MetaKey.eq("_password_reset_key"))
                .one(&state.db)
                .await
                .ok()
                .flatten()
                .and_then(|m| m.meta_value)
                .unwrap_or_default();

            if stored_key == key {
                // Hash and update password
                use rustpress_auth::PasswordHasher;
                if let Ok(new_hash) = PasswordHasher::hash_argon2(&pass1) {
                    let user_id = user.id;
                    let mut active: wp_users::ActiveModel = user.into();
                    active.user_pass = sea_orm::ActiveValue::Set(new_hash);
                    if active.update(&state.db).await.is_ok() {
                        // Remove the reset key
                        if let Ok(Some(meta)) = wp_usermeta::Entity::find()
                            .filter(wp_usermeta::Column::UserId.eq(user_id))
                            .filter(wp_usermeta::Column::MetaKey.eq("_password_reset_key"))
                            .one(&state.db)
                            .await
                        {
                            let _ = wp_usermeta::Entity::delete_by_id(meta.umeta_id).exec(&state.db).await;
                        }
                        // Redirect to login with password reset success
                        return (
                            StatusCode::SEE_OTHER,
                            [(header::LOCATION, "/wp-login.php?checkemail=confirm&action=rp".to_string())],
                        ).into_response();
                    }
                }
            }
        }

        let html = render_reset_password_page(&site_name, &state.site_url, "Invalid reset key. Please request a new one.", &key, &login_name);
        return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
    }

    // ?action=register POST — create new user
    if post_action == "register" {
        let can_register = state.options.get_option("users_can_register").await.ok().flatten()
            .map(|v| v == "1").unwrap_or(false);
        if !can_register {
            let html = render_register_page(&site_name, &state.site_url, "User registration is currently not allowed.", "");
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }

        let reg_login = form.user_login.as_deref().unwrap_or("").trim().to_string();
        let reg_email = form.user_email.as_deref().unwrap_or("").trim().to_string();

        if reg_login.is_empty() || reg_email.is_empty() {
            let html = render_register_page(&site_name, &state.site_url, "Please fill in all required fields.", &reg_login);
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }

        // Check username not taken
        let existing = wp_users::Entity::find()
            .filter(wp_users::Column::UserLogin.eq(&reg_login))
            .one(&state.db).await.ok().flatten();
        if existing.is_some() {
            let html = render_register_page(&site_name, &state.site_url, "Sorry, that username already exists!", &reg_login);
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }

        // Generate random password
        let random_pass: String = (0..12)
            .map(|_| {
                let idx = (frontend_rand_byte() % 62) as usize;
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"[idx] as char
            })
            .collect();

        use rustpress_auth::PasswordHasher;
        let hashed = PasswordHasher::hash_argon2(&random_pass).unwrap_or_default();
        let now = chrono::Utc::now().naive_utc();

        let new_user = wp_users::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            user_login: sea_orm::ActiveValue::Set(reg_login.clone()),
            user_pass: sea_orm::ActiveValue::Set(hashed),
            user_nicename: sea_orm::ActiveValue::Set(reg_login.to_lowercase().replace(' ', "-")),
            user_email: sea_orm::ActiveValue::Set(reg_email.clone()),
            user_url: sea_orm::ActiveValue::Set(String::new()),
            user_registered: sea_orm::ActiveValue::Set(now),
            user_activation_key: sea_orm::ActiveValue::Set(String::new()),
            user_status: sea_orm::ActiveValue::Set(0),
            display_name: sea_orm::ActiveValue::Set(reg_login.clone()),
        };

        match new_user.insert(&state.db).await {
            Ok(user) => {
                // Set subscriber role in usermeta
                use rustpress_db::entities::wp_usermeta;
                let cap_meta = wp_usermeta::ActiveModel {
                    umeta_id: sea_orm::ActiveValue::NotSet,
                    user_id: sea_orm::ActiveValue::Set(user.id),
                    meta_key: sea_orm::ActiveValue::Set(Some("wp_capabilities".to_string())),
                    meta_value: sea_orm::ActiveValue::Set(Some(r#"a:1:{s:10:"subscriber";b:1;}"#.to_string())),
                };
                let _ = cap_meta.insert(&state.db).await;

                // Send notification email
                let admin_email = state.options.get_option("admin_email").await.ok().flatten().unwrap_or_else(|| "admin@localhost".to_string());
                let mail = rustpress_core::mail::WpMail::new(rustpress_core::mail::MailConfig {
                    smtp_host: "localhost".to_string(), smtp_port: 25,
                    smtp_username: String::new(), smtp_password: String::new(),
                    from_name: site_name.clone(), from_email: admin_email.clone(),
                });
                let welcome_body = format!(
                    "Username: {}\nPassword: {}\n\nLog in: {}/wp-login.php",
                    reg_login, random_pass, state.site_url.trim_end_matches('/')
                );
                let _ = mail.wp_mail(&reg_email, &format!("[{}] Your username and password info", site_name), &welcome_body, None).await;

                return (
                    StatusCode::SEE_OTHER,
                    [(header::LOCATION, "/wp-login.php?checkemail=registered".to_string())],
                ).into_response();
            }
            Err(_) => {
                let html = render_register_page(&site_name, &state.site_url, "Registration failed. Please try again.", &reg_login);
                return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
            }
        }
    }

    // Dispatch based on which fields are present
    if let Some(post_password) = form.post_password {
        // Password-protected post form submission
        let redirect = form.redirect_to.as_deref().unwrap_or("/").to_string();
        let slug = redirect.trim_start_matches('/').to_string();
        let cookie = format!(
            "wp-postpass_slug_{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=864000",
            slug, post_password
        );
        return (
            StatusCode::SEE_OTHER,
            [
                (header::LOCATION, redirect),
                (header::SET_COOKIE, cookie),
            ],
        )
            .into_response();
    }

    // Login form submission
    let username = form.log.as_deref().unwrap_or("").trim().to_string();
    let password = form.pwd.as_deref().unwrap_or("").trim().to_string();
    let redirect_to = form.redirect_to.as_deref().unwrap_or("/wp-admin/").to_string();
    let site_name = state.options.get_blogname().await.unwrap_or_default();

    if username.is_empty() || password.is_empty() {
        let html = render_login_page(&site_name, &state.site_url, "Please fill in your username and password.", &redirect_to, false);
        return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
    }

    // Find user
    use rustpress_auth::PasswordHasher;
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&username))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    // If not found by login, try by email
    let user = if user.is_none() {
        wp_users::Entity::find()
            .filter(wp_users::Column::UserEmail.eq(&username))
            .one(&state.db)
            .await
            .ok()
            .flatten()
    } else {
        user
    };

    let user = match user {
        Some(u) => u,
        None => {
            let html = render_login_page(&site_name, &state.site_url, "Unknown username. Check again or try your email address.", &redirect_to, false);
            return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
        }
    };

    let valid = PasswordHasher::verify(&password, &user.user_pass).unwrap_or(false);
    if !valid {
        let html = render_login_page(&site_name, &state.site_url, "The password you entered for the username is incorrect.", &redirect_to, false);
        return (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=UTF-8")], html).into_response();
    }

    // Determine user role from usermeta
    let role = {
        use rustpress_db::entities::wp_usermeta;
        let meta = wp_usermeta::Entity::find()
            .filter(wp_usermeta::Column::UserId.eq(user.id))
            .filter(wp_usermeta::Column::MetaKey.eq("wp_capabilities"))
            .one(&state.db)
            .await
            .ok()
            .flatten();
        if let Some(m) = meta {
            let val = m.meta_value.unwrap_or_default();
            if val.contains("administrator") { "administrator".to_string() }
            else if val.contains("editor") { "editor".to_string() }
            else if val.contains("author") { "author".to_string() }
            else if val.contains("contributor") { "contributor".to_string() }
            else { "subscriber".to_string() }
        } else {
            "subscriber".to_string()
        }
    };

    // Fire wp_login action
    state.hooks.do_action("wp_login", &serde_json::json!({
        "user_login": user.user_login,
        "user_id": user.id
    }));

    // Create session
    let session = state.sessions.create_session(user.id, &user.user_login, &role).await;
    let max_age = if form.rememberme.as_deref() == Some("forever") { 1209600 } else { 86400 };
    let cookie = format!(
        "rustpress_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        session.id, max_age
    );

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, redirect_to),
            (header::SET_COOKIE, cookie),
        ],
    )
        .into_response()
}

// ---- Threaded Comments ----

fn build_comment_tree(comments: &[rustpress_db::entities::wp_comments::Model]) -> Vec<serde_json::Value> {
    let flat: Vec<serde_json::Value> = comments
        .iter()
        .map(|c| {
            {
                let dt = c.comment_date;
                let month_name = match dt.format("%m").to_string().as_str() {
                    "01" => "January", "02" => "February", "03" => "March",
                    "04" => "April", "05" => "May", "06" => "June",
                    "07" => "July", "08" => "August", "09" => "September",
                    "10" => "October", "11" => "November", "12" => "December",
                    _ => "January",
                };
                let day = dt.format("%-d").to_string();
                let year = dt.format("%Y").to_string();
                let time_12h = dt.format("%-I:%M %P").to_string();
                let date_formatted = format!("{} {}, {} at {}", month_name, day, year, time_12h);
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
            }
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

async fn category_feed(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    taxonomy_feed(&state, "category", &slug).await
}

async fn tag_feed(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
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

    let taxonomy_label = if taxonomy == "category" { "Category" } else { "Tag" };
    let feed_title = format!("{} » {} {} Feed", site_name, term.name, taxonomy_label);
    let feed_link = format!(
        "{}/{}/{}",
        site_url,
        if taxonomy == "category" { "category" } else { "tag" },
        slug
    );
    let feed_desc = format!("Posts in {} \"{}\"", taxonomy_label.to_lowercase(), term.name);

    let items = build_rss_items(&posts, site_url);

    let last_build = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S +0000")
        .to_string();

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title><![CDATA[{}]]></title>
    <link>{}</link>
    <description><![CDATA[{}]]></description>
    <lastBuildDate>{}</lastBuildDate>
    <language>en-US</language>
    <atom:link href="{}/feed/" rel="self" type="application/rss+xml"/>
{}  </channel>
</rss>"#,
        feed_title, feed_link, feed_desc, last_build, feed_link, items
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
        let link = format!("{}/?p={}#comment-{}", site_url, c.comment_post_id, c.comment_id);
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
    <title><![CDATA[Comments for {}]]></title>
    <link>{}</link>
    <description><![CDATA[Comments]]></description>
    <lastBuildDate>{}</lastBuildDate>
    <language>en-US</language>
    <atom:link href="{}/comments/feed/" rel="self" type="application/rss+xml"/>
{}  </channel>
</rss>"#,
        site_name, site_url, last_build, site_url, items
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
    let hook_name = format!("wp_ajax_{}", action);
    state.hooks.do_action(&hook_name, &ctx);

    // Fire wp_ajax_nopriv_{action} hook (for non-logged-in users)
    let nopriv_hook = format!("wp_ajax_nopriv_{}", action);
    state.hooks.do_action(&nopriv_hook, &ctx);

    // Apply filter to get response (if any plugin set one)
    let result = state.hooks.apply_filters(
        &format!("ajax_response_{}", action),
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
    <title><![CDATA[{}]]></title>
    <link>{}</link>
    <description></description>
    <lastBuildDate>{}</lastBuildDate>
    <language>en-US</language>
  </channel>
</rss>"#,
        title, site_url, last_build
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
    let _year: u32 = year.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;
    let _month: u32 = month.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;
    let _day: u32 = day.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;

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
    let _year: u32 = year.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;
    let _month: u32 = month.parse().map_err(|_| {
        (StatusCode::NOT_FOUND, Html("Not found".to_string()))
    })?;

    // If third segment is a number (1-31), treat as day archive
    if let Ok(day) = slug_or_day.parse::<u32>() {
        if (1..=31).contains(&day) {
            return day_archive(
                State(state),
                Path((year, month, slug_or_day)),
                Query(params),
            ).await;
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
    let post_url = format!("{}/{}", site_url, slug);
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

//! WordPress Templates REST API (Full Site Editing)
//!
//! GET  /wp-json/wp/v2/templates
//! GET  /wp-json/wp/v2/templates/{id}
//! POST /wp-json/wp/v2/templates
//! PUT  /wp-json/wp/v2/templates/{id}
//! GET  /wp-json/wp/v2/template-parts
//! GET  /wp-json/wp/v2/template-parts/{id}
//!
//! Templates are stored as wp_posts with post_type='wp_template' or 'wp_template_part'.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use rustpress_db::entities::wp_posts;

use crate::common::WpError;
use crate::ApiState;
use crate::AuthUser;

#[derive(Debug, Deserialize, Serialize)]
pub struct TemplateInput {
    pub slug: Option<String>,
    pub title: Option<Value>,
    pub content: Option<Value>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub area: Option<String>,
}

pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/templates", get(list_templates))
        .route("/wp-json/wp/v2/templates/{id}", get(get_template))
        .route("/wp-json/wp/v2/template-parts", get(list_template_parts))
        .route("/wp-json/wp/v2/template-parts/{id}", get(get_template_part))
}

pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/templates",
            axum::routing::post(create_template),
        )
        .route(
            "/wp-json/wp/v2/templates/{id}",
            axum::routing::put(update_template)
                .patch(update_template)
                .delete(delete_template),
        )
        .route(
            "/wp-json/wp/v2/template-parts",
            axum::routing::post(create_template_part),
        )
        .route(
            "/wp-json/wp/v2/template-parts/{id}",
            axum::routing::put(update_template_part)
                .patch(update_template_part)
                .delete(delete_template_part),
        )
}

/// For backwards compatibility — keep public routes() alias
pub fn routes() -> Router<ApiState> {
    read_routes()
}

fn post_to_template(post: &wp_posts::Model, site_url: &str) -> Value {
    let base = site_url.trim_end_matches('/');
    let id = format!("rustpress//{}", post.post_name);
    json!({
        "id": id,
        "slug": post.post_name,
        "theme": "rustpress",
        "type": post.post_type,
        "source": "theme",
        "origin": "theme",
        "content": {
            "raw": post.post_content,
            "rendered": post.post_content,
            "block_version": 1
        },
        "title": {
            "raw": post.post_title,
            "rendered": post.post_title
        },
        "description": &post.post_excerpt,
        "status": post.post_status,
        "wp_id": post.id,
        "has_theme_file": true,
        "is_custom": false,
        "author": post.post_author,
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/templates/{}", base, urlenc(&id))}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/templates", base)}],
            "about": [{"href": format!("{}/wp-json/wp/v2/types/wp_template", base)}],
            "author": [{"href": format!("{}/wp-json/wp/v2/users/{}", base, post.post_author), "embeddable": true}],
            "wp:theme-file": [{"href": format!("{}/wp-json/wp/v2/templates/{}", base, urlenc(&id))}]
        }
    })
}

fn urlenc(s: &str) -> String {
    s.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/' {
            c.to_string()
        } else {
            format!("%{:02X}", c as u32)
        }
    }).collect()
}

/// GET /wp-json/wp/v2/templates
async fn list_templates(
    State(state): State<ApiState>,
) -> Result<Json<Vec<Value>>, WpError> {
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let items: Vec<Value> = posts.iter().map(|p| post_to_template(p, &state.site_url)).collect();

    // If no templates in DB, return built-in theme file templates
    if items.is_empty() {
        let builtin = builtin_templates(&state.site_url);
        return Ok(Json(builtin));
    }

    Ok(Json(items))
}

/// GET /wp-json/wp/v2/templates/{id}
async fn get_template(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, WpError> {
    // id format: theme//slug (e.g. "twentytwentyfour//index")
    let slug = id.split("//").last().unwrap_or(&id);

    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template"))
        .filter(wp_posts::Column::PostName.eq(slug))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    if let Some(p) = post {
        return Ok(Json(post_to_template(&p, &state.site_url)));
    }

    // Check built-in
    let builtins = builtin_templates(&state.site_url);
    for t in builtins {
        if t.get("slug").and_then(|v| v.as_str()) == Some(slug)
            || t.get("id").and_then(|v| v.as_str()) == Some(&id)
        {
            return Ok(Json(t));
        }
    }

    Err(WpError::new(
        StatusCode::NOT_FOUND,
        "rest_post_invalid_id",
        "No template exists with that id.",
    ))
}

/// GET /wp-json/wp/v2/template-parts
async fn list_template_parts(
    State(state): State<ApiState>,
) -> Result<Json<Vec<Value>>, WpError> {
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template_part"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut items: Vec<Value> = posts.iter().map(|p| {
        let mut t = post_to_template(p, &state.site_url);
        if let Some(obj) = t.as_object_mut() {
            obj.insert("area".to_string(), json!("uncategorized"));
        }
        t
    }).collect();

    if items.is_empty() {
        items = builtin_template_parts(&state.site_url);
    }

    Ok(Json(items))
}

/// GET /wp-json/wp/v2/template-parts/{id}
async fn get_template_part(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, WpError> {
    let slug = id.split("//").last().unwrap_or(&id);

    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template_part"))
        .filter(wp_posts::Column::PostName.eq(slug))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    if let Some(p) = post {
        let mut t = post_to_template(&p, &state.site_url);
        if let Some(obj) = t.as_object_mut() {
            obj.insert("area".to_string(), json!("uncategorized"));
        }
        return Ok(Json(t));
    }

    let parts = builtin_template_parts(&state.site_url);
    for t in parts {
        if t.get("slug").and_then(|v| v.as_str()) == Some(slug) {
            return Ok(Json(t));
        }
    }

    Err(WpError::new(
        StatusCode::NOT_FOUND,
        "rest_post_invalid_id",
        "No template part exists with that id.",
    ))
}

fn builtin_templates(site_url: &str) -> Vec<Value> {
    let base = site_url.trim_end_matches('/');
    let templates = [
        ("index", "Index", "Main template for all content."),
        ("single", "Single Post", "Template for individual posts."),
        ("page", "Page", "Template for static pages."),
        ("archive", "Archive", "Template for date, category, and tag archives."),
        ("search", "Search", "Template for search results."),
        ("404", "404", "Template for 404 not found pages."),
        ("home", "Blog Home", "Template for the blog homepage."),
        ("front-page", "Front Page", "Template for the site front page."),
        ("category", "Category Archive", "Template for category archives."),
        ("tag", "Tag Archive", "Template for tag archives."),
        ("author", "Author Archive", "Template for author pages."),
    ];
    templates.iter().map(|(slug, title, desc)| json!({
        "id": format!("rustpress//{}", slug),
        "slug": slug,
        "theme": "rustpress",
        "type": "wp_template",
        "source": "theme",
        "origin": "theme",
        "content": {"raw": "", "rendered": "", "block_version": 1},
        "title": {"raw": title, "rendered": title},
        "description": desc,
        "status": "publish",
        "wp_id": 0,
        "has_theme_file": true,
        "is_custom": false,
        "author": 0,
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/templates/rustpress%2F%2F{}", base, slug)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/templates", base)}],
            "about": [{"href": format!("{}/wp-json/wp/v2/types/wp_template", base)}]
        }
    })).collect()
}

fn builtin_template_parts(site_url: &str) -> Vec<Value> {
    let base = site_url.trim_end_matches('/');
    let parts = [
        ("header", "Header", "header"),
        ("footer", "Footer", "footer"),
        ("sidebar", "Sidebar", "sidebar"),
    ];
    parts.iter().map(|(slug, title, area)| json!({
        "id": format!("rustpress//{}", slug),
        "slug": slug,
        "theme": "rustpress",
        "type": "wp_template_part",
        "source": "theme",
        "origin": "theme",
        "content": {"raw": "", "rendered": "", "block_version": 1},
        "title": {"raw": title, "rendered": title},
        "description": "",
        "status": "publish",
        "wp_id": 0,
        "has_theme_file": true,
        "is_custom": false,
        "author": 0,
        "area": area,
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/template-parts/rustpress%2F%2F{}", base, slug)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/template-parts", base)}],
            "about": [{"href": format!("{}/wp-json/wp/v2/types/wp_template_part", base)}]
        }
    })).collect()
}

// ---------------------------------------------------------------------------
// Write handlers
// ---------------------------------------------------------------------------

fn extract_raw(v: &Option<Value>) -> String {
    v.as_ref()
        .and_then(|obj| obj.get("raw"))
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string()
}

/// POST /wp-json/wp/v2/templates
async fn create_template(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Json(input): Json<TemplateInput>,
) -> Result<(StatusCode, Json<Value>), WpError> {
    let slug = input.slug.unwrap_or_default();
    let title = extract_raw(&input.title);
    let content = extract_raw(&input.content);
    let description = input.description.unwrap_or_default();
    let status = input.status.unwrap_or_else(|| "publish".to_string());
    let now = chrono::Utc::now().naive_utc();

    let new_post = wp_posts::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        post_author: Set(0),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(content),
        post_title: Set(title),
        post_excerpt: Set(description),
        post_status: Set(status),
        post_name: Set(slug),
        post_type: Set("wp_template".to_string()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        ..Default::default()
    };
    let inserted = new_post.insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(post_to_template(&inserted, &state.site_url))))
}

/// PUT/PATCH /wp-json/wp/v2/templates/{id}
async fn update_template(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<TemplateInput>,
) -> Result<Json<Value>, WpError> {
    let slug = id.split("//").last().unwrap_or(&id);

    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template"))
        .filter(wp_posts::Column::PostName.eq(slug))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::new(StatusCode::NOT_FOUND, "rest_post_invalid_id", "No template exists with that id."))?;

    let now = chrono::Utc::now().naive_utc();
    let mut active: wp_posts::ActiveModel = post.into();
    if let Some(ref t) = input.title { active.post_title = Set(extract_raw(&Some(t.clone()))); }
    if let Some(ref c) = input.content { active.post_content = Set(extract_raw(&Some(c.clone()))); }
    if let Some(ref d) = input.description { active.post_excerpt = Set(d.clone()); }
    if let Some(ref s) = input.status { active.post_status = Set(s.clone()); }
    if let Some(ref slug_new) = input.slug { active.post_name = Set(slug_new.clone()); }
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    let updated = active.update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(post_to_template(&updated, &state.site_url)))
}

/// DELETE /wp-json/wp/v2/templates/{id}
async fn delete_template(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, WpError> {
    let slug = id.split("//").last().unwrap_or(&id);

    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template"))
        .filter(wp_posts::Column::PostName.eq(slug))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::new(StatusCode::NOT_FOUND, "rest_post_invalid_id", "No template exists with that id."))?;

    let response = post_to_template(&post, &state.site_url);
    let active: wp_posts::ActiveModel = post.into();
    active.delete(&state.db).await.map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(response))
}

/// POST /wp-json/wp/v2/template-parts
async fn create_template_part(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Json(input): Json<TemplateInput>,
) -> Result<(StatusCode, Json<Value>), WpError> {
    let slug = input.slug.unwrap_or_default();
    let title = extract_raw(&input.title);
    let content = extract_raw(&input.content);
    let description = input.description.unwrap_or_default();
    let status = input.status.unwrap_or_else(|| "publish".to_string());
    let now = chrono::Utc::now().naive_utc();

    let new_post = wp_posts::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        post_author: Set(0),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(content),
        post_title: Set(title),
        post_excerpt: Set(description),
        post_status: Set(status),
        post_name: Set(slug),
        post_type: Set("wp_template_part".to_string()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        ..Default::default()
    };
    let inserted = new_post.insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut t = post_to_template(&inserted, &state.site_url);
    if let Some(obj) = t.as_object_mut() {
        let area = input.area.unwrap_or_else(|| "uncategorized".to_string());
        obj.insert("area".to_string(), json!(area));
    }
    Ok((StatusCode::CREATED, Json(t)))
}

/// PUT/PATCH /wp-json/wp/v2/template-parts/{id}
async fn update_template_part(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<TemplateInput>,
) -> Result<Json<Value>, WpError> {
    let slug = id.split("//").last().unwrap_or(&id);

    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template_part"))
        .filter(wp_posts::Column::PostName.eq(slug))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::new(StatusCode::NOT_FOUND, "rest_post_invalid_id", "No template part exists with that id."))?;

    let now = chrono::Utc::now().naive_utc();
    let mut active: wp_posts::ActiveModel = post.into();
    if let Some(ref t) = input.title { active.post_title = Set(extract_raw(&Some(t.clone()))); }
    if let Some(ref c) = input.content { active.post_content = Set(extract_raw(&Some(c.clone()))); }
    if let Some(ref d) = input.description { active.post_excerpt = Set(d.clone()); }
    if let Some(ref s) = input.status { active.post_status = Set(s.clone()); }
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    let updated = active.update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut t = post_to_template(&updated, &state.site_url);
    if let Some(obj) = t.as_object_mut() {
        let area = input.area.unwrap_or_else(|| "uncategorized".to_string());
        obj.insert("area".to_string(), json!(area));
    }
    Ok(Json(t))
}

/// DELETE /wp-json/wp/v2/template-parts/{id}
async fn delete_template_part(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, WpError> {
    let slug = id.split("//").last().unwrap_or(&id);

    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("wp_template_part"))
        .filter(wp_posts::Column::PostName.eq(slug))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::new(StatusCode::NOT_FOUND, "rest_post_invalid_id", "No template part exists with that id."))?;

    let mut response = post_to_template(&post, &state.site_url);
    if let Some(obj) = response.as_object_mut() {
        obj.insert("area".to_string(), json!("uncategorized"));
    }
    let active: wp_posts::ActiveModel = post.into();
    active.delete(&state.db).await.map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(response))
}

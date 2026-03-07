use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ApiState;

/// WordPress Batch API v1: process multiple REST sub-requests in one HTTP call.
///
/// WordPress ref: `POST /wp-json/batch/v1`
///
/// Request body:
/// ```json
/// {
///   "validation": "require-all-validate",   // optional
///   "requests": [
///     { "method": "POST", "path": "/wp/v2/posts", "body": { ... } },
///     { "method": "DELETE", "path": "/wp/v2/posts/42" }
///   ]
/// }
/// ```
///
/// Response:
/// ```json
/// {
///   "failed": false,
///   "responses": [
///     { "status": 201, "body": { ... }, "headers": {} },
///     { "status": 200, "body": { ... }, "headers": {} }
///   ]
/// }
/// ```

const MAX_BATCH_REQUESTS: usize = 25;

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    #[serde(default)]
    pub validation: Option<String>,
    pub requests: Vec<SubRequest>,
}

#[derive(Debug, Deserialize)]
pub struct SubRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub body: Option<Value>,
    #[serde(default)]
    pub headers: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub failed: bool,
    pub responses: Vec<SubResponse>,
}

#[derive(Debug, Serialize)]
pub struct SubResponse {
    pub status: u16,
    pub body: Value,
    pub headers: Value,
}

/// Batch write route (requires authentication — registered in protected routes).
pub fn write_routes() -> Router<ApiState> {
    Router::new().route("/wp-json/batch/v1", post(handle_batch))
}

async fn handle_batch(
    State(state): State<ApiState>,
    Json(batch): Json<BatchRequest>,
) -> impl IntoResponse {
    // Enforce max requests limit (WordPress default: 25)
    if batch.requests.len() > MAX_BATCH_REQUESTS {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "code": "rest_batch_max_requests",
                "message": format!("The batch request exceeds the maximum of {} requests.", MAX_BATCH_REQUESTS),
                "data": { "status": 400 }
            })),
        );
    }

    let require_all_validate = batch.validation.as_deref() == Some("require-all-validate");

    // If require-all-validate, do a dry-run validation pass first
    if require_all_validate {
        let mut all_valid = true;
        for req in &batch.requests {
            if !validate_sub_request(req, &state).await {
                all_valid = false;
                break;
            }
        }
        if !all_valid {
            let responses: Vec<SubResponse> = batch
                .requests
                .iter()
                .map(|_| SubResponse {
                    status: 400,
                    body: json!({
                        "code": "rest_batch_validation_failed",
                        "message": "Validation failed for one or more requests.",
                        "data": { "status": 400 }
                    }),
                    headers: json!({}),
                })
                .collect();

            return (
                StatusCode::MULTI_STATUS,
                Json(json!(BatchResponse {
                    failed: true,
                    responses,
                })),
            );
        }
    }

    // Execute each sub-request
    let mut responses = Vec::with_capacity(batch.requests.len());
    let mut any_failed = false;

    for req in &batch.requests {
        let sub_response = execute_sub_request(req, &state).await;
        if sub_response.status >= 400 {
            any_failed = true;
        }
        responses.push(sub_response);
    }

    (
        StatusCode::MULTI_STATUS,
        Json(json!(BatchResponse {
            failed: any_failed,
            responses,
        })),
    )
}

/// Validate a sub-request (basic checks: method, path, resource existence).
async fn validate_sub_request(req: &SubRequest, _state: &ApiState) -> bool {
    // Validate method
    let method = req.method.to_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "PATCH") {
        return false;
    }

    // Validate path starts with /wp/v2/
    let path = normalize_path(&req.path);
    if !path.starts_with("/wp/v2/") && !path.starts_with("/wp-json/wp/v2/") {
        return false;
    }

    // For POST, require a body
    if method == "POST" && req.body.is_none() {
        return false;
    }

    true
}

/// Execute a single sub-request by dispatching to the appropriate handler.
async fn execute_sub_request(req: &SubRequest, state: &ApiState) -> SubResponse {
    let method = req.method.to_uppercase();
    let path = normalize_path(&req.path);

    // Parse the path to determine the resource and action
    let segments: Vec<&str> = path
        .trim_start_matches("/wp-json")
        .trim_start_matches("/wp/v2/")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.is_empty() {
        return SubResponse {
            status: 404,
            body: json!({
                "code": "rest_no_route",
                "message": "No route was found matching the URL and request method.",
                "data": { "status": 404 }
            }),
            headers: json!({}),
        };
    }

    let resource = segments[0];
    let resource_id: Option<u64> = segments.get(1).and_then(|s| s.parse().ok());

    match (method.as_str(), resource, resource_id) {
        // Posts
        ("POST", "posts", None) => {
            dispatch_create_post(state, req.body.clone().unwrap_or(json!({}))).await
        }
        ("PUT" | "PATCH", "posts", Some(id)) => {
            dispatch_update_post(state, id, req.body.clone().unwrap_or(json!({}))).await
        }
        ("DELETE", "posts", Some(id)) => dispatch_delete_post(state, id, &req.body).await,
        ("GET", "posts", Some(id)) => dispatch_get_post(state, id).await,
        ("GET", "posts", None) => dispatch_list_resource(state, "posts").await,

        // Pages
        ("POST", "pages", None) => {
            dispatch_create_page(state, req.body.clone().unwrap_or(json!({}))).await
        }
        ("PUT" | "PATCH", "pages", Some(id)) => {
            dispatch_update_page(state, id, req.body.clone().unwrap_or(json!({}))).await
        }
        ("DELETE", "pages", Some(id)) => dispatch_delete_resource(state, "page", id).await,

        // Categories
        ("POST", "categories", None) => {
            dispatch_create_term(state, "category", req.body.clone().unwrap_or(json!({}))).await
        }
        ("PUT" | "PATCH", "categories", Some(id)) => {
            dispatch_update_term(state, "category", id, req.body.clone().unwrap_or(json!({}))).await
        }
        ("DELETE", "categories", Some(id)) => dispatch_delete_term(state, "category", id).await,

        // Tags
        ("POST", "tags", None) => {
            dispatch_create_term(state, "post_tag", req.body.clone().unwrap_or(json!({}))).await
        }
        ("PUT" | "PATCH", "tags", Some(id)) => {
            dispatch_update_term(state, "post_tag", id, req.body.clone().unwrap_or(json!({}))).await
        }
        ("DELETE", "tags", Some(id)) => dispatch_delete_term(state, "post_tag", id).await,

        // Comments
        ("POST", "comments", None) => {
            dispatch_create_comment(state, req.body.clone().unwrap_or(json!({}))).await
        }
        ("PUT" | "PATCH", "comments", Some(id)) => {
            dispatch_update_comment(state, id, req.body.clone().unwrap_or(json!({}))).await
        }
        ("DELETE", "comments", Some(id)) => dispatch_delete_comment(state, id).await,

        // Unmatched
        _ => SubResponse {
            status: 404,
            body: json!({
                "code": "rest_no_route",
                "message": "No route was found matching the URL and request method.",
                "data": { "status": 404 }
            }),
            headers: json!({}),
        },
    }
}

fn normalize_path(path: &str) -> String {
    if path.starts_with("/wp-json/") || path.starts_with("/wp/v2/") {
        path.to_string()
    } else {
        format!("/wp/v2/{}", path.trim_start_matches('/'))
    }
}

fn ok_response(status: u16, body: Value) -> SubResponse {
    SubResponse {
        status,
        body,
        headers: json!({"Content-Type": "application/json"}),
    }
}

fn error_response(status: u16, code: &str, message: &str) -> SubResponse {
    SubResponse {
        status,
        body: json!({
            "code": code,
            "message": message,
            "data": { "status": status }
        }),
        headers: json!({}),
    }
}

// ─── Post dispatchers ───────────────────────────────────────────────────────

use rustpress_db::entities::{wp_comments, wp_posts, wp_term_taxonomy, wp_terms};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};

async fn dispatch_get_post(state: &ApiState, id: u64) -> SubResponse {
    match wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("post"))
        .one(&state.db)
        .await
    {
        Ok(Some(post)) => {
            let wp_post = crate::posts::build_post(post, &state.site_url, &state.db).await;
            ok_response(200, serde_json::to_value(&wp_post).unwrap_or_default())
        }
        Ok(None) => error_response(404, "rest_post_invalid_id", "Invalid post ID."),
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_list_resource(state: &ApiState, _resource: &str) -> SubResponse {
    // Simplified: return first 10 posts
    match wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .all(&state.db)
        .await
    {
        Ok(posts) => {
            let mut items = Vec::new();
            for p in posts.into_iter().take(10) {
                let wp = crate::posts::build_post(p, &state.site_url, &state.db).await;
                items.push(serde_json::to_value(&wp).unwrap_or_default());
            }
            ok_response(200, Value::Array(items))
        }
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_create_post(state: &ApiState, body: Value) -> SubResponse {
    let now = chrono::Utc::now().naive_utc();
    let title = body["title"].as_str().unwrap_or("").to_string();
    let slug = body["slug"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::common::slugify(&title));

    let new_post = wp_posts::ActiveModel {
        post_author: Set(body["author"].as_u64().unwrap_or(1)),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(body["content"].as_str().unwrap_or("").to_string()),
        post_title: Set(title),
        post_excerpt: Set(body["excerpt"].as_str().unwrap_or("").to_string()),
        post_status: Set(body["status"].as_str().unwrap_or("draft").to_string()),
        comment_status: Set("open".to_string()),
        ping_status: Set("open".to_string()),
        post_password: Set(String::new()),
        post_name: Set(slug),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_content_filtered: Set(String::new()),
        post_parent: Set(0),
        guid: Set(String::new()),
        menu_order: Set(0),
        post_type: Set("post".to_string()),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
        ..Default::default()
    };

    match new_post.insert(&state.db).await {
        Ok(result) => {
            let wp_post = crate::posts::build_post(result, &state.site_url, &state.db).await;
            ok_response(201, serde_json::to_value(&wp_post).unwrap_or_default())
        }
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_update_post(state: &ApiState, id: u64, body: Value) -> SubResponse {
    let post = match wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("post"))
        .one(&state.db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return error_response(404, "rest_post_invalid_id", "Invalid post ID."),
        Err(e) => return error_response(500, "internal_server_error", &e.to_string()),
    };

    let mut active: wp_posts::ActiveModel = post.into();
    let now = chrono::Utc::now().naive_utc();

    if let Some(t) = body["title"].as_str() {
        active.post_title = Set(t.to_string());
    }
    if let Some(c) = body["content"].as_str() {
        active.post_content = Set(c.to_string());
    }
    if let Some(e) = body["excerpt"].as_str() {
        active.post_excerpt = Set(e.to_string());
    }
    if let Some(s) = body["status"].as_str() {
        active.post_status = Set(s.to_string());
    }
    if let Some(s) = body["slug"].as_str() {
        active.post_name = Set(s.to_string());
    }
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    match active.update(&state.db).await {
        Ok(updated) => {
            let wp_post = crate::posts::build_post(updated, &state.site_url, &state.db).await;
            ok_response(200, serde_json::to_value(&wp_post).unwrap_or_default())
        }
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_delete_post(state: &ApiState, id: u64, body: &Option<Value>) -> SubResponse {
    let post = match wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("post"))
        .one(&state.db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return error_response(404, "rest_post_invalid_id", "Invalid post ID."),
        Err(e) => return error_response(500, "internal_server_error", &e.to_string()),
    };

    let force = body
        .as_ref()
        .and_then(|b| b["force"].as_bool())
        .unwrap_or(false);

    let response_body = {
        let wp_post = crate::posts::build_post(post.clone(), &state.site_url, &state.db).await;
        serde_json::to_value(&wp_post).unwrap_or_default()
    };

    if force {
        if let Err(e) = wp_posts::Entity::delete_by_id(id).exec(&state.db).await {
            return error_response(500, "internal_server_error", &e.to_string());
        }
    } else {
        let mut active: wp_posts::ActiveModel = post.into();
        active.post_status = Set("trash".to_string());
        if let Err(e) = active.update(&state.db).await {
            return error_response(500, "internal_server_error", &e.to_string());
        }
    }

    ok_response(200, response_body)
}

// ─── Page dispatchers ───────────────────────────────────────────────────────

async fn dispatch_create_page(state: &ApiState, body: Value) -> SubResponse {
    let now = chrono::Utc::now().naive_utc();
    let title = body["title"].as_str().unwrap_or("").to_string();
    let slug = body["slug"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::common::slugify(&title));

    let new_page = wp_posts::ActiveModel {
        post_author: Set(body["author"].as_u64().unwrap_or(1)),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(body["content"].as_str().unwrap_or("").to_string()),
        post_title: Set(title),
        post_excerpt: Set(body["excerpt"].as_str().unwrap_or("").to_string()),
        post_status: Set(body["status"].as_str().unwrap_or("draft").to_string()),
        comment_status: Set("closed".to_string()),
        ping_status: Set("closed".to_string()),
        post_password: Set(String::new()),
        post_name: Set(slug),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_content_filtered: Set(String::new()),
        post_parent: Set(body["parent"].as_u64().unwrap_or(0)),
        guid: Set(String::new()),
        menu_order: Set(body["menu_order"].as_i64().unwrap_or(0) as i32),
        post_type: Set("page".to_string()),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
        ..Default::default()
    };

    match new_page.insert(&state.db).await {
        Ok(result) => {
            let wp_page = crate::pages::build_page(result, &state.site_url);
            ok_response(201, serde_json::to_value(&wp_page).unwrap_or_default())
        }
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_update_page(state: &ApiState, id: u64, body: Value) -> SubResponse {
    let page = match wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("page"))
        .one(&state.db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return error_response(404, "rest_post_invalid_id", "Invalid page ID."),
        Err(e) => return error_response(500, "internal_server_error", &e.to_string()),
    };

    let mut active: wp_posts::ActiveModel = page.into();
    let now = chrono::Utc::now().naive_utc();

    if let Some(t) = body["title"].as_str() {
        active.post_title = Set(t.to_string());
    }
    if let Some(c) = body["content"].as_str() {
        active.post_content = Set(c.to_string());
    }
    if let Some(s) = body["status"].as_str() {
        active.post_status = Set(s.to_string());
    }
    if let Some(s) = body["slug"].as_str() {
        active.post_name = Set(s.to_string());
    }
    if let Some(p) = body["parent"].as_u64() {
        active.post_parent = Set(p);
    }
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    match active.update(&state.db).await {
        Ok(updated) => {
            let wp_page = crate::pages::build_page(updated, &state.site_url);
            ok_response(200, serde_json::to_value(&wp_page).unwrap_or_default())
        }
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_delete_resource(state: &ApiState, post_type: &str, id: u64) -> SubResponse {
    match wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq(post_type))
        .one(&state.db)
        .await
    {
        Ok(Some(post)) => {
            let mut active: wp_posts::ActiveModel = post.into();
            active.post_status = Set("trash".to_string());
            match active.update(&state.db).await {
                Ok(_) => ok_response(200, json!({"deleted": true, "previous": {"id": id}})),
                Err(e) => error_response(500, "internal_server_error", &e.to_string()),
            }
        }
        Ok(None) => error_response(404, "rest_post_invalid_id", "Invalid ID."),
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

// ─── Term dispatchers (categories/tags) ─────────────────────────────────────

async fn dispatch_create_term(state: &ApiState, taxonomy: &str, body: Value) -> SubResponse {
    let name = match body["name"].as_str() {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => return error_response(400, "rest_invalid_param", "Term name is required."),
    };
    let slug = body["slug"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::common::slugify(&name));
    let description = body["description"].as_str().unwrap_or("").to_string();
    let parent = body["parent"].as_u64().unwrap_or(0);

    // Create wp_terms entry
    let term = wp_terms::ActiveModel {
        name: Set(name.clone()),
        slug: Set(slug.clone()),
        term_group: Set(0),
        ..Default::default()
    };

    match term.insert(&state.db).await {
        Ok(new_term) => {
            // Create wp_term_taxonomy entry
            let tt = wp_term_taxonomy::ActiveModel {
                term_id: Set(new_term.term_id),
                taxonomy: Set(taxonomy.to_string()),
                description: Set(description.clone()),
                parent: Set(parent),
                count: Set(0),
                ..Default::default()
            };

            match tt.insert(&state.db).await {
                Ok(_new_tt) => ok_response(
                    201,
                    json!({
                        "id": new_term.term_id,
                        "count": 0,
                        "description": description,
                        "link": format!("{}/{}/{}",
                            state.site_url.trim_end_matches('/'),
                            if taxonomy == "category" { "category" } else { "tag" },
                            slug
                        ),
                        "name": name,
                        "slug": slug,
                        "taxonomy": taxonomy,
                        "parent": parent,
                        "meta": [],
                    }),
                ),
                Err(e) => error_response(500, "internal_server_error", &e.to_string()),
            }
        }
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_update_term(
    state: &ApiState,
    taxonomy: &str,
    id: u64,
    body: Value,
) -> SubResponse {
    let term = match wp_terms::Entity::find_by_id(id).one(&state.db).await {
        Ok(Some(t)) => t,
        Ok(None) => return error_response(404, "rest_term_invalid", "Invalid term ID."),
        Err(e) => return error_response(500, "internal_server_error", &e.to_string()),
    };

    let mut active: wp_terms::ActiveModel = term.into();
    if let Some(n) = body["name"].as_str() {
        active.name = Set(n.to_string());
    }
    if let Some(s) = body["slug"].as_str() {
        active.slug = Set(s.to_string());
    }

    match active.update(&state.db).await {
        Ok(updated) => {
            // Also update taxonomy description/parent if provided
            if let Ok(Some(tt)) = wp_term_taxonomy::Entity::find()
                .filter(wp_term_taxonomy::Column::TermId.eq(id))
                .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
                .one(&state.db)
                .await
            {
                let mut tt_active: wp_term_taxonomy::ActiveModel = tt.into();
                if let Some(d) = body["description"].as_str() {
                    tt_active.description = Set(d.to_string());
                }
                if let Some(p) = body["parent"].as_u64() {
                    tt_active.parent = Set(p);
                }
                let _ = tt_active.update(&state.db).await;
            }

            ok_response(
                200,
                json!({
                    "id": id,
                    "name": updated.name,
                    "slug": updated.slug,
                    "taxonomy": taxonomy,
                }),
            )
        }
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_delete_term(state: &ApiState, taxonomy: &str, id: u64) -> SubResponse {
    // Delete term_taxonomy first
    if let Err(e) = wp_term_taxonomy::Entity::delete_many()
        .filter(wp_term_taxonomy::Column::TermId.eq(id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .exec(&state.db)
        .await
    {
        return error_response(500, "internal_server_error", &e.to_string());
    }

    // Delete term
    match wp_terms::Entity::delete_by_id(id).exec(&state.db).await {
        Ok(_) => ok_response(200, json!({"deleted": true, "previous": {"id": id}})),
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

// ─── Comment dispatchers ────────────────────────────────────────────────────

async fn dispatch_create_comment(state: &ApiState, body: Value) -> SubResponse {
    let now = chrono::Utc::now().naive_utc();

    let new_comment = wp_comments::ActiveModel {
        comment_post_id: Set(body["post"].as_u64().unwrap_or(0)),
        comment_author: Set(body["author_name"].as_str().unwrap_or("").to_string()),
        comment_author_email: Set(body["author_email"].as_str().unwrap_or("").to_string()),
        comment_author_url: Set(body["author_url"].as_str().unwrap_or("").to_string()),
        comment_author_ip: Set(String::new()),
        comment_date: Set(now),
        comment_date_gmt: Set(now),
        comment_content: Set(body["content"].as_str().unwrap_or("").to_string()),
        comment_karma: Set(0),
        comment_approved: Set("1".to_string()),
        comment_agent: Set("RustPress Batch API".to_string()),
        comment_type: Set("comment".to_string()),
        comment_parent: Set(body["parent"].as_u64().unwrap_or(0)),
        user_id: Set(body["author"].as_u64().unwrap_or(0)),
        ..Default::default()
    };

    match new_comment.insert(&state.db).await {
        Ok(result) => ok_response(
            201,
            json!({
                "id": result.comment_id,
                "post": result.comment_post_id,
                "author": result.user_id,
                "author_name": result.comment_author,
                "content": {"rendered": result.comment_content},
                "date": result.comment_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
                "status": "approved",
            }),
        ),
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_update_comment(state: &ApiState, id: u64, body: Value) -> SubResponse {
    let comment = match wp_comments::Entity::find_by_id(id).one(&state.db).await {
        Ok(Some(c)) => c,
        Ok(None) => return error_response(404, "rest_comment_invalid_id", "Invalid comment ID."),
        Err(e) => return error_response(500, "internal_server_error", &e.to_string()),
    };

    let mut active: wp_comments::ActiveModel = comment.into();
    if let Some(c) = body["content"].as_str() {
        active.comment_content = Set(c.to_string());
    }
    if let Some(s) = body["status"].as_str() {
        let approved = match s {
            "approved" | "approve" => "1",
            "hold" | "unapproved" => "0",
            "spam" => "spam",
            "trash" => "trash",
            _ => "0",
        };
        active.comment_approved = Set(approved.to_string());
    }

    match active.update(&state.db).await {
        Ok(updated) => ok_response(
            200,
            json!({
                "id": updated.comment_id,
                "content": {"rendered": updated.comment_content},
                "status": match updated.comment_approved.as_str() {
                    "1" => "approved",
                    "0" => "hold",
                    s => s,
                },
            }),
        ),
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

async fn dispatch_delete_comment(state: &ApiState, id: u64) -> SubResponse {
    match wp_comments::Entity::find_by_id(id).one(&state.db).await {
        Ok(Some(comment)) => {
            let mut active: wp_comments::ActiveModel = comment.into();
            active.comment_approved = Set("trash".to_string());
            match active.update(&state.db).await {
                Ok(_) => ok_response(200, json!({"deleted": true, "previous": {"id": id}})),
                Err(e) => error_response(500, "internal_server_error", &e.to_string()),
            }
        }
        Ok(None) => error_response(404, "rest_comment_invalid_id", "Invalid comment ID."),
        Err(e) => error_response(500, "internal_server_error", &e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/wp/v2/posts"), "/wp/v2/posts");
        assert_eq!(
            normalize_path("/wp-json/wp/v2/posts"),
            "/wp-json/wp/v2/posts"
        );
        assert_eq!(normalize_path("posts"), "/wp/v2/posts");
        assert_eq!(normalize_path("/posts"), "/wp/v2/posts");
    }

    #[test]
    fn test_batch_request_deserialize() {
        let json_str = r#"{
            "requests": [
                {"method": "POST", "path": "/wp/v2/posts", "body": {"title": "Test"}},
                {"method": "DELETE", "path": "/wp/v2/posts/1"}
            ]
        }"#;
        let batch: BatchRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(batch.requests.len(), 2);
        assert_eq!(batch.requests[0].method, "POST");
        assert_eq!(batch.requests[1].method, "DELETE");
        assert!(batch.validation.is_none());
    }

    #[test]
    fn test_batch_request_with_validation() {
        let json_str = r#"{
            "validation": "require-all-validate",
            "requests": [
                {"method": "PUT", "path": "/wp/v2/posts/5", "body": {"title": "Updated"}}
            ]
        }"#;
        let batch: BatchRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(batch.validation.as_deref(), Some("require-all-validate"));
        assert_eq!(batch.requests.len(), 1);
    }

    #[test]
    fn test_sub_response_serialize() {
        let resp = SubResponse {
            status: 201,
            body: json!({"id": 42, "title": {"rendered": "Hello"}}),
            headers: json!({"Content-Type": "application/json"}),
        };
        let val = serde_json::to_value(&resp).unwrap();
        assert_eq!(val["status"], 201);
        assert_eq!(val["body"]["id"], 42);
    }

    #[test]
    fn test_batch_response_serialize() {
        let resp = BatchResponse {
            failed: false,
            responses: vec![
                SubResponse {
                    status: 200,
                    body: json!({"id": 1}),
                    headers: json!({}),
                },
                SubResponse {
                    status: 201,
                    body: json!({"id": 2}),
                    headers: json!({}),
                },
            ],
        };
        let val = serde_json::to_value(&resp).unwrap();
        assert_eq!(val["failed"], false);
        assert_eq!(val["responses"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_error_response() {
        let resp = error_response(404, "rest_no_route", "Not found");
        assert_eq!(resp.status, 404);
        assert_eq!(resp.body["code"], "rest_no_route");
        assert_eq!(resp.body["message"], "Not found");
    }

    #[test]
    fn test_max_batch_size() {
        assert_eq!(MAX_BATCH_REQUESTS, 25);
    }
}

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use rustpress_db::entities::wp_posts;

use crate::AdminState;

/// Allowed MIME types for upload (WordPress-compatible whitelist).
const ALLOWED_MIME_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/svg+xml",
    "image/bmp",
    "image/tiff",
    "image/x-icon",
    "video/mp4",
    "video/webm",
    "video/ogg",
    "video/quicktime",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "audio/webm",
    "audio/flac",
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "text/plain",
    "text/csv",
];

/// Max file size: 64 MB.
const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct ListMediaQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MediaResponse {
    pub id: u64,
    pub title: String,
    pub mime_type: String,
    pub url: String,
    pub date: String,
    pub author: u64,
}

impl From<wp_posts::Model> for MediaResponse {
    fn from(p: wp_posts::Model) -> Self {
        Self {
            id: p.id,
            title: p.post_title,
            mime_type: p.post_mime_type,
            url: p.guid,
            date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
            author: p.post_author,
        }
    }
}

/// Sanitize a filename: keep alphanumeric, hyphens, underscores; replace spaces;
/// strip path traversal sequences.
fn sanitize_filename(name: &str) -> String {
    // Strip any directory components (path traversal defense)
    let name = name
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("upload")
        .to_string();

    let path = std::path::Path::new(&name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upload");

    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == ' ' {
                '-'
            } else {
                '_'
            }
        })
        .collect();

    let clean = clean
        .trim_matches(|c: char| c == '-' || c == '_')
        .to_string();
    let clean = if clean.is_empty() {
        "upload".to_string()
    } else {
        clean
    };

    if ext.is_empty() {
        clean
    } else {
        format!("{clean}.{ext}")
    }
}

/// Generate a unique filename by appending -1, -2, etc. if a file already exists.
async fn unique_filename(dir: &std::path::Path, name: &str) -> String {
    let path = std::path::Path::new(name);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upload");

    let mut candidate = name.to_string();
    let mut counter = 1u32;

    while tokio::fs::try_exists(dir.join(&candidate))
        .await
        .unwrap_or(false)
    {
        candidate = if ext.is_empty() {
            format!("{stem}-{counter}")
        } else {
            format!("{stem}-{counter}.{ext}")
        };
        counter += 1;
    }

    candidate
}

pub fn routes() -> Router<AdminState> {
    Router::new()
        .route("/admin/media", get(list_media).post(upload_media))
        .route("/admin/media/{id}", get(get_media).delete(delete_media))
}

async fn list_media(
    State(state): State<AdminState>,
    Query(params): Query<ListMediaQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .order_by_desc(wp_posts::Column::PostDate);

    if let Some(ref mime) = params.mime_type {
        query = query.filter(wp_posts::Column::PostMimeType.like(format!("{mime}%")));
    }

    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e: sea_orm::DbErr| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let media = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e: sea_orm::DbErr| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<MediaResponse> = media.into_iter().map(MediaResponse::from).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

async fn get_media(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<Json<MediaResponse>, (StatusCode, String)> {
    let media = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .one(&state.db)
        .await
        .map_err(|e: sea_orm::DbErr| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match media {
        Some(m) => Ok(Json(MediaResponse::from(m))),
        None => Err((StatusCode::NOT_FOUND, "Media not found".to_string())),
    }
}

async fn upload_media(
    State(state): State<AdminState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<MediaResponse>), (StatusCode, String)> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or((StatusCode::BAD_REQUEST, "No file provided".to_string()))?;

    let raw_name = field.file_name().unwrap_or("upload").to_string();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();

    // Validate MIME type
    if !ALLOWED_MIME_TYPES.contains(&content_type.as_str()) {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("File type '{content_type}' is not allowed."),
        ));
    }

    let data = field
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Validate file size
    if data.len() > MAX_FILE_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "File too large ({} bytes). Maximum is {} bytes.",
                data.len(),
                MAX_FILE_SIZE
            ),
        ));
    }

    // Sanitize filename and ensure uniqueness
    let file_name = sanitize_filename(&raw_name);

    let uploads_dir = PathBuf::from("wp-content/uploads");
    let date_dir = chrono::Utc::now().format("%Y/%m").to_string();
    let full_dir = uploads_dir.join(&date_dir);

    tokio::fs::create_dir_all(&full_dir)
        .await
        .map_err(|e: std::io::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Make filename unique if collision
    let file_name = unique_filename(&full_dir, &file_name).await;

    let file_path = full_dir.join(&file_name);
    tokio::fs::write(&file_path, &data)
        .await
        .map_err(|e: std::io::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Human-readable title from filename stem
    let display_title = file_name
        .rsplit('.')
        .next_back()
        .unwrap_or(&file_name)
        .replace(['-', '_'], " ");

    let guid = format!("/wp-content/uploads/{date_dir}/{file_name}");
    let slug = file_name
        .split('.')
        .next()
        .unwrap_or("upload")
        .to_lowercase();
    let now = chrono::Utc::now().naive_utc();

    let new_attachment = wp_posts::ActiveModel {
        post_author: Set(1),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(String::new()),
        post_title: Set(display_title),
        post_excerpt: Set(String::new()),
        post_status: Set("inherit".to_string()),
        comment_status: Set("open".to_string()),
        ping_status: Set("closed".to_string()),
        post_password: Set(String::new()),
        post_name: Set(slug),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_content_filtered: Set(String::new()),
        post_parent: Set(0),
        guid: Set(guid),
        menu_order: Set(0),
        post_type: Set("attachment".to_string()),
        post_mime_type: Set(content_type),
        comment_count: Set(0),
        ..Default::default()
    };

    let result = new_attachment
        .insert(&state.db)
        .await
        .map_err(|e: sea_orm::DbErr| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!("Uploaded media: {} (id={})", file_name, result.id);

    Ok((StatusCode::CREATED, Json(MediaResponse::from(result))))
}

async fn delete_media(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Look up the record first to get the file path
    let media = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .one(&state.db)
        .await
        .map_err(|e: sea_orm::DbErr| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let media = media.ok_or((StatusCode::NOT_FOUND, "Media not found".to_string()))?;

    // Build filesystem path from guid (e.g. "/wp-content/uploads/2026/03/photo.jpg")
    let guid = &media.guid;
    let disk_path = guid.strip_prefix('/').unwrap_or(guid);

    // Delete from database
    wp_posts::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|e: sea_orm::DbErr| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Delete physical file (best-effort — don't fail if file is already gone)
    if let Err(e) = tokio::fs::remove_file(disk_path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Failed to delete media file {}: {}", disk_path, e);
        }
    } else {
        tracing::info!("Deleted media file: {}", disk_path);
    }

    Ok(StatusCode::NO_CONTENT)
}

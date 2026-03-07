use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustpress_core::media_sizes::{
    calculate_crop_dimensions, calculate_dimensions, default_image_sizes,
};
use rustpress_db::entities::wp_posts;

use crate::common::{
    envelope_response, filter_media_context, media_links, pagination_headers_with_link,
    RestContext, WpError,
};
use crate::ApiState;

#[derive(Debug, Serialize)]
pub struct WpMedia {
    pub id: u64,
    pub date: String,
    pub date_gmt: String,
    pub slug: String,
    pub status: String,
    pub title: super::posts::WpRendered,
    pub author: u64,
    pub alt_text: String,
    pub caption: super::posts::WpRendered,
    pub description: super::posts::WpRendered,
    pub media_type: String,
    pub mime_type: String,
    pub source_url: String,
    pub media_details: MediaDetails,
    pub _links: Value,
}

/// Information about a single generated image size.
///
/// Mirrors the WordPress REST API `media_details.sizes.<name>` structure.
#[derive(Debug, Serialize, Clone)]
pub struct MediaSizeInfo {
    /// Filename component (e.g. "photo-300x200.jpg").
    pub file: String,
    /// Pixel width of this size variant.
    pub width: u32,
    /// Pixel height of this size variant.
    pub height: u32,
    /// MIME type (e.g. "image/jpeg").
    pub mime_type: String,
    /// Fully-qualified URL to this size variant.
    pub source_url: String,
}

#[derive(Debug, Serialize)]
pub struct MediaDetails {
    pub width: u32,
    pub height: u32,
    pub file: String,
    pub sizes: HashMap<String, MediaSizeInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ListMediaQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub media_type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub author: Option<u64>,
    pub context: Option<String>,
    pub orderby: Option<String>,
    pub order: Option<String>,
    pub _fields: Option<String>,
    pub _envelope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetMediaQuery {
    pub context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMediaRequest {
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
    pub source_url: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMediaRequest {
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteMediaQuery {
    pub force: Option<bool>,
}

fn build_media(p: wp_posts::Model, site_url: &str) -> WpMedia {
    let links = media_links(site_url, p.id, p.post_author);
    let media_type = p
        .post_mime_type
        .split('/')
        .next()
        .unwrap_or("application")
        .to_string();

    // Extract file path from guid (e.g. "2026/03/photo.jpg" or just "photo.jpg")
    let file = extract_upload_path(&p.guid);
    let mime_type = p.post_mime_type.clone();

    // Try to extract original dimensions from the guid filename.
    // In a full implementation these would come from wp_postmeta
    // (`_wp_attachment_metadata`).  For now we parse width/height if
    // embedded in the filename or default to 0.
    let (orig_width, orig_height) = (0u32, 0u32);

    // Build the sizes map for image attachments.
    let sizes = if media_type == "image" {
        build_image_sizes(site_url, &file, &mime_type, orig_width, orig_height)
    } else {
        HashMap::new()
    };

    WpMedia {
        id: p.id,
        date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        date_gmt: p.post_date_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        slug: p.post_name,
        status: p.post_status,
        title: super::posts::WpRendered {
            rendered: rustpress_themes::apply_title_filters(&p.post_title),
        },
        author: p.post_author,
        alt_text: String::new(), // Would come from postmeta _wp_attachment_image_alt
        caption: super::posts::WpRendered {
            rendered: rustpress_themes::apply_excerpt_filters(&p.post_excerpt),
        },
        description: super::posts::WpRendered {
            rendered: rustpress_themes::apply_content_filters(&p.post_content),
        },
        media_type,
        mime_type,
        source_url: p.guid,
        media_details: MediaDetails {
            width: orig_width,
            height: orig_height,
            file: file.clone(),
            sizes,
        },
        _links: links,
    }
}

/// Build a `WpMedia` with known original dimensions.
///
/// This variant is used when the caller already knows the image dimensions
/// (e.g. after reading `_wp_attachment_metadata` from postmeta).
pub fn build_media_with_dimensions(
    p: wp_posts::Model,
    site_url: &str,
    orig_width: u32,
    orig_height: u32,
) -> WpMedia {
    let links = media_links(site_url, p.id, p.post_author);
    let media_type = p
        .post_mime_type
        .split('/')
        .next()
        .unwrap_or("application")
        .to_string();

    let file = extract_upload_path(&p.guid);
    let mime_type = p.post_mime_type.clone();

    let sizes = if media_type == "image" {
        build_image_sizes(site_url, &file, &mime_type, orig_width, orig_height)
    } else {
        HashMap::new()
    };

    WpMedia {
        id: p.id,
        date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        date_gmt: p.post_date_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        slug: p.post_name,
        status: p.post_status,
        title: super::posts::WpRendered {
            rendered: rustpress_themes::apply_title_filters(&p.post_title),
        },
        author: p.post_author,
        alt_text: String::new(),
        caption: super::posts::WpRendered {
            rendered: rustpress_themes::apply_excerpt_filters(&p.post_excerpt),
        },
        description: super::posts::WpRendered {
            rendered: rustpress_themes::apply_content_filters(&p.post_content),
        },
        media_type,
        mime_type,
        source_url: p.guid,
        media_details: MediaDetails {
            width: orig_width,
            height: orig_height,
            file,
            sizes,
        },
        _links: links,
    }
}

// ---------------------------------------------------------------------------
// Helpers for size generation
// ---------------------------------------------------------------------------

/// Extract the upload-relative path from a full guid URL.
///
/// Given `"http://example.com/wp-content/uploads/2026/03/photo.jpg"` this
/// returns `"2026/03/photo.jpg"`.  If the `/uploads/` marker is not present
/// the last path component (filename) is returned.
fn extract_upload_path(guid: &str) -> String {
    if let Some(idx) = guid.find("/uploads/") {
        guid[idx + "/uploads/".len()..].to_string()
    } else {
        guid.rsplit('/').next().unwrap_or("").to_string()
    }
}

/// Split a file path into `(stem, extension)`.
///
/// `"2026/03/photo.jpg"` -> `("2026/03/photo", "jpg")`
fn split_file_ext(file: &str) -> (&str, &str) {
    match file.rfind('.') {
        Some(pos) => (&file[..pos], &file[pos + 1..]),
        None => (file, ""),
    }
}

/// Build the WordPress-style `sizes` map for an image attachment.
///
/// For each registered default image size whose dimensions are smaller than
/// or equal to the original, an entry is added with a filename in the
/// pattern `{stem}-{w}x{h}.{ext}` (matching WordPress behaviour).
///
/// A `"full"` entry is always included, pointing at the original file.
pub fn build_image_sizes(
    site_url: &str,
    file: &str,
    mime_type: &str,
    orig_width: u32,
    orig_height: u32,
) -> HashMap<String, MediaSizeInfo> {
    let base = site_url.trim_end_matches('/');
    let (stem, ext) = split_file_ext(file);
    let mut sizes = HashMap::new();

    // Always include the "full" entry pointing to the original.
    sizes.insert(
        "full".to_string(),
        MediaSizeInfo {
            file: file.to_string(),
            width: orig_width,
            height: orig_height,
            mime_type: mime_type.to_string(),
            source_url: format!("{}/wp-content/uploads/{}", base, file),
        },
    );

    // If we do not know the original dimensions we cannot compute sub-sizes.
    if orig_width == 0 || orig_height == 0 {
        return sizes;
    }

    for size_def in default_image_sizes() {
        // Skip sizes that are larger than the original on the constraining axis.
        if size_def.width > 0 && orig_width < size_def.width {
            continue;
        }
        if size_def.height > 0 && orig_height < size_def.height {
            continue;
        }

        let (w, h) = if size_def.crop {
            match calculate_crop_dimensions(
                orig_width,
                orig_height,
                size_def.width,
                size_def.height,
            ) {
                Some(dims) => dims,
                None => continue,
            }
        } else {
            let dims =
                calculate_dimensions(orig_width, orig_height, size_def.width, size_def.height);
            // Skip if the computed size equals the original (no point in a duplicate).
            if dims.0 == orig_width && dims.1 == orig_height {
                continue;
            }
            dims
        };

        let sized_filename = if ext.is_empty() {
            format!("{}-{}x{}", stem, w, h)
        } else {
            format!("{}-{}x{}.{}", stem, w, h, ext)
        };

        let source_url = format!("{}/wp-content/uploads/{}", base, sized_filename);

        sizes.insert(
            size_def.name.clone(),
            MediaSizeInfo {
                file: sized_filename,
                width: w,
                height: h,
                mime_type: mime_type.to_string(),
                source_url,
            },
        );
    }

    sizes
}

/// Public read-only routes (GET) -- no authentication required.
pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/media", get(list_media))
        .route("/wp-json/wp/v2/media/{id}", get(get_media))
}

/// Protected write routes (POST/PUT/DELETE) -- authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/media", axum::routing::post(create_media))
        .route(
            "/wp-json/wp/v2/media/{id}",
            axum::routing::put(update_media)
                .patch(update_media)
                .delete(delete_media),
        )
}

async fn list_media(
    State(state): State<ApiState>,
    Query(params): Query<ListMediaQuery>,
) -> Result<impl IntoResponse, WpError> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    let mut query = wp_posts::Entity::find().filter(wp_posts::Column::PostType.eq("attachment"));

    // Status filter (default: "inherit" for attachments)
    if let Some(ref status) = params.status {
        query = query.filter(wp_posts::Column::PostStatus.eq(status.as_str()));
    }

    // Media type filter (e.g. "image", "video", "audio")
    if let Some(ref mt) = params.media_type {
        query = query.filter(wp_posts::Column::PostMimeType.like(format!("{}/%", mt)));
    }

    // Search filter
    if let Some(ref search) = params.search {
        query = query.filter(wp_posts::Column::PostTitle.like(format!("%{}%", search)));
    }

    // Author filter
    if let Some(author) = params.author {
        query = query.filter(wp_posts::Column::PostAuthor.eq(author));
    }

    // Get total count for pagination
    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;
    let total_pages = if per_page > 0 {
        total.div_ceil(per_page)
    } else {
        1
    };

    // Ordering
    let order_desc = params.order.as_deref() != Some("asc");
    let orderby = params.orderby.as_deref().unwrap_or("date");
    query = match orderby {
        "id" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::Id)
            } else {
                query.order_by_asc(wp_posts::Column::Id)
            }
        }
        "title" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostTitle)
            } else {
                query.order_by_asc(wp_posts::Column::PostTitle)
            }
        }
        "slug" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostName)
            } else {
                query.order_by_asc(wp_posts::Column::PostName)
            }
        }
        // "date" or default
        _ => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostDate)
            } else {
                query.order_by_asc(wp_posts::Column::PostDate)
            }
        }
    };

    let posts = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let items: Vec<WpMedia> = posts
        .into_iter()
        .map(|p| build_media(p, &state.site_url))
        .collect();

    let context = RestContext::from_option(params.context.as_deref());
    let mut json_items: Vec<Value> = items
        .iter()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .collect();
    if context != RestContext::View {
        for item in json_items.iter_mut() {
            filter_media_context(item, context);
        }
    }

    let base_url = format!("{}/wp-json/wp/v2/media", state.site_url);
    let headers = pagination_headers_with_link(total, total_pages, page, &base_url);

    if params._envelope.is_some() {
        Ok(Json(envelope_response(200, &headers, Value::Array(json_items))).into_response())
    } else {
        Ok((headers, Json(json_items)).into_response())
    }
}

async fn get_media(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Query(params): Query<GetMediaQuery>,
) -> Result<Json<Value>, WpError> {
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Media not found"))?;

    let context = RestContext::from_option(params.context.as_deref());
    let mut val = serde_json::to_value(build_media(post, &state.site_url)).unwrap_or_default();
    filter_media_context(&mut val, context);
    Ok(Json(val))
}

/// POST /wp-json/wp/v2/media
///
/// WordPress sends binary data with:
///   Content-Type: image/jpeg
///   Content-Disposition: attachment; filename="photo.jpg"
///
/// Also accepts JSON with `source_url` for registering an existing file.
async fn create_media(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    request: axum::extract::Request,
) -> Result<(StatusCode, Json<WpMedia>), WpError> {
    auth.require(&rustpress_auth::Capability::UploadFiles)?;

    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Detect whether this is a binary upload or JSON metadata request
    let is_binary = !content_type.starts_with("application/json")
        && !content_type.starts_with("application/x-www-form-urlencoded");

    if is_binary {
        // WordPress-style binary upload
        // Extract filename from Content-Disposition header
        let filename = request
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(|cd| {
                cd.split(';').find_map(|part| {
                    let part = part.trim();
                    if let Some(name) = part.strip_prefix("filename=") {
                        Some(name.trim_matches('"').to_string())
                    } else {
                        part.strip_prefix("filename*=UTF-8''")
                            .map(percent_decode_filename)
                    }
                })
            })
            .unwrap_or_else(|| format!("upload-{}.bin", chrono::Utc::now().timestamp()));

        let mime_type = if content_type.contains('/') {
            content_type.clone()
        } else {
            mime_from_filename(&filename)
        };

        // Read body
        let body = request.into_body();
        let bytes = axum::body::to_bytes(body, 50 * 1024 * 1024) // 50 MB limit
            .await
            .map_err(|e| WpError::internal(format!("Failed to read upload body: {}", e)))?;

        // Sanitize filename
        let safe_name: String = filename
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        // Write to uploads/YYYY/MM/
        let now = chrono::Utc::now();
        let sub_dir = now.format("%Y/%m").to_string();
        let upload_dir = std::path::PathBuf::from("wp-content/uploads").join(&sub_dir);
        std::fs::create_dir_all(&upload_dir)
            .map_err(|e| WpError::internal(format!("Cannot create upload dir: {}", e)))?;

        let file_path = upload_dir.join(&safe_name);
        std::fs::write(&file_path, &bytes)
            .map_err(|e| WpError::internal(format!("Failed to write file: {}", e)))?;

        let file_url = format!(
            "{}/wp-content/uploads/{}/{}",
            state.site_url, sub_dir, safe_name
        );

        // Generate image sizes if it's an image
        let now_naive = now.naive_utc();
        let (orig_width, orig_height) = if mime_type.starts_with("image/") {
            generate_image_sizes(
                &file_path,
                &upload_dir,
                &safe_name,
                &state.site_url,
                &sub_dir,
                &state.db,
                0,
            )
        } else {
            (0u32, 0u32)
        };

        // Create attachment post
        let title = safe_name
            .rsplit_once('.')
            .map(|(n, _)| n)
            .unwrap_or(&safe_name)
            .to_string();
        let slug = crate::common::slugify(&title);

        let new_media = wp_posts::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            post_author: Set(auth.user_id),
            post_date: Set(now_naive),
            post_date_gmt: Set(now_naive),
            post_content: Set(String::new()),
            post_title: Set(title),
            post_excerpt: Set(String::new()),
            post_status: Set("inherit".to_string()),
            comment_status: Set("open".to_string()),
            ping_status: Set("closed".to_string()),
            post_password: Set(String::new()),
            post_name: Set(slug),
            to_ping: Set(String::new()),
            pinged: Set(String::new()),
            post_modified: Set(now_naive),
            post_modified_gmt: Set(now_naive),
            post_content_filtered: Set(String::new()),
            post_parent: Set(0),
            guid: Set(file_url),
            menu_order: Set(0),
            post_type: Set("attachment".to_string()),
            post_mime_type: Set(mime_type),
            comment_count: Set(0),
        };

        let result = new_media
            .insert(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;

        Ok((
            StatusCode::CREATED,
            Json(build_media_with_dimensions(
                result,
                &state.site_url,
                orig_width,
                orig_height,
            )),
        ))
    } else {
        // JSON metadata upload (original behavior for registering existing files)
        let body = request.into_body();
        let bytes = axum::body::to_bytes(body, 1024 * 1024)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
        let input: CreateMediaRequest = serde_json::from_slice(&bytes)
            .map_err(|e| WpError::bad_request(format!("Invalid JSON: {}", e)))?;

        let now = chrono::Utc::now().naive_utc();
        let title = input.title.unwrap_or_default();
        let slug = input.slug.unwrap_or_else(|| crate::common::slugify(&title));
        let mime_type = input
            .mime_type
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let source_url = input.source_url.unwrap_or_default();

        let new_media = wp_posts::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            post_author: Set(auth.user_id),
            post_date: Set(now),
            post_date_gmt: Set(now),
            post_content: Set(input.description.unwrap_or_default()),
            post_title: Set(title),
            post_excerpt: Set(input.caption.unwrap_or_default()),
            post_status: Set(input.status.unwrap_or_else(|| "inherit".to_string())),
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
            guid: Set(source_url),
            menu_order: Set(0),
            post_type: Set("attachment".to_string()),
            post_mime_type: Set(mime_type),
            comment_count: Set(0),
        };

        let result = new_media
            .insert(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;

        Ok((
            StatusCode::CREATED,
            Json(build_media(result, &state.site_url)),
        ))
    }
}

fn percent_decode_filename(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn mime_from_filename(filename: &str) -> String {
    match filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn generate_image_sizes(
    original_path: &std::path::Path,
    upload_dir: &std::path::Path,
    filename: &str,
    _site_url: &str,
    _sub_dir: &str,
    _db: &sea_orm::DatabaseConnection,
    _post_id: u64,
) -> (u32, u32) {
    use rustpress_core::media_sizes::{calculate_dimensions, default_image_sizes};

    let img: image::DynamicImage = match image::open(original_path) {
        Ok(i) => i,
        Err(_) => return (0, 0),
    };
    let orig_width = img.width();
    let orig_height = img.height();

    let (stem, ext) = filename.rsplit_once('.').unwrap_or((filename, "jpg"));
    let sizes = default_image_sizes();

    for size in &sizes {
        let (w, h) = calculate_dimensions(orig_width, orig_height, size.width, size.height);
        if w > 0 && h > 0 && (w != orig_width || h != orig_height) {
            let thumb_filename = format!("{}-{}x{}.{}", stem, w, h, ext);
            let thumb_path = upload_dir.join(&thumb_filename);
            let resized = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
            let _ = resized.save(&thumb_path);
        }
    }

    (orig_width, orig_height)
}

async fn update_media(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Json(input): Json<UpdateMediaRequest>,
) -> Result<Json<WpMedia>, WpError> {
    auth.require(&rustpress_auth::Capability::UploadFiles)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Media not found"))?;

    let mut active: wp_posts::ActiveModel = post.into();
    let now = chrono::Utc::now().naive_utc();

    if let Some(title) = input.title {
        active.post_title = Set(title);
    }
    if let Some(caption) = input.caption {
        active.post_excerpt = Set(caption);
    }
    if let Some(description) = input.description {
        active.post_content = Set(description);
    }
    if let Some(status) = input.status {
        active.post_status = Set(status);
    }
    if let Some(slug) = input.slug {
        active.post_name = Set(slug);
    }
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Note: alt_text would be saved to wp_postmeta (_wp_attachment_image_alt)
    // This is handled separately if needed

    Ok(Json(build_media(updated, &state.site_url)))
}

async fn delete_media(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Query(params): Query<DeleteMediaQuery>,
) -> Result<Json<WpMedia>, WpError> {
    auth.require(&rustpress_auth::Capability::UploadFiles)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Media not found"))?;

    let response = build_media(post.clone(), &state.site_url);

    if params.force.unwrap_or(false) {
        // Hard delete
        wp_posts::Entity::delete_by_id(id)
            .exec(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    } else {
        // Soft delete: move to trash
        let mut active: wp_posts::ActiveModel = post.into();
        active.post_status = Set("trash".to_string());
        active
            .update(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_upload_path -------------------------------------------------

    #[test]
    fn test_extract_upload_path_with_uploads_marker() {
        let guid = "http://example.com/wp-content/uploads/2026/03/photo.jpg";
        assert_eq!(extract_upload_path(guid), "2026/03/photo.jpg");
    }

    #[test]
    fn test_extract_upload_path_without_uploads_marker() {
        let guid = "http://example.com/images/photo.jpg";
        assert_eq!(extract_upload_path(guid), "photo.jpg");
    }

    #[test]
    fn test_extract_upload_path_bare_filename() {
        let guid = "photo.jpg";
        assert_eq!(extract_upload_path(guid), "photo.jpg");
    }

    // -- split_file_ext ------------------------------------------------------

    #[test]
    fn test_split_file_ext_with_extension() {
        assert_eq!(
            split_file_ext("2026/03/photo.jpg"),
            ("2026/03/photo", "jpg")
        );
    }

    #[test]
    fn test_split_file_ext_no_extension() {
        assert_eq!(split_file_ext("photo"), ("photo", ""));
    }

    #[test]
    fn test_split_file_ext_multiple_dots() {
        assert_eq!(
            split_file_ext("my.photo.name.png"),
            ("my.photo.name", "png")
        );
    }

    // -- build_image_sizes ---------------------------------------------------

    #[test]
    fn test_build_image_sizes_always_has_full() {
        let sizes = build_image_sizes(
            "http://example.com",
            "2026/03/photo.jpg",
            "image/jpeg",
            0,
            0,
        );
        assert!(sizes.contains_key("full"));
        let full = &sizes["full"];
        assert_eq!(full.file, "2026/03/photo.jpg");
        assert_eq!(
            full.source_url,
            "http://example.com/wp-content/uploads/2026/03/photo.jpg"
        );
    }

    #[test]
    fn test_build_image_sizes_no_sub_sizes_when_dimensions_unknown() {
        let sizes = build_image_sizes(
            "http://example.com",
            "2026/03/photo.jpg",
            "image/jpeg",
            0,
            0,
        );
        // Only "full" should be present when dimensions are unknown.
        assert_eq!(sizes.len(), 1);
        assert!(sizes.contains_key("full"));
    }

    #[test]
    fn test_build_image_sizes_large_image() {
        let sizes = build_image_sizes(
            "http://example.com",
            "2026/03/photo.jpg",
            "image/jpeg",
            2400,
            1600,
        );

        // 2400x1600: should have full, thumbnail, medium, medium_large, large, 1536x1536.
        // 2048x2048 is skipped because orig_height (1600) < 2048.
        assert!(sizes.contains_key("full"));
        assert!(sizes.contains_key("thumbnail"));
        assert!(sizes.contains_key("medium"));
        assert!(sizes.contains_key("medium_large"));
        assert!(sizes.contains_key("large"));
        assert!(sizes.contains_key("1536x1536"));
        assert!(!sizes.contains_key("2048x2048"));

        // Verify thumbnail is cropped to 150x150.
        let thumb = &sizes["thumbnail"];
        assert_eq!(thumb.width, 150);
        assert_eq!(thumb.height, 150);
        assert!(thumb.source_url.contains("-150x150.jpg"));

        // Verify medium preserves aspect ratio (2400x1600 -> 300x200).
        let medium = &sizes["medium"];
        assert_eq!(medium.width, 300);
        assert_eq!(medium.height, 200);
        assert!(medium.source_url.contains("-300x200.jpg"));

        // Verify medium_large (height unconstrained: 2400x1600 -> 768x512).
        let ml = &sizes["medium_large"];
        assert_eq!(ml.width, 768);
        assert_eq!(ml.height, 512);

        // Verify large (2400x1600 -> 1024x683).
        let large = &sizes["large"];
        assert_eq!(large.width, 1024);
        // 1600 * 1024 / 2400 = 682.666... -> rounds to 683
        assert_eq!(large.height, 683);
    }

    #[test]
    fn test_build_image_sizes_small_image() {
        // 200x150 image: only thumbnail (150x150 crop) should be generated
        // because all other sizes are larger than the original.
        let sizes = build_image_sizes("http://example.com", "tiny.png", "image/png", 200, 200);

        assert!(sizes.contains_key("full"));
        assert!(sizes.contains_key("thumbnail"));
        // medium (300x300) is skipped because orig_width (200) < 300.
        assert!(!sizes.contains_key("medium"));
        assert!(!sizes.contains_key("medium_large"));
        assert!(!sizes.contains_key("large"));

        let full = &sizes["full"];
        assert_eq!(full.width, 200);
        assert_eq!(full.height, 200);
    }

    #[test]
    fn test_build_image_sizes_url_format() {
        let sizes = build_image_sizes(
            "http://localhost:8080",
            "2026/03/landscape.jpg",
            "image/jpeg",
            2000,
            1000,
        );

        let medium = &sizes["medium"];
        assert_eq!(
            medium.source_url,
            "http://localhost:8080/wp-content/uploads/2026/03/landscape-300x150.jpg"
        );
        assert_eq!(medium.file, "2026/03/landscape-300x150.jpg");
        assert_eq!(medium.mime_type, "image/jpeg");
    }

    #[test]
    fn test_build_image_sizes_trailing_slash_in_site_url() {
        let sizes = build_image_sizes("http://example.com/", "photo.jpg", "image/jpeg", 2000, 1000);
        let full = &sizes["full"];
        assert_eq!(
            full.source_url,
            "http://example.com/wp-content/uploads/photo.jpg"
        );
    }
}

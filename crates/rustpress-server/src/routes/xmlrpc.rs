//! WordPress XML-RPC API compatibility module.
//!
//! Implements the subset of the WordPress XML-RPC API used by desktop blogging
//! clients such as MarsEdit, Windows Live Writer, and Open Live Writer.
//!
//! **Security**: XML-RPC is disabled by default. All requests to `/xmlrpc.php`
//! return 405 Method Not Allowed. The implementation is retained for optional
//! re-enablement via configuration.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, warn};

use rustpress_auth::PasswordHasher;
use rustpress_db::entities::{
    wp_comments, wp_postmeta, wp_posts, wp_term_relationships, wp_term_taxonomy, wp_terms,
    wp_usermeta, wp_users,
};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/xmlrpc.php", any(xmlrpc_blocked))
}

/// XML-RPC is disabled for security. Returns 405 Method Not Allowed
/// without an X-Pingback header.
async fn xmlrpc_blocked() -> impl IntoResponse {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        "XML-RPC services are disabled on this site.",
    )
}

// ---------------------------------------------------------------------------
// GET handler – RSD discovery & informational page (legacy, kept for reference)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RsdQuery {
    rsd: Option<String>,
}

async fn xmlrpc_get_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RsdQuery>,
) -> Response {
    // If ?rsd is present (any value, including empty), return RSD XML
    if params.rsd.is_some() {
        let site_url = &state.site_url;
        let rsd = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rsd version="1.0" xmlns="http://archipelago.phrasewise.com/rsd">
  <service>
    <engineName>RustPress</engineName>
    <engineLink>https://github.com/rustpress/rustpress</engineLink>
    <homePageLink>{site_url}</homePageLink>
    <apis>
      <api name="WordPress" blogID="1" preferred="true" apiLink="{site_url}/xmlrpc.php" />
      <api name="Movable Type" blogID="1" preferred="false" apiLink="{site_url}/xmlrpc.php" />
      <api name="MetaWeblog" blogID="1" preferred="false" apiLink="{site_url}/xmlrpc.php" />
      <api name="Blogger" blogID="1" preferred="false" apiLink="{site_url}/xmlrpc.php" />
    </apis>
  </service>
</rsd>"#
        );
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/rsd+xml; charset=UTF-8")],
            rsd,
        )
            .into_response();
    }

    // Plain GET – show a brief message (matches WordPress behaviour)
    let body = "XML-RPC server accepts POST requests only.";
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=UTF-8")],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// POST handler – XML-RPC dispatch
// ---------------------------------------------------------------------------

async fn xmlrpc_post_handler(State(state): State<Arc<AppState>>, body: Body) -> Response {
    // Read the full request body
    let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return xml_response(xml_rpc_fault(400, "Failed to read request body")),
    };

    let body_str = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return xml_response(xml_rpc_fault(400, "Invalid UTF-8 in request body")),
    };

    debug!("XML-RPC request body: {}", body_str);

    // Parse the XML-RPC request
    let (method_name, params) = match parse_xml_rpc_request(body_str) {
        Ok(r) => r,
        Err(e) => return xml_response(xml_rpc_fault(400, &format!("Parse error: {e}"))),
    };

    debug!(
        "XML-RPC method: {} with {} params",
        method_name,
        params.len()
    );

    // Dispatch to the appropriate handler
    let result = dispatch_method(&state, &method_name, &params).await;
    xml_response(result)
}

// ---------------------------------------------------------------------------
// Method dispatcher
// ---------------------------------------------------------------------------

async fn dispatch_method(state: &AppState, method: &str, params: &[XmlRpcValue]) -> String {
    match method {
        // System
        "system.listMethods" => handle_list_methods(),

        // WordPress API
        "wp.getUsersBlogs" => handle_get_users_blogs(state, params).await,
        "wp.getPost" => handle_get_post(state, params).await,
        "wp.getPosts" => handle_get_posts(state, params).await,
        "wp.newPost" => handle_new_post(state, params).await,
        "wp.editPost" => handle_edit_post(state, params).await,
        "wp.deletePost" => handle_delete_post(state, params).await,
        "wp.getCategories" | "wp.getTags" => handle_get_taxonomies(state, params, method).await,
        "wp.getOptions" => handle_get_options(state, params).await,
        "wp.setOptions" => handle_set_options(state, params).await,
        "wp.getProfile" => handle_get_profile(state, params).await,
        "wp.editProfile" => handle_edit_profile(state, params).await,
        "wp.getUsers" => handle_get_users(state, params).await,
        // Term methods
        "wp.getTaxonomy" => handle_get_taxonomy(state, params).await,
        "wp.getTaxonomies" => handle_get_taxonomies_list(state, params).await,
        "wp.getTerm" => handle_get_term(state, params).await,
        "wp.getTerms" => handle_get_terms(state, params).await,
        "wp.newTerm" => handle_new_term(state, params).await,
        "wp.editTerm" => handle_edit_term(state, params).await,
        "wp.deleteTerm" => handle_delete_term(state, params).await,
        "wp.newCategory" => handle_new_category(state, params).await,
        "wp.deleteCategory" => handle_delete_category(state, params).await,
        "wp.suggestCategories" => handle_suggest_categories(state, params).await,

        // Blogger legacy API
        "blogger.getUserInfo" => handle_blogger_get_user_info(state, params).await,
        "blogger.getUsersBlogs" => handle_get_users_blogs(state, params).await,

        // MetaWeblog API (many clients still use these)
        "metaWeblog.getPost" => handle_metaweblog_get_post(state, params).await,
        "metaWeblog.getRecentPosts" => handle_metaweblog_get_recent_posts(state, params).await,
        "metaWeblog.newPost" => handle_metaweblog_new_post(state, params).await,
        "metaWeblog.editPost" => handle_metaweblog_edit_post(state, params).await,
        "metaWeblog.newMediaObject" => handle_upload_file(state, params, 1, 2).await,

        // WordPress comment API
        "wp.getComments" => handle_get_comments(state, params).await,
        "wp.newComment" => handle_new_comment(state, params).await,
        "wp.editComment" => handle_edit_comment(state, params).await,
        "wp.deleteComment" => handle_delete_comment(state, params).await,

        // WordPress media API
        "wp.uploadFile" => handle_upload_file(state, params, 1, 2).await,
        "wp.getMediaItem" => handle_get_media_item(state, params).await,
        "wp.getMediaLibrary" => handle_get_media_library(state, params).await,

        // Pingback API
        "pingback.ping" => handle_pingback_ping(state, params).await,
        "pingback.extensions.getPingbacks" => handle_get_pingbacks(state, params).await,

        _ => {
            warn!("Unknown XML-RPC method: {}", method);
            xml_rpc_fault(405, &format!("Method not found: {method}"))
        }
    }
}

// ---------------------------------------------------------------------------
// XML-RPC value representation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum XmlRpcValue {
    String(String),
    Int(i64),
    Boolean(bool),
    Double(f64),
    DateTime(String), // ISO 8601
    Base64(String),
    Array(Vec<XmlRpcValue>),
    Struct(Vec<(String, XmlRpcValue)>),
}

impl XmlRpcValue {
    fn as_str(&self) -> &str {
        match self {
            XmlRpcValue::String(s) => s.as_str(),
            XmlRpcValue::Int(_) => {
                // Fallback – callers should use as_i64() instead
                ""
            }
            _ => "",
        }
    }

    fn as_i64(&self) -> i64 {
        match self {
            XmlRpcValue::Int(i) => *i,
            XmlRpcValue::String(s) => s.parse().unwrap_or(0),
            XmlRpcValue::Boolean(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    #[allow(dead_code)]
    fn as_bool(&self) -> bool {
        match self {
            XmlRpcValue::Boolean(b) => *b,
            XmlRpcValue::Int(i) => *i != 0,
            XmlRpcValue::String(s) => s == "1" || s.eq_ignore_ascii_case("true"),
            _ => false,
        }
    }

    fn get_member(&self, name: &str) -> Option<&XmlRpcValue> {
        match self {
            XmlRpcValue::Struct(members) => members.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    fn get_member_str(&self, name: &str) -> String {
        self.get_member(name)
            .map(|v| v.as_str().to_string())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    fn get_member_i64(&self, name: &str) -> i64 {
        self.get_member(name).map(|v| v.as_i64()).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// XML-RPC request parser (manual, no external XML-RPC crate)
// ---------------------------------------------------------------------------

fn parse_xml_rpc_request(xml: &str) -> Result<(String, Vec<XmlRpcValue>), String> {
    // Extract methodName
    let method_name = extract_tag_content(xml, "methodName")
        .ok_or("Missing <methodName>")?
        .to_string();

    // Extract params
    let mut params = Vec::new();
    if let Some(params_block) = extract_tag_content(xml, "params") {
        let mut rest = params_block;
        while let Some(param_content) = extract_tag_content(rest, "param") {
            if let Some(value_content) = extract_tag_content(param_content, "value") {
                params.push(parse_value(value_content)?);
            }
            // Advance past this <param>...</param> (nesting-aware)
            let skip = find_closing_tag_end(rest, "param");
            if skip > 0 {
                rest = &rest[skip..];
            } else {
                break;
            }
        }
    }

    Ok((method_name, params))
}

fn extract_tag_content<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start_pos = xml.find(&open)?;
    // Find the end of the opening tag (handle attributes)
    let after_open = &xml[start_pos + open.len()..];
    let tag_end = after_open.find('>')?;
    let content_start = start_pos + open.len() + tag_end + 1;

    // Find the *matching* closing tag by counting nesting depth.
    let search_region = &xml[content_start..];
    let mut depth = 1i32;
    let mut pos = 0;
    while pos < search_region.len() {
        if search_region[pos..].starts_with(&close) {
            depth -= 1;
            if depth == 0 {
                return Some(&xml[content_start..content_start + pos]);
            }
            pos += close.len();
        } else if search_region[pos..].starts_with(&open)
            && search_region
                .get(pos + open.len()..pos + open.len() + 1)
                .is_some_and(|ch| ch == ">" || ch == " " || ch == "/")
        {
            depth += 1;
            pos += open.len();
        } else {
            pos += 1;
        }
    }

    None
}

fn parse_value(value_xml: &str) -> Result<XmlRpcValue, String> {
    let trimmed = value_xml.trim();

    // Determine the type by looking at the FIRST tag in the content.
    // This prevents matching a nested tag (e.g. <string> inside a <struct>).
    let first_tag = detect_first_tag(trimmed);

    match first_tag.as_deref() {
        Some("string") => {
            let content = extract_tag_content(trimmed, "string").unwrap_or("");
            Ok(XmlRpcValue::String(xml_unescape(content)))
        }
        Some("int") => {
            let content = extract_tag_content(trimmed, "int").unwrap_or("0");
            let n = content
                .trim()
                .parse::<i64>()
                .map_err(|e| format!("Invalid int: {e}"))?;
            Ok(XmlRpcValue::Int(n))
        }
        Some("i4") => {
            let content = extract_tag_content(trimmed, "i4").unwrap_or("0");
            let n = content
                .trim()
                .parse::<i64>()
                .map_err(|e| format!("Invalid i4: {e}"))?;
            Ok(XmlRpcValue::Int(n))
        }
        Some("boolean") => {
            let content = extract_tag_content(trimmed, "boolean").unwrap_or("0");
            let b = content.trim() == "1";
            Ok(XmlRpcValue::Boolean(b))
        }
        Some("double") => {
            let content = extract_tag_content(trimmed, "double").unwrap_or("0");
            let d = content
                .trim()
                .parse::<f64>()
                .map_err(|e| format!("Invalid double: {e}"))?;
            Ok(XmlRpcValue::Double(d))
        }
        Some("dateTime.iso8601") => {
            let content = extract_tag_content(trimmed, "dateTime.iso8601").unwrap_or("");
            Ok(XmlRpcValue::DateTime(content.trim().to_string()))
        }
        Some("base64") => {
            let content = extract_tag_content(trimmed, "base64").unwrap_or("");
            Ok(XmlRpcValue::Base64(content.trim().to_string()))
        }
        Some("array") => {
            let array_content = extract_tag_content(trimmed, "array").unwrap_or("");
            let data_content = extract_tag_content(array_content, "data").unwrap_or(array_content);
            let mut items = Vec::new();
            let mut rest = data_content;
            while let Some(val_content) = extract_tag_content(rest, "value") {
                items.push(parse_value(val_content)?);
                // Advance past the matched </value> using nesting-aware skip
                let skip = find_closing_tag_end(rest, "value");
                if skip > 0 {
                    rest = &rest[skip..];
                } else {
                    break;
                }
            }
            Ok(XmlRpcValue::Array(items))
        }
        Some("struct") => {
            let struct_content = extract_tag_content(trimmed, "struct").unwrap_or("");
            let mut members = Vec::new();
            let mut rest = struct_content;
            while let Some(member_content) = extract_tag_content(rest, "member") {
                let name = extract_tag_content(member_content, "name")
                    .unwrap_or("")
                    .to_string();
                let value = if let Some(val_content) = extract_tag_content(member_content, "value")
                {
                    parse_value(val_content)?
                } else {
                    XmlRpcValue::String(String::new())
                };
                members.push((name, value));
                // Advance past the matched </member> using nesting-aware skip
                let skip = find_closing_tag_end(rest, "member");
                if skip > 0 {
                    rest = &rest[skip..];
                } else {
                    break;
                }
            }
            Ok(XmlRpcValue::Struct(members))
        }
        _ => {
            // Bare text inside <value>text</value> is treated as a string (per XML-RPC spec)
            Ok(XmlRpcValue::String(xml_unescape(trimmed)))
        }
    }
}

/// Detect the tag name of the first XML element in the given content.
/// Returns None if no tag is found (bare text).
fn detect_first_tag(xml: &str) -> Option<String> {
    let trimmed = xml.trim();
    if !trimmed.starts_with('<') {
        return None;
    }
    // Skip the '<'
    let rest = &trimmed[1..];
    // Find the end of the tag name (space, >, or /)
    let end = rest.find(['>', ' ', '/', '\n', '\r']).unwrap_or(rest.len());
    let tag_name = &rest[..end];
    if tag_name.is_empty() || tag_name.starts_with('/') {
        None
    } else {
        Some(tag_name.to_string())
    }
}

/// Find the end position (byte offset from start of `xml`) of the first
/// nesting-aware matched closing tag for `tag`.  Returns 0 if not found.
fn find_closing_tag_end(xml: &str, tag: &str) -> usize {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start_pos = match xml.find(&open) {
        Some(p) => p,
        None => return 0,
    };
    let after_open = &xml[start_pos + open.len()..];
    let tag_end = match after_open.find('>') {
        Some(p) => p,
        None => return 0,
    };
    let content_start = start_pos + open.len() + tag_end + 1;

    let search_region = &xml[content_start..];
    let mut depth = 1i32;
    let mut pos = 0;
    while pos < search_region.len() {
        if search_region[pos..].starts_with(&close) {
            depth -= 1;
            if depth == 0 {
                return content_start + pos + close.len();
            }
            pos += close.len();
        } else if search_region[pos..].starts_with(&open)
            && search_region
                .get(pos + open.len()..pos + open.len() + 1)
                .is_some_and(|ch| ch == ">" || ch == " " || ch == "/")
        {
            depth += 1;
            pos += open.len();
        } else {
            pos += 1;
        }
    }

    0
}

// ---------------------------------------------------------------------------
// XML-RPC response builders
// ---------------------------------------------------------------------------

fn xml_rpc_response(value: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<methodResponse>
<params>
<param>
{value}
</param>
</params>
</methodResponse>"#
    )
}

fn xml_rpc_fault(code: i32, message: &str) -> String {
    let escaped_msg = xml_escape(message);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<methodResponse>
<fault>
<value><struct>
<member><name>faultCode</name><value><int>{code}</int></value></member>
<member><name>faultString</name><value><string>{escaped_msg}</string></value></member>
</struct></value>
</fault>
</methodResponse>"#
    )
}

fn xml_response(body: String) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=UTF-8")],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// XML-RPC value serializers
// ---------------------------------------------------------------------------

fn value_string(s: &str) -> String {
    format!("<value><string>{}</string></value>", xml_escape(s))
}

fn value_int(i: i64) -> String {
    format!("<value><int>{i}</int></value>")
}

fn value_bool(b: bool) -> String {
    format!(
        "<value><boolean>{}</boolean></value>",
        if b { "1" } else { "0" }
    )
}

fn value_datetime(dt: &chrono::NaiveDateTime) -> String {
    format!(
        "<value><dateTime.iso8601>{}</dateTime.iso8601></value>",
        dt.format("%Y%m%dT%H:%M:%S")
    )
}

fn value_struct(members: &[(&str, String)]) -> String {
    let mut s = String::from("<value><struct>");
    for (name, value) in members {
        s.push_str(&format!(
            "<member><name>{}</name>{}</member>",
            xml_escape(name),
            value
        ));
    }
    s.push_str("</struct></value>");
    s
}

fn value_array(items: &[String]) -> String {
    let mut s = String::from("<value><array><data>");
    for item in items {
        s.push_str(item);
    }
    s.push_str("</data></array></value>");
    s
}

// ---------------------------------------------------------------------------
// XML escaping helpers
// ---------------------------------------------------------------------------

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

// ---------------------------------------------------------------------------
// Authentication helper
// ---------------------------------------------------------------------------

async fn authenticate(
    state: &AppState,
    username: &str,
    password: &str,
) -> Result<wp_users::Model, String> {
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(username))
        .one(&state.db)
        .await
        .map_err(|e| format!("Database error: {e}"))?;

    let user = user.ok_or_else(|| "Invalid username".to_string())?;

    let valid = PasswordHasher::verify(password, &user.user_pass)
        .map_err(|e| format!("Password check error: {e}"))?;

    if !valid {
        return Err("Invalid password".to_string());
    }

    Ok(user)
}

/// Authenticate using the first two params (username, password) – used by most methods.
async fn auth_from_params(
    state: &AppState,
    params: &[XmlRpcValue],
    username_idx: usize,
    password_idx: usize,
) -> Result<wp_users::Model, String> {
    let username = params
        .get(username_idx)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    let password = params
        .get(password_idx)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    authenticate(state, &username, &password).await
}

// ---------------------------------------------------------------------------
// Method handlers
// ---------------------------------------------------------------------------

/// system.listMethods – returns all supported method names.
fn handle_list_methods() -> String {
    let methods = [
        "system.listMethods",
        "wp.getUsersBlogs",
        "wp.getPost",
        "wp.getPosts",
        "wp.newPost",
        "wp.editPost",
        "wp.deletePost",
        "wp.getCategories",
        "wp.getTags",
        "wp.getOptions",
        "wp.setOptions",
        "wp.getProfile",
        "wp.editProfile",
        "wp.getUsers",
        "wp.getTaxonomy",
        "wp.getTaxonomies",
        "wp.getTerm",
        "wp.getTerms",
        "wp.newTerm",
        "wp.editTerm",
        "wp.deleteTerm",
        "wp.newCategory",
        "wp.deleteCategory",
        "wp.suggestCategories",
        "wp.getComments",
        "wp.newComment",
        "wp.editComment",
        "wp.deleteComment",
        "wp.uploadFile",
        "wp.getMediaItem",
        "wp.getMediaLibrary",
        "blogger.getUserInfo",
        "blogger.getUsersBlogs",
        "metaWeblog.getPost",
        "metaWeblog.getRecentPosts",
        "metaWeblog.newPost",
        "metaWeblog.editPost",
        "metaWeblog.newMediaObject",
        "pingback.ping",
        "pingback.extensions.getPingbacks",
    ];

    let items: Vec<String> = methods.iter().map(|m| value_string(m)).collect();
    xml_rpc_response(&value_array(&items))
}

/// wp.getUsersBlogs (username, password) -> array of blog structs
async fn handle_get_users_blogs(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 0, 1).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let blog_name = state.options.get_blogname().await.unwrap_or_default();
    let site_url = &state.site_url;

    let blog = value_struct(&[
        ("isAdmin", value_bool(true)),
        ("blogid", value_string("1")),
        ("blogName", value_string(&blog_name)),
        ("url", value_string(site_url)),
        ("xmlrpc", value_string(&format!("{site_url}/xmlrpc.php"))),
    ]);

    xml_rpc_response(&value_array(&[blog]))
}

/// wp.getPost (blog_id, username, password, post_id) -> post struct
async fn handle_get_post(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let post_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;

    let post = match wp_posts::Entity::find_by_id(post_id).one(&state.db).await {
        Ok(Some(p)) => p,
        Ok(None) => return xml_rpc_fault(404, "Post not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    xml_rpc_response(&post_to_xmlrpc(&post, &state.site_url))
}

/// wp.getPosts (blog_id, username, password, filter?) -> array of posts
async fn handle_get_posts(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    // Parse optional filter struct
    let filter = params.get(3);
    let post_type = filter
        .and_then(|f| f.get_member("post_type"))
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| "post".to_string());
    let post_status = filter
        .and_then(|f| f.get_member("post_status"))
        .map(|v| v.as_str().to_string());
    let number = filter
        .and_then(|f| f.get_member("number"))
        .map(|v| v.as_i64())
        .unwrap_or(10) as u64;
    let offset = filter
        .and_then(|f| f.get_member("offset"))
        .map(|v| v.as_i64())
        .unwrap_or(0) as u64;

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(&post_type))
        .order_by_desc(wp_posts::Column::PostDate);

    if let Some(status) = &post_status {
        query = query.filter(wp_posts::Column::PostStatus.eq(status.as_str()));
    }

    let posts = match query.offset(offset).limit(number).all(&state.db).await {
        Ok(p) => p,
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let items: Vec<String> = posts
        .iter()
        .map(|p| post_to_xmlrpc(p, &state.site_url))
        .collect();

    xml_rpc_response(&value_array(&items))
}

/// wp.newPost (blog_id, username, password, content_struct) -> post_id (string)
async fn handle_new_post(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let content = match params.get(3) {
        Some(c) => c,
        None => return xml_rpc_fault(400, "Missing content parameter"),
    };

    let title = content.get_member_str("post_title");
    let body = content.get_member_str("post_content");
    let status = {
        let s = content.get_member_str("post_status");
        if s.is_empty() {
            "draft".to_string()
        } else {
            s
        }
    };
    let post_type = {
        let t = content.get_member_str("post_type");
        if t.is_empty() {
            "post".to_string()
        } else {
            t
        }
    };
    let excerpt = content.get_member_str("post_excerpt");

    // Generate a slug from the title
    let slug = slugify(&title);
    let now = chrono::Utc::now().naive_utc();

    let new_post = wp_posts::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        post_author: Set(user.id),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(body),
        post_title: Set(title),
        post_excerpt: Set(excerpt),
        post_status: Set(status),
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
        post_type: Set(post_type),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
    };

    match new_post.insert(&state.db).await {
        Ok(inserted) => {
            // Update the guid to include the post ID (WordPress convention)
            let guid = format!("{}/?p={}", state.site_url, inserted.id);
            let mut active: wp_posts::ActiveModel = inserted.clone().into();
            active.guid = Set(guid);
            let _ = active.update(&state.db).await;

            xml_rpc_response(&value_string(&inserted.id.to_string()))
        }
        Err(e) => xml_rpc_fault(500, &format!("Failed to create post: {e}")),
    }
}

/// wp.editPost (blog_id, username, password, post_id, content_struct) -> boolean
async fn handle_edit_post(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let post_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;

    let existing = match wp_posts::Entity::find_by_id(post_id).one(&state.db).await {
        Ok(Some(p)) => p,
        Ok(None) => return xml_rpc_fault(404, "Post not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let content = match params.get(4) {
        Some(c) => c,
        None => return xml_rpc_fault(400, "Missing content parameter"),
    };

    let mut active: wp_posts::ActiveModel = existing.into();
    let now = chrono::Utc::now().naive_utc();
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    if let Some(v) = content.get_member("post_title") {
        let title = v.as_str().to_string();
        active.post_name = Set(slugify(&title));
        active.post_title = Set(title);
    }
    if let Some(v) = content.get_member("post_content") {
        active.post_content = Set(v.as_str().to_string());
    }
    if let Some(v) = content.get_member("post_status") {
        active.post_status = Set(v.as_str().to_string());
    }
    if let Some(v) = content.get_member("post_excerpt") {
        active.post_excerpt = Set(v.as_str().to_string());
    }

    match active.update(&state.db).await {
        Ok(_) => xml_rpc_response(&value_bool(true)),
        Err(e) => xml_rpc_fault(500, &format!("Failed to update post: {e}")),
    }
}

/// wp.deletePost (blog_id, username, password, post_id) -> boolean
/// Moves the post to trash (WordPress convention).
async fn handle_delete_post(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let post_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;

    let existing = match wp_posts::Entity::find_by_id(post_id).one(&state.db).await {
        Ok(Some(p)) => p,
        Ok(None) => return xml_rpc_fault(404, "Post not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let mut active: wp_posts::ActiveModel = existing.into();
    active.post_status = Set("trash".to_string());
    let now = chrono::Utc::now().naive_utc();
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    match active.update(&state.db).await {
        Ok(_) => xml_rpc_response(&value_bool(true)),
        Err(e) => xml_rpc_fault(500, &format!("Failed to delete post: {e}")),
    }
}

/// wp.getCategories / wp.getTags (blog_id, username, password) -> array of taxonomy structs
async fn handle_get_taxonomies(state: &AppState, params: &[XmlRpcValue], method: &str) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let taxonomy = if method.contains("Categor") {
        "category"
    } else {
        "post_tag"
    };

    // Join wp_terms with wp_term_taxonomy
    let taxonomies = match wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .all(&state.db)
        .await
    {
        Ok(t) => t,
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    // Build a map of term_id -> term_taxonomy, then fetch the term names
    let term_ids: Vec<u64> = taxonomies.iter().map(|t| t.term_id).collect();

    let terms = if term_ids.is_empty() {
        vec![]
    } else {
        wp_terms::Entity::find()
            .filter(wp_terms::Column::TermId.is_in(term_ids))
            .all(&state.db)
            .await
            .unwrap_or_default()
    };

    let term_map: std::collections::HashMap<u64, &wp_terms::Model> =
        terms.iter().map(|t| (t.term_id, t)).collect();

    let items: Vec<String> = taxonomies
        .iter()
        .filter_map(|tt| {
            let term = term_map.get(&tt.term_id)?;
            if taxonomy == "category" {
                Some(value_struct(&[
                    ("categoryId", value_string(&tt.term_taxonomy_id.to_string())),
                    ("categoryName", value_string(&term.name)),
                    ("categoryDescription", value_string(&tt.description)),
                    (
                        "htmlUrl",
                        value_string(&format!("{}/category/{}", state.site_url, term.slug)),
                    ),
                    (
                        "rssUrl",
                        value_string(&format!("{}/category/{}/feed", state.site_url, term.slug)),
                    ),
                ]))
            } else {
                Some(value_struct(&[
                    ("tag_id", value_string(&tt.term_taxonomy_id.to_string())),
                    ("name", value_string(&term.name)),
                    ("slug", value_string(&term.slug)),
                    ("count", value_int(tt.count)),
                ]))
            }
        })
        .collect();

    xml_rpc_response(&value_array(&items))
}

/// wp.getOptions (blog_id, username, password) -> struct of site options
async fn handle_get_options(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let blogname = state.options.get_blogname().await.unwrap_or_default();
    let blogdescription = state
        .options
        .get_blogdescription()
        .await
        .unwrap_or_default();
    let siteurl = state.options.get_siteurl().await.unwrap_or_default();
    let software_version = env!("CARGO_PKG_VERSION");

    let option_struct = |name: &str, desc: &str, value: &str, readonly: bool| -> (String, String) {
        let member_value = value_struct(&[
            ("desc", value_string(desc)),
            ("value", value_string(value)),
            ("readonly", value_bool(readonly)),
        ]);
        (name.to_string(), member_value)
    };

    let options = vec![
        option_struct("software_name", "Software Name", "RustPress", true),
        option_struct(
            "software_version",
            "Software Version",
            software_version,
            true,
        ),
        option_struct("blog_url", "WordPress Address (URL)", &siteurl, true),
        option_struct("home_url", "Site Address (URL)", &siteurl, true),
        option_struct("blog_title", "Site Title", &blogname, false),
        option_struct("blog_tagline", "Site Tagline", &blogdescription, false),
        option_struct("date_format", "Date Format", "Y-m-d", false),
        option_struct("time_format", "Time Format", "H:i", false),
        option_struct("time_zone", "Time Zone", "0", false),
    ];

    // Build the top-level struct manually
    let mut s = String::from("<value><struct>");
    for (name, value) in &options {
        s.push_str(&format!(
            "<member><name>{}</name>{}</member>",
            xml_escape(name),
            value
        ));
    }
    s.push_str("</struct></value>");

    xml_rpc_response(&s)
}

/// wp.getProfile (blog_id, username, password) -> user profile struct
async fn handle_get_profile(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let profile = value_struct(&[
        ("user_id", value_string(&user.id.to_string())),
        ("username", value_string(&user.user_login)),
        ("first_name", value_string("")),
        ("last_name", value_string("")),
        ("bio", value_string("")),
        ("email", value_string(&user.user_email)),
        ("nickname", value_string(&user.display_name)),
        ("nicename", value_string(&user.user_nicename)),
        ("url", value_string(&user.user_url)),
        ("display_name", value_string(&user.display_name)),
        (
            "registered",
            value_string(&user.user_registered.format("%Y-%m-%dT%H:%M:%S").to_string()),
        ),
    ]);

    xml_rpc_response(&profile)
}

/// wp.getUsers (blog_id, username, password, filter?) -> array of user structs
async fn handle_get_users(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _auth = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    // Get limit from filter struct (param index 3)
    let limit = params
        .get(3)
        .and_then(|v| {
            if let XmlRpcValue::Struct(fields) = v {
                fields
                    .iter()
                    .find(|(k, _)| k == "number")
                    .map(|(_, v)| v.as_i64() as u64)
            } else {
                None
            }
        })
        .unwrap_or(50);

    let users = match wp_users::Entity::find().limit(limit).all(&state.db).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let items: Vec<String> = users
        .iter()
        .map(|u| {
            value_struct(&[
                ("user_id", value_string(&u.id.to_string())),
                ("username", value_string(&u.user_login)),
                ("first_name", value_string("")),
                ("last_name", value_string("")),
                ("bio", value_string("")),
                ("email", value_string(&u.user_email)),
                ("nickname", value_string(&u.display_name)),
                ("nicename", value_string(&u.user_nicename)),
                ("url", value_string(&u.user_url)),
                ("display_name", value_string(&u.display_name)),
                (
                    "registered",
                    value_string(&u.user_registered.format("%Y-%m-%dT%H:%M:%S").to_string()),
                ),
                ("roles", value_array(&[value_string("administrator")])),
            ])
        })
        .collect();

    xml_rpc_response(&value_array(&items))
}

/// blogger.getUserInfo (appkey, username, password) -> user struct
async fn handle_blogger_get_user_info(state: &AppState, params: &[XmlRpcValue]) -> String {
    // blogger.getUserInfo has params: appkey, username, password
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let info = value_struct(&[
        ("userid", value_string(&user.id.to_string())),
        ("nickname", value_string(&user.display_name)),
        ("firstname", value_string("")),
        ("lastname", value_string("")),
        ("email", value_string(&user.user_email)),
        ("url", value_string(&user.user_url)),
    ]);

    xml_rpc_response(&info)
}

// ---------------------------------------------------------------------------
// MetaWeblog API handlers (used by many legacy clients)
// ---------------------------------------------------------------------------

/// metaWeblog.getPost (post_id, username, password) -> post struct
async fn handle_metaweblog_get_post(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let post_id = params.first().map(|v| v.as_i64()).unwrap_or(0) as u64;

    let post = match wp_posts::Entity::find_by_id(post_id).one(&state.db).await {
        Ok(Some(p)) => p,
        Ok(None) => return xml_rpc_fault(404, "Post not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    xml_rpc_response(&post_to_metaweblog(&post, &state.site_url))
}

/// metaWeblog.getRecentPosts (blog_id, username, password, numberOfPosts) -> array
async fn handle_metaweblog_get_recent_posts(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let num = params.get(3).map(|v| v.as_i64()).unwrap_or(10) as u64;

    let posts = match wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .order_by_desc(wp_posts::Column::PostDate)
        .limit(num)
        .all(&state.db)
        .await
    {
        Ok(p) => p,
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let items: Vec<String> = posts
        .iter()
        .map(|p| post_to_metaweblog(p, &state.site_url))
        .collect();

    xml_rpc_response(&value_array(&items))
}

/// metaWeblog.newPost (blog_id, username, password, content_struct, publish) -> post_id
async fn handle_metaweblog_new_post(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let content = match params.get(3) {
        Some(c) => c,
        None => return xml_rpc_fault(400, "Missing content parameter"),
    };

    let publish = params.get(4).map(|v| v.as_bool()).unwrap_or(false);

    let title = content.get_member_str("title");
    let body = content.get_member_str("description");
    let excerpt = content.get_member_str("mt_excerpt");
    let status = if publish { "publish" } else { "draft" };

    let slug = slugify(&title);
    let now = chrono::Utc::now().naive_utc();

    let new_post = wp_posts::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        post_author: Set(user.id),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(body),
        post_title: Set(title),
        post_excerpt: Set(excerpt),
        post_status: Set(status.to_string()),
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
    };

    match new_post.insert(&state.db).await {
        Ok(inserted) => {
            let guid = format!("{}/?p={}", state.site_url, inserted.id);
            let mut active: wp_posts::ActiveModel = inserted.clone().into();
            active.guid = Set(guid);
            let _ = active.update(&state.db).await;

            xml_rpc_response(&value_string(&inserted.id.to_string()))
        }
        Err(e) => xml_rpc_fault(500, &format!("Failed to create post: {e}")),
    }
}

/// metaWeblog.editPost (post_id, username, password, content_struct, publish) -> boolean
async fn handle_metaweblog_edit_post(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let post_id = params.first().map(|v| v.as_i64()).unwrap_or(0) as u64;

    let existing = match wp_posts::Entity::find_by_id(post_id).one(&state.db).await {
        Ok(Some(p)) => p,
        Ok(None) => return xml_rpc_fault(404, "Post not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let content = match params.get(3) {
        Some(c) => c,
        None => return xml_rpc_fault(400, "Missing content parameter"),
    };

    let publish = params.get(4).map(|v| v.as_bool()).unwrap_or(false);

    let mut active: wp_posts::ActiveModel = existing.into();
    let now = chrono::Utc::now().naive_utc();
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    if let Some(v) = content.get_member("title") {
        let title = v.as_str().to_string();
        active.post_name = Set(slugify(&title));
        active.post_title = Set(title);
    }
    if let Some(v) = content.get_member("description") {
        active.post_content = Set(v.as_str().to_string());
    }
    if let Some(v) = content.get_member("mt_excerpt") {
        active.post_excerpt = Set(v.as_str().to_string());
    }

    active.post_status = Set(if publish {
        "publish".to_string()
    } else {
        "draft".to_string()
    });

    match active.update(&state.db).await {
        Ok(_) => xml_rpc_response(&value_bool(true)),
        Err(e) => xml_rpc_fault(500, &format!("Failed to update post: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Comment methods
// ---------------------------------------------------------------------------

/// wp.getComments (blog_id, username, password, struct) -> array of comment structs
async fn handle_get_comments(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let filter = params.get(3);
    let post_id = filter
        .and_then(|f| f.get_member("post_id"))
        .map(|v| v.as_i64())
        .unwrap_or(0) as u64;
    let number = filter
        .and_then(|f| f.get_member("number"))
        .map(|v| v.as_i64())
        .unwrap_or(20) as u64;
    let status = filter
        .and_then(|f| f.get_member("status"))
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| "approve".to_string());

    let wp_status = match status.as_str() {
        "approve" | "approved" => "1",
        "hold" => "0",
        "spam" => "spam",
        _ => "1",
    };

    let mut query = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq(wp_status))
        .order_by_desc(wp_comments::Column::CommentDate)
        .limit(number);

    if post_id > 0 {
        query = query.filter(wp_comments::Column::CommentPostId.eq(post_id));
    }

    let comments = match query.all(&state.db).await {
        Ok(c) => c,
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let items: Vec<String> = comments.iter().map(comment_to_xmlrpc).collect();
    xml_rpc_response(&value_array(&items))
}

/// wp.newComment (blog_id, username, password, post_id, comment_struct) -> comment_id
async fn handle_new_comment(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let post_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;
    let comment_struct = match params.get(4) {
        Some(s) => s,
        None => return xml_rpc_fault(400, "Missing comment struct"),
    };

    let content = comment_struct.get_member_str("content");
    let comment_parent = comment_struct
        .get_member("comment_parent")
        .map(|v| v.as_i64())
        .unwrap_or(0) as u64;

    if content.is_empty() {
        return xml_rpc_fault(400, "Comment content is required");
    }

    let now = chrono::Utc::now().naive_utc();
    let new_comment = wp_comments::ActiveModel {
        comment_id: sea_orm::ActiveValue::NotSet,
        comment_post_id: Set(post_id),
        comment_author: Set(user.display_name.clone()),
        comment_author_email: Set(user.user_email.clone()),
        comment_author_url: Set(user.user_url.clone()),
        comment_author_ip: Set("127.0.0.1".to_string()),
        comment_date: Set(now),
        comment_date_gmt: Set(now),
        comment_content: Set(content),
        comment_karma: Set(0),
        comment_approved: Set("1".to_string()),
        comment_agent: Set("XML-RPC".to_string()),
        comment_type: Set("comment".to_string()),
        comment_parent: Set(comment_parent),
        user_id: Set(user.id),
    };

    match new_comment.insert(&state.db).await {
        Ok(inserted) => xml_rpc_response(&value_int(inserted.comment_id as i64)),
        Err(e) => xml_rpc_fault(500, &format!("Failed to create comment: {e}")),
    }
}

/// wp.editComment (blog_id, username, password, comment_id, comment_struct) -> boolean
async fn handle_edit_comment(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let comment_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;
    let existing = match wp_comments::Entity::find_by_id(comment_id)
        .one(&state.db)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => return xml_rpc_fault(404, "Comment not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let comment_struct = match params.get(4) {
        Some(s) => s,
        None => return xml_rpc_fault(400, "Missing comment struct"),
    };

    let mut active: wp_comments::ActiveModel = existing.into();

    if let Some(v) = comment_struct.get_member("content") {
        active.comment_content = Set(v.as_str().to_string());
    }
    if let Some(v) = comment_struct.get_member("status") {
        let approved = match v.as_str() {
            "approve" | "approved" => "1",
            "hold" => "0",
            "spam" => "spam",
            _ => "1",
        };
        active.comment_approved = Set(approved.to_string());
    }
    if let Some(v) = comment_struct.get_member("author") {
        active.comment_author = Set(v.as_str().to_string());
    }
    if let Some(v) = comment_struct.get_member("author_url") {
        active.comment_author_url = Set(v.as_str().to_string());
    }
    if let Some(v) = comment_struct.get_member("author_email") {
        active.comment_author_email = Set(v.as_str().to_string());
    }

    match active.update(&state.db).await {
        Ok(_) => xml_rpc_response(&value_bool(true)),
        Err(e) => xml_rpc_fault(500, &format!("Failed to update comment: {e}")),
    }
}

/// wp.deleteComment (blog_id, username, password, comment_id) -> boolean
async fn handle_delete_comment(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let comment_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;
    let existing = match wp_comments::Entity::find_by_id(comment_id)
        .one(&state.db)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => return xml_rpc_fault(404, "Comment not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let mut active: wp_comments::ActiveModel = existing.into();
    active.comment_approved = Set("trash".to_string());

    match active.update(&state.db).await {
        Ok(_) => xml_rpc_response(&value_bool(true)),
        Err(e) => xml_rpc_fault(500, &format!("Failed to delete comment: {e}")),
    }
}

fn comment_to_xmlrpc(c: &wp_comments::Model) -> String {
    let status = match c.comment_approved.as_str() {
        "1" => "approve",
        "0" => "hold",
        "spam" => "spam",
        _ => "hold",
    };
    value_struct(&[
        ("comment_id", value_string(&c.comment_id.to_string())),
        ("post_id", value_string(&c.comment_post_id.to_string())),
        ("parent", value_string(&c.comment_parent.to_string())),
        ("user_id", value_string(&c.user_id.to_string())),
        ("author", value_string(&c.comment_author)),
        ("author_url", value_string(&c.comment_author_url)),
        ("author_email", value_string(&c.comment_author_email)),
        ("author_ip", value_string(&c.comment_author_ip)),
        ("date", value_datetime(&c.comment_date)),
        ("date_gmt", value_datetime(&c.comment_date_gmt)),
        ("content", value_string(&c.comment_content)),
        ("status", value_string(status)),
        ("type", value_string(&c.comment_type)),
    ])
}

// ---------------------------------------------------------------------------
// Media methods
// ---------------------------------------------------------------------------

/// wp.uploadFile / metaWeblog.newMediaObject
/// (blog_id, username, password, data_struct) -> struct {file, url, type}
async fn handle_upload_file(
    state: &AppState,
    params: &[XmlRpcValue],
    user_idx: usize,
    pass_idx: usize,
) -> String {
    let user = match auth_from_params(state, params, user_idx, pass_idx).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let data_struct = match params.get(pass_idx + 1) {
        Some(d) => d,
        None => return xml_rpc_fault(400, "Missing data struct"),
    };

    let name = data_struct.get_member_str("name");
    let mime_type = data_struct.get_member_str("type");
    let bits_b64 = data_struct.get_member_str("bits");

    if name.is_empty() {
        return xml_rpc_fault(400, "Filename is required");
    }

    // Decode base64 content
    use base64::{engine::general_purpose, Engine as _};
    let file_bytes = match general_purpose::STANDARD
        .decode(bits_b64.replace(['\n', '\r', ' '], "").as_bytes())
    {
        Ok(b) => b,
        Err(e) => return xml_rpc_fault(400, &format!("Invalid base64: {e}")),
    };

    // Determine upload path (uploads/YYYY/MM/)
    let now = chrono::Utc::now();
    let sub_dir = now.format("%Y/%m").to_string();
    let upload_dir = std::path::PathBuf::from("wp-content/uploads").join(&sub_dir);
    if let Err(e) = std::fs::create_dir_all(&upload_dir) {
        return xml_rpc_fault(500, &format!("Failed to create upload dir: {e}"));
    }

    // Sanitize filename
    let safe_name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_path = upload_dir.join(&safe_name);

    if let Err(e) = std::fs::write(&file_path, &file_bytes) {
        return xml_rpc_fault(500, &format!("Failed to write file: {e}"));
    }

    let file_url = format!(
        "{}/wp-content/uploads/{}/{}",
        state.site_url, sub_dir, safe_name
    );

    // Insert attachment post record
    let now_naive = now.naive_utc();
    let new_attachment = wp_posts::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        post_author: Set(user.id),
        post_date: Set(now_naive),
        post_date_gmt: Set(now_naive),
        post_content: Set(String::new()),
        post_title: Set(safe_name.clone()),
        post_excerpt: Set(String::new()),
        post_status: Set("inherit".to_string()),
        comment_status: Set("open".to_string()),
        ping_status: Set("closed".to_string()),
        post_password: Set(String::new()),
        post_name: Set(safe_name.clone()),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now_naive),
        post_modified_gmt: Set(now_naive),
        post_content_filtered: Set(String::new()),
        post_parent: Set(0),
        guid: Set(file_url.clone()),
        menu_order: Set(0),
        post_type: Set("attachment".to_string()),
        post_mime_type: Set(if mime_type.is_empty() {
            "application/octet-stream".to_string()
        } else {
            mime_type.clone()
        }),
        comment_count: Set(0),
    };

    let attachment_id = match new_attachment.insert(&state.db).await {
        Ok(ins) => ins.id,
        Err(_) => 0,
    };

    // Store _wp_attached_file postmeta
    if attachment_id > 0 {
        let meta = wp_postmeta::ActiveModel {
            meta_id: sea_orm::ActiveValue::NotSet,
            post_id: Set(attachment_id),
            meta_key: Set(Some("_wp_attached_file".to_string())),
            meta_value: Set(Some(format!("{sub_dir}/{safe_name}"))),
        };
        let _ = meta.insert(&state.db).await;
    }

    xml_rpc_response(&value_struct(&[
        ("id", value_string(&attachment_id.to_string())),
        ("file", value_string(&safe_name)),
        ("url", value_string(&file_url)),
        ("type", value_string(&mime_type)),
    ]))
}

/// wp.getMediaItem (blog_id, username, password, attachment_id) -> struct
async fn handle_get_media_item(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let attachment_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;

    let post = match wp_posts::Entity::find_by_id(attachment_id)
        .one(&state.db)
        .await
    {
        Ok(Some(p)) if p.post_type == "attachment" => p,
        Ok(Some(_)) => return xml_rpc_fault(404, "Not an attachment"),
        Ok(None) => return xml_rpc_fault(404, "Media item not found"),
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    xml_rpc_response(&media_item_to_xmlrpc(&post))
}

/// wp.getMediaLibrary (blog_id, username, password, filter) -> array
async fn handle_get_media_library(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let filter = params.get(3);
    let number = filter
        .and_then(|f| f.get_member("number"))
        .map(|v| v.as_i64())
        .unwrap_or(20) as u64;
    let mime_type_filter = filter
        .and_then(|f| f.get_member("mime_type"))
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .order_by_desc(wp_posts::Column::PostDate)
        .limit(number);

    if !mime_type_filter.is_empty() {
        // Allow prefix match: "image" matches "image/jpeg", "image/png", etc.
        if !mime_type_filter.contains('/') {
            query =
                query.filter(wp_posts::Column::PostMimeType.like(format!("{mime_type_filter}/%")));
        } else {
            query = query.filter(wp_posts::Column::PostMimeType.eq(&mime_type_filter));
        }
    }

    let attachments = match query.all(&state.db).await {
        Ok(a) => a,
        Err(e) => return xml_rpc_fault(500, &format!("Database error: {e}")),
    };

    let items: Vec<String> = attachments.iter().map(media_item_to_xmlrpc).collect();
    xml_rpc_response(&value_array(&items))
}

fn media_item_to_xmlrpc(post: &wp_posts::Model) -> String {
    value_struct(&[
        ("attachment_id", value_string(&post.id.to_string())),
        ("date_created_gmt", value_datetime(&post.post_date_gmt)),
        ("parent", value_string(&post.post_parent.to_string())),
        ("link", value_string(&post.guid)),
        ("title", value_string(&post.post_title)),
        ("caption", value_string(&post.post_excerpt)),
        ("description", value_string(&post.post_content)),
        ("metadata", value_struct(&[])),
        ("type", value_string(&post.post_mime_type)),
    ])
}

// ---------------------------------------------------------------------------
// Pingback methods
// ---------------------------------------------------------------------------

/// pingback.ping (source_uri, target_uri) -> string
/// Called by remote blogs to notify us of incoming links.
async fn handle_pingback_ping(state: &AppState, params: &[XmlRpcValue]) -> String {
    let source_uri = params
        .first()
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    let target_uri = params
        .get(1)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();

    if source_uri.is_empty() || target_uri.is_empty() {
        return xml_rpc_fault(
            0x0010, // pingback error: source URI does not exist
            "Source URI or target URI is missing",
        );
    }

    // Extract the slug from target_uri
    let slug = target_uri
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();

    if slug.is_empty() {
        return xml_rpc_fault(0x0021, "Target is not a valid entry"); // pingback error
    }

    // Find the target post
    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(&slug))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let post = match post {
        Some(p) if p.post_type != "attachment" => p,
        _ => return xml_rpc_fault(0x0021, "Target is not a valid entry"),
    };

    // Check if a pingback from this source already exists
    let existing = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentPostId.eq(post.id))
        .filter(wp_comments::Column::CommentType.eq("pingback"))
        .filter(wp_comments::Column::CommentAuthorUrl.eq(&source_uri))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    if existing.is_some() {
        return xml_rpc_fault(
            0x0030,
            "The source URI has already been used for a pingback to the target URI",
        );
    }

    // Insert pingback comment
    let now = chrono::Utc::now().naive_utc();
    let pingback_content =
        format!("[…] <a href=\"{source_uri}\">Pingback from {source_uri}</a> […]");

    let new_pingback = wp_comments::ActiveModel {
        comment_id: sea_orm::ActiveValue::NotSet,
        comment_post_id: Set(post.id),
        comment_author: Set(source_uri.clone()),
        comment_author_email: Set(String::new()),
        comment_author_url: Set(source_uri.clone()),
        comment_author_ip: Set("0.0.0.0".to_string()),
        comment_date: Set(now),
        comment_date_gmt: Set(now),
        comment_content: Set(pingback_content),
        comment_karma: Set(0),
        comment_approved: Set("1".to_string()),
        comment_agent: Set("pingback".to_string()),
        comment_type: Set("pingback".to_string()),
        comment_parent: Set(0),
        user_id: Set(0),
    };

    match new_pingback.insert(&state.db).await {
        Ok(_) => xml_rpc_response(&value_string("Pingback registered.")),
        Err(e) => xml_rpc_fault(500, &format!("Failed to register pingback: {e}")),
    }
}

/// pingback.extensions.getPingbacks (post_uri) -> array of source URIs
async fn handle_get_pingbacks(state: &AppState, params: &[XmlRpcValue]) -> String {
    let target_uri = params
        .first()
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    let slug = target_uri
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();

    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(&slug))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let post_id = match post {
        Some(p) => p.id,
        None => return xml_rpc_fault(0x0021, "Target is not a valid entry"),
    };

    let pingbacks = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentPostId.eq(post_id))
        .filter(wp_comments::Column::CommentType.eq("pingback"))
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .all(&state.db)
        .await
        .unwrap_or_default();

    let urls: Vec<String> = pingbacks
        .iter()
        .map(|c| value_string(&c.comment_author_url))
        .collect();

    xml_rpc_response(&value_array(&urls))
}

// ---------------------------------------------------------------------------
// Post -> XML-RPC conversion helpers
// ---------------------------------------------------------------------------

fn post_to_xmlrpc(post: &wp_posts::Model, site_url: &str) -> String {
    let permalink = format!("{}/{}", site_url, post.post_name);

    value_struct(&[
        ("post_id", value_string(&post.id.to_string())),
        ("post_title", value_string(&post.post_title)),
        ("post_date", value_datetime(&post.post_date)),
        ("post_date_gmt", value_datetime(&post.post_date_gmt)),
        ("post_modified", value_datetime(&post.post_modified)),
        ("post_modified_gmt", value_datetime(&post.post_modified_gmt)),
        ("post_status", value_string(&post.post_status)),
        ("post_type", value_string(&post.post_type)),
        ("post_name", value_string(&post.post_name)),
        ("post_author", value_string(&post.post_author.to_string())),
        ("post_excerpt", value_string(&post.post_excerpt)),
        ("post_content", value_string(&post.post_content)),
        ("post_parent", value_string(&post.post_parent.to_string())),
        ("post_mime_type", value_string(&post.post_mime_type)),
        ("comment_status", value_string(&post.comment_status)),
        ("ping_status", value_string(&post.ping_status)),
        ("guid", value_string(&post.guid)),
        ("menu_order", value_int(post.menu_order as i64)),
        ("comment_count", value_int(post.comment_count)),
        ("link", value_string(&permalink)),
        ("terms", value_array(&[])),
        ("custom_fields", value_array(&[])),
    ])
}

fn post_to_metaweblog(post: &wp_posts::Model, site_url: &str) -> String {
    let permalink = format!("{}/{}", site_url, post.post_name);

    value_struct(&[
        ("postid", value_string(&post.id.to_string())),
        ("title", value_string(&post.post_title)),
        ("description", value_string(&post.post_content)),
        ("mt_excerpt", value_string(&post.post_excerpt)),
        ("link", value_string(&permalink)),
        ("permaLink", value_string(&permalink)),
        ("userid", value_string(&post.post_author.to_string())),
        ("dateCreated", value_datetime(&post.post_date_gmt)),
        ("date_created_gmt", value_datetime(&post.post_date_gmt)),
        ("date_modified", value_datetime(&post.post_modified_gmt)),
        ("date_modified_gmt", value_datetime(&post.post_modified_gmt)),
        ("post_status", value_string(&post.post_status)),
        ("wp_slug", value_string(&post.post_name)),
        ("wp_author_display_name", value_string("")),
        ("categories", value_array(&[])),
        ("mt_keywords", value_string("")),
    ])
}

// ---------------------------------------------------------------------------
// Term helpers
// ---------------------------------------------------------------------------

fn term_to_xmlrpc(term: &wp_terms::Model, tt: &wp_term_taxonomy::Model) -> String {
    value_struct(&[
        ("term_id", value_string(&term.term_id.to_string())),
        ("name", value_string(&term.name)),
        ("slug", value_string(&term.slug)),
        ("term_group", value_string(&term.term_group.to_string())),
        (
            "term_taxonomy_id",
            value_string(&tt.term_taxonomy_id.to_string()),
        ),
        ("taxonomy", value_string(&tt.taxonomy)),
        ("description", value_string(&tt.description)),
        ("parent", value_string(&tt.parent.to_string())),
        ("count", value_int(tt.count)),
        ("filter", value_string("raw")),
    ])
}

/// wp.getTaxonomy (blog_id, username, password, taxonomy)
async fn handle_get_taxonomy(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let taxonomy = params
        .get(3)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    xml_rpc_response(&value_struct(&[
        ("name", value_string(&taxonomy)),
        ("label", value_string(&taxonomy)),
        ("hierarchical", value_bool(taxonomy == "category")),
        ("public", value_bool(true)),
        ("show_ui", value_bool(true)),
        (
            "_builtin",
            value_bool(taxonomy == "category" || taxonomy == "post_tag"),
        ),
    ]))
}

/// wp.getTaxonomies (blog_id, username, password)
async fn handle_get_taxonomies_list(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let taxonomies = [
        "category",
        "post_tag",
        "post_format",
        "nav_menu",
        "link_category",
    ];
    let items: Vec<String> = taxonomies
        .iter()
        .map(|t| {
            value_struct(&[
                ("name", value_string(t)),
                ("label", value_string(t)),
                ("hierarchical", value_bool(*t == "category")),
                ("public", value_bool(true)),
                ("show_ui", value_bool(true)),
                ("_builtin", value_bool(*t == "category" || *t == "post_tag")),
            ])
        })
        .collect();
    xml_rpc_response(&value_array(&items))
}

/// wp.getTerm (blog_id, username, password, taxonomy, term_id)
async fn handle_get_term(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let taxonomy = params
        .get(3)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    let term_id = params.get(4).map(|v| v.as_i64()).unwrap_or(0) as u64;

    let term = match wp_terms::Entity::find_by_id(term_id).one(&state.db).await {
        Ok(Some(t)) => t,
        Ok(None) => return xml_rpc_fault(404, "Term not found"),
        Err(e) => return xml_rpc_fault(500, &e.to_string()),
    };

    let tt = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(term_id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(&taxonomy))
        .one(&state.db)
        .await
        .ok()
        .flatten();
    let tt = match tt {
        Some(t) => t,
        None => return xml_rpc_fault(404, "Term not found in taxonomy"),
    };

    xml_rpc_response(&term_to_xmlrpc(&term, &tt))
}

/// wp.getTerms (blog_id, username, password, taxonomy, filter?)
async fn handle_get_terms(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let taxonomy = params
        .get(3)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    let filter = params.get(4);
    let number = filter
        .and_then(|f| f.get_member("number"))
        .map(|v| v.as_i64())
        .unwrap_or(100) as u64;
    let search = filter
        .and_then(|f| f.get_member("search"))
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();

    let tts = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(&taxonomy))
        .limit(number)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut items = Vec::new();
    for tt in &tts {
        let term = wp_terms::Entity::find_by_id(tt.term_id)
            .one(&state.db)
            .await
            .ok()
            .flatten();
        if let Some(t) = term {
            if search.is_empty() || t.name.to_lowercase().contains(&search.to_lowercase()) {
                items.push(term_to_xmlrpc(&t, tt));
            }
        }
    }
    xml_rpc_response(&value_array(&items))
}

/// wp.newTerm (blog_id, username, password, content)
async fn handle_new_term(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    // Require edit_posts or manage_categories capability
    let role = xmlrpc_get_user_role(&state.db, user.id).await;
    if role == "subscriber" {
        return xml_rpc_fault(401, "You are not allowed to add terms.");
    }

    let content = match params.get(3) {
        Some(s) => s,
        None => return xml_rpc_fault(400, "Missing term content"),
    };
    let name = content.get_member_str("name");
    let taxonomy = content.get_member_str("taxonomy");
    let slug = {
        let s = content.get_member_str("slug");
        if s.is_empty() {
            slugify(&name)
        } else {
            s
        }
    };
    let description = content.get_member_str("description");
    let parent = content
        .get_member("parent")
        .map(|v| v.as_i64())
        .unwrap_or(0) as u64;

    if name.is_empty() || taxonomy.is_empty() {
        return xml_rpc_fault(400, "Term name and taxonomy are required");
    }

    // Insert into wp_terms
    let new_term = wp_terms::ActiveModel {
        term_id: sea_orm::ActiveValue::NotSet,
        name: Set(name.clone()),
        slug: Set(slug.clone()),
        term_group: Set(0),
    };
    let inserted = match new_term.insert(&state.db).await {
        Ok(t) => t,
        Err(e) => return xml_rpc_fault(500, &format!("Failed to insert term: {e}")),
    };

    // Insert into wp_term_taxonomy
    let new_tt = wp_term_taxonomy::ActiveModel {
        term_taxonomy_id: sea_orm::ActiveValue::NotSet,
        term_id: Set(inserted.term_id),
        taxonomy: Set(taxonomy.clone()),
        description: Set(description),
        parent: Set(parent),
        count: Set(0),
    };
    if let Err(e) = new_tt.insert(&state.db).await {
        return xml_rpc_fault(500, &format!("Failed to insert term taxonomy: {e}"));
    }

    xml_rpc_response(&value_string(&inserted.term_id.to_string()))
}

/// wp.editTerm (blog_id, username, password, term_id, content)
async fn handle_edit_term(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let role = xmlrpc_get_user_role(&state.db, user.id).await;
    if role == "subscriber" {
        return xml_rpc_fault(401, "You are not allowed to edit terms.");
    }

    let term_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;
    let content = match params.get(4) {
        Some(s) => s,
        None => return xml_rpc_fault(400, "Missing term content"),
    };

    let term = match wp_terms::Entity::find_by_id(term_id).one(&state.db).await {
        Ok(Some(t)) => t,
        Ok(None) => return xml_rpc_fault(404, "Term not found"),
        Err(e) => return xml_rpc_fault(500, &e.to_string()),
    };

    let mut active: wp_terms::ActiveModel = term.into();
    if let Some(v) = content.get_member("name") {
        active.name = Set(v.as_str().to_string());
    }
    if let Some(v) = content.get_member("slug") {
        active.slug = Set(v.as_str().to_string());
    }

    if let Err(e) = active.update(&state.db).await {
        return xml_rpc_fault(500, &format!("Failed to update term: {e}"));
    }

    // Update term_taxonomy
    if let Some(tt) = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(term_id))
        .one(&state.db)
        .await
        .ok()
        .flatten()
    {
        let mut tta: wp_term_taxonomy::ActiveModel = tt.into();
        if let Some(v) = content.get_member("description") {
            tta.description = Set(v.as_str().to_string());
        }
        if let Some(v) = content.get_member("parent") {
            tta.parent = Set(v.as_i64() as u64);
        }
        tta.update(&state.db).await.ok();
    }

    xml_rpc_response(&value_bool(true))
}

/// wp.deleteTerm (blog_id, username, password, taxonomy, term_id)
async fn handle_delete_term(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let role = xmlrpc_get_user_role(&state.db, user.id).await;
    if role == "subscriber" {
        return xml_rpc_fault(401, "You are not allowed to delete terms.");
    }

    let _taxonomy = params
        .get(3)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    let term_id = params.get(4).map(|v| v.as_i64()).unwrap_or(0) as u64;

    // Find all term_taxonomy_ids for this term, then delete relationships
    let tt_ids: Vec<u64> = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(term_id))
        .all(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|tt| tt.term_taxonomy_id)
        .collect();
    if !tt_ids.is_empty() {
        wp_term_relationships::Entity::delete_many()
            .filter(wp_term_relationships::Column::TermTaxonomyId.is_in(tt_ids))
            .exec(&state.db)
            .await
            .ok();
    }

    // Delete term taxonomy entries
    wp_term_taxonomy::Entity::delete_many()
        .filter(wp_term_taxonomy::Column::TermId.eq(term_id))
        .exec(&state.db)
        .await
        .ok();

    // Delete term
    if let Ok(Some(term)) = wp_terms::Entity::find_by_id(term_id).one(&state.db).await {
        let active: wp_terms::ActiveModel = term.into();
        if let Err(e) = active.delete(&state.db).await {
            return xml_rpc_fault(500, &format!("Failed to delete term: {e}"));
        }
    } else {
        return xml_rpc_fault(404, "Term not found");
    }

    xml_rpc_response(&value_bool(true))
}

/// wp.newCategory (blog_id, username, password, category) — legacy
async fn handle_new_category(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let role = xmlrpc_get_user_role(&state.db, user.id).await;
    if role == "subscriber" {
        return xml_rpc_fault(401, "You are not allowed to add categories.");
    }

    let cat = match params.get(3) {
        Some(s) => s,
        None => return xml_rpc_fault(400, "Missing category struct"),
    };
    let name = cat.get_member_str("name");
    let slug = {
        let s = cat.get_member_str("slug");
        if s.is_empty() {
            slugify(&name)
        } else {
            s
        }
    };
    let description = cat.get_member_str("categoryDescription");
    let parent = cat
        .get_member("category_parent")
        .map(|v| v.as_i64())
        .unwrap_or(0) as u64;

    if name.is_empty() {
        return xml_rpc_fault(400, "Category name is required");
    }

    let new_term = wp_terms::ActiveModel {
        term_id: sea_orm::ActiveValue::NotSet,
        name: Set(name),
        slug: Set(slug),
        term_group: Set(0),
    };
    let inserted = match new_term.insert(&state.db).await {
        Ok(t) => t,
        Err(e) => return xml_rpc_fault(500, &format!("Failed to create category: {e}")),
    };

    let new_tt = wp_term_taxonomy::ActiveModel {
        term_taxonomy_id: sea_orm::ActiveValue::NotSet,
        term_id: Set(inserted.term_id),
        taxonomy: Set("category".to_string()),
        description: Set(description),
        parent: Set(parent),
        count: Set(0),
    };
    new_tt.insert(&state.db).await.ok();

    xml_rpc_response(&value_int(inserted.term_id as i64))
}

/// wp.deleteCategory (blog_id, username, password, category_id) — legacy
async fn handle_delete_category(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let role = xmlrpc_get_user_role(&state.db, user.id).await;
    if role == "subscriber" {
        return xml_rpc_fault(401, "You are not allowed to delete categories.");
    }

    let cat_id = params.get(3).map(|v| v.as_i64()).unwrap_or(0) as u64;

    wp_term_taxonomy::Entity::delete_many()
        .filter(wp_term_taxonomy::Column::TermId.eq(cat_id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .exec(&state.db)
        .await
        .ok();

    if let Ok(Some(term)) = wp_terms::Entity::find_by_id(cat_id).one(&state.db).await {
        let active: wp_terms::ActiveModel = term.into();
        active.delete(&state.db).await.ok();
    }

    xml_rpc_response(&value_bool(true))
}

/// wp.suggestCategories (blog_id, username, password, suggest, max_results?)
async fn handle_suggest_categories(state: &AppState, params: &[XmlRpcValue]) -> String {
    let _user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let suggest = params
        .get(3)
        .map(|v| v.as_str().to_string())
        .unwrap_or_default();
    let max = params.get(4).map(|v| v.as_i64()).unwrap_or(10) as u64;

    let tts = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .limit(100)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut items = Vec::new();
    for tt in &tts {
        if items.len() >= max as usize {
            break;
        }
        let term = wp_terms::Entity::find_by_id(tt.term_id)
            .one(&state.db)
            .await
            .ok()
            .flatten();
        if let Some(t) = term {
            if suggest.is_empty() || t.name.to_lowercase().contains(&suggest.to_lowercase()) {
                items.push(value_struct(&[
                    ("category_id", value_int(t.term_id as i64)),
                    ("category_name", value_string(&t.name)),
                ]));
            }
        }
    }
    xml_rpc_response(&value_array(&items))
}

/// wp.setOptions (blog_id, username, password, options)
async fn handle_set_options(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };
    let role = xmlrpc_get_user_role(&state.db, user.id).await;
    if role != "administrator" {
        return xml_rpc_fault(401, "You are not allowed to manage options.");
    }

    let options = match params.get(3) {
        Some(XmlRpcValue::Struct(members)) => members.clone(),
        _ => return xml_rpc_fault(400, "Options must be a struct"),
    };

    let mut updated = Vec::new();
    for (key, val) in &options {
        // Each value is a struct with a "value" member, or a plain value
        let new_val = if let Some(v) = val.get_member("value") {
            v.as_str().to_string()
        } else {
            val.as_str().to_string()
        };
        state.options.update_option(key, &new_val).await.ok();
        updated.push((
            key.as_str(),
            value_struct(&[
                ("value", value_string(&new_val)),
                ("readonly", value_bool(false)),
            ]),
        ));
    }

    // Return the updated options
    let items: Vec<(&str, String)> = updated.iter().map(|(k, v)| (*k, v.clone())).collect();
    xml_rpc_response(&value_struct(&items))
}

/// wp.editProfile (blog_id, username, password, profile)
async fn handle_edit_profile(state: &AppState, params: &[XmlRpcValue]) -> String {
    let user = match auth_from_params(state, params, 1, 2).await {
        Ok(u) => u,
        Err(e) => return xml_rpc_fault(403, &e),
    };

    let profile = match params.get(3) {
        Some(s) => s,
        None => return xml_rpc_fault(400, "Missing profile struct"),
    };

    let user_id = user.id;
    let mut active: wp_users::ActiveModel = user.into();
    if let Some(v) = profile.get_member("email") {
        active.user_email = Set(v.as_str().to_string());
    }
    if let Some(v) = profile.get_member("display_name") {
        active.display_name = Set(v.as_str().to_string());
    }
    if let Some(v) = profile.get_member("nicename") {
        active.user_nicename = Set(v.as_str().to_string());
    }
    if let Some(v) = profile.get_member("url") {
        active.user_url = Set(v.as_str().to_string());
    }
    if let Some(v) = profile.get_member("password") {
        let new_pass = v.as_str();
        if !new_pass.is_empty() {
            if let Ok(hash) = PasswordHasher::hash_argon2(new_pass) {
                active.user_pass = Set(hash);
            }
        }
    }

    match active.update(&state.db).await {
        Ok(_) => {
            if let Some(v) = profile.get_member("nickname") {
                xmlrpc_upsert_usermeta(&state.db, user_id, "nickname", v.as_str()).await;
            }
            if let Some(v) = profile.get_member("bio") {
                xmlrpc_upsert_usermeta(&state.db, user_id, "description", v.as_str()).await;
            }
            xml_rpc_response(&value_bool(true))
        }
        Err(e) => xml_rpc_fault(500, &format!("Failed to update profile: {e}")),
    }
}

async fn xmlrpc_upsert_usermeta(
    db: &sea_orm::DatabaseConnection,
    user_id: u64,
    key: &str,
    value: &str,
) {
    if let Ok(Some(meta)) = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq(key))
        .one(db)
        .await
    {
        let mut active: wp_usermeta::ActiveModel = meta.into();
        active.meta_value = Set(Some(value.to_string()));
        active.update(db).await.ok();
    } else {
        wp_usermeta::ActiveModel {
            umeta_id: sea_orm::ActiveValue::NotSet,
            user_id: Set(user_id),
            meta_key: Set(Some(key.to_string())),
            meta_value: Set(Some(value.to_string())),
        }
        .insert(db)
        .await
        .ok();
    }
}

/// Helper: get role string for a user from wp_usermeta.
async fn xmlrpc_get_user_role(db: &sea_orm::DatabaseConnection, user_id: u64) -> String {
    let meta = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq("wp_capabilities"))
        .one(db)
        .await
        .ok()
        .flatten();
    if let Some(m) = meta {
        if let Some(val) = m.meta_value {
            if val.contains("administrator") {
                return "administrator".to_string();
            }
            if val.contains("editor") {
                return "editor".to_string();
            }
            if val.contains("author") {
                return "author".to_string();
            }
            if val.contains("contributor") {
                return "contributor".to_string();
            }
        }
    }
    "subscriber".to_string()
}

// ---------------------------------------------------------------------------
// Slug generation
// ---------------------------------------------------------------------------

fn slugify(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c == ' ' || c == '_' || c == '-' {
                '-'
            } else {
                // Skip non-ASCII-alphanumeric chars
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect();

    // Collapse multiple dashes
    let mut result = String::with_capacity(slug.len());
    let mut last_was_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !last_was_dash && !result.is_empty() {
                result.push('-');
                last_was_dash = true;
            }
        } else {
            result.push(c);
            last_was_dash = false;
        }
    }

    // Trim trailing dash
    if result.ends_with('-') {
        result.pop();
    }

    if result.is_empty() {
        "untitled".to_string()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
        assert_eq!(slugify("Special!@#$%Chars"), "specialchars");
        assert_eq!(slugify("Already-a-slug"), "already-a-slug");
        assert_eq!(slugify(""), "untitled");
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("Hello & World"), "Hello &amp; World");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("a\"b'c"), "a&quot;b&apos;c");
    }

    #[test]
    fn test_parse_xml_rpc_request_simple() {
        let xml = r#"<?xml version="1.0"?>
<methodCall>
  <methodName>system.listMethods</methodName>
  <params></params>
</methodCall>"#;

        let (method, params) = parse_xml_rpc_request(xml).unwrap();
        assert_eq!(method, "system.listMethods");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_parse_xml_rpc_request_with_params() {
        let xml = r#"<?xml version="1.0"?>
<methodCall>
  <methodName>wp.getUsersBlogs</methodName>
  <params>
    <param><value><string>admin</string></value></param>
    <param><value><string>password</string></value></param>
  </params>
</methodCall>"#;

        let (method, params) = parse_xml_rpc_request(xml).unwrap();
        assert_eq!(method, "wp.getUsersBlogs");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].as_str(), "admin");
        assert_eq!(params[1].as_str(), "password");
    }

    #[test]
    fn test_parse_xml_rpc_struct_param() {
        let xml = r#"<?xml version="1.0"?>
<methodCall>
  <methodName>wp.newPost</methodName>
  <params>
    <param><value><string>1</string></value></param>
    <param><value><string>admin</string></value></param>
    <param><value><string>password</string></value></param>
    <param><value><struct>
      <member><name>post_title</name><value><string>Test Title</string></value></member>
      <member><name>post_content</name><value><string>Test Content</string></value></member>
      <member><name>post_status</name><value><string>draft</string></value></member>
    </struct></value></param>
  </params>
</methodCall>"#;

        let (method, params) = parse_xml_rpc_request(xml).unwrap();
        assert_eq!(method, "wp.newPost");
        assert_eq!(params.len(), 4);
        assert_eq!(params[3].get_member_str("post_title"), "Test Title");
        assert_eq!(params[3].get_member_str("post_content"), "Test Content");
        assert_eq!(params[3].get_member_str("post_status"), "draft");
    }

    #[test]
    fn test_parse_xml_rpc_int_and_bool() {
        let xml = r#"<?xml version="1.0"?>
<methodCall>
  <methodName>test</methodName>
  <params>
    <param><value><int>42</int></value></param>
    <param><value><boolean>1</boolean></value></param>
    <param><value><i4>-7</i4></value></param>
  </params>
</methodCall>"#;

        let (_, params) = parse_xml_rpc_request(xml).unwrap();
        assert_eq!(params[0].as_i64(), 42);
        assert!(params[1].as_bool());
        assert_eq!(params[2].as_i64(), -7);
    }

    #[test]
    fn test_xml_rpc_fault_output() {
        let fault = xml_rpc_fault(403, "Access denied");
        assert!(fault.contains("<fault>"));
        assert!(fault.contains("faultCode"));
        assert!(fault.contains("403"));
        assert!(fault.contains("Access denied"));
    }

    #[test]
    fn test_value_serializers() {
        assert!(value_string("hello").contains("<string>hello</string>"));
        assert!(value_int(42).contains("<int>42</int>"));
        assert!(value_bool(true).contains("<boolean>1</boolean>"));
        assert!(value_bool(false).contains("<boolean>0</boolean>"));
    }

    #[tokio::test]
    async fn test_xmlrpc_blocked_returns_405() {
        let resp = xmlrpc_blocked().await.into_response();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn test_xmlrpc_blocked_no_pingback_header() {
        let resp = xmlrpc_blocked().await.into_response();
        assert!(resp.headers().get("X-Pingback").is_none());
    }

    #[tokio::test]
    async fn test_xmlrpc_blocked_body() {
        let resp = xmlrpc_blocked().await.into_response();
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("XML-RPC services are disabled"));
    }

    #[tokio::test]
    async fn test_xmlrpc_options_blocked() {
        use axum::body::Body;
        use axum::http::Request as HttpRequest;
        use tower::ServiceExt;

        // Build a minimal router with the xmlrpc routes (state not needed for blocked handler)
        let app = Router::new().route("/xmlrpc.php", any(xmlrpc_blocked));

        let req = HttpRequest::builder()
            .method("OPTIONS")
            .uri("/xmlrpc.php")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}

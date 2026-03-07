use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
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
use std::collections::HashMap;

use rustpress_auth::PasswordHasher;
use rustpress_db::entities::{wp_posts, wp_usermeta, wp_users};

use crate::common::{
    avatar_urls, envelope_response, extract_user_id, filter_user_context,
    pagination_headers_with_link, slugify, user_links, RestContext, WpError,
};
use crate::ApiState;

/// WordPress REST API User response format.
#[derive(Debug, Serialize)]
pub struct WpUser {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub url: String,
    pub link: String,
    pub avatar_urls: HashMap<String, String>,
    pub meta: Vec<Value>,
    pub _links: Value,
    /// Only included when the request is authenticated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<String>>,
    /// WordPress capability map — required by Gutenberg for permission checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<HashMap<String, bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub include: Option<String>,
    pub exclude: Option<String>,
    pub roles: Option<String>,
    pub slug: Option<String>,
    pub context: Option<String>,
    pub orderby: Option<String>,
    pub order: Option<String>,
    pub _envelope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetUserQuery {
    pub context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub roles: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub password: Option<String>,
    pub roles: Option<Vec<String>>,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteUserQuery {
    pub force: Option<bool>,
    pub reassign: Option<u64>,
}

/// Public read-only routes (GET) -- no authentication required.
pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/users", get(list_users))
        .route("/wp-json/wp/v2/users/{id}", get(get_user))
        .route("/wp-json/wp/v2/users/me", get(get_current_user))
}

/// Protected write routes (POST/PUT/DELETE) -- authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/users", axum::routing::post(create_user))
        .route(
            "/wp-json/wp/v2/users/{id}",
            axum::routing::put(update_user)
                .patch(update_user)
                .delete(delete_user),
        )
}

/// Parse a comma-separated string of u64 IDs.
fn parse_id_list(s: &str) -> Vec<u64> {
    s.split(',')
        .filter_map(|v| v.trim().parse::<u64>().ok())
        .collect()
}

/// Build a WpUser from a wp_users::Model. `authenticated` controls whether
/// private fields (email, roles, registered_date) are included.
pub async fn build_wp_user(
    db: &sea_orm::DatabaseConnection,
    user: &wp_users::Model,
    site_url: &str,
    authenticated: bool,
) -> WpUser {
    // Fetch description from usermeta
    let description = get_usermeta(db, user.id, "description")
        .await
        .unwrap_or_default();

    // Fetch roles from usermeta (wp_capabilities)
    let roles = if authenticated {
        Some(get_user_roles(db, user.id).await)
    } else {
        None
    };

    // Build capabilities map (needed by Gutenberg for permission checks)
    let capabilities = if authenticated {
        let role_list = roles.as_deref().unwrap_or(&[]);
        let mut caps: HashMap<String, bool> = HashMap::new();
        for role in role_list {
            caps.insert(role.clone(), true);
            // Add WordPress capability level for the role
            match role.as_str() {
                "administrator" => {
                    caps.insert("level_10".to_string(), true);
                    caps.insert("manage_options".to_string(), true);
                    caps.insert("edit_posts".to_string(), true);
                    caps.insert("edit_others_posts".to_string(), true);
                    caps.insert("publish_posts".to_string(), true);
                    caps.insert("delete_posts".to_string(), true);
                    caps.insert("upload_files".to_string(), true);
                }
                "editor" => {
                    caps.insert("level_7".to_string(), true);
                    caps.insert("edit_posts".to_string(), true);
                    caps.insert("edit_others_posts".to_string(), true);
                    caps.insert("publish_posts".to_string(), true);
                    caps.insert("delete_posts".to_string(), true);
                    caps.insert("upload_files".to_string(), true);
                }
                "author" => {
                    caps.insert("level_2".to_string(), true);
                    caps.insert("edit_posts".to_string(), true);
                    caps.insert("publish_posts".to_string(), true);
                    caps.insert("delete_posts".to_string(), true);
                    caps.insert("upload_files".to_string(), true);
                }
                "contributor" => {
                    caps.insert("level_1".to_string(), true);
                    caps.insert("edit_posts".to_string(), true);
                    caps.insert("delete_posts".to_string(), true);
                }
                "subscriber" => {
                    caps.insert("level_0".to_string(), true);
                    caps.insert("read".to_string(), true);
                }
                _ => {}
            }
        }
        Some(caps)
    } else {
        None
    };

    WpUser {
        id: user.id,
        name: user.display_name.clone(),
        slug: user.user_nicename.clone(),
        description,
        url: user.user_url.clone(),
        link: format!(
            "{}/author/{}",
            site_url.trim_end_matches('/'),
            user.user_login
        ),
        avatar_urls: avatar_urls(&user.user_email),
        meta: vec![],
        _links: user_links(site_url, user.id),
        roles,
        capabilities,
        email: if authenticated {
            Some(user.user_email.clone())
        } else {
            None
        },
        registered_date: if authenticated {
            Some(user.user_registered.format("%Y-%m-%dT%H:%M:%S").to_string())
        } else {
            None
        },
    }
}

/// Get a single usermeta value by key.
async fn get_usermeta(
    db: &sea_orm::DatabaseConnection,
    user_id: u64,
    meta_key: &str,
) -> Option<String> {
    wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq(meta_key))
        .one(db)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.meta_value)
}

/// Extract user roles from wp_usermeta wp_capabilities field.
/// WordPress stores capabilities in serialized PHP format:
///   a:1:{s:13:"administrator";b:1;}
/// We parse the role names out of this.
async fn get_user_roles(db: &sea_orm::DatabaseConnection, user_id: u64) -> Vec<String> {
    let caps = get_usermeta(db, user_id, "wp_capabilities").await;
    match caps {
        Some(ref value) => parse_wp_capabilities(value),
        None => vec![],
    }
}

/// Parse a WordPress serialized PHP capabilities string to extract role names.
/// Format: a:1:{s:13:"administrator";b:1;}
fn parse_wp_capabilities(serialized: &str) -> Vec<String> {
    let mut roles = Vec::new();
    // Simple parser: find all s:N:"rolename";b:1 patterns
    let mut rest = serialized;
    while let Some(pos) = rest.find("s:") {
        rest = &rest[pos + 2..];
        // Skip the length number and colon
        if let Some(colon_pos) = rest.find(':') {
            rest = &rest[colon_pos + 1..];
            // Extract the quoted string
            if rest.starts_with('"') {
                rest = &rest[1..];
                if let Some(end_quote) = rest.find('"') {
                    let role = rest[..end_quote].to_string();
                    rest = &rest[end_quote + 1..];
                    // Check if followed by ;b:1 (role is active)
                    if rest.starts_with(";b:1") {
                        roles.push(role);
                    }
                }
            }
        }
    }
    roles
}

/// Serialize roles into WordPress PHP serialized capabilities format.
/// e.g. for ["administrator"]: a:1:{s:13:"administrator";b:1;}
fn serialize_wp_capabilities(roles: &[String]) -> String {
    let count = roles.len();
    let mut inner = String::new();
    for role in roles {
        inner.push_str(&format!("s:{}:\"{}\";b:1;", role.len(), role));
    }
    format!("a:{count}:{{{inner}}}")
}

async fn list_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(params): Query<ListUsersQuery>,
) -> Result<impl IntoResponse, WpError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    // Check authentication for private fields
    let authenticated = extract_user_id(&state.jwt, &state.sessions, &headers)
        .await
        .is_some();

    // Build base query for count
    let mut count_query = wp_users::Entity::find();
    // Build paginated query
    let mut query = wp_users::Entity::find();

    // Filter: search (on user_login, display_name, user_email)
    if let Some(ref search) = params.search {
        let pattern = format!("%{search}%");
        count_query = count_query.filter(
            sea_orm::Condition::any()
                .add(wp_users::Column::UserLogin.like(&pattern))
                .add(wp_users::Column::DisplayName.like(&pattern))
                .add(wp_users::Column::UserEmail.like(&pattern)),
        );
        query = query.filter(
            sea_orm::Condition::any()
                .add(wp_users::Column::UserLogin.like(&pattern))
                .add(wp_users::Column::DisplayName.like(&pattern))
                .add(wp_users::Column::UserEmail.like(&pattern)),
        );
    }

    // Filter: include
    if let Some(ref include) = params.include {
        let ids = parse_id_list(include);
        if !ids.is_empty() {
            count_query = count_query.filter(wp_users::Column::Id.is_in(ids.clone()));
            query = query.filter(wp_users::Column::Id.is_in(ids));
        }
    }

    // Filter: exclude
    if let Some(ref exclude) = params.exclude {
        let ids = parse_id_list(exclude);
        if !ids.is_empty() {
            count_query = count_query.filter(wp_users::Column::Id.is_not_in(ids.clone()));
            query = query.filter(wp_users::Column::Id.is_not_in(ids));
        }
    }

    // Filter: slug
    if let Some(ref slug) = params.slug {
        count_query = count_query.filter(wp_users::Column::UserNicename.eq(slug.as_str()));
        query = query.filter(wp_users::Column::UserNicename.eq(slug.as_str()));
    }

    // Total count
    let total = count_query
        .count(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;
    let total_pages = if per_page > 0 {
        total.div_ceil(per_page)
    } else {
        1
    };

    // Ordering
    let order_asc = params.order.as_deref() != Some("desc");
    let orderby = params.orderby.as_deref().unwrap_or("name");
    query = match orderby {
        "id" => {
            if order_asc {
                query.order_by_asc(wp_users::Column::Id)
            } else {
                query.order_by_desc(wp_users::Column::Id)
            }
        }
        "email" => {
            if order_asc {
                query.order_by_asc(wp_users::Column::UserEmail)
            } else {
                query.order_by_desc(wp_users::Column::UserEmail)
            }
        }
        "registered_date" => {
            if order_asc {
                query.order_by_asc(wp_users::Column::UserRegistered)
            } else {
                query.order_by_desc(wp_users::Column::UserRegistered)
            }
        }
        "slug" => {
            if order_asc {
                query.order_by_asc(wp_users::Column::UserNicename)
            } else {
                query.order_by_desc(wp_users::Column::UserNicename)
            }
        }
        // "name" or default
        _ => {
            if order_asc {
                query.order_by_asc(wp_users::Column::DisplayName)
            } else {
                query.order_by_desc(wp_users::Column::DisplayName)
            }
        }
    };

    let users = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Filter by roles if specified (requires usermeta lookup)
    let mut items = Vec::new();
    for user in &users {
        if let Some(ref roles_filter) = params.roles {
            let user_roles = get_user_roles(&state.db, user.id).await;
            let requested_roles: Vec<&str> = roles_filter.split(',').map(|s| s.trim()).collect();
            if !user_roles
                .iter()
                .any(|r| requested_roles.contains(&r.as_str()))
            {
                continue;
            }
        }
        items.push(build_wp_user(&state.db, user, &state.site_url, authenticated).await);
    }

    let context = RestContext::from_option(params.context.as_deref());
    let mut json_items: Vec<Value> = items
        .iter()
        .map(|u| serde_json::to_value(u).unwrap_or_default())
        .collect();
    if context != RestContext::View {
        for item in json_items.iter_mut() {
            filter_user_context(item, context);
        }
    }

    let base_url = format!("{}/wp-json/wp/v2/users", state.site_url);
    let resp_headers = pagination_headers_with_link(total, total_pages, page, &base_url);

    if params._envelope.is_some() {
        Ok(Json(envelope_response(
            200,
            &resp_headers,
            Value::Array(json_items),
        ))
        .into_response())
    } else {
        Ok((resp_headers, Json(json_items)).into_response())
    }
}

async fn get_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<u64>,
    Query(params): Query<GetUserQuery>,
) -> Result<Json<Value>, WpError> {
    let authenticated = extract_user_id(&state.jwt, &state.sessions, &headers)
        .await
        .is_some();

    let user = wp_users::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("User not found"))?;

    let wp_user = build_wp_user(&state.db, &user, &state.site_url, authenticated).await;
    let context = RestContext::from_option(params.context.as_deref());
    let mut val = serde_json::to_value(&wp_user).unwrap_or_default();
    filter_user_context(&mut val, context);
    Ok(Json(val))
}

async fn get_current_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<WpUser>, WpError> {
    let user_id = extract_user_id(&state.jwt, &state.sessions, &headers)
        .await
        .ok_or(WpError::unauthorized())?;

    let user = wp_users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("User not found"))?;

    Ok(Json(
        build_wp_user(&state.db, &user, &state.site_url, true).await,
    ))
}

async fn create_user(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Json(input): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<WpUser>), WpError> {
    auth.require(&rustpress_auth::Capability::CreateUsers)?;
    // Check if username already exists
    let existing = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&input.username))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    if existing.is_some() {
        return Err(WpError::bad_request("Username already exists."));
    }

    // Check if email already exists
    let existing_email = wp_users::Entity::find()
        .filter(wp_users::Column::UserEmail.eq(&input.email))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    if existing_email.is_some() {
        return Err(WpError::bad_request("Email already exists."));
    }

    // Hash the password
    let hashed_password = PasswordHasher::hash_argon2(&input.password)
        .map_err(|e| WpError::internal(e.to_string()))?;

    let now = chrono::Utc::now().naive_utc();
    let display_name = input.name.unwrap_or_else(|| input.username.clone());
    let nicename = slugify(&input.username);

    let new_user = wp_users::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_login: Set(input.username.clone()),
        user_pass: Set(hashed_password),
        user_nicename: Set(nicename),
        user_email: Set(input.email),
        user_url: Set(input.url.unwrap_or_default()),
        user_registered: Set(now),
        user_activation_key: Set(String::new()),
        user_status: Set(0),
        display_name: Set(display_name),
    };

    let user = new_user
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Set role via wp_usermeta (wp_capabilities)
    let roles = input
        .roles
        .unwrap_or_else(|| vec!["subscriber".to_string()]);
    let caps_serialized = serialize_wp_capabilities(&roles);

    let caps_meta = wp_usermeta::ActiveModel {
        umeta_id: sea_orm::ActiveValue::NotSet,
        user_id: Set(user.id),
        meta_key: Set(Some("wp_capabilities".to_string())),
        meta_value: Set(Some(caps_serialized)),
    };
    caps_meta
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Set wp_user_level meta (WordPress compat)
    let user_level = role_to_level(roles.first().map(|s| s.as_str()).unwrap_or("subscriber"));
    let level_meta = wp_usermeta::ActiveModel {
        umeta_id: sea_orm::ActiveValue::NotSet,
        user_id: Set(user.id),
        meta_key: Set(Some("wp_user_level".to_string())),
        meta_value: Set(Some(user_level.to_string())),
    };
    level_meta
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Set first_name / last_name / description if provided
    if let Some(first_name) = input.first_name {
        set_usermeta(&state.db, user.id, "first_name", &first_name).await;
    }
    if let Some(last_name) = input.last_name {
        set_usermeta(&state.db, user.id, "last_name", &last_name).await;
    }
    if let Some(description) = input.description {
        set_usermeta(&state.db, user.id, "description", &description).await;
    }

    Ok((
        StatusCode::CREATED,
        Json(build_wp_user(&state.db, &user, &state.site_url, true).await),
    ))
}

async fn update_user(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Json(input): Json<UpdateUserRequest>,
) -> Result<Json<WpUser>, WpError> {
    // Users can edit themselves; editing others requires EditUsers
    if auth.user_id != id {
        auth.require(&rustpress_auth::Capability::EditUsers)?;
    }
    let user = wp_users::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("User not found"))?;

    let mut active: wp_users::ActiveModel = user.into();

    if let Some(name) = input.name {
        active.display_name = Set(name);
    }
    if let Some(email) = input.email {
        active.user_email = Set(email);
    }
    if let Some(url) = input.url {
        active.user_url = Set(url);
    }
    if let Some(slug) = input.slug {
        active.user_nicename = Set(slug);
    }
    if let Some(password) = input.password {
        let hashed =
            PasswordHasher::hash_argon2(&password).map_err(|e| WpError::internal(e.to_string()))?;
        active.user_pass = Set(hashed);
    }

    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Update usermeta fields
    if let Some(first_name) = input.first_name {
        set_usermeta(&state.db, id, "first_name", &first_name).await;
    }
    if let Some(last_name) = input.last_name {
        set_usermeta(&state.db, id, "last_name", &last_name).await;
    }
    if let Some(description) = input.description {
        set_usermeta(&state.db, id, "description", &description).await;
    }
    if let Some(roles) = input.roles {
        let caps_serialized = serialize_wp_capabilities(&roles);
        set_usermeta(&state.db, id, "wp_capabilities", &caps_serialized).await;
        let user_level = role_to_level(roles.first().map(|s| s.as_str()).unwrap_or("subscriber"));
        set_usermeta(&state.db, id, "wp_user_level", &user_level.to_string()).await;
    }

    Ok(Json(
        build_wp_user(&state.db, &updated, &state.site_url, true).await,
    ))
}

async fn delete_user(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Query(params): Query<DeleteUserQuery>,
) -> Result<Json<Value>, WpError> {
    auth.require(&rustpress_auth::Capability::DeleteUsers)?;
    let force = params.force.unwrap_or(false);
    if !force {
        return Err(WpError::bad_request(
            "Users do not support trashing. Set force=true to delete.",
        ));
    }

    let user = wp_users::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("User not found"))?;

    let response_user = build_wp_user(&state.db, &user, &state.site_url, true).await;

    // Reassign posts to another user if specified
    if let Some(reassign_id) = params.reassign {
        // Verify the reassign target exists
        let target = wp_users::Entity::find_by_id(reassign_id)
            .one(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;

        if target.is_none() {
            return Err(WpError::bad_request(format!(
                "Reassign target user {reassign_id} not found."
            )));
        }

        // Reassign all posts by this user
        wp_posts::Entity::update_many()
            .col_expr(
                wp_posts::Column::PostAuthor,
                sea_orm::sea_query::Expr::value(reassign_id),
            )
            .filter(wp_posts::Column::PostAuthor.eq(id))
            .exec(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }

    // Delete usermeta
    wp_usermeta::Entity::delete_many()
        .filter(wp_usermeta::Column::UserId.eq(id))
        .exec(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Delete the user
    let user_active: wp_users::ActiveModel = user.into();
    user_active
        .delete(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "deleted": true,
        "previous": response_user
    })))
}

/// Set or update a usermeta key/value pair.
async fn set_usermeta(db: &sea_orm::DatabaseConnection, user_id: u64, key: &str, value: &str) {
    // Try to find existing
    let existing = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq(key))
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(meta) = existing {
        let mut active: wp_usermeta::ActiveModel = meta.into();
        active.meta_value = Set(Some(value.to_string()));
        let _ = active.update(db).await;
    } else {
        let new_meta = wp_usermeta::ActiveModel {
            umeta_id: sea_orm::ActiveValue::NotSet,
            user_id: Set(user_id),
            meta_key: Set(Some(key.to_string())),
            meta_value: Set(Some(value.to_string())),
        };
        let _ = new_meta.insert(db).await;
    }
}

/// Map a WordPress role name to its numeric user level (WordPress compat).
fn role_to_level(role: &str) -> u32 {
    match role {
        "administrator" => 10,
        "editor" => 7,
        "author" => 2,
        "contributor" => 1,
        "subscriber" => 0,
        _ => 0,
    }
}

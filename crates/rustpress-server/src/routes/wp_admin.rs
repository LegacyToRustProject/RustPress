use axum::{
    extract::{Extension, Form, Multipart, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustpress_auth::{session::Session, PasswordHasher};
use rustpress_db::entities::{wp_comments, wp_postmeta, wp_posts, wp_term_relationships, wp_term_taxonomy, wp_terms, wp_usermeta, wp_users};
use rustpress_themes::ThemeEngine;

use rustpress_auth::roles::Capability;

use crate::middleware::{
    get_user_role, require_activate_plugins, require_admin, require_admin_session,
    require_list_users, resolve_session_role,
};
use crate::state::AppState;
use crate::widgets;

#[derive(Deserialize)]
pub struct LoginForm {
    #[serde(alias = "log")]
    pub username: String,
    #[serde(alias = "pwd")]
    pub password: String,
}

#[derive(Deserialize)]
pub struct AdminPostsQuery {
    pub post_type: Option<String>,
    pub status: Option<String>,
    pub page: Option<u64>,
    pub s: Option<String>,
    pub cat: Option<u64>,
    pub m: Option<String>,
}

#[derive(Deserialize)]
pub struct PostNewQuery {
    pub post_type: Option<String>,
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub blogname: String,
    pub blogdescription: String,
    pub posts_per_page: String,
    #[serde(rename = "WPLANG")]
    pub wplang: Option<String>,
    #[serde(rename = "_wpnonce")]
    pub wpnonce: Option<String>,
}

#[derive(Deserialize)]
pub struct MenuForm {
    pub header_menu: String,
    pub footer_menu: String,
}

#[derive(Deserialize)]
pub struct WritingSettingsForm {
    pub default_category: String,
    pub default_post_format: String,
}

#[derive(Deserialize)]
pub struct ReadingSettingsForm {
    pub show_on_front: String,
    pub page_on_front: Option<String>,
    pub page_for_posts: Option<String>,
    pub posts_per_page: String,
    pub blog_public: Option<String>,
}

#[derive(Deserialize)]
pub struct DiscussionSettingsForm {
    pub default_comment_status: Option<String>,
    pub require_name_email: Option<String>,
    pub comment_moderation: Option<String>,
}

#[derive(Deserialize)]
pub struct MediaSettingsForm {
    pub thumbnail_size_w: String,
    pub thumbnail_size_h: String,
    pub medium_size_w: String,
    pub medium_size_h: String,
    pub large_size_w: String,
    pub large_size_h: String,
}

#[derive(Deserialize)]
pub struct PermalinkSettingsForm {
    pub permalink_structure: String,
    pub custom_structure: Option<String>,
}

#[derive(Deserialize)]
pub struct CommentsQuery {
    pub status: Option<String>,
    pub page: Option<u64>,
}

#[derive(Deserialize)]
pub struct TaxonomyQuery {
    pub taxonomy: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginQuery {
    pub action: Option<String>,
    pub key: Option<String>,
    pub login: Option<String>,
}

#[derive(Deserialize)]
pub struct ProfileForm {
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub email: Option<String>,
    pub user_url: Option<String>,
    pub description: Option<String>,
    pub new_password: Option<String>,
    pub confirm_password: Option<String>,
}

#[derive(Deserialize)]
pub struct LostPasswordForm {
    pub user_login: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordForm {
    pub rp_key: String,
    pub rp_login: String,
    pub new_password: String,
    pub confirm_password: String,
}

#[derive(Deserialize)]
pub struct ThemesQuery {
    pub action: Option<String>,
    pub theme: Option<String>,
}

#[derive(Deserialize)]
pub struct MediaQuery {
    pub paged: Option<u64>,
    pub item: Option<u64>,
}

#[derive(Deserialize)]
pub struct UsersQuery {
    pub role: Option<String>,
    pub deleted: Option<String>,
}

#[derive(Deserialize)]
pub struct UserEditQuery {
    pub user_id: Option<u64>,
}

#[derive(Deserialize)]
pub struct UserNewForm {
    pub user_login: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub user_url: Option<String>,
    pub password: String,
    pub role: Option<String>,
}

#[derive(Deserialize)]
pub struct UserEditForm {
    pub user_id: u64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub user_url: Option<String>,
    pub description: Option<String>,
    pub role: Option<String>,
    pub new_password: Option<String>,
    pub confirm_password: Option<String>,
}

pub fn routes(state: Arc<AppState>) -> Router {
    let public = Router::new()
        .route("/wp-login.php", get(login_page_dispatch).post(login_post_dispatch))
        // Keep old path as redirect for backwards compat
        .route("/wp-admin/login", get(login_page_redirect))
        .with_state(state.clone());

    // Routes requiring manage_options capability (admin only):
    // Settings page, Navigation Menus, Themes
    let admin_only = Router::new()
        .route(
            "/wp-admin/options-general.php",
            get(settings_page).post(settings_save),
        )
        .route(
            "/wp-admin/nav-menus.php",
            get(menus_page).post(menus_save),
        )
        .route(
            "/wp-admin/themes.php",
            get(themes_page).post(themes_activate),
        )
        .route(
            "/wp-admin/widgets.php",
            get(widgets_page).post(widgets_save),
        )
        .route("/wp-admin/tools.php", get(tools_page))
        .route(
            "/wp-admin/export.php",
            get(export_page).post(export_download),
        )
        .route(
            "/wp-admin/import.php",
            get(import_page).post(import_upload),
        )
        .route("/wp-admin/site-health.php", get(site_health_page))
        .route(
            "/wp-admin/options-writing.php",
            get(settings_writing_page).post(settings_writing_save),
        )
        .route(
            "/wp-admin/options-reading.php",
            get(settings_reading_page).post(settings_reading_save),
        )
        .route(
            "/wp-admin/options-discussion.php",
            get(settings_discussion_page).post(settings_discussion_save),
        )
        .route(
            "/wp-admin/options-media.php",
            get(settings_media_page).post(settings_media_save),
        )
        .route(
            "/wp-admin/options-permalink.php",
            get(settings_permalink_page).post(settings_permalink_save),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin_session,
        ))
        .with_state(state.clone());

    // Routes requiring list_users capability (admin only):
    // Users management page
    let users_admin = Router::new()
        .route("/wp-admin/users.php", get(users_list))
        .route(
            "/wp-admin/user-new.php",
            get(user_new_page).post(user_new_save),
        )
        .route(
            "/wp-admin/user-edit.php",
            get(user_edit_page).post(user_edit_save),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_list_users,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin_session,
        ))
        .with_state(state.clone());

    // Routes requiring activate_plugins capability (admin only):
    // Plugin management page
    let plugins_admin = Router::new()
        .route("/wp-admin/plugins.php", get(plugins_list))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_activate_plugins,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin_session,
        ))
        .with_state(state.clone());

    // Routes available to any authenticated user
    let protected = Router::new()
        .route("/wp-admin", get(dashboard_redirect))
        .route("/wp-admin/", get(dashboard))
        .route("/wp-admin/index.php", get(dashboard))
        .route("/wp-admin/edit.php", get(posts_list))
        .route("/wp-admin/post-new.php", get(post_editor_new))
        .route("/wp-admin/post.php", get(post_editor_edit))
        .route("/wp-admin/upload.php", get(media_library).post(media_edit_save))
        .route("/wp-admin/media-upload", post(media_upload))
        .route("/wp-admin/edit-comments.php", get(comments_list))
        .route("/wp-admin/edit-tags.php", get(taxonomy_page))
        .route(
            "/wp-admin/profile.php",
            get(profile_page).post(profile_save),
        )
        .route("/wp-admin/logout", get(logout))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin_session,
        ))
        .with_state(state);

    public
        .merge(admin_only)
        .merge(users_admin)
        .merge(plugins_admin)
        .merge(protected)
}

async fn login_page_redirect() -> Redirect {
    Redirect::permanent("/wp-login.php")
}

fn render_admin(state: &AppState, template: &str, context: &tera::Context) -> Html<String> {
    match state.admin_tera.render(template, context) {
        Ok(html) => Html(html),
        Err(e) => {
            tracing::error!("Admin template error: {}", e);
            Html(format!(
                "<h1>Admin Template Error</h1><pre>{}</pre>",
                e
            ))
        }
    }
}

async fn admin_context(state: &AppState, session: &Session) -> tera::Context {
    let mut ctx = tera::Context::new();
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    ctx.insert("site_name", &site_name);
    ctx.insert("site_url", &state.site_url);

    // Resolve the authoritative role from the database, falling back to
    // the role cached in the session.
    let role = resolve_session_role(session, &state.db).await;
    let role_str = role
        .as_ref()
        .map(|r| r.as_str())
        .unwrap_or(&session.role);

    ctx.insert(
        "current_user",
        &serde_json::json!({
            "login": session.login,
            "role": role_str,
            "user_id": session.user_id,
        }),
    );

    // Expose per-capability booleans so templates can gate UI elements.
    let can = |cap: &Capability| -> bool {
        role.as_ref().map(|r| r.can(cap)).unwrap_or(false)
    };

    ctx.insert("can_manage_options", &can(&Capability::ManageOptions));
    ctx.insert("can_list_users", &can(&Capability::ListUsers));
    ctx.insert("can_edit_others_posts", &can(&Capability::EditOthersPosts));
    ctx.insert("can_moderate_comments", &can(&Capability::ModerateComments));
    ctx.insert("can_manage_categories", &can(&Capability::ManageCategories));
    ctx.insert("can_upload_files", &can(&Capability::UploadFiles));
    ctx.insert("can_edit_posts", &can(&Capability::EditPosts));
    ctx.insert("can_edit_pages", &can(&Capability::EditPages));
    ctx.insert("can_edit_theme_options", &can(&Capability::EditThemeOptions));
    ctx.insert("can_activate_plugins", &can(&Capability::ActivatePlugins));

    // Generate nonces for common admin actions
    let user_id = session.user_id;
    ctx.insert("wpnonce_general", &state.nonces.create_nonce("general", user_id));
    ctx.insert("wpnonce_save_post", &state.nonces.create_nonce("save_post", user_id));
    ctx.insert("wpnonce_delete_post", &state.nonces.create_nonce("delete_post", user_id));
    ctx.insert("wpnonce_update_settings", &state.nonces.create_nonce("update_settings", user_id));
    ctx.insert("wpnonce_manage_users", &state.nonces.create_nonce("manage_users", user_id));

    ctx
}

// --- Login dispatch ---
// The /wp-login.php endpoint handles multiple actions via query param:
//   (none)          -> standard login page
//   lostpassword    -> show/process lost password form
//   rp              -> show reset password form (with key + login)
//   resetpass       -> process new password submission

async fn login_page_dispatch(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LoginQuery>,
) -> Response {
    match params.action.as_deref() {
        Some("lostpassword") => lost_password_page(State(state)).await.into_response(),
        Some("rp") => {
            let key = params.key.unwrap_or_default();
            let login = params.login.unwrap_or_default();
            reset_password_page(State(state), key, login)
                .await
                .into_response()
        }
        _ => {
            let mut ctx = tera::Context::new();
            let site_name = state.options.get_blogname().await.unwrap_or_default();
            ctx.insert("site_name", &site_name);
            ctx.insert("error", &"");
            render_admin(&state, "admin/login.html", &ctx).into_response()
        }
    }
}

async fn login_post_dispatch(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LoginQuery>,
    form_bytes: axum::body::Bytes,
) -> Response {
    match params.action.as_deref() {
        Some("lostpassword") => {
            let form: LostPasswordForm = match serde_urlencoded::from_bytes(&form_bytes) {
                Ok(f) => f,
                Err(_) => {
                    let mut ctx = tera::Context::new();
                    let site_name = state.options.get_blogname().await.unwrap_or_default();
                    ctx.insert("site_name", &site_name);
                    ctx.insert("error", "Invalid form data.");
                    ctx.insert("success", &false);
                    return render_admin(&state, "admin/lost-password.html", &ctx).into_response();
                }
            };
            lost_password_submit(State(state), form)
                .await
                .into_response()
        }
        Some("resetpass") => {
            let form: ResetPasswordForm = match serde_urlencoded::from_bytes(&form_bytes) {
                Ok(f) => f,
                Err(_) => {
                    let mut ctx = tera::Context::new();
                    let site_name = state.options.get_blogname().await.unwrap_or_default();
                    ctx.insert("site_name", &site_name);
                    ctx.insert("error", "Invalid form data.");
                    ctx.insert("success", &false);
                    ctx.insert("invalid_token", &false);
                    return render_admin(&state, "admin/reset-password.html", &ctx)
                        .into_response();
                }
            };
            reset_password_submit(State(state), form)
                .await
                .into_response()
        }
        _ => {
            let form: LoginForm = match serde_urlencoded::from_bytes(&form_bytes) {
                Ok(f) => f,
                Err(_) => {
                    let mut ctx = tera::Context::new();
                    let site_name = state.options.get_blogname().await.unwrap_or_default();
                    ctx.insert("site_name", &site_name);
                    ctx.insert("error", "Invalid form data.");
                    return render_admin(&state, "admin/login.html", &ctx).into_response();
                }
            };
            login_submit(State(state), form).await
        }
    }
}

// --- Login ---

async fn login_submit(State(state): State<Arc<AppState>>, form: LoginForm) -> Response {
    // Find user
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&form.username))
        .one(&state.db)
        .await;

    let user = match user {
        Ok(Some(u)) => u,
        _ => {
            let mut ctx = tera::Context::new();
            let site_name = state.options.get_blogname().await.unwrap_or_default();
            ctx.insert("site_name", &site_name);
            ctx.insert("error", "Invalid username or password.");
            return render_admin(&state, "admin/login.html", &ctx).into_response();
        }
    };

    // Verify password
    let valid = PasswordHasher::verify(&form.password, &user.user_pass).unwrap_or(false);

    if !valid {
        let mut ctx = tera::Context::new();
        let site_name = state.options.get_blogname().await.unwrap_or_default();
        ctx.insert("site_name", &site_name);
        ctx.insert("error", "Invalid username or password.");
        return render_admin(&state, "admin/login.html", &ctx).into_response();
    }

    // Resolve role from wp_usermeta; default to "subscriber" if not found
    let role_str = get_user_role(user.id, &state.db)
        .await
        .unwrap_or_else(|| "subscriber".to_string());

    // Create session
    let session = state
        .sessions
        .create_session(user.id, &user.user_login, &role_str)
        .await;

    // Set cookie and redirect
    let cookie = format!(
        "rustpress_session={}; HttpOnly; Path=/; SameSite=Lax",
        session.id
    );

    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, cookie),
            (header::LOCATION, "/wp-admin/index.php".to_string()),
        ],
    )
        .into_response()
}

async fn logout(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Response {
    state.sessions.destroy_session(&session.id).await;
    let cookie = "rustpress_session=; HttpOnly; Path=/; Max-Age=0";

    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, cookie.to_string()),
            (header::LOCATION, "/wp-login.php".to_string()),
        ],
    )
        .into_response()
}

async fn dashboard_redirect() -> Redirect {
    Redirect::permanent("/wp-admin/")
}

// --- Dashboard ---

async fn dashboard(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "dashboard");

    // Get counts
    let post_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let page_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("page"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let user_count = wp_users::Entity::find()
        .count(&state.db)
        .await
        .unwrap_or(0);

    let draft_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostStatus.eq("draft"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let comment_count = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let pending_comments = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("0"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    // Recent posts
    let recent_posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .order_by_desc(wp_posts::Column::PostDate)
        .limit(5)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let recent: Vec<serde_json::Value> = recent_posts
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "title": p.post_title,
                "status": p.post_status,
                "date": p.post_date.format("%Y-%m-%d").to_string(),
            })
        })
        .collect();

    ctx.insert("post_count", &post_count);
    ctx.insert("page_count", &page_count);
    ctx.insert("user_count", &user_count);
    ctx.insert("comment_count", &comment_count);
    ctx.insert("pending_comments", &pending_comments);
    ctx.insert("draft_count", &draft_count);
    ctx.insert("recent_posts", &recent);

    render_admin(&state, "admin/dashboard.html", &ctx)
}

// --- Posts ---

async fn posts_list(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<AdminPostsQuery>,
) -> Html<String> {
    let post_type = params.post_type.as_deref().unwrap_or("post");

    // Dispatch WooCommerce post types to dedicated plugin admin pages
    if post_type == "product" {
        let q = super::plugin_admin::PluginPostsQuery {
            post_type: params.post_type.clone(),
            status: params.status.clone(),
            page: params.page,
        };
        return super::plugin_admin::wc_products_page(state, session, q).await;
    }
    if post_type == "shop_order" {
        let q = super::plugin_admin::PluginPostsQuery {
            post_type: params.post_type.clone(),
            status: params.status.clone(),
            page: params.page,
        };
        return super::plugin_admin::wc_orders_page(state, session, q).await;
    }

    let mut ctx = admin_context(&state, &session).await;

    let (active_page, type_label) = if post_type == "page" {
        ("pages", "Pages")
    } else {
        ("posts", "Posts")
    };
    ctx.insert("active_page", active_page);
    ctx.insert("post_type", post_type);
    ctx.insert("type_label", type_label);

    let page = params.page.unwrap_or(1);
    let per_page = 20u64;
    let status_filter = params.status.as_deref().unwrap_or("all");
    let search = params.s.clone().unwrap_or_default();
    let cat_filter = params.cat.unwrap_or(0);
    let month_filter = params.m.clone().unwrap_or_default();

    // Resolve the user's role to decide whether they can see all posts
    // or only their own.
    let role = resolve_session_role(&session, &state.db).await;
    let can_see_all = role
        .as_ref()
        .map(|r| r.can(&Capability::EditOthersPosts))
        .unwrap_or(false);

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .order_by_desc(wp_posts::Column::PostDate);

    // Non-editors (authors, contributors, subscribers) only see their own
    // posts/pages.
    if !can_see_all {
        query = query.filter(wp_posts::Column::PostAuthor.eq(session.user_id));
    }

    if status_filter != "all" {
        query = query.filter(wp_posts::Column::PostStatus.eq(status_filter));
    }

    if !search.is_empty() {
        let like = format!("%{}%", search);
        query = query.filter(wp_posts::Column::PostTitle.like(&like));
    }

    // Filter by category: look up term_taxonomy_id, then get matching post IDs
    if cat_filter != 0 {
        let tt = wp_term_taxonomy::Entity::find()
            .filter(wp_term_taxonomy::Column::TermId.eq(cat_filter))
            .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
            .one(&state.db)
            .await
            .ok()
            .flatten();
        if let Some(tt) = tt {
            let rels = wp_term_relationships::Entity::find()
                .filter(wp_term_relationships::Column::TermTaxonomyId.eq(tt.term_taxonomy_id))
                .all(&state.db)
                .await
                .unwrap_or_default();
            let cat_post_ids: Vec<u64> = rels.iter().map(|r| r.object_id).collect();
            if cat_post_ids.is_empty() {
                query = query.filter(wp_posts::Column::Id.eq(0u64));
            } else {
                query = query.filter(wp_posts::Column::Id.is_in(cat_post_ids));
            }
        } else {
            query = query.filter(wp_posts::Column::Id.eq(0u64));
        }
    }

    // Filter by year-month (e.g. "202603" -> year=2026, month=03)
    if !month_filter.is_empty() && month_filter != "0" && month_filter.len() == 6 {
        if let (Ok(year), Ok(month)) = (
            month_filter[..4].parse::<i32>(),
            month_filter[4..6].parse::<u32>(),
        ) {
            if let Some(start) = chrono::NaiveDate::from_ymd_opt(year, month, 1) {
                let start_dt = start.and_hms_opt(0, 0, 0).unwrap();
                let end = if month == 12 {
                    chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
                } else {
                    chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
                };
                if let Some(end_date) = end {
                    let end_dt = end_date.and_hms_opt(0, 0, 0).unwrap();
                    query = query
                        .filter(wp_posts::Column::PostDate.gte(start_dt))
                        .filter(wp_posts::Column::PostDate.lt(end_dt));
                }
            }
        }
    }

    let total = query.clone().count(&state.db).await.unwrap_or(0);
    let total_pages = if total == 0 {
        1
    } else {
        (total + per_page - 1) / per_page
    };

    let posts = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    // Build a map of post_id -> comma-separated category names
    let post_ids: Vec<u64> = posts.iter().map(|p| p.id).collect();
    let mut post_categories: std::collections::HashMap<u64, String> =
        std::collections::HashMap::new();
    if !post_ids.is_empty() && post_type != "page" {
        let rels = wp_term_relationships::Entity::find()
            .filter(wp_term_relationships::Column::ObjectId.is_in(post_ids.clone()))
            .all(&state.db)
            .await
            .unwrap_or_default();

        let tt_ids: Vec<u64> = rels
            .iter()
            .map(|r| r.term_taxonomy_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if !tt_ids.is_empty() {
            let taxonomies = wp_term_taxonomy::Entity::find()
                .filter(wp_term_taxonomy::Column::TermTaxonomyId.is_in(tt_ids))
                .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
                .all(&state.db)
                .await
                .unwrap_or_default();

            let tt_to_term: std::collections::HashMap<u64, u64> = taxonomies
                .iter()
                .map(|t| (t.term_taxonomy_id, t.term_id))
                .collect();

            let term_ids: Vec<u64> = tt_to_term
                .values()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            let terms = wp_terms::Entity::find()
                .filter(wp_terms::Column::TermId.is_in(term_ids))
                .all(&state.db)
                .await
                .unwrap_or_default();
            let term_names: std::collections::HashMap<u64, String> =
                terms.iter().map(|t| (t.term_id, t.name.clone())).collect();

            let mut post_cat_names: std::collections::HashMap<u64, Vec<String>> =
                std::collections::HashMap::new();
            for rel in &rels {
                if let Some(term_id) = tt_to_term.get(&rel.term_taxonomy_id) {
                    if let Some(name) = term_names.get(term_id) {
                        post_cat_names
                            .entry(rel.object_id)
                            .or_default()
                            .push(name.clone());
                    }
                }
            }
            for (pid, names) in post_cat_names {
                post_categories.insert(pid, names.join(", "));
            }
        }
    }

    let items: Vec<serde_json::Value> = posts
        .iter()
        .map(|p| {
            let cats = post_categories.get(&p.id).cloned().unwrap_or_default();
            serde_json::json!({
                "id": p.id,
                "title": p.post_title,
                "status": p.post_status,
                "slug": p.post_name,
                "date": p.post_date.format("%Y-%m-%d %H:%M").to_string(),
                "author": p.post_author,
                "categories": cats,
            })
        })
        .collect();

    // Count by status (scoped to author when the user cannot see all posts)
    let mut publish_q = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.eq("publish"));
    let mut draft_q = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.eq("draft"));
    let mut trash_q = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.eq("trash"));

    if !can_see_all {
        publish_q = publish_q.filter(wp_posts::Column::PostAuthor.eq(session.user_id));
        draft_q = draft_q.filter(wp_posts::Column::PostAuthor.eq(session.user_id));
        trash_q = trash_q.filter(wp_posts::Column::PostAuthor.eq(session.user_id));
    }

    let publish_count = publish_q.count(&state.db).await.unwrap_or(0);
    let draft_count = draft_q.count(&state.db).await.unwrap_or(0);
    let trash_count = trash_q.count(&state.db).await.unwrap_or(0);

    // Load all categories for the filter dropdown
    let all_cat_taxonomies = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let all_cat_term_ids: Vec<u64> = all_cat_taxonomies.iter().map(|t| t.term_id).collect();
    let categories: Vec<serde_json::Value> = if all_cat_term_ids.is_empty() {
        vec![]
    } else {
        let cat_terms = wp_terms::Entity::find()
            .filter(wp_terms::Column::TermId.is_in(all_cat_term_ids))
            .order_by_asc(wp_terms::Column::Name)
            .all(&state.db)
            .await
            .unwrap_or_default();
        cat_terms
            .iter()
            .map(|t| serde_json::json!({"term_id": t.term_id, "name": t.name}))
            .collect()
    };

    // Build date/month options from distinct post_date values
    let all_posts_dates = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.ne("auto-draft"))
        .order_by_desc(wp_posts::Column::PostDate)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let mut seen_months = std::collections::HashSet::new();
    let mut date_months: Vec<serde_json::Value> = Vec::new();
    let month_names = [
        "", "January", "February", "March", "April", "May", "June", "July", "August",
        "September", "October", "November", "December",
    ];
    for p in &all_posts_dates {
        let y = p.post_date.format("%Y").to_string();
        let m = p.post_date.format("%m").to_string();
        let key = format!("{}{}", y, m);
        if seen_months.insert(key.clone()) {
            let month_num = m.parse::<usize>().unwrap_or(0);
            let label = if month_num > 0 && month_num <= 12 {
                format!("{} {}", month_names[month_num], y)
            } else {
                format!("{}-{}", y, m)
            };
            date_months.push(serde_json::json!({"value": key, "label": label}));
        }
    }

    ctx.insert("posts", &items);
    ctx.insert("total", &total);
    ctx.insert("page", &page);
    ctx.insert("total_pages", &total_pages);
    ctx.insert("status_filter", &status_filter);
    ctx.insert("search", &search);
    ctx.insert("publish_count", &publish_count);
    ctx.insert("draft_count", &draft_count);
    ctx.insert("trash_count", &trash_count);
    ctx.insert("categories", &categories);
    ctx.insert("date_months", &date_months);
    ctx.insert("cat_filter", &cat_filter);
    ctx.insert("month_filter", &month_filter);

    render_admin(&state, "admin/posts.html", &ctx)
}

// --- Post Editor ---

#[derive(Deserialize)]
pub struct PostEditQuery {
    pub post: Option<u64>,
    #[allow(dead_code)]
    pub action: Option<String>,
}

async fn post_editor_new(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<PostNewQuery>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    let post_type = params.post_type.as_deref().unwrap_or("post");
    ctx.insert(
        "active_page",
        if post_type == "page" { "pages" } else { "posts" },
    );
    ctx.insert("editing", &false);

    let post_json = serde_json::json!({
        "apiRoot": "/wp-json/",
        "siteUrl": state.site_url,
        "postId": 0,
        "postTitle": "",
        "postContent": "",
        "postExcerpt": "",
        "postStatus": "draft",
        "postSlug": "",
        "postType": post_type,
        "postAuthor": session.user_id,
        "commentStatus": "open",
        "pingStatus": "open",
        "sticky": false,
        "isNew": true,
    });
    ctx.insert("post_json", &post_json.to_string());

    render_admin(&state, "admin/post-edit.html", &ctx)
}

/// WordPress-compatible post editor: /wp-admin/post.php?post=ID&action=edit
async fn post_editor_edit(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<PostEditQuery>,
) -> Response {
    let id = match params.post {
        Some(id) => id,
        None => return Redirect::to("/wp-admin/edit.php").into_response(),
    };

    let post = wp_posts::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    match post {
        Some(p) => {
            let mut ctx = admin_context(&state, &session).await;
            ctx.insert(
                "active_page",
                if p.post_type == "page" {
                    "pages"
                } else {
                    "posts"
                },
            );
            ctx.insert("editing", &true);

            // Load featured image (_thumbnail_id from postmeta)
            let mut featured_image_id: u64 = 0;
            let mut featured_image_url = String::new();
            let thumb_meta = wp_postmeta::Entity::find()
                .filter(wp_postmeta::Column::PostId.eq(p.id))
                .filter(wp_postmeta::Column::MetaKey.eq("_thumbnail_id"))
                .one(&state.db)
                .await
                .ok()
                .flatten();
            if let Some(meta) = thumb_meta {
                if let Some(ref val) = meta.meta_value {
                    if let Ok(mid) = val.parse::<u64>() {
                        featured_image_id = mid;
                        // Resolve attachment URL
                        if let Ok(Some(att)) =
                            wp_posts::Entity::find_by_id(mid).one(&state.db).await
                        {
                            featured_image_url = att.guid;
                        }
                    }
                }
            }

            // Load custom fields (non-internal postmeta)
            let all_meta = wp_postmeta::Entity::find()
                .filter(wp_postmeta::Column::PostId.eq(p.id))
                .all(&state.db)
                .await
                .unwrap_or_default();

            let custom_fields: Vec<serde_json::Value> = all_meta
                .iter()
                .filter(|m| {
                    m.meta_key
                        .as_deref()
                        .map(|k| !k.starts_with('_'))
                        .unwrap_or(false)
                })
                .map(|m| {
                    serde_json::json!({
                        "meta_id": m.meta_id,
                        "key": m.meta_key.as_deref().unwrap_or(""),
                        "value": m.meta_value.as_deref().unwrap_or(""),
                    })
                })
                .collect();

            let post_json = serde_json::json!({
                "apiRoot": "/wp-json/",
                "siteUrl": state.site_url,
                "postId": p.id,
                "postTitle": p.post_title,
                "postContent": p.post_content,
                "postExcerpt": p.post_excerpt,
                "postStatus": p.post_status,
                "postSlug": p.post_name,
                "postType": p.post_type,
                "postDate": p.post_date.format("%Y-%m-%dT%H:%M").to_string(),
                "postAuthor": p.post_author,
                "commentStatus": p.comment_status,
                "pingStatus": p.ping_status,
                "sticky": false,
                "featuredImageId": featured_image_id,
                "featuredImageUrl": featured_image_url,
                "customFields": custom_fields,
                "isNew": false,
            });
            ctx.insert("post_json", &post_json.to_string());

            render_admin(&state, "admin/post-edit.html", &ctx).into_response()
        }
        None => Redirect::to("/wp-admin/edit.php").into_response(),
    }
}

// --- Media ---

async fn media_library(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<MediaQuery>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "media");

    // If ?item=ID, show media detail/edit view
    if let Some(item_id) = params.item {
        let media_item = wp_posts::Entity::find_by_id(item_id)
            .filter(wp_posts::Column::PostType.eq("attachment"))
            .one(&state.db)
            .await
            .ok()
            .flatten();

        if let Some(m) = media_item {
            // Get alt text from postmeta
            let alt_text = wp_postmeta::Entity::find()
                .filter(wp_postmeta::Column::PostId.eq(m.id))
                .filter(wp_postmeta::Column::MetaKey.eq("_wp_attachment_image_alt"))
                .one(&state.db)
                .await
                .ok()
                .flatten()
                .and_then(|meta| meta.meta_value)
                .unwrap_or_default();

            ctx.insert("media_item", &serde_json::json!({
                "id": m.id,
                "title": m.post_title,
                "url": m.guid,
                "mime_type": m.post_mime_type,
                "date": m.post_date.format("%Y-%m-%d").to_string(),
                "alt_text": alt_text,
                "caption": m.post_excerpt,
                "description": m.post_content,
            }));
            ctx.insert("edit_mode", &true);
            return render_admin(&state, "admin/media.html", &ctx);
        }
    }

    // Pagination
    let page = params.paged.unwrap_or(1).max(1);
    let per_page = 40u64;

    let total = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .count(&state.db)
        .await
        .unwrap_or(0);
    let total_pages = if total == 0 { 1 } else { (total + per_page - 1) / per_page };

    let media = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .order_by_desc(wp_posts::Column::PostDate)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let items: Vec<serde_json::Value> = media
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "title": m.post_title,
                "url": m.guid,
                "mime_type": m.post_mime_type,
                "date": m.post_date.format("%Y-%m-%d").to_string(),
            })
        })
        .collect();

    ctx.insert("media_items", &items);
    ctx.insert("current_page", &page);
    ctx.insert("total_pages", &total_pages);
    ctx.insert("total_items", &total);
    ctx.insert("edit_mode", &false);

    render_admin(&state, "admin/media.html", &ctx)
}

#[derive(Deserialize)]
pub struct MediaEditForm {
    pub attachment_id: u64,
    pub title: String,
    pub alt_text: String,
    pub caption: String,
    pub description: String,
}

async fn media_edit_save(
    State(state): State<Arc<AppState>>,
    Extension(_session): Extension<Session>,
    Form(form): Form<MediaEditForm>,
) -> Response {
    // Update the attachment post
    let media = wp_posts::Entity::find_by_id(form.attachment_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    if let Some(m) = media {
        let mut active: wp_posts::ActiveModel = m.into();
        active.post_title = sea_orm::ActiveValue::Set(form.title);
        active.post_excerpt = sea_orm::ActiveValue::Set(form.caption);
        active.post_content = sea_orm::ActiveValue::Set(form.description);
        let _ = active.update(&state.db).await;

        // Update alt text in postmeta
        let existing_alt = wp_postmeta::Entity::find()
            .filter(wp_postmeta::Column::PostId.eq(form.attachment_id))
            .filter(wp_postmeta::Column::MetaKey.eq("_wp_attachment_image_alt"))
            .one(&state.db)
            .await
            .ok()
            .flatten();

        if let Some(meta) = existing_alt {
            let mut active_meta: wp_postmeta::ActiveModel = meta.into();
            active_meta.meta_value = sea_orm::ActiveValue::Set(Some(form.alt_text));
            let _ = active_meta.update(&state.db).await;
        } else {
            let new_meta = wp_postmeta::ActiveModel {
                meta_id: sea_orm::ActiveValue::NotSet,
                post_id: sea_orm::ActiveValue::Set(form.attachment_id),
                meta_key: sea_orm::ActiveValue::Set(Some("_wp_attachment_image_alt".to_string())),
                meta_value: sea_orm::ActiveValue::Set(Some(form.alt_text)),
            };
            let _ = new_meta.insert(&state.db).await;
        }
    }

    Redirect::to(&format!("/wp-admin/upload.php?item={}", form.attachment_id)).into_response()
}

// --- Users ---

async fn users_list(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<UsersQuery>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "users");

    let role_filter = params.role.unwrap_or_default();
    ctx.insert("role_filter", &role_filter);
    ctx.insert("deleted", &params.deleted.is_some());
    ctx.insert("error", &"");

    let users = wp_users::Entity::find()
        .order_by_asc(wp_users::Column::UserLogin)
        .all(&state.db)
        .await
        .unwrap_or_default();

    // Build user items with role and post_count
    let mut items: Vec<serde_json::Value> = Vec::new();
    for u in &users {
        let role = get_user_role(u.id, &state.db)
            .await
            .unwrap_or_else(|| "subscriber".to_string());

        // Apply role filter
        if !role_filter.is_empty() && role != role_filter {
            continue;
        }

        let post_count = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostAuthor.eq(u.id))
            .filter(wp_posts::Column::PostType.eq("post"))
            .filter(wp_posts::Column::PostStatus.ne("auto-draft"))
            .count(&state.db)
            .await
            .unwrap_or(0);

        items.push(serde_json::json!({
            "id": u.id,
            "login": u.user_login,
            "email": u.user_email,
            "display_name": u.display_name,
            "registered": u.user_registered.format("%Y-%m-%d").to_string(),
            "role": role,
            "post_count": post_count,
        }));
    }

    ctx.insert("users", &items);

    render_admin(&state, "admin/users.html", &ctx)
}

// --- User New ---

async fn user_new_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "users");
    ctx.insert("saved", &false);
    ctx.insert("error", &"");

    render_admin(&state, "admin/user-new.html", &ctx)
}

async fn user_new_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<UserNewForm>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "users");

    let login = form.user_login.trim().to_string();
    let email = form.email.trim().to_string();

    // Validate required fields
    if login.is_empty() || email.is_empty() {
        ctx.insert("saved", &false);
        ctx.insert("error", "Username and email are required.");
        return render_admin(&state, "admin/user-new.html", &ctx);
    }

    if form.password.len() < 6 {
        ctx.insert("saved", &false);
        ctx.insert("error", "Password must be at least 6 characters.");
        return render_admin(&state, "admin/user-new.html", &ctx);
    }

    // Check if username already exists
    let existing = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&login))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    if existing.is_some() {
        ctx.insert("saved", &false);
        ctx.insert("error", "Username already exists.");
        return render_admin(&state, "admin/user-new.html", &ctx);
    }

    // Check if email already exists
    let existing_email = wp_users::Entity::find()
        .filter(wp_users::Column::UserEmail.eq(&email))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    if existing_email.is_some() {
        ctx.insert("saved", &false);
        ctx.insert("error", "Email address is already in use.");
        return render_admin(&state, "admin/user-new.html", &ctx);
    }

    // Hash password
    let password_hash = match PasswordHasher::hash_argon2(&form.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Failed to hash password: {}", e);
            ctx.insert("saved", &false);
            ctx.insert("error", "Failed to hash password.");
            return render_admin(&state, "admin/user-new.html", &ctx);
        }
    };

    let display_name = if let Some(ref first) = form.first_name {
        if let Some(ref last) = form.last_name {
            if !first.is_empty() && !last.is_empty() {
                format!("{} {}", first, last)
            } else if !first.is_empty() {
                first.clone()
            } else {
                login.to_string()
            }
        } else if !first.is_empty() {
            first.clone()
        } else {
            login.to_string()
        }
    } else {
        login.to_string()
    };

    let now = chrono::Utc::now().naive_utc();
    let nicename = login.to_lowercase().replace(' ', "-");

    let new_user = wp_users::ActiveModel {
        user_login: Set(login.to_string()),
        user_pass: Set(password_hash),
        user_nicename: Set(nicename),
        user_email: Set(email.to_string()),
        user_url: Set(form.user_url.unwrap_or_default()),
        user_registered: Set(now),
        user_activation_key: Set(String::new()),
        user_status: Set(0),
        display_name: Set(display_name),
        ..Default::default()
    };

    match new_user.insert(&state.db).await {
        Ok(inserted) => {
            let role = form.role.unwrap_or_else(|| "subscriber".to_string());
            // Store role in wp_capabilities format (WordPress PHP serialized)
            let caps_value = format!("a:1:{{s:{}:\"{}\";b:1;}}", role.len(), role);
            set_usermeta(&state.db, inserted.id, "wp_capabilities", &caps_value).await;
            set_usermeta(
                &state.db,
                inserted.id,
                "wp_user_level",
                match role.as_str() {
                    "administrator" => "10",
                    "editor" => "7",
                    "author" => "2",
                    "contributor" => "1",
                    _ => "0",
                },
            )
            .await;

            // Store first_name and last_name in usermeta
            if let Some(ref first) = form.first_name {
                set_usermeta(&state.db, inserted.id, "first_name", first).await;
            }
            if let Some(ref last) = form.last_name {
                set_usermeta(&state.db, inserted.id, "last_name", last).await;
            }

            ctx.insert("saved", &true);
            ctx.insert("error", &"");
        }
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            ctx.insert("saved", &false);
            ctx.insert("error", &format!("Failed to create user: {}", e));
        }
    }

    render_admin(&state, "admin/user-new.html", &ctx)
}

// --- User Edit ---

async fn user_edit_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<UserEditQuery>,
) -> Response {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "users");
    ctx.insert("saved", &false);
    ctx.insert("error", &"");

    let user_id = match params.user_id {
        Some(id) => id,
        None => {
            return Redirect::to("/wp-admin/users.php").into_response();
        }
    };

    let user = wp_users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let user = match user {
        Some(u) => u,
        None => {
            return Redirect::to("/wp-admin/users.php").into_response();
        }
    };

    let first_name = get_usermeta(&state.db, user.id, "first_name")
        .await
        .unwrap_or_default();
    let last_name = get_usermeta(&state.db, user.id, "last_name")
        .await
        .unwrap_or_default();
    let description = get_usermeta(&state.db, user.id, "description")
        .await
        .unwrap_or_default();
    let role = get_user_role(user.id, &state.db)
        .await
        .unwrap_or_else(|| "subscriber".to_string());

    ctx.insert(
        "user",
        &serde_json::json!({
            "id": user.id,
            "login": user.user_login,
            "display_name": user.display_name,
            "email": user.user_email,
            "url": user.user_url,
            "first_name": first_name,
            "last_name": last_name,
            "description": description,
            "role": role,
        }),
    );

    render_admin(&state, "admin/user-edit.html", &ctx).into_response()
}

async fn user_edit_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(_params): Query<UserEditQuery>,
    Form(form): Form<UserEditForm>,
) -> Response {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "users");

    let user_id = form.user_id;

    let user = wp_users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let user = match user {
        Some(u) => u,
        None => {
            ctx.insert("saved", &false);
            ctx.insert("error", "User not found.");
            ctx.insert(
                "user",
                &serde_json::json!({
                    "id": user_id,
                    "login": "",
                    "display_name": "",
                    "email": "",
                    "url": "",
                    "first_name": "",
                    "last_name": "",
                    "description": "",
                    "role": "subscriber",
                }),
            );
            return render_admin(&state, "admin/user-edit.html", &ctx).into_response();
        }
    };

    // Password change validation
    let new_pw = form.new_password.as_deref().unwrap_or("").trim();
    let confirm_pw = form.confirm_password.as_deref().unwrap_or("").trim();

    if !new_pw.is_empty() {
        if new_pw != confirm_pw {
            let first_name = get_usermeta(&state.db, user.id, "first_name")
                .await
                .unwrap_or_default();
            let last_name = get_usermeta(&state.db, user.id, "last_name")
                .await
                .unwrap_or_default();
            let description = get_usermeta(&state.db, user.id, "description")
                .await
                .unwrap_or_default();
            let role = get_user_role(user.id, &state.db)
                .await
                .unwrap_or_else(|| "subscriber".to_string());

            ctx.insert("saved", &false);
            ctx.insert("error", "New passwords do not match.");
            ctx.insert(
                "user",
                &serde_json::json!({
                    "id": user.id,
                    "login": user.user_login,
                    "display_name": form.display_name.as_deref().unwrap_or(&user.display_name),
                    "email": form.email.as_deref().unwrap_or(&user.user_email),
                    "url": form.user_url.as_deref().unwrap_or(&user.user_url),
                    "first_name": form.first_name.as_deref().unwrap_or(&first_name),
                    "last_name": form.last_name.as_deref().unwrap_or(&last_name),
                    "description": form.description.as_deref().unwrap_or(&description),
                    "role": form.role.as_deref().unwrap_or(&role),
                }),
            );
            return render_admin(&state, "admin/user-edit.html", &ctx).into_response();
        }
        if new_pw.len() < 6 {
            let first_name = get_usermeta(&state.db, user.id, "first_name")
                .await
                .unwrap_or_default();
            let last_name = get_usermeta(&state.db, user.id, "last_name")
                .await
                .unwrap_or_default();
            let description = get_usermeta(&state.db, user.id, "description")
                .await
                .unwrap_or_default();
            let role = get_user_role(user.id, &state.db)
                .await
                .unwrap_or_else(|| "subscriber".to_string());

            ctx.insert("saved", &false);
            ctx.insert("error", "Password must be at least 6 characters.");
            ctx.insert(
                "user",
                &serde_json::json!({
                    "id": user.id,
                    "login": user.user_login,
                    "display_name": form.display_name.as_deref().unwrap_or(&user.display_name),
                    "email": form.email.as_deref().unwrap_or(&user.user_email),
                    "url": form.user_url.as_deref().unwrap_or(&user.user_url),
                    "first_name": form.first_name.as_deref().unwrap_or(&first_name),
                    "last_name": form.last_name.as_deref().unwrap_or(&last_name),
                    "description": form.description.as_deref().unwrap_or(&description),
                    "role": form.role.as_deref().unwrap_or(&role),
                }),
            );
            return render_admin(&state, "admin/user-edit.html", &ctx).into_response();
        }
    }

    // Update wp_users fields
    let mut active: wp_users::ActiveModel = user.into();

    if let Some(ref display_name) = form.display_name {
        active.display_name = Set(display_name.clone());
    }
    if let Some(ref email) = form.email {
        active.user_email = Set(email.clone());
    }
    if let Some(ref user_url) = form.user_url {
        active.user_url = Set(user_url.clone());
    }

    // Hash and set new password if provided
    if !new_pw.is_empty() {
        match PasswordHasher::hash_argon2(new_pw) {
            Ok(hash) => {
                active.user_pass = Set(hash);
            }
            Err(e) => {
                tracing::error!("Failed to hash password: {}", e);
            }
        }
    }

    let _ = active.update(&state.db).await;

    // Update usermeta fields
    if let Some(ref first_name) = form.first_name {
        set_usermeta(&state.db, user_id, "first_name", first_name).await;
    }
    if let Some(ref last_name) = form.last_name {
        set_usermeta(&state.db, user_id, "last_name", last_name).await;
    }
    if let Some(ref description) = form.description {
        set_usermeta(&state.db, user_id, "description", description).await;
    }

    // Update role if changed
    if let Some(ref role) = form.role {
        let caps_value = format!("a:1:{{s:{}:\"{}\";b:1;}}", role.len(), role);
        set_usermeta(&state.db, user_id, "wp_capabilities", &caps_value).await;
        set_usermeta(
            &state.db,
            user_id,
            "wp_user_level",
            match role.as_str() {
                "administrator" => "10",
                "editor" => "7",
                "author" => "2",
                "contributor" => "1",
                _ => "0",
            },
        )
        .await;
    }

    // Reload the user for display
    let updated_user = wp_users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let user_data = if let Some(u) = updated_user {
        let first_name = get_usermeta(&state.db, u.id, "first_name")
            .await
            .unwrap_or_default();
        let last_name = get_usermeta(&state.db, u.id, "last_name")
            .await
            .unwrap_or_default();
        let description = get_usermeta(&state.db, u.id, "description")
            .await
            .unwrap_or_default();
        let role = get_user_role(u.id, &state.db)
            .await
            .unwrap_or_else(|| "subscriber".to_string());
        serde_json::json!({
            "id": u.id,
            "login": u.user_login,
            "display_name": u.display_name,
            "email": u.user_email,
            "url": u.user_url,
            "first_name": first_name,
            "last_name": last_name,
            "description": description,
            "role": role,
        })
    } else {
        serde_json::json!({
            "id": user_id,
            "login": "",
            "display_name": "",
            "email": "",
            "url": "",
            "first_name": "",
            "last_name": "",
            "description": "",
            "role": "subscriber",
        })
    };

    ctx.insert("saved", &true);
    ctx.insert("error", &"");
    ctx.insert("user", &user_data);

    render_admin(&state, "admin/user-edit.html", &ctx).into_response()
}

// --- Settings ---

async fn settings_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");

    let blogname = state.options.get_blogname().await.unwrap_or_default();
    let blogdescription = state
        .options
        .get_blogdescription()
        .await
        .unwrap_or_default();
    let posts_per_page = state.options.get_posts_per_page().await.unwrap_or(10);
    let siteurl = state
        .options
        .get_siteurl()
        .await
        .unwrap_or_else(|_| state.site_url.clone());

    // i18n: current locale and available locales for the language dropdown
    let current_locale = state.translations.get_locale().await;
    let available_locales = state.translations.available_locales();

    ctx.insert("blogname", &blogname);
    ctx.insert("blogdescription", &blogdescription);
    ctx.insert("posts_per_page", &posts_per_page);
    ctx.insert("siteurl", &siteurl);
    ctx.insert("saved", &false);
    ctx.insert("current_locale", &current_locale);
    ctx.insert("available_locales", &available_locales);

    render_admin(&state, "admin/settings.html", &ctx)
}

async fn settings_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<SettingsForm>,
) -> Html<String> {
    // Verify nonce
    if let Some(ref nonce) = form.wpnonce {
        if state.nonces.verify_nonce(nonce, "update_settings", session.user_id).is_none() {
            tracing::warn!("Invalid nonce for settings save");
        }
    }

    // Save options
    let _ = state
        .options
        .update_option("blogname", &form.blogname)
        .await;
    let _ = state
        .options
        .update_option("blogdescription", &form.blogdescription)
        .await;
    let _ = state
        .options
        .update_option("posts_per_page", &form.posts_per_page)
        .await;

    // Save WPLANG and switch active locale
    if let Some(ref wplang) = form.wplang {
        let _ = state.options.update_option("WPLANG", wplang).await;
        state.translations.set_locale(wplang).await;
    }

    let current_locale = state.translations.get_locale().await;
    let available_locales = state.translations.available_locales();

    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");
    ctx.insert("blogname", &form.blogname);
    ctx.insert("blogdescription", &form.blogdescription);
    ctx.insert("posts_per_page", &form.posts_per_page);
    ctx.insert("siteurl", &state.site_url);
    ctx.insert("saved", &true);
    ctx.insert("current_locale", &current_locale);
    ctx.insert("available_locales", &available_locales);

    render_admin(&state, "admin/settings.html", &ctx)
}

// --- Writing Settings ---

async fn settings_writing_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");

    let default_category = state
        .options
        .get_option("default_category")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "1".to_string());
    let default_post_format = state
        .options
        .get_option("default_post_format")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "standard".to_string());

    // Load categories for the dropdown
    let tt_records = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let term_ids: Vec<u64> = tt_records.iter().map(|tt| tt.term_id).collect();
    let terms = if term_ids.is_empty() {
        vec![]
    } else {
        wp_terms::Entity::find()
            .filter(wp_terms::Column::TermId.is_in(term_ids))
            .order_by_asc(wp_terms::Column::Name)
            .all(&state.db)
            .await
            .unwrap_or_default()
    };
    let categories: Vec<serde_json::Value> = terms
        .iter()
        .map(|t| {
            serde_json::json!({
                "term_id": t.term_id,
                "name": t.name,
            })
        })
        .collect();

    ctx.insert("categories", &categories);
    ctx.insert("default_category", &default_category);
    ctx.insert("default_post_format", &default_post_format);
    ctx.insert("saved", &false);

    render_admin(&state, "admin/settings-writing.html", &ctx)
}

async fn settings_writing_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<WritingSettingsForm>,
) -> Html<String> {
    let _ = state
        .options
        .update_option("default_category", &form.default_category)
        .await;
    let _ = state
        .options
        .update_option("default_post_format", &form.default_post_format)
        .await;

    // Reload categories for re-render
    let tt_records = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let term_ids: Vec<u64> = tt_records.iter().map(|tt| tt.term_id).collect();
    let terms = if term_ids.is_empty() {
        vec![]
    } else {
        wp_terms::Entity::find()
            .filter(wp_terms::Column::TermId.is_in(term_ids))
            .order_by_asc(wp_terms::Column::Name)
            .all(&state.db)
            .await
            .unwrap_or_default()
    };
    let categories: Vec<serde_json::Value> = terms
        .iter()
        .map(|t| {
            serde_json::json!({
                "term_id": t.term_id,
                "name": t.name,
            })
        })
        .collect();

    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");
    ctx.insert("categories", &categories);
    ctx.insert("default_category", &form.default_category);
    ctx.insert("default_post_format", &form.default_post_format);
    ctx.insert("saved", &true);

    render_admin(&state, "admin/settings-writing.html", &ctx)
}

// --- Reading Settings ---

async fn settings_reading_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");

    let show_on_front = state
        .options
        .get_option_or("show_on_front", "posts")
        .await
        .unwrap_or_else(|_| "posts".to_string());
    let page_on_front: i64 = state
        .options
        .get_option("page_on_front")
        .await
        .unwrap_or(None)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let page_for_posts: i64 = state
        .options
        .get_option("page_for_posts")
        .await
        .unwrap_or(None)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let posts_per_page = state.options.get_posts_per_page().await.unwrap_or(10);
    let blog_public = state
        .options
        .get_option_or("blog_public", "1")
        .await
        .unwrap_or_else(|_| "1".to_string());

    // Load published pages for the dropdowns
    let pages = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("page"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_asc(wp_posts::Column::PostTitle)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let pages_json: Vec<serde_json::Value> = pages
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "post_title": p.post_title,
            })
        })
        .collect();

    ctx.insert("show_on_front", &show_on_front);
    ctx.insert("page_on_front", &page_on_front);
    ctx.insert("page_for_posts", &page_for_posts);
    ctx.insert("posts_per_page", &posts_per_page);
    ctx.insert("blog_public", &blog_public);
    ctx.insert("pages", &pages_json);
    ctx.insert("saved", &false);

    render_admin(&state, "admin/settings-reading.html", &ctx)
}

async fn settings_reading_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<ReadingSettingsForm>,
) -> Html<String> {
    let _ = state
        .options
        .update_option("show_on_front", &form.show_on_front)
        .await;
    let _ = state
        .options
        .update_option(
            "page_on_front",
            form.page_on_front.as_deref().unwrap_or("0"),
        )
        .await;
    let _ = state
        .options
        .update_option(
            "page_for_posts",
            form.page_for_posts.as_deref().unwrap_or("0"),
        )
        .await;
    let _ = state
        .options
        .update_option("posts_per_page", &form.posts_per_page)
        .await;
    // Checkbox: if unchecked, value is absent -> blog is public (1)
    let blog_public = if form.blog_public.is_some() { "0" } else { "1" };
    let _ = state
        .options
        .update_option("blog_public", blog_public)
        .await;

    // Reload pages for re-render
    let pages = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("page"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_asc(wp_posts::Column::PostTitle)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let pages_json: Vec<serde_json::Value> = pages
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "post_title": p.post_title,
            })
        })
        .collect();

    let page_on_front: i64 = form
        .page_on_front
        .as_deref()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);
    let page_for_posts: i64 = form
        .page_for_posts
        .as_deref()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");
    ctx.insert("show_on_front", &form.show_on_front);
    ctx.insert("page_on_front", &page_on_front);
    ctx.insert("page_for_posts", &page_for_posts);
    ctx.insert("posts_per_page", &form.posts_per_page);
    ctx.insert("blog_public", &blog_public);
    ctx.insert("pages", &pages_json);
    ctx.insert("saved", &true);

    render_admin(&state, "admin/settings-reading.html", &ctx)
}

// --- Discussion Settings ---

async fn settings_discussion_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");

    let default_comment_status = state
        .options
        .get_option_or("default_comment_status", "open")
        .await
        .unwrap_or_else(|_| "open".to_string());
    let require_name_email = state
        .options
        .get_option_or("require_name_email", "1")
        .await
        .unwrap_or_else(|_| "1".to_string());
    let comment_moderation = state
        .options
        .get_option_or("comment_moderation", "0")
        .await
        .unwrap_or_else(|_| "0".to_string());

    ctx.insert("default_comment_status", &default_comment_status);
    ctx.insert("require_name_email", &require_name_email);
    ctx.insert("comment_moderation", &comment_moderation);
    ctx.insert("saved", &false);

    render_admin(&state, "admin/settings-discussion.html", &ctx)
}

async fn settings_discussion_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<DiscussionSettingsForm>,
) -> Html<String> {
    // Checkbox: if unchecked, the field is absent
    let comment_status = if form.default_comment_status.is_some() {
        "open"
    } else {
        "closed"
    };
    let _ = state
        .options
        .update_option("default_comment_status", comment_status)
        .await;
    let require_email = if form.require_name_email.is_some() {
        "1"
    } else {
        "0"
    };
    let _ = state
        .options
        .update_option("require_name_email", require_email)
        .await;
    let moderation = if form.comment_moderation.is_some() {
        "1"
    } else {
        "0"
    };
    let _ = state
        .options
        .update_option("comment_moderation", moderation)
        .await;

    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");
    ctx.insert("default_comment_status", &comment_status);
    ctx.insert("require_name_email", &require_email);
    ctx.insert("comment_moderation", &moderation);
    ctx.insert("saved", &true);

    render_admin(&state, "admin/settings-discussion.html", &ctx)
}

// --- Media Settings ---

async fn settings_media_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");

    let thumbnail_size_w = state
        .options
        .get_option_or("thumbnail_size_w", "150")
        .await
        .unwrap_or_else(|_| "150".to_string());
    let thumbnail_size_h = state
        .options
        .get_option_or("thumbnail_size_h", "150")
        .await
        .unwrap_or_else(|_| "150".to_string());
    let medium_size_w = state
        .options
        .get_option_or("medium_size_w", "300")
        .await
        .unwrap_or_else(|_| "300".to_string());
    let medium_size_h = state
        .options
        .get_option_or("medium_size_h", "300")
        .await
        .unwrap_or_else(|_| "300".to_string());
    let large_size_w = state
        .options
        .get_option_or("large_size_w", "1024")
        .await
        .unwrap_or_else(|_| "1024".to_string());
    let large_size_h = state
        .options
        .get_option_or("large_size_h", "1024")
        .await
        .unwrap_or_else(|_| "1024".to_string());

    ctx.insert("thumbnail_size_w", &thumbnail_size_w);
    ctx.insert("thumbnail_size_h", &thumbnail_size_h);
    ctx.insert("medium_size_w", &medium_size_w);
    ctx.insert("medium_size_h", &medium_size_h);
    ctx.insert("large_size_w", &large_size_w);
    ctx.insert("large_size_h", &large_size_h);
    ctx.insert("saved", &false);

    render_admin(&state, "admin/settings-media.html", &ctx)
}

async fn settings_media_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<MediaSettingsForm>,
) -> Html<String> {
    let _ = state
        .options
        .update_option("thumbnail_size_w", &form.thumbnail_size_w)
        .await;
    let _ = state
        .options
        .update_option("thumbnail_size_h", &form.thumbnail_size_h)
        .await;
    let _ = state
        .options
        .update_option("medium_size_w", &form.medium_size_w)
        .await;
    let _ = state
        .options
        .update_option("medium_size_h", &form.medium_size_h)
        .await;
    let _ = state
        .options
        .update_option("large_size_w", &form.large_size_w)
        .await;
    let _ = state
        .options
        .update_option("large_size_h", &form.large_size_h)
        .await;

    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");
    ctx.insert("thumbnail_size_w", &form.thumbnail_size_w);
    ctx.insert("thumbnail_size_h", &form.thumbnail_size_h);
    ctx.insert("medium_size_w", &form.medium_size_w);
    ctx.insert("medium_size_h", &form.medium_size_h);
    ctx.insert("large_size_w", &form.large_size_w);
    ctx.insert("large_size_h", &form.large_size_h);
    ctx.insert("saved", &true);

    render_admin(&state, "admin/settings-media.html", &ctx)
}

// --- Permalink Settings ---

async fn settings_permalink_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");

    let permalink_structure = state
        .options
        .get_option_or("permalink_structure", "/%postname%/")
        .await
        .unwrap_or_else(|_| "/%postname%/".to_string());

    ctx.insert("permalink_structure", &permalink_structure);
    ctx.insert("saved", &false);

    render_admin(&state, "admin/settings-permalink.html", &ctx)
}

async fn settings_permalink_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<PermalinkSettingsForm>,
) -> Html<String> {
    // If "custom" is selected, use the custom_structure field; otherwise use permalink_structure
    let structure = if form.permalink_structure == "custom" {
        form.custom_structure
            .as_deref()
            .unwrap_or("/%postname%/")
            .to_string()
    } else {
        form.permalink_structure.clone()
    };

    let _ = state
        .options
        .update_option("permalink_structure", &structure)
        .await;

    // Update the live rewrite rules so new URLs resolve immediately
    {
        let mut rules = state.rewrite_rules.write().await;
        rules.set_structure(&structure);
    }

    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "settings");
    ctx.insert("permalink_structure", &structure);
    ctx.insert("saved", &true);

    render_admin(&state, "admin/settings-permalink.html", &ctx)
}

// --- Navigation Menus ---

fn parse_menu_links(menu_text: &str) -> Vec<serde_json::Value> {
    menu_text
        .lines()
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

async fn menus_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "menus");

    let header_menu = state
        .options
        .get_option("nav_menu_header")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let footer_menu = state
        .options
        .get_option("nav_menu_footer")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    let header_links = parse_menu_links(&header_menu);
    let footer_links = parse_menu_links(&footer_menu);

    ctx.insert("header_menu", &header_menu);
    ctx.insert("footer_menu", &footer_menu);
    ctx.insert("header_links", &header_links);
    ctx.insert("footer_links", &footer_links);
    ctx.insert("saved", &false);

    render_admin(&state, "admin/menus.html", &ctx)
}

async fn menus_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<MenuForm>,
) -> Html<String> {
    let _ = state
        .options
        .update_option("nav_menu_header", &form.header_menu)
        .await;
    let _ = state
        .options
        .update_option("nav_menu_footer", &form.footer_menu)
        .await;

    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "menus");
    ctx.insert("header_menu", &form.header_menu);
    ctx.insert("footer_menu", &form.footer_menu);

    let header_links = parse_menu_links(&form.header_menu);
    let footer_links = parse_menu_links(&form.footer_menu);
    ctx.insert("header_links", &header_links);
    ctx.insert("footer_links", &footer_links);
    ctx.insert("saved", &true);

    render_admin(&state, "admin/menus.html", &ctx)
}

// --- Comments Management ---

async fn comments_list(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<CommentsQuery>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "comments");

    let page = params.page.unwrap_or(1);
    let per_page = 20u64;
    let status_filter = params.status.as_deref().unwrap_or("all");

    let mut query = wp_comments::Entity::find()
        .order_by_desc(wp_comments::Column::CommentDate);

    let db_status = match status_filter {
        "approved" => Some("1"),
        "pending" => Some("0"),
        "spam" => Some("spam"),
        "trash" => Some("trash"),
        _ => None,
    };

    if let Some(s) = db_status {
        query = query.filter(wp_comments::Column::CommentApproved.eq(s));
    }

    let total = query.clone().count(&state.db).await.unwrap_or(0);
    let total_pages = if total == 0 { 1 } else { (total + per_page - 1) / per_page };

    let comments = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let items: Vec<serde_json::Value> = comments
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.comment_id,
                "post_id": c.comment_post_id,
                "author": c.comment_author,
                "author_email": c.comment_author_email,
                "content": c.comment_content,
                "status": c.comment_approved,
                "date": c.comment_date.format("%Y-%m-%d %H:%M").to_string(),
            })
        })
        .collect();

    // Count by status
    let approved_count = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .count(&state.db).await.unwrap_or(0);
    let pending_count = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("0"))
        .count(&state.db).await.unwrap_or(0);
    let spam_count = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("spam"))
        .count(&state.db).await.unwrap_or(0);
    let trash_count = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentApproved.eq("trash"))
        .count(&state.db).await.unwrap_or(0);

    ctx.insert("comments", &items);
    ctx.insert("total", &total);
    ctx.insert("page", &page);
    ctx.insert("total_pages", &total_pages);
    ctx.insert("status_filter", &status_filter);
    ctx.insert("approved_count", &approved_count);
    ctx.insert("pending_count", &pending_count);
    ctx.insert("spam_count", &spam_count);
    ctx.insert("trash_count", &trash_count);

    render_admin(&state, "admin/comments.html", &ctx)
}

// --- Taxonomy Management ---

async fn taxonomy_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<TaxonomyQuery>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    let taxonomy = params.taxonomy.as_deref().unwrap_or("category");

    let (taxonomy_label, singular_label, active_page) = match taxonomy {
        "post_tag" => ("Tags", "Tag", "tags"),
        _ => ("Categories", "Category", "categories"),
    };

    ctx.insert("active_page", active_page);
    ctx.insert("taxonomy", taxonomy);
    ctx.insert("taxonomy_label", taxonomy_label);
    ctx.insert("singular_label", singular_label);

    // Load terms for this taxonomy
    let tt_records = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .order_by_asc(wp_term_taxonomy::Column::TermId)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let term_ids: Vec<u64> = tt_records.iter().map(|tt| tt.term_id).collect();

    let terms_list = if !term_ids.is_empty() {
        wp_terms::Entity::find()
            .filter(wp_terms::Column::TermId.is_in(term_ids.clone()))
            .order_by_asc(wp_terms::Column::Name)
            .all(&state.db)
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };

    let tt_map: std::collections::HashMap<u64, &wp_term_taxonomy::Model> =
        tt_records.iter().map(|tt| (tt.term_id, tt)).collect();

    let items: Vec<serde_json::Value> = terms_list
        .iter()
        .map(|t| {
            let tt = tt_map.get(&t.term_id);
            serde_json::json!({
                "term_id": t.term_id,
                "name": t.name,
                "slug": t.slug,
                "description": tt.map(|x| x.description.as_str()).unwrap_or(""),
                "count": tt.map(|x| x.count).unwrap_or(0),
            })
        })
        .collect();

    ctx.insert("terms", &items);

    render_admin(&state, "admin/taxonomies.html", &ctx)
}

// --- User Profile ---

/// Helper: read a usermeta value for a given user_id and meta_key.
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

/// Helper: set (upsert) a usermeta value for a given user_id and meta_key.
async fn set_usermeta(
    db: &sea_orm::DatabaseConnection,
    user_id: u64,
    meta_key: &str,
    meta_value: &str,
) {
    let existing = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq(meta_key))
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(row) = existing {
        let mut active: wp_usermeta::ActiveModel = row.into();
        active.meta_value = Set(Some(meta_value.to_string()));
        let _ = active.update(db).await;
    } else {
        let new_meta = wp_usermeta::ActiveModel {
            user_id: Set(user_id),
            meta_key: Set(Some(meta_key.to_string())),
            meta_value: Set(Some(meta_value.to_string())),
            ..Default::default()
        };
        let _ = new_meta.insert(db).await;
    }
}

/// Helper: delete usermeta rows matching a user_id and meta_key.
async fn delete_usermeta(
    db: &sea_orm::DatabaseConnection,
    user_id: u64,
    meta_key: &str,
) {
    let _ = wp_usermeta::Entity::delete_many()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq(meta_key))
        .exec(db)
        .await;
}

async fn profile_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "profile");
    ctx.insert("saved", &false);
    ctx.insert("error", &"");

    // Load the current user from the database
    let user = wp_users::Entity::find_by_id(session.user_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let profile = if let Some(u) = user {
        let first_name = get_usermeta(&state.db, u.id, "first_name")
            .await
            .unwrap_or_default();
        let description = get_usermeta(&state.db, u.id, "description")
            .await
            .unwrap_or_default();
        serde_json::json!({
            "login": u.user_login,
            "display_name": u.display_name,
            "email": u.user_email,
            "user_url": u.user_url,
            "first_name": first_name,
            "description": description,
        })
    } else {
        serde_json::json!({
            "login": session.login,
            "display_name": "",
            "email": "",
            "user_url": "",
            "first_name": "",
            "description": "",
        })
    };

    ctx.insert("profile", &profile);

    render_admin(&state, "admin/profile.html", &ctx)
}

async fn profile_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<ProfileForm>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "profile");

    let user = wp_users::Entity::find_by_id(session.user_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let user = match user {
        Some(u) => u,
        None => {
            ctx.insert("saved", &false);
            ctx.insert("error", "User not found.");
            ctx.insert(
                "profile",
                &serde_json::json!({
                    "login": session.login,
                    "display_name": "",
                    "email": "",
                    "user_url": "",
                    "first_name": "",
                    "description": "",
                }),
            );
            return render_admin(&state, "admin/profile.html", &ctx);
        }
    };

    // Password change validation
    let new_pw = form.new_password.as_deref().unwrap_or("").trim();
    let confirm_pw = form.confirm_password.as_deref().unwrap_or("").trim();

    if !new_pw.is_empty() {
        if new_pw != confirm_pw {
            let first_name = get_usermeta(&state.db, user.id, "first_name")
                .await
                .unwrap_or_default();
            let description = get_usermeta(&state.db, user.id, "description")
                .await
                .unwrap_or_default();
            ctx.insert("saved", &false);
            ctx.insert("error", "New passwords do not match.");
            ctx.insert(
                "profile",
                &serde_json::json!({
                    "login": user.user_login,
                    "display_name": form.display_name.as_deref().unwrap_or(&user.display_name),
                    "email": form.email.as_deref().unwrap_or(&user.user_email),
                    "user_url": form.user_url.as_deref().unwrap_or(&user.user_url),
                    "first_name": form.first_name.as_deref().unwrap_or(&first_name),
                    "description": form.description.as_deref().unwrap_or(&description),
                }),
            );
            return render_admin(&state, "admin/profile.html", &ctx);
        }
        if new_pw.len() < 6 {
            let first_name = get_usermeta(&state.db, user.id, "first_name")
                .await
                .unwrap_or_default();
            let description = get_usermeta(&state.db, user.id, "description")
                .await
                .unwrap_or_default();
            ctx.insert("saved", &false);
            ctx.insert("error", "Password must be at least 6 characters.");
            ctx.insert(
                "profile",
                &serde_json::json!({
                    "login": user.user_login,
                    "display_name": form.display_name.as_deref().unwrap_or(&user.display_name),
                    "email": form.email.as_deref().unwrap_or(&user.user_email),
                    "user_url": form.user_url.as_deref().unwrap_or(&user.user_url),
                    "first_name": form.first_name.as_deref().unwrap_or(&first_name),
                    "description": form.description.as_deref().unwrap_or(&description),
                }),
            );
            return render_admin(&state, "admin/profile.html", &ctx);
        }
    }

    // Update wp_users fields
    let mut active: wp_users::ActiveModel = user.into();

    if let Some(ref display_name) = form.display_name {
        active.display_name = Set(display_name.clone());
    }
    if let Some(ref email) = form.email {
        active.user_email = Set(email.clone());
    }
    if let Some(ref user_url) = form.user_url {
        active.user_url = Set(user_url.clone());
    }
    if let Some(ref first_name) = form.first_name {
        // Store first_name (nickname) in user_nicename as well as usermeta
        active.user_nicename = Set(first_name.clone());
    }

    // Hash and set new password if provided
    if !new_pw.is_empty() {
        match PasswordHasher::hash_argon2(new_pw) {
            Ok(hash) => {
                active.user_pass = Set(hash);
            }
            Err(e) => {
                tracing::error!("Failed to hash password: {}", e);
            }
        }
    }

    let _ = active.update(&state.db).await;

    // Update usermeta fields
    if let Some(ref first_name) = form.first_name {
        set_usermeta(&state.db, session.user_id, "first_name", first_name).await;
    }
    if let Some(ref description) = form.description {
        set_usermeta(&state.db, session.user_id, "description", description).await;
    }

    // Reload the user for display
    let updated_user = wp_users::Entity::find_by_id(session.user_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let profile = if let Some(u) = updated_user {
        let first_name = get_usermeta(&state.db, u.id, "first_name")
            .await
            .unwrap_or_default();
        let description = get_usermeta(&state.db, u.id, "description")
            .await
            .unwrap_or_default();
        serde_json::json!({
            "login": u.user_login,
            "display_name": u.display_name,
            "email": u.user_email,
            "user_url": u.user_url,
            "first_name": first_name,
            "description": description,
        })
    } else {
        serde_json::json!({
            "login": session.login,
            "display_name": "",
            "email": "",
            "user_url": "",
            "first_name": "",
            "description": "",
        })
    };

    ctx.insert("saved", &true);
    ctx.insert("error", &"");
    ctx.insert("profile", &profile);

    render_admin(&state, "admin/profile.html", &ctx)
}

// --- Lost Password ---

async fn lost_password_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let mut ctx = tera::Context::new();
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    ctx.insert("site_name", &site_name);
    ctx.insert("error", &"");
    ctx.insert("success", &false);
    render_admin(&state, "admin/lost-password.html", &ctx)
}

async fn lost_password_submit(
    State(state): State<Arc<AppState>>,
    form: LostPasswordForm,
) -> Html<String> {
    let mut ctx = tera::Context::new();
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    ctx.insert("site_name", &site_name);
    ctx.insert("error", &"");

    // Always show success to avoid user enumeration
    ctx.insert("success", &true);

    // Look up user by login or email
    let user_login = form.user_login.trim();
    let user = wp_users::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(wp_users::Column::UserLogin.eq(user_login))
                .add(wp_users::Column::UserEmail.eq(user_login)),
        )
        .one(&state.db)
        .await;

    if let Ok(Some(user)) = user {
        // Generate a reset token: two UUIDs concatenated (32 random bytes as hex)
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );

        // Store token and expiry in wp_usermeta
        // Token expires in 24 hours
        let expiry = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(24))
            .unwrap_or_else(chrono::Utc::now);
        let expiry_str = expiry.timestamp().to_string();

        set_usermeta(&state.db, user.id, "rp_reset_token", &token).await;
        set_usermeta(&state.db, user.id, "rp_reset_token_expiry", &expiry_str).await;

        // Build the reset link
        let site_url = state
            .options
            .get_siteurl()
            .await
            .unwrap_or_else(|_| state.site_url.clone());
        let reset_url = format!(
            "{}/wp-login.php?action=rp&key={}&login={}",
            site_url, token, user.user_login
        );

        // Log the reset link to the console (no email sending yet)
        tracing::info!(
            "PASSWORD RESET LINK for user '{}': {}",
            user.user_login,
            reset_url
        );
    }

    render_admin(&state, "admin/lost-password.html", &ctx)
}

// --- Reset Password ---

async fn reset_password_page(
    State(state): State<Arc<AppState>>,
    key: String,
    login: String,
) -> Html<String> {
    let mut ctx = tera::Context::new();
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    ctx.insert("site_name", &site_name);
    ctx.insert("error", &"");
    ctx.insert("success", &false);
    ctx.insert("invalid_token", &false);

    if key.is_empty() || login.is_empty() {
        ctx.insert("invalid_token", &true);
        return render_admin(&state, "admin/reset-password.html", &ctx);
    }

    // Validate the token
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&login))
        .one(&state.db)
        .await;

    let valid = if let Ok(Some(ref user)) = user {
        validate_reset_token(&state.db, user.id, &key).await
    } else {
        false
    };

    if !valid {
        ctx.insert("invalid_token", &true);
    } else {
        ctx.insert("rp_key", &key);
        ctx.insert("rp_login", &login);
    }

    render_admin(&state, "admin/reset-password.html", &ctx)
}

async fn reset_password_submit(
    State(state): State<Arc<AppState>>,
    form: ResetPasswordForm,
) -> Html<String> {
    let mut ctx = tera::Context::new();
    let site_name = state.options.get_blogname().await.unwrap_or_default();
    ctx.insert("site_name", &site_name);
    ctx.insert("success", &false);
    ctx.insert("invalid_token", &false);
    ctx.insert("rp_key", &form.rp_key);
    ctx.insert("rp_login", &form.rp_login);

    // Validate passwords match
    if form.new_password != form.confirm_password {
        ctx.insert("error", "Passwords do not match.");
        return render_admin(&state, "admin/reset-password.html", &ctx);
    }

    if form.new_password.len() < 6 {
        ctx.insert("error", "Password must be at least 6 characters.");
        return render_admin(&state, "admin/reset-password.html", &ctx);
    }

    // Look up user
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&form.rp_login))
        .one(&state.db)
        .await;

    let user = match user {
        Ok(Some(u)) => u,
        _ => {
            ctx.insert("invalid_token", &true);
            ctx.insert("error", &"");
            return render_admin(&state, "admin/reset-password.html", &ctx);
        }
    };

    // Validate token
    if !validate_reset_token(&state.db, user.id, &form.rp_key).await {
        ctx.insert("invalid_token", &true);
        ctx.insert("error", &"");
        return render_admin(&state, "admin/reset-password.html", &ctx);
    }

    // Save user id before moving user into ActiveModel
    let user_id = user.id;

    // Hash and update the password
    match PasswordHasher::hash_argon2(&form.new_password) {
        Ok(hash) => {
            let mut active: wp_users::ActiveModel = user.into();
            active.user_pass = Set(hash);
            let _ = active.update(&state.db).await;
        }
        Err(e) => {
            tracing::error!("Failed to hash password during reset: {}", e);
            ctx.insert("error", "An error occurred. Please try again.");
            return render_admin(&state, "admin/reset-password.html", &ctx);
        }
    }

    // Clear the reset token
    delete_usermeta(&state.db, user_id, "rp_reset_token").await;
    delete_usermeta(&state.db, user_id, "rp_reset_token_expiry").await;

    ctx.insert("success", &true);
    ctx.insert("error", &"");

    render_admin(&state, "admin/reset-password.html", &ctx)
}

/// Validate a password reset token against stored values in wp_usermeta.
async fn validate_reset_token(
    db: &sea_orm::DatabaseConnection,
    user_id: u64,
    token: &str,
) -> bool {
    let stored_token = match get_usermeta(db, user_id, "rp_reset_token").await {
        Some(t) => t,
        None => return false,
    };

    if stored_token != token {
        return false;
    }

    // Check expiry
    let expiry_str = match get_usermeta(db, user_id, "rp_reset_token_expiry").await {
        Some(e) => e,
        None => return false,
    };

    let expiry_ts: i64 = match expiry_str.parse() {
        Ok(ts) => ts,
        Err(_) => return false,
    };

    let now = chrono::Utc::now().timestamp();
    now < expiry_ts
}

// --- Plugins ---

async fn plugins_list(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "plugins");

    // Get all plugins from the plugin registry
    let all_plugins = state.plugin_registry.list().await;

    let total_count = all_plugins.len();
    let active_count = all_plugins
        .iter()
        .filter(|p| p.status == rustpress_plugins::registry::PluginStatus::Active)
        .count();
    let inactive_count = total_count - active_count;

    // Build template-friendly plugin data
    let plugins: Vec<serde_json::Value> = all_plugins
        .iter()
        .map(|p| {
            let status = match &p.status {
                rustpress_plugins::registry::PluginStatus::Active => "Active",
                rustpress_plugins::registry::PluginStatus::Inactive => "Inactive",
                rustpress_plugins::registry::PluginStatus::Error(_) => "Error",
            };
            let plugin_type = match &p.meta.plugin_type {
                rustpress_plugins::registry::PluginType::Native => "Native",
                rustpress_plugins::registry::PluginType::Wasm => "Wasm",
            };
            serde_json::json!({
                "name": p.meta.name,
                "version": p.meta.version,
                "description": p.meta.description,
                "author": p.meta.author,
                "status": status,
                "plugin_type": plugin_type,
            })
        })
        .collect();

    ctx.insert("plugins", &plugins);
    ctx.insert("total_count", &total_count);
    ctx.insert("active_count", &active_count);
    ctx.insert("inactive_count", &inactive_count);

    render_admin(&state, "admin/plugins.html", &ctx)
}

// --- Widgets Management ---

async fn widgets_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "widgets");
    ctx.insert("saved", &false);

    let config = widgets::load_widget_config(&state.options).await;

    // Build available_widgets list for template
    let available: Vec<serde_json::Value> = widgets::AVAILABLE_WIDGETS
        .iter()
        .map(|aw| {
            serde_json::json!({
                "type_key": aw.type_key,
                "label": aw.label,
                "description": aw.description,
            })
        })
        .collect();
    ctx.insert("available_widgets", &available);

    // Build widget_areas list for template
    let areas: Vec<serde_json::Value> = widgets::WIDGET_AREAS
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "name": a.name,
                "description": a.description,
            })
        })
        .collect();
    ctx.insert("widget_areas", &areas);

    // Build area_widgets: map of area_id -> vec of widget data for the template
    let mut area_widgets = serde_json::Map::new();
    for area_info in widgets::WIDGET_AREAS {
        let instances = config.areas.get(area_info.id).cloned().unwrap_or_default();
        let items: Vec<serde_json::Value> = instances
            .iter()
            .map(|inst| widget_instance_to_json(inst))
            .collect();
        area_widgets.insert(area_info.id.to_string(), serde_json::Value::Array(items));
    }
    ctx.insert("area_widgets", &area_widgets);

    render_admin(&state, "admin/widgets.html", &ctx)
}

/// Convert a WidgetInstance to a JSON value for the admin template.
fn widget_instance_to_json(inst: &widgets::WidgetInstance) -> serde_json::Value {
    match &inst.widget {
        widgets::WidgetType::RecentPosts { title, count } => serde_json::json!({
            "id": inst.id,
            "type_name": "RecentPosts",
            "title": title,
            "count": count,
        }),
        widgets::WidgetType::Categories { title, display } => serde_json::json!({
            "id": inst.id,
            "type_name": "Categories",
            "title": title,
            "display": display,
        }),
        widgets::WidgetType::Archives { title } => serde_json::json!({
            "id": inst.id,
            "type_name": "Archives",
            "title": title,
        }),
        widgets::WidgetType::Search { title } => serde_json::json!({
            "id": inst.id,
            "type_name": "Search",
            "title": title,
        }),
        widgets::WidgetType::Text { title, content } => serde_json::json!({
            "id": inst.id,
            "type_name": "Text",
            "title": title,
            "content": content,
        }),
        widgets::WidgetType::CustomHTML { title, content } => serde_json::json!({
            "id": inst.id,
            "type_name": "CustomHTML",
            "title": title,
            "content": content,
        }),
        widgets::WidgetType::Meta { title } => serde_json::json!({
            "id": inst.id,
            "type_name": "Meta",
            "title": title,
        }),
        widgets::WidgetType::RecentComments { title, count } => serde_json::json!({
            "id": inst.id,
            "type_name": "RecentComments",
            "title": title,
            "count": count,
        }),
        widgets::WidgetType::Calendar { title } => serde_json::json!({
            "id": inst.id,
            "type_name": "Calendar",
            "title": title,
        }),
        widgets::WidgetType::TagCloud { title } => serde_json::json!({
            "id": inst.id,
            "type_name": "TagCloud",
            "title": title,
        }),
    }
}

async fn widgets_save(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    body: axum::body::Bytes,
) -> Html<String> {
    // Parse the URL-encoded body manually because the nested bracket syntax
    // widgets[sidebar-1][0][type]=... is not handled by serde_urlencoded.
    let body_str = String::from_utf8_lossy(&body).to_string();
    let config = parse_widget_form(&body_str);

    let _ = widgets::save_widget_config(&state.options, &config).await;

    // Invalidate page cache since sidebar content changed
    state.page_cache.flush().await;

    // Re-render the page with saved=true
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "widgets");
    ctx.insert("saved", &true);

    let available: Vec<serde_json::Value> = widgets::AVAILABLE_WIDGETS
        .iter()
        .map(|aw| {
            serde_json::json!({
                "type_key": aw.type_key,
                "label": aw.label,
                "description": aw.description,
            })
        })
        .collect();
    ctx.insert("available_widgets", &available);

    let areas: Vec<serde_json::Value> = widgets::WIDGET_AREAS
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "name": a.name,
                "description": a.description,
            })
        })
        .collect();
    ctx.insert("widget_areas", &areas);

    let mut area_widgets = serde_json::Map::new();
    for area_info in widgets::WIDGET_AREAS {
        let instances = config.areas.get(area_info.id).cloned().unwrap_or_default();
        let items: Vec<serde_json::Value> = instances
            .iter()
            .map(|inst| widget_instance_to_json(inst))
            .collect();
        area_widgets.insert(area_info.id.to_string(), serde_json::Value::Array(items));
    }
    ctx.insert("area_widgets", &area_widgets);

    render_admin(&state, "admin/widgets.html", &ctx)
}

/// Decode a percent-encoded URL component (also handles '+' as space).
fn form_decode(s: &str) -> String {
    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                result.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(hi), Some(lo)) = (
                    hex_val(bytes[i + 1]),
                    hex_val(bytes[i + 2]),
                ) {
                    result.push(hi << 4 | lo);
                    i += 3;
                } else {
                    result.push(b'%');
                    i += 1;
                }
            }
            c => {
                result.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&result).to_string()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Parse the nested form data `widgets[area_id][index][field]=value` into a WidgetConfig.
fn parse_widget_form(body: &str) -> widgets::WidgetConfig {
    use std::collections::HashMap;

    // Collect all form fields: key=value pairs
    let pairs: Vec<(String, String)> = body
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((form_decode(key), form_decode(value)))
        })
        .collect();

    // Group by area_id and index:
    //   widgets[sidebar-1][0][type] -> area="sidebar-1", idx=0, field="type"
    let mut area_map: HashMap<String, HashMap<usize, HashMap<String, String>>> = HashMap::new();

    for (key, value) in &pairs {
        if !key.starts_with("widgets[") {
            continue;
        }
        let rest = &key[8..]; // skip "widgets["
        let close = match rest.find(']') {
            Some(i) => i,
            None => continue,
        };
        let area_id = rest[..close].to_string();
        let rest2 = &rest[close + 1..]; // skip "]"
        if !rest2.starts_with('[') {
            continue;
        }
        let rest3 = &rest2[1..]; // skip "["
        let close2 = match rest3.find(']') {
            Some(i) => i,
            None => continue,
        };
        let idx_str = &rest3[..close2];
        let idx: usize = match idx_str.parse() {
            Ok(i) => i,
            Err(_) => continue,
        };
        let rest4 = &rest3[close2 + 1..]; // skip "]"
        if !rest4.starts_with('[') {
            continue;
        }
        let rest5 = &rest4[1..]; // skip "["
        let close3 = match rest5.find(']') {
            Some(i) => i,
            None => continue,
        };
        let field = rest5[..close3].to_string();

        area_map
            .entry(area_id)
            .or_default()
            .entry(idx)
            .or_default()
            .insert(field, value.clone());
    }

    // Build WidgetConfig from the parsed map
    let mut areas: HashMap<String, Vec<widgets::WidgetInstance>> = HashMap::new();

    // Ensure all registered areas exist even if empty
    for area_info in widgets::WIDGET_AREAS {
        areas.entry(area_info.id.to_string()).or_default();
    }

    for (area_id, idx_map) in &area_map {
        let mut indices: Vec<usize> = idx_map.keys().copied().collect();
        indices.sort();

        let instances: Vec<widgets::WidgetInstance> = indices
            .iter()
            .filter_map(|idx| {
                let fields = idx_map.get(idx)?;
                let widget_type = fields.get("type")?.as_str();
                let title = fields.get("title").cloned().unwrap_or_default();
                let id = fields
                    .get("id")
                    .cloned()
                    .unwrap_or_else(|| format!("{}-{}", widget_type.to_lowercase(), idx));

                let widget = match widget_type {
                    "RecentPosts" => {
                        let count: u32 = fields
                            .get("count")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(5);
                        widgets::WidgetType::RecentPosts { title, count }
                    }
                    "Categories" => {
                        let display = fields
                            .get("display")
                            .cloned()
                            .unwrap_or_else(|| "list".to_string());
                        widgets::WidgetType::Categories { title, display }
                    }
                    "Archives" => widgets::WidgetType::Archives { title },
                    "Search" => widgets::WidgetType::Search { title },
                    "Text" => {
                        let content = fields.get("content").cloned().unwrap_or_default();
                        widgets::WidgetType::Text { title, content }
                    }
                    "CustomHTML" => {
                        let content = fields.get("content").cloned().unwrap_or_default();
                        widgets::WidgetType::CustomHTML { title, content }
                    }
                    "Meta" => widgets::WidgetType::Meta { title },
                    "RecentComments" => {
                        let count: u32 = fields
                            .get("count")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(5);
                        widgets::WidgetType::RecentComments { title, count }
                    }
                    _ => return None,
                };

                Some(widgets::WidgetInstance { id, widget })
            })
            .collect();

        areas.insert(area_id.clone(), instances);
    }

    widgets::WidgetConfig { areas }
}

// --- Themes ---

async fn themes_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<ThemesQuery>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "themes");

    // Determine the active theme slug from wp_options (default: "default").
    let active_slug = state
        .options
        .get_option("current_theme")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "default".to_string());

    // Discover available themes.
    let themes_dir = Path::new("themes");
    let templates_dir = Path::new("templates");
    let themes = ThemeEngine::discover_themes(themes_dir, templates_dir, &active_slug);

    // Check if we just activated a theme (via redirect query param).
    let activated = params.action.as_deref() == Some("activated");
    ctx.insert("activated", &activated);
    ctx.insert("error", &"");
    ctx.insert("themes", &themes);

    render_admin(&state, "admin/themes.html", &ctx)
}

async fn themes_activate(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(params): Query<ThemesQuery>,
) -> Response {
    let action = params.action.as_deref().unwrap_or("");
    let theme_slug = params.theme.as_deref().unwrap_or("");

    if action != "activate" || theme_slug.is_empty() {
        return Redirect::to("/wp-admin/themes.php").into_response();
    }

    // Verify the theme actually exists before activating.
    let themes_dir = Path::new("themes");
    let templates_dir = Path::new("templates");
    let active_slug = state
        .options
        .get_option("current_theme")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "default".to_string());

    let available = ThemeEngine::discover_themes(themes_dir, templates_dir, &active_slug);
    let theme_exists = available.iter().any(|t| t.slug == theme_slug);

    if !theme_exists {
        // Theme not found - redirect back with error.
        let mut ctx = admin_context(&state, &session).await;
        ctx.insert("active_page", "themes");
        ctx.insert("activated", &false);
        ctx.insert("error", &format!("Theme '{}' not found.", theme_slug));
        ctx.insert("themes", &available);
        return render_admin(&state, "admin/themes.html", &ctx).into_response();
    }

    // Save the new active theme to wp_options.
    if let Err(e) = state.options.update_option("current_theme", theme_slug).await {
        tracing::error!("Failed to save theme option: {}", e);
        let mut ctx = admin_context(&state, &session).await;
        ctx.insert("active_page", "themes");
        ctx.insert("activated", &false);
        ctx.insert("error", "Failed to save theme setting.");
        let themes = ThemeEngine::discover_themes(themes_dir, templates_dir, &active_slug);
        ctx.insert("themes", &themes);
        return render_admin(&state, "admin/themes.html", &ctx).into_response();
    }

    // Reload the ThemeEngine to pick up the newly activated theme.
    {
        let mut engine = state.theme_engine.write().await;
        // Attempt to switch to the new theme. For the "default" slug we use
        // from_templates_dir; for anything else we use the themes base dir.
        let new_engine = if theme_slug == "default" {
            ThemeEngine::from_templates_dir(templates_dir)
        } else {
            ThemeEngine::new(themes_dir, theme_slug)
        };

        match new_engine {
            Ok(eng) => *engine = eng,
            Err(e) => {
                tracing::error!("Failed to reload theme engine: {}", e);
                // Rollback the option to the previous theme.
                let _ = state.options.update_option("current_theme", &active_slug).await;
            }
        }
    }

    Redirect::to("/wp-admin/themes.php?action=activated").into_response()
}

// --- Tools ---

async fn tools_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "tools");
    render_admin(&state, "admin/tools.html", &ctx)
}

async fn export_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "tools");
    render_admin(&state, "admin/export.html", &ctx)
}

#[derive(Deserialize)]
pub struct ExportForm {
    pub content: Option<String>,
}

async fn export_download(
    State(state): State<Arc<AppState>>,
    Extension(_session): Extension<Session>,
    Form(form): Form<ExportForm>,
) -> Response {
    let content_type = form.content.as_deref().unwrap_or("all");

    // Build WXR XML export
    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostStatus.ne("auto-draft"));

    if content_type != "all" {
        query = query.filter(wp_posts::Column::PostType.eq(content_type));
    }

    let posts = query
        .order_by_asc(wp_posts::Column::PostDate)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let site_name = state.options.get_blogname().await.unwrap_or_else(|_| "RustPress".to_string());
    let site_url = &state.site_url;

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<rss version=\"2.0\"\n");
    xml.push_str("  xmlns:excerpt=\"http://wordpress.org/export/1.2/excerpt/\"\n");
    xml.push_str("  xmlns:content=\"http://purl.org/rss/1.0/modules/content/\"\n");
    xml.push_str("  xmlns:wp=\"http://wordpress.org/export/1.2/\"\n");
    xml.push_str(">\n<channel>\n");
    xml.push_str(&format!("  <title>{}</title>\n", xml_escape(&site_name)));
    xml.push_str(&format!("  <link>{}</link>\n", xml_escape(site_url)));
    xml.push_str("  <wp:wxr_version>1.2</wp:wxr_version>\n");

    for p in &posts {
        xml.push_str("  <item>\n");
        xml.push_str(&format!("    <title>{}</title>\n", xml_escape(&p.post_title)));
        xml.push_str(&format!("    <wp:post_id>{}</wp:post_id>\n", p.id));
        xml.push_str(&format!("    <wp:post_date>{}</wp:post_date>\n", p.post_date.format("%Y-%m-%d %H:%M:%S")));
        xml.push_str(&format!("    <wp:post_name>{}</wp:post_name>\n", xml_escape(&p.post_name)));
        xml.push_str(&format!("    <wp:post_type>{}</wp:post_type>\n", xml_escape(&p.post_type)));
        xml.push_str(&format!("    <wp:status>{}</wp:status>\n", xml_escape(&p.post_status)));
        xml.push_str(&format!("    <wp:post_parent>{}</wp:post_parent>\n", p.post_parent));
        xml.push_str(&format!("    <wp:menu_order>{}</wp:menu_order>\n", p.menu_order));
        xml.push_str(&format!("    <content:encoded><![CDATA[{}]]></content:encoded>\n", p.post_content));
        xml.push_str(&format!("    <excerpt:encoded><![CDATA[{}]]></excerpt:encoded>\n", p.post_excerpt));
        xml.push_str("  </item>\n");
    }

    xml.push_str("</channel>\n</rss>\n");

    let filename = format!("rustpress-export-{}.xml", chrono::Utc::now().format("%Y-%m-%d"));
    (
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            (header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", filename)),
        ],
        xml,
    )
        .into_response()
}

async fn import_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "tools");
    ctx.insert("success", &false);
    ctx.insert("error", "");
    render_admin(&state, "admin/import.html", &ctx)
}

async fn import_upload(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    mut multipart: axum::extract::Multipart,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "tools");

    let mut imported = 0u64;
    let mut error_msg = String::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("import_file") {
            match field.bytes().await {
                Ok(data) => {
                    let xml_str = String::from_utf8_lossy(&data);
                    // Simple WXR parser: extract <item> elements
                    imported = parse_and_import_wxr(&state.db, &xml_str).await;
                }
                Err(e) => {
                    error_msg = format!("Failed to read file: {}", e);
                }
            }
        }
    }

    if error_msg.is_empty() {
        ctx.insert("success", &true);
        ctx.insert("imported_count", &imported);
        ctx.insert("error", "");
    } else {
        ctx.insert("success", &false);
        ctx.insert("imported_count", &0u64);
        ctx.insert("error", &error_msg);
    }
    render_admin(&state, "admin/import.html", &ctx)
}

/// Simple WXR import: parse XML items and insert as posts.
async fn parse_and_import_wxr(db: &sea_orm::DatabaseConnection, xml: &str) -> u64 {
    let mut count = 0u64;
    let now = chrono::Utc::now().naive_utc();

    // Very basic XML parsing - find <item>...</item> blocks
    for item in xml.split("<item>").skip(1) {
        let end = item.find("</item>").unwrap_or(item.len());
        let item = &item[..end];

        let title = extract_xml_tag(item, "title").unwrap_or_default();
        let content = extract_cdata(item, "content:encoded").unwrap_or_default();
        let excerpt = extract_cdata(item, "excerpt:encoded").unwrap_or_default();
        let post_name = extract_xml_tag(item, "wp:post_name").unwrap_or_default();
        let post_type = extract_xml_tag(item, "wp:post_type").unwrap_or_else(|| "post".to_string());
        let status = extract_xml_tag(item, "wp:status").unwrap_or_else(|| "publish".to_string());

        let new_post = wp_posts::ActiveModel {
            post_author: Set(1),
            post_date: Set(now),
            post_date_gmt: Set(now),
            post_content: Set(content),
            post_title: Set(title),
            post_excerpt: Set(excerpt),
            post_status: Set(status),
            comment_status: Set("open".to_string()),
            ping_status: Set("open".to_string()),
            post_password: Set(String::new()),
            post_name: Set(post_name),
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
            ..Default::default()
        };

        if new_post.insert(db).await.is_ok() {
            count += 1;
        }
    }
    count
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml.find(&close)?;
    if start <= end {
        Some(xml[start..end].to_string())
    } else {
        None
    }
}

fn extract_cdata(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml.find(&close)?;
    let content = &xml[start..end];
    // Strip CDATA wrapper
    let content = content
        .trim()
        .strip_prefix("<![CDATA[")
        .unwrap_or(content);
    let content = content
        .strip_suffix("]]>")
        .unwrap_or(content);
    Some(content.to_string())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn site_health_page(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    ctx.insert("active_page", "tools");

    // Server info
    ctx.insert("rust_version", env!("CARGO_PKG_RUST_VERSION", "stable"));
    ctx.insert("rustpress_version", env!("CARGO_PKG_VERSION"));
    ctx.insert("db_type", "MySQL");
    ctx.insert("os_info", std::env::consts::OS);

    // Content stats
    let post_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"))
        .filter(wp_posts::Column::PostStatus.ne("auto-draft"))
        .count(&state.db)
        .await
        .unwrap_or(0);
    let page_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("page"))
        .count(&state.db)
        .await
        .unwrap_or(0);
    let comment_count = wp_comments::Entity::find()
        .count(&state.db)
        .await
        .unwrap_or(0);
    let user_count = wp_users::Entity::find()
        .count(&state.db)
        .await
        .unwrap_or(0);
    let media_count = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("attachment"))
        .count(&state.db)
        .await
        .unwrap_or(0);

    ctx.insert("post_count", &post_count);
    ctx.insert("page_count", &page_count);
    ctx.insert("comment_count", &comment_count);
    ctx.insert("user_count", &user_count);
    ctx.insert("media_count", &media_count);

    // HTTPS status
    let https_status = if state.site_url.starts_with("https") {
        "Active"
    } else {
        "Not Active"
    };
    ctx.insert("https_status", https_status);

    render_admin(&state, "admin/site-health.html", &ctx)
}

// ---------------------------------------------------------------------------
// Media upload handler (drag-and-drop / file picker)
// ---------------------------------------------------------------------------

const ALLOWED_UPLOAD_MIME_TYPES: &[&str] = &[
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

const MAX_UPLOAD_SIZE: usize = 64 * 1024 * 1024;

async fn media_upload(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<Session>,
    mut multipart: Multipart,
) -> Response {
    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No file provided"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Multipart error: {}", e)})),
            )
                .into_response();
        }
    };

    let raw_name = field
        .file_name()
        .unwrap_or("upload")
        .to_string();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();

    if !ALLOWED_UPLOAD_MIME_TYPES.contains(&content_type.as_str()) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(serde_json::json!({"error": format!("File type '{}' is not allowed.", content_type)})),
        )
            .into_response();
    }

    let data = match field.bytes().await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Failed to read file: {}", e)})),
            )
                .into_response();
        }
    };

    if data.len() > MAX_UPLOAD_SIZE {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({"error": format!("File too large ({} bytes). Maximum is {} bytes.", data.len(), MAX_UPLOAD_SIZE)})),
        )
            .into_response();
    }

    let file_name = raw_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect::<String>();
    let file_name = if file_name.is_empty() { "upload".to_string() } else { file_name };

    let uploads_dir = PathBuf::from(
        std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "wp-content/uploads".to_string()),
    );
    let date_dir = chrono::Utc::now().format("%Y/%m").to_string();
    let full_dir = uploads_dir.join(&date_dir);

    if let Err(e) = tokio::fs::create_dir_all(&full_dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create upload directory: {}", e)})),
        )
            .into_response();
    }

    let file_path = full_dir.join(&file_name);

    if let Err(e) = tokio::fs::write(&file_path, &data).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write file: {}", e)})),
        )
            .into_response();
    }

    let display_title = file_name
        .rsplit('.')
        .last()
        .unwrap_or(&file_name)
        .replace(['-', '_'], " ");

    let guid = format!("/wp-content/uploads/{}/{}", date_dir, file_name);
    let slug = file_name
        .split('.')
        .next()
        .unwrap_or("upload")
        .to_lowercase();
    let now = chrono::Utc::now().naive_utc();

    let author_id = session.user_id;

    let new_attachment = wp_posts::ActiveModel {
        post_author: Set(author_id),
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
        guid: Set(guid.clone()),
        menu_order: Set(0),
        post_type: Set("attachment".to_string()),
        post_mime_type: Set(content_type.clone()),
        comment_count: Set(0),
        ..Default::default()
    };

    match new_attachment.insert(&state.db).await {
        Ok(result) => {
            tracing::info!("Uploaded media: {} (id={})", file_name, result.id);
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": result.id,
                    "url": guid,
                    "filename": file_name,
                    "mime_type": content_type,
                })),
            )
                .into_response()
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&file_path).await;
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Database error: {}", e)})),
            )
                .into_response()
        }
    }
}


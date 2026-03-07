use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use rustpress_db::options::OptionsManager;

use crate::common::WpError;
use crate::ApiState;

#[derive(Debug, Serialize)]
pub struct WpSettings {
    pub title: String,
    pub description: String,
    pub url: String,
    pub email: String,
    pub timezone: String,
    pub date_format: String,
    pub time_format: String,
    pub posts_per_page: i64,
    pub default_comment_status: String,
    pub start_of_week: i64,
    pub use_smilies: bool,
    pub default_category: i64,
    pub default_post_format: String,
    pub language: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub email: Option<String>,
    pub timezone: Option<String>,
    pub date_format: Option<String>,
    pub time_format: Option<String>,
    pub posts_per_page: Option<i64>,
    pub default_comment_status: Option<String>,
    pub start_of_week: Option<i64>,
    pub use_smilies: Option<bool>,
    pub default_category: Option<i64>,
    pub default_post_format: Option<String>,
    pub language: Option<String>,
}

/// Public read-only routes (GET).
pub fn read_routes() -> Router<ApiState> {
    Router::new().route("/wp-json/wp/v2/settings", get(get_settings))
}

/// Protected write routes (POST) — authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new().route(
        "/wp-json/wp/v2/settings",
        axum::routing::post(update_settings),
    )
}

async fn get_settings(State(state): State<ApiState>) -> Result<Json<WpSettings>, WpError> {
    let options = OptionsManager::new(state.db.clone());
    let settings = load_settings(&options, &state.site_url).await?;
    Ok(Json(settings))
}

async fn update_settings(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Json(input): Json<UpdateSettingsRequest>,
) -> Result<Json<WpSettings>, WpError> {
    auth.require(&rustpress_auth::Capability::ManageOptions)?;
    let options = OptionsManager::new(state.db.clone());

    if let Some(ref title) = input.title {
        options
            .update_option("blogname", title)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref description) = input.description {
        options
            .update_option("blogdescription", description)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref email) = input.email {
        options
            .update_option("admin_email", email)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref timezone) = input.timezone {
        options
            .update_option("timezone_string", timezone)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref date_format) = input.date_format {
        options
            .update_option("date_format", date_format)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref time_format) = input.time_format {
        options
            .update_option("time_format", time_format)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(posts_per_page) = input.posts_per_page {
        options
            .update_option("posts_per_page", &posts_per_page.to_string())
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref default_comment_status) = input.default_comment_status {
        options
            .update_option("default_comment_status", default_comment_status)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(start_of_week) = input.start_of_week {
        options
            .update_option("start_of_week", &start_of_week.to_string())
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(use_smilies) = input.use_smilies {
        options
            .update_option("use_smilies", if use_smilies { "1" } else { "0" })
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(default_category) = input.default_category {
        options
            .update_option("default_category", &default_category.to_string())
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref default_post_format) = input.default_post_format {
        options
            .update_option("default_post_format", default_post_format)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }
    if let Some(ref language) = input.language {
        options
            .update_option("WPLANG", language)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }

    // Return updated settings
    let settings = load_settings(&options, &state.site_url).await?;
    Ok(Json(settings))
}

async fn load_settings(options: &OptionsManager, site_url: &str) -> Result<WpSettings, WpError> {
    let err = |e: sea_orm::DbErr| WpError::internal(e.to_string());

    let title = options
        .get_option_or("blogname", "RustPress Site")
        .await
        .map_err(err)?;
    let description = options
        .get_option_or("blogdescription", "Just another RustPress site")
        .await
        .map_err(err)?;
    let email = options
        .get_option_or("admin_email", "admin@example.com")
        .await
        .map_err(err)?;
    let timezone = options
        .get_option_or("timezone_string", "UTC")
        .await
        .map_err(err)?;
    let date_format = options
        .get_option_or("date_format", "F j, Y")
        .await
        .map_err(err)?;
    let time_format = options
        .get_option_or("time_format", "g:i a")
        .await
        .map_err(err)?;
    let posts_per_page = options.get_posts_per_page().await.map_err(err)?;
    let comment_status = options
        .get_option_or("default_comment_status", "open")
        .await
        .map_err(err)?;
    let start_of_week = options
        .get_option_or("start_of_week", "1")
        .await
        .map_err(err)?
        .parse()
        .unwrap_or(1);
    let use_smilies = options
        .get_option_or("use_smilies", "1")
        .await
        .map_err(err)?
        == "1";
    let default_category = options
        .get_option_or("default_category", "1")
        .await
        .map_err(err)?
        .parse()
        .unwrap_or(1);
    let default_post_format = options
        .get_option_or("default_post_format", "")
        .await
        .map_err(err)?;
    let language = options.get_option_or("WPLANG", "").await.map_err(err)?;

    Ok(WpSettings {
        title,
        description,
        url: site_url.to_string(),
        email,
        timezone,
        date_format,
        time_format,
        posts_per_page,
        default_comment_status: comment_status,
        start_of_week,
        use_smilies,
        default_category,
        default_post_format,
        language,
    })
}

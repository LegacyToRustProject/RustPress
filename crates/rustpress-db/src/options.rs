use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::entities::wp_options;

/// WordPress Options API - manages site settings from wp_options table.
///
/// Supports autoloaded options (cached in memory at startup) and
/// on-demand option fetching for non-autoloaded values.
#[derive(Clone)]
pub struct OptionsManager {
    cache: Arc<RwLock<HashMap<String, String>>>,
    db: DatabaseConnection,
}

impl OptionsManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            db,
        }
    }

    /// Load all autoload=yes options into memory cache.
    /// Called once at startup for performance.
    pub async fn load_autoload_options(&self) -> Result<usize, sea_orm::DbErr> {
        let options = wp_options::Entity::find()
            .filter(wp_options::Column::Autoload.eq("yes"))
            .all(&self.db)
            .await?;

        let count = options.len();
        let mut cache = self.cache.write().await;
        for opt in options {
            cache.insert(opt.option_name, opt.option_value);
        }

        info!(count, "autoload options loaded into cache");
        Ok(count)
    }

    /// Get an option value by name.
    /// Checks cache first, then falls back to database.
    pub async fn get_option(&self, key: &str) -> Result<Option<String>, sea_orm::DbErr> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(value) = cache.get(key) {
                debug!(key, "option found in cache");
                return Ok(Some(value.clone()));
            }
        }

        // Fallback to database
        let option = wp_options::Entity::find()
            .filter(wp_options::Column::OptionName.eq(key))
            .one(&self.db)
            .await?;

        if let Some(opt) = &option {
            let mut cache = self.cache.write().await;
            cache.insert(opt.option_name.clone(), opt.option_value.clone());
        }

        Ok(option.map(|o| o.option_value))
    }

    /// Get an option with a default value if not found.
    pub async fn get_option_or(
        &self,
        key: &str,
        default: &str,
    ) -> Result<String, sea_orm::DbErr> {
        Ok(self.get_option(key).await?.unwrap_or_else(|| default.to_string()))
    }

    /// Check if an option exists.
    pub async fn option_exists(&self, key: &str) -> Result<bool, sea_orm::DbErr> {
        Ok(self.get_option(key).await?.is_some())
    }

    /// Update or insert an option value.
    pub async fn update_option(&self, key: &str, value: &str) -> Result<(), sea_orm::DbErr> {
        use sea_orm::ActiveModelTrait;
        use sea_orm::ActiveValue::Set;

        let existing = wp_options::Entity::find()
            .filter(wp_options::Column::OptionName.eq(key))
            .one(&self.db)
            .await?;

        if let Some(opt) = existing {
            let mut active: wp_options::ActiveModel = opt.into();
            active.option_value = Set(value.to_string());
            active.update(&self.db).await?;
        } else {
            let new_option = wp_options::ActiveModel {
                option_name: Set(key.to_string()),
                option_value: Set(value.to_string()),
                autoload: Set("yes".to_string()),
                ..Default::default()
            };
            new_option.insert(&self.db).await?;
        }

        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(key.to_string(), value.to_string());

        Ok(())
    }

    /// Delete an option.
    pub async fn delete_option(&self, key: &str) -> Result<bool, sea_orm::DbErr> {
        let result = wp_options::Entity::delete_many()
            .filter(wp_options::Column::OptionName.eq(key))
            .exec(&self.db)
            .await?;

        let mut cache = self.cache.write().await;
        cache.remove(key);

        Ok(result.rows_affected > 0)
    }

    /// Get the site name (blogname option).
    pub async fn get_blogname(&self) -> Result<String, sea_orm::DbErr> {
        self.get_option_or("blogname", "RustPress Site").await
    }

    /// Get the site description (blogdescription option).
    pub async fn get_blogdescription(&self) -> Result<String, sea_orm::DbErr> {
        self.get_option_or("blogdescription", "Just another RustPress site")
            .await
    }

    /// Get the site URL.
    pub async fn get_siteurl(&self) -> Result<String, sea_orm::DbErr> {
        self.get_option_or("siteurl", "http://localhost:3000").await
    }

    /// Get posts per page setting.
    pub async fn get_posts_per_page(&self) -> Result<i64, sea_orm::DbErr> {
        let val = self.get_option_or("posts_per_page", "10").await?;
        Ok(val.parse().unwrap_or(10))
    }
}

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Serialize;

use crate::entities::{wp_postmeta, wp_posts, wp_users};

/// Pagination parameters.
#[derive(Debug, Clone)]
pub struct Pagination {
    pub page: u64,
    pub per_page: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 10,
        }
    }
}

/// Paginated result wrapper.
#[derive(Debug, Serialize)]
pub struct PaginatedResult<T: Serialize> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}

/// Fetch published posts with pagination.
pub async fn get_posts(
    db: &DatabaseConnection,
    post_type: &str,
    post_status: &str,
    pagination: &Pagination,
) -> Result<PaginatedResult<wp_posts::Model>, sea_orm::DbErr> {
    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.eq(post_status))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = query.clone().count(db).await?;
    let total_pages = (total + pagination.per_page - 1) / pagination.per_page;

    let items = query
        .offset((pagination.page - 1) * pagination.per_page)
        .limit(pagination.per_page)
        .all(db)
        .await?;

    Ok(PaginatedResult {
        items,
        total,
        page: pagination.page,
        per_page: pagination.per_page,
        total_pages,
    })
}

/// Fetch a single post by ID.
pub async fn get_post_by_id(
    db: &DatabaseConnection,
    id: u64,
) -> Result<Option<wp_posts::Model>, sea_orm::DbErr> {
    wp_posts::Entity::find_by_id(id).one(db).await
}

/// Fetch a single post by slug.
pub async fn get_post_by_slug(
    db: &DatabaseConnection,
    slug: &str,
) -> Result<Option<wp_posts::Model>, sea_orm::DbErr> {
    wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(slug))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(db)
        .await
}

/// Fetch post meta for a given post ID.
pub async fn get_post_meta(
    db: &DatabaseConnection,
    post_id: u64,
) -> Result<Vec<wp_postmeta::Model>, sea_orm::DbErr> {
    wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .all(db)
        .await
}

/// Fetch a specific meta value for a post.
pub async fn get_post_meta_value(
    db: &DatabaseConnection,
    post_id: u64,
    meta_key: &str,
) -> Result<Option<String>, sea_orm::DbErr> {
    let meta = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .filter(wp_postmeta::Column::MetaKey.eq(meta_key))
        .one(db)
        .await?;

    Ok(meta.and_then(|m| m.meta_value))
}

/// Fetch a user by ID (excludes password).
pub async fn get_user_by_id(
    db: &DatabaseConnection,
    id: u64,
) -> Result<Option<wp_users::Model>, sea_orm::DbErr> {
    wp_users::Entity::find_by_id(id).one(db).await
}

/// Fetch a user by login name.
pub async fn get_user_by_login(
    db: &DatabaseConnection,
    login: &str,
) -> Result<Option<wp_users::Model>, sea_orm::DbErr> {
    wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(login))
        .one(db)
        .await
}

/// Fetch a user by email.
pub async fn get_user_by_email(
    db: &DatabaseConnection,
    email: &str,
) -> Result<Option<wp_users::Model>, sea_orm::DbErr> {
    wp_users::Entity::find()
        .filter(wp_users::Column::UserEmail.eq(email))
        .one(db)
        .await
}

/// Fetch all users with pagination.
pub async fn get_users(
    db: &DatabaseConnection,
    pagination: &Pagination,
) -> Result<PaginatedResult<wp_users::Model>, sea_orm::DbErr> {
    let query = wp_users::Entity::find().order_by_asc(wp_users::Column::UserLogin);

    let total = query.clone().count(db).await?;
    let total_pages = (total + pagination.per_page - 1) / pagination.per_page;

    let items = query
        .offset((pagination.page - 1) * pagination.per_page)
        .limit(pagination.per_page)
        .all(db)
        .await?;

    Ok(PaginatedResult {
        items,
        total,
        page: pagination.page,
        per_page: pagination.per_page,
        total_pages,
    })
}

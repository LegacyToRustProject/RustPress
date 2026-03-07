use std::collections::HashMap;

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Serialize;

use crate::entities::{wc_order_itemmeta, wc_order_items, wp_postmeta, wp_posts, wp_users};

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
    let total_pages = total.div_ceil(pagination.per_page);

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
    let total_pages = total.div_ceil(pagination.per_page);

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

// =====================================================================
// Plugin-specific query functions
// =====================================================================

/// Fetch all post meta for a post as a HashMap (key → value).
///
/// Used by plugin compat layers (Yoast, ACF, WooCommerce) that need
/// to parse multiple meta keys at once via `from_meta()`.
pub async fn get_post_meta_map(
    db: &DatabaseConnection,
    post_id: u64,
) -> Result<HashMap<String, String>, sea_orm::DbErr> {
    let rows = get_post_meta(db, post_id).await?;
    let mut map = HashMap::new();
    for row in rows {
        if let (Some(key), Some(value)) = (row.meta_key, row.meta_value) {
            map.insert(key, value);
        }
    }
    Ok(map)
}

/// Fetch only specific meta keys for a post as a HashMap.
///
/// More efficient than `get_post_meta_map` when you only need
/// a known set of keys (e.g., Yoast's 14 meta keys).
pub async fn get_post_meta_by_keys(
    db: &DatabaseConnection,
    post_id: u64,
    keys: &[&str],
) -> Result<HashMap<String, String>, sea_orm::DbErr> {
    let rows = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .filter(wp_postmeta::Column::MetaKey.is_in(keys.iter().map(|k| k.to_string())))
        .all(db)
        .await?;

    let mut map = HashMap::new();
    for row in rows {
        if let (Some(key), Some(value)) = (row.meta_key, row.meta_value) {
            map.insert(key, value);
        }
    }
    Ok(map)
}

/// Write or update a single post meta value.
///
/// If the meta key already exists for the post, updates it.
/// Otherwise, inserts a new row.
pub async fn set_post_meta(
    db: &DatabaseConnection,
    post_id: u64,
    meta_key: &str,
    meta_value: &str,
) -> Result<(), sea_orm::DbErr> {
    use sea_orm::ActiveModelTrait;
    use sea_orm::Set;

    let existing = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .filter(wp_postmeta::Column::MetaKey.eq(meta_key))
        .one(db)
        .await?;

    if let Some(row) = existing {
        let mut active: wp_postmeta::ActiveModel = row.into();
        active.meta_value = Set(Some(meta_value.to_string()));
        active.update(db).await?;
    } else {
        let active = wp_postmeta::ActiveModel {
            meta_id: Default::default(),
            post_id: Set(post_id),
            meta_key: Set(Some(meta_key.to_string())),
            meta_value: Set(Some(meta_value.to_string())),
        };
        active.insert(db).await?;
    }

    Ok(())
}

/// Write multiple meta key-value pairs for a post.
///
/// Used by plugin compat layers that serialize via `to_meta()`.
pub async fn set_post_meta_bulk(
    db: &DatabaseConnection,
    post_id: u64,
    pairs: &[(String, String)],
) -> Result<(), sea_orm::DbErr> {
    for (key, value) in pairs {
        set_post_meta(db, post_id, key, value).await?;
    }
    Ok(())
}

/// Delete a specific meta key for a post.
pub async fn delete_post_meta(
    db: &DatabaseConnection,
    post_id: u64,
    meta_key: &str,
) -> Result<bool, sea_orm::DbErr> {
    let result = wp_postmeta::Entity::delete_many()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .filter(wp_postmeta::Column::MetaKey.eq(meta_key))
        .exec(db)
        .await?;

    Ok(result.rows_affected > 0)
}

/// Fetch posts by post_type with pagination (any status except trash).
///
/// Used for WooCommerce products, CF7 forms, ACF field groups, etc.
pub async fn get_posts_by_type(
    db: &DatabaseConnection,
    post_type: &str,
    pagination: &Pagination,
) -> Result<PaginatedResult<wp_posts::Model>, sea_orm::DbErr> {
    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.ne("trash"))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = query.clone().count(db).await?;
    let total_pages = if pagination.per_page > 0 {
        total.div_ceil(pagination.per_page)
    } else {
        1
    };

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

/// Fetch child posts (used for ACF field definitions under field groups,
/// WooCommerce product variations, etc.).
pub async fn get_child_posts(
    db: &DatabaseConnection,
    parent_id: u64,
    post_type: &str,
) -> Result<Vec<wp_posts::Model>, sea_orm::DbErr> {
    wp_posts::Entity::find()
        .filter(wp_posts::Column::PostParent.eq(parent_id))
        .filter(wp_posts::Column::PostType.eq(post_type))
        .order_by_asc(wp_posts::Column::MenuOrder)
        .all(db)
        .await
}

/// Fetch WooCommerce order items for an order.
pub async fn get_order_items(
    db: &DatabaseConnection,
    order_id: u64,
) -> Result<Vec<wc_order_items::Model>, sea_orm::DbErr> {
    wc_order_items::Entity::find()
        .filter(wc_order_items::Column::OrderId.eq(order_id))
        .all(db)
        .await
}

/// Fetch meta for a WooCommerce order item.
pub async fn get_order_item_meta(
    db: &DatabaseConnection,
    order_item_id: u64,
) -> Result<HashMap<String, String>, sea_orm::DbErr> {
    let rows = wc_order_itemmeta::Entity::find()
        .filter(wc_order_itemmeta::Column::OrderItemId.eq(order_item_id))
        .all(db)
        .await?;

    let mut map = HashMap::new();
    for row in rows {
        if let (Some(key), Some(value)) = (row.meta_key, row.meta_value) {
            map.insert(key, value);
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_default() {
        let p = Pagination::default();
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, 10);
    }

    #[test]
    fn test_pagination_custom() {
        let p = Pagination {
            page: 3,
            per_page: 25,
        };
        assert_eq!(p.page, 3);
        assert_eq!(p.per_page, 25);
    }

    #[test]
    fn test_pagination_clone() {
        let p = Pagination {
            page: 5,
            per_page: 20,
        };
        let p2 = p.clone();
        assert_eq!(p2.page, 5);
        assert_eq!(p2.per_page, 20);
    }

    /// Helper to build a PaginatedResult<String> for testing total_pages logic.
    /// Uses the same formula as the production code: (total + per_page - 1) / per_page.
    fn make_paginated(total: u64, per_page: u64) -> PaginatedResult<String> {
        let total_pages = if total == 0 {
            0
        } else {
            (total + per_page - 1) / per_page
        };
        PaginatedResult {
            items: Vec::new(),
            total,
            page: 1,
            per_page,
            total_pages,
        }
    }

    #[test]
    fn test_paginated_result_total_pages_25_items() {
        // 25 items, 10 per page => 3 pages
        let r = make_paginated(25, 10);
        assert_eq!(r.total_pages, 3);
        assert_eq!(r.total, 25);
        assert_eq!(r.per_page, 10);
        assert_eq!(r.page, 1);
    }

    #[test]
    fn test_paginated_result_total_pages_exact_fit() {
        // 10 items, 10 per page => 1 page
        let r = make_paginated(10, 10);
        assert_eq!(r.total_pages, 1);
    }

    #[test]
    fn test_paginated_result_total_pages_zero_items() {
        // 0 items => 0 pages
        let r = make_paginated(0, 10);
        assert_eq!(r.total_pages, 0);
    }

    #[test]
    fn test_paginated_result_total_pages_one_item() {
        // 1 item, 10 per page => 1 page
        let r = make_paginated(1, 10);
        assert_eq!(r.total_pages, 1);
    }

    #[test]
    fn test_paginated_result_total_pages_11_items() {
        // 11 items, 10 per page => 2 pages
        let r = make_paginated(11, 10);
        assert_eq!(r.total_pages, 2);
    }

    #[test]
    fn test_paginated_result_fields() {
        let r = PaginatedResult {
            items: vec!["a".to_string(), "b".to_string()],
            total: 50,
            page: 3,
            per_page: 20,
            total_pages: 3,
        };
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.items[0], "a");
        assert_eq!(r.items[1], "b");
        assert_eq!(r.total, 50);
        assert_eq!(r.page, 3);
        assert_eq!(r.per_page, 20);
        assert_eq!(r.total_pages, 3);
    }
}

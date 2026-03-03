use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_posts")]
pub struct Model {
    #[sea_orm(primary_key, column_name = "ID")]
    pub id: u64,
    pub post_author: u64,
    pub post_date: DateTime,
    pub post_date_gmt: DateTime,
    #[sea_orm(column_type = "custom(\"LONGTEXT\")")]
    pub post_content: String,
    #[sea_orm(column_type = "Text")]
    pub post_title: String,
    #[sea_orm(column_type = "Text")]
    pub post_excerpt: String,
    #[sea_orm(string_len = 20)]
    pub post_status: String,
    #[sea_orm(string_len = 20)]
    pub comment_status: String,
    #[sea_orm(string_len = 20)]
    pub ping_status: String,
    #[sea_orm(string_len = 255)]
    pub post_password: String,
    #[sea_orm(string_len = 200)]
    pub post_name: String,
    #[sea_orm(column_type = "Text")]
    pub to_ping: String,
    #[sea_orm(column_type = "Text")]
    pub pinged: String,
    pub post_modified: DateTime,
    pub post_modified_gmt: DateTime,
    #[sea_orm(column_type = "custom(\"LONGTEXT\")")]
    pub post_content_filtered: String,
    pub post_parent: u64,
    #[sea_orm(string_len = 255)]
    pub guid: String,
    pub menu_order: i32,
    #[sea_orm(string_len = 20)]
    pub post_type: String,
    #[sea_orm(string_len = 100)]
    pub post_mime_type: String,
    pub comment_count: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

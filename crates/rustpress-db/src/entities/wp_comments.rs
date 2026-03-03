use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_comments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub comment_id: u64,
    pub comment_post_id: u64,
    #[sea_orm(column_type = "Text")]
    pub comment_author: String,
    #[sea_orm(string_len = 100)]
    pub comment_author_email: String,
    #[sea_orm(string_len = 200)]
    pub comment_author_url: String,
    #[sea_orm(string_len = 100)]
    pub comment_author_ip: String,
    pub comment_date: DateTime,
    pub comment_date_gmt: DateTime,
    #[sea_orm(column_type = "Text")]
    pub comment_content: String,
    pub comment_karma: i32,
    #[sea_orm(string_len = 20)]
    pub comment_approved: String,
    #[sea_orm(string_len = 255)]
    pub comment_agent: String,
    #[sea_orm(string_len = 20)]
    pub comment_type: String,
    pub comment_parent: u64,
    pub user_id: u64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_links")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub link_id: u64,
    #[sea_orm(string_len = 255)]
    pub link_url: String,
    #[sea_orm(string_len = 255)]
    pub link_name: String,
    #[sea_orm(string_len = 255)]
    pub link_image: String,
    #[sea_orm(string_len = 255)]
    pub link_target: String,
    #[sea_orm(string_len = 255)]
    pub link_description: String,
    #[sea_orm(string_len = 20)]
    pub link_visible: String,
    pub link_owner: u64,
    pub link_rating: i32,
    pub link_updated: DateTime,
    #[sea_orm(string_len = 255)]
    pub link_rel: String,
    #[sea_orm(column_type = "Text")]
    pub link_notes: String,
    #[sea_orm(string_len = 255)]
    pub link_rss: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_users")]
pub struct Model {
    #[sea_orm(primary_key, column_name = "ID")]
    pub id: u64,
    #[sea_orm(string_len = 60)]
    pub user_login: String,
    #[sea_orm(string_len = 255)]
    pub user_pass: String,
    #[sea_orm(string_len = 50)]
    pub user_nicename: String,
    #[sea_orm(string_len = 100)]
    pub user_email: String,
    #[sea_orm(string_len = 100)]
    pub user_url: String,
    pub user_registered: DateTime,
    #[sea_orm(string_len = 255)]
    pub user_activation_key: String,
    pub user_status: i32,
    #[sea_orm(string_len = 250)]
    pub display_name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

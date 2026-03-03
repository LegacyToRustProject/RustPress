use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_term_taxonomy")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub term_taxonomy_id: u64,
    pub term_id: u64,
    #[sea_orm(string_len = 32)]
    pub taxonomy: String,
    #[sea_orm(column_type = "custom(\"LONGTEXT\")")]
    pub description: String,
    pub parent: u64,
    pub count: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

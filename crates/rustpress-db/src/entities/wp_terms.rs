use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_terms")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub term_id: u64,
    #[sea_orm(string_len = 200)]
    pub name: String,
    #[sea_orm(string_len = 200)]
    pub slug: String,
    pub term_group: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

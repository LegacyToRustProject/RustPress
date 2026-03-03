use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_term_relationships")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub object_id: u64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub term_taxonomy_id: u64,
    pub term_order: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

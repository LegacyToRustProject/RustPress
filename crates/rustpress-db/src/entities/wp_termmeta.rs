use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_termmeta")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub meta_id: u64,
    pub term_id: u64,
    #[sea_orm(string_len = 255, nullable)]
    pub meta_key: Option<String>,
    #[sea_orm(column_type = "custom(\"LONGTEXT\")", nullable)]
    pub meta_value: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

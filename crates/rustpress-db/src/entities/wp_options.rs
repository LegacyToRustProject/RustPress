use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_options")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub option_id: u64,
    #[sea_orm(string_len = 191, unique)]
    pub option_name: String,
    #[sea_orm(column_type = "custom(\"LONGTEXT\")")]
    pub option_value: String,
    #[sea_orm(string_len = 20)]
    pub autoload: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// WooCommerce order items stored in `wp_woocommerce_order_items`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wp_woocommerce_order_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub order_item_id: u64,
    #[sea_orm(string_len = 200)]
    pub order_item_name: String,
    #[sea_orm(string_len = 200)]
    pub order_item_type: String,
    pub order_id: u64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

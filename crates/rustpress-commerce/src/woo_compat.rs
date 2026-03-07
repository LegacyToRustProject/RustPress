//! WooCommerce compatibility layer.
//!
//! Maps RustPress commerce data to/from WooCommerce's storage format
//! in wp_posts and wp_postmeta, enabling seamless migration from
//! WordPress + WooCommerce to RustPress.
//!
//! ## WooCommerce Post Types
//! - `product`      — Products (wp_posts)
//! - `product_variation` — Product variations
//! - `shop_order`   — Orders
//! - `shop_coupon`  — Coupons
//! - `shop_order_refund` — Refunds
//!
//! ## Product Meta Keys (wp_postmeta)
//! - `_price`, `_regular_price`, `_sale_price`
//! - `_sku`, `_stock`, `_stock_status`, `_manage_stock`
//! - `_weight`, `_length`, `_width`, `_height`
//! - `_virtual`, `_downloadable`
//! - `_product_image_gallery` (comma-separated attachment IDs)
//! - `_tax_status`, `_tax_class`
//! - `_backorders`
//!
//! ## Order Meta Keys (wp_postmeta)
//! - `_order_total`, `_order_tax`, `_order_shipping`, `_order_discount`
//! - `_billing_first_name`, `_billing_last_name`, `_billing_email`, etc.
//! - `_shipping_first_name`, `_shipping_last_name`, etc.
//! - `_payment_method`, `_payment_method_title`
//! - `_customer_user`, `_order_currency`, `_order_key`

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::order::{Address, OrderStatus};
use crate::product::{Product, ProductType, StockStatus};

/// WooCommerce post types.
pub mod post_types {
    pub const PRODUCT: &str = "product";
    pub const PRODUCT_VARIATION: &str = "product_variation";
    pub const ORDER: &str = "shop_order";
    pub const COUPON: &str = "shop_coupon";
    pub const REFUND: &str = "shop_order_refund";
}

/// WooCommerce product meta keys.
pub mod product_keys {
    pub const PRICE: &str = "_price";
    pub const REGULAR_PRICE: &str = "_regular_price";
    pub const SALE_PRICE: &str = "_sale_price";
    pub const SKU: &str = "_sku";
    pub const STOCK: &str = "_stock";
    pub const STOCK_STATUS: &str = "_stock_status";
    pub const MANAGE_STOCK: &str = "_manage_stock";
    pub const WEIGHT: &str = "_weight";
    pub const LENGTH: &str = "_length";
    pub const WIDTH: &str = "_width";
    pub const HEIGHT: &str = "_height";
    pub const VIRTUAL: &str = "_virtual";
    pub const DOWNLOADABLE: &str = "_downloadable";
    pub const IMAGE_GALLERY: &str = "_product_image_gallery";
    pub const TAX_STATUS: &str = "_tax_status";
    pub const TAX_CLASS: &str = "_tax_class";
    pub const BACKORDERS: &str = "_backorders";
    pub const PRODUCT_TYPE: &str = "product_type";

    pub const ALL: &[&str] = &[
        PRICE,
        REGULAR_PRICE,
        SALE_PRICE,
        SKU,
        STOCK,
        STOCK_STATUS,
        MANAGE_STOCK,
        WEIGHT,
        LENGTH,
        WIDTH,
        HEIGHT,
        VIRTUAL,
        DOWNLOADABLE,
        IMAGE_GALLERY,
        TAX_STATUS,
        TAX_CLASS,
        BACKORDERS,
    ];
}

/// WooCommerce order meta keys.
pub mod order_keys {
    pub const ORDER_TOTAL: &str = "_order_total";
    pub const ORDER_TAX: &str = "_order_tax";
    pub const ORDER_SHIPPING: &str = "_order_shipping";
    pub const ORDER_DISCOUNT: &str = "_order_discount";
    pub const ORDER_CURRENCY: &str = "_order_currency";
    pub const ORDER_KEY: &str = "_order_key";
    pub const PAYMENT_METHOD: &str = "_payment_method";
    pub const PAYMENT_METHOD_TITLE: &str = "_payment_method_title";
    pub const CUSTOMER_USER: &str = "_customer_user";
    pub const CUSTOMER_NOTE: &str = "_customer_note";

    // Billing address
    pub const BILLING_FIRST_NAME: &str = "_billing_first_name";
    pub const BILLING_LAST_NAME: &str = "_billing_last_name";
    pub const BILLING_COMPANY: &str = "_billing_company";
    pub const BILLING_ADDRESS_1: &str = "_billing_address_1";
    pub const BILLING_ADDRESS_2: &str = "_billing_address_2";
    pub const BILLING_CITY: &str = "_billing_city";
    pub const BILLING_STATE: &str = "_billing_state";
    pub const BILLING_POSTCODE: &str = "_billing_postcode";
    pub const BILLING_COUNTRY: &str = "_billing_country";
    pub const BILLING_EMAIL: &str = "_billing_email";
    pub const BILLING_PHONE: &str = "_billing_phone";

    // Shipping address
    pub const SHIPPING_FIRST_NAME: &str = "_shipping_first_name";
    pub const SHIPPING_LAST_NAME: &str = "_shipping_last_name";
    pub const SHIPPING_COMPANY: &str = "_shipping_company";
    pub const SHIPPING_ADDRESS_1: &str = "_shipping_address_1";
    pub const SHIPPING_ADDRESS_2: &str = "_shipping_address_2";
    pub const SHIPPING_CITY: &str = "_shipping_city";
    pub const SHIPPING_STATE: &str = "_shipping_state";
    pub const SHIPPING_POSTCODE: &str = "_shipping_postcode";
    pub const SHIPPING_COUNTRY: &str = "_shipping_country";
}

/// WooCommerce order status values as stored in wp_posts.post_status.
pub mod order_statuses {
    pub const PENDING: &str = "wc-pending";
    pub const PROCESSING: &str = "wc-processing";
    pub const ON_HOLD: &str = "wc-on-hold";
    pub const COMPLETED: &str = "wc-completed";
    pub const CANCELLED: &str = "wc-cancelled";
    pub const REFUNDED: &str = "wc-refunded";
    pub const FAILED: &str = "wc-failed";
}

/// WooCommerce-compatible product data read from wp_posts + wp_postmeta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WooProductData {
    pub post_id: u64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub short_description: String,
    pub sku: String,
    pub price: f64,
    pub regular_price: f64,
    pub sale_price: Option<f64>,
    pub stock_quantity: Option<i64>,
    pub stock_status: String,
    pub manage_stock: bool,
    pub product_type: String,
    pub weight: Option<f64>,
    pub length: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub is_virtual: bool,
    pub is_downloadable: bool,
    pub image_gallery: Vec<u64>,
    pub tax_status: String,
    pub tax_class: String,
    pub backorders: String,
}

impl WooProductData {
    /// Parse product data from wp_posts fields and wp_postmeta key-value pairs.
    ///
    /// `post_title`, `post_name`, `post_content`, `post_excerpt` come from wp_posts.
    /// All other fields are read from the meta HashMap.
    pub fn from_post_and_meta(
        post_id: u64,
        post_title: &str,
        post_name: &str,
        post_content: &str,
        post_excerpt: &str,
        meta: &HashMap<String, String>,
    ) -> Self {
        let gallery_ids = meta
            .get(product_keys::IMAGE_GALLERY)
            .map(|v| {
                v.split(',')
                    .filter_map(|s| s.trim().parse::<u64>().ok())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            post_id,
            name: post_title.to_string(),
            slug: post_name.to_string(),
            description: post_content.to_string(),
            short_description: post_excerpt.to_string(),
            sku: get_str(meta, product_keys::SKU),
            price: get_f64(meta, product_keys::PRICE),
            regular_price: get_f64(meta, product_keys::REGULAR_PRICE),
            sale_price: non_empty_f64(meta.get(product_keys::SALE_PRICE)),
            stock_quantity: meta.get(product_keys::STOCK).and_then(|v| v.parse().ok()),
            stock_status: meta
                .get(product_keys::STOCK_STATUS)
                .cloned()
                .unwrap_or_else(|| "instock".to_string()),
            manage_stock: meta
                .get(product_keys::MANAGE_STOCK)
                .is_some_and(|v| v == "yes"),
            product_type: get_str(meta, product_keys::PRODUCT_TYPE),
            weight: non_empty_f64(meta.get(product_keys::WEIGHT)),
            length: non_empty_f64(meta.get(product_keys::LENGTH)),
            width: non_empty_f64(meta.get(product_keys::WIDTH)),
            height: non_empty_f64(meta.get(product_keys::HEIGHT)),
            is_virtual: meta
                .get(product_keys::VIRTUAL)
                .is_some_and(|v| v == "yes"),
            is_downloadable: meta
                .get(product_keys::DOWNLOADABLE)
                .is_some_and(|v| v == "yes"),
            image_gallery: gallery_ids,
            tax_status: meta
                .get(product_keys::TAX_STATUS)
                .cloned()
                .unwrap_or_else(|| "taxable".to_string()),
            tax_class: get_str(meta, product_keys::TAX_CLASS),
            backorders: meta
                .get(product_keys::BACKORDERS)
                .cloned()
                .unwrap_or_else(|| "no".to_string()),
        }
    }

    /// Convert to wp_postmeta key-value pairs for writing.
    pub fn to_meta(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();

        pairs.push((product_keys::PRICE.into(), self.price.to_string()));
        pairs.push((
            product_keys::REGULAR_PRICE.into(),
            self.regular_price.to_string(),
        ));
        if let Some(sale) = self.sale_price {
            pairs.push((product_keys::SALE_PRICE.into(), sale.to_string()));
        }
        if !self.sku.is_empty() {
            pairs.push((product_keys::SKU.into(), self.sku.clone()));
        }
        if let Some(qty) = self.stock_quantity {
            pairs.push((product_keys::STOCK.into(), qty.to_string()));
        }
        pairs.push((product_keys::STOCK_STATUS.into(), self.stock_status.clone()));
        pairs.push((
            product_keys::MANAGE_STOCK.into(),
            if self.manage_stock { "yes" } else { "no" }.into(),
        ));
        if let Some(w) = self.weight {
            pairs.push((product_keys::WEIGHT.into(), w.to_string()));
        }
        if let Some(l) = self.length {
            pairs.push((product_keys::LENGTH.into(), l.to_string()));
        }
        if let Some(w) = self.width {
            pairs.push((product_keys::WIDTH.into(), w.to_string()));
        }
        if let Some(h) = self.height {
            pairs.push((product_keys::HEIGHT.into(), h.to_string()));
        }
        pairs.push((
            product_keys::VIRTUAL.into(),
            if self.is_virtual { "yes" } else { "no" }.into(),
        ));
        pairs.push((
            product_keys::DOWNLOADABLE.into(),
            if self.is_downloadable { "yes" } else { "no" }.into(),
        ));
        if !self.image_gallery.is_empty() {
            let ids: Vec<String> = self.image_gallery.iter().map(|id| id.to_string()).collect();
            pairs.push((product_keys::IMAGE_GALLERY.into(), ids.join(",")));
        }
        pairs.push((product_keys::TAX_STATUS.into(), self.tax_status.clone()));
        if !self.tax_class.is_empty() {
            pairs.push((product_keys::TAX_CLASS.into(), self.tax_class.clone()));
        }
        pairs.push((product_keys::BACKORDERS.into(), self.backorders.clone()));

        pairs
    }

    /// Convert to the internal Product struct used by the commerce engine.
    pub fn to_product(&self) -> Product {
        use chrono::Utc;

        let stock_status = match self.stock_status.as_str() {
            "instock" => StockStatus::InStock,
            "outofstock" => StockStatus::OutOfStock,
            "onbackorder" => StockStatus::OnBackorder,
            _ => StockStatus::InStock,
        };

        let product_type = match self.product_type.as_str() {
            "simple" => ProductType::Simple,
            "variable" => ProductType::Variable,
            "grouped" => ProductType::Grouped,
            "external" => ProductType::External,
            _ => ProductType::Simple,
        };

        let dimensions = match (self.length, self.width, self.height) {
            (Some(l), Some(w), Some(h)) => Some(crate::product::Dimensions {
                length: l,
                width: w,
                height: h,
                unit: "cm".to_string(),
            }),
            _ => None,
        };

        let now = Utc::now();

        Product {
            id: self.post_id,
            name: self.name.clone(),
            slug: self.slug.clone(),
            description: self.description.clone(),
            short_description: self.short_description.clone(),
            sku: self.sku.clone(),
            price: self.price,
            regular_price: self.regular_price,
            sale_price: self.sale_price,
            stock_quantity: self.stock_quantity,
            stock_status,
            product_type,
            categories: vec![],
            tags: vec![],
            images: self.image_gallery.iter().map(|id| id.to_string()).collect(),
            weight: self.weight,
            dimensions,
            attributes: vec![],
            variations: vec![],
            created_at: now,
            updated_at: now,
        }
    }
}

/// WooCommerce-compatible order data read from wp_posts + wp_postmeta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WooOrderData {
    pub post_id: u64,
    pub order_key: String,
    pub status: String,
    pub currency: String,
    pub total: f64,
    pub tax_total: f64,
    pub shipping_total: f64,
    pub discount_total: f64,
    pub payment_method: String,
    pub payment_method_title: String,
    pub customer_id: Option<u64>,
    pub customer_note: String,
    pub billing: Address,
    pub shipping: Address,
}

impl WooOrderData {
    /// Parse order data from wp_posts.post_status and wp_postmeta.
    pub fn from_post_and_meta(
        post_id: u64,
        post_status: &str,
        meta: &HashMap<String, String>,
    ) -> Self {
        Self {
            post_id,
            order_key: get_str(meta, order_keys::ORDER_KEY),
            status: post_status.to_string(),
            currency: meta
                .get(order_keys::ORDER_CURRENCY)
                .cloned()
                .unwrap_or_else(|| "USD".to_string()),
            total: get_f64(meta, order_keys::ORDER_TOTAL),
            tax_total: get_f64(meta, order_keys::ORDER_TAX),
            shipping_total: get_f64(meta, order_keys::ORDER_SHIPPING),
            discount_total: get_f64(meta, order_keys::ORDER_DISCOUNT),
            payment_method: get_str(meta, order_keys::PAYMENT_METHOD),
            payment_method_title: get_str(meta, order_keys::PAYMENT_METHOD_TITLE),
            customer_id: meta
                .get(order_keys::CUSTOMER_USER)
                .and_then(|v| v.parse().ok())
                .filter(|&id: &u64| id > 0),
            customer_note: get_str(meta, order_keys::CUSTOMER_NOTE),
            billing: Address {
                first_name: get_str(meta, order_keys::BILLING_FIRST_NAME),
                last_name: get_str(meta, order_keys::BILLING_LAST_NAME),
                company: get_str(meta, order_keys::BILLING_COMPANY),
                address_1: get_str(meta, order_keys::BILLING_ADDRESS_1),
                address_2: get_str(meta, order_keys::BILLING_ADDRESS_2),
                city: get_str(meta, order_keys::BILLING_CITY),
                state: get_str(meta, order_keys::BILLING_STATE),
                postcode: get_str(meta, order_keys::BILLING_POSTCODE),
                country: get_str(meta, order_keys::BILLING_COUNTRY),
                email: get_str(meta, order_keys::BILLING_EMAIL),
                phone: get_str(meta, order_keys::BILLING_PHONE),
            },
            shipping: Address {
                first_name: get_str(meta, order_keys::SHIPPING_FIRST_NAME),
                last_name: get_str(meta, order_keys::SHIPPING_LAST_NAME),
                company: get_str(meta, order_keys::SHIPPING_COMPANY),
                address_1: get_str(meta, order_keys::SHIPPING_ADDRESS_1),
                address_2: get_str(meta, order_keys::SHIPPING_ADDRESS_2),
                city: get_str(meta, order_keys::SHIPPING_CITY),
                state: get_str(meta, order_keys::SHIPPING_STATE),
                postcode: get_str(meta, order_keys::SHIPPING_POSTCODE),
                country: get_str(meta, order_keys::SHIPPING_COUNTRY),
                email: String::new(),
                phone: String::new(),
            },
        }
    }

    /// Convert to wp_postmeta key-value pairs for writing.
    pub fn to_meta(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();

        pairs.push((order_keys::ORDER_TOTAL.into(), self.total.to_string()));
        pairs.push((order_keys::ORDER_TAX.into(), self.tax_total.to_string()));
        pairs.push((
            order_keys::ORDER_SHIPPING.into(),
            self.shipping_total.to_string(),
        ));
        pairs.push((
            order_keys::ORDER_DISCOUNT.into(),
            self.discount_total.to_string(),
        ));
        pairs.push((order_keys::ORDER_CURRENCY.into(), self.currency.clone()));
        if !self.order_key.is_empty() {
            pairs.push((order_keys::ORDER_KEY.into(), self.order_key.clone()));
        }
        if !self.payment_method.is_empty() {
            pairs.push((
                order_keys::PAYMENT_METHOD.into(),
                self.payment_method.clone(),
            ));
        }
        if !self.payment_method_title.is_empty() {
            pairs.push((
                order_keys::PAYMENT_METHOD_TITLE.into(),
                self.payment_method_title.clone(),
            ));
        }
        if let Some(cid) = self.customer_id {
            pairs.push((order_keys::CUSTOMER_USER.into(), cid.to_string()));
        }

        // Billing
        push_if_nonempty(
            &mut pairs,
            order_keys::BILLING_FIRST_NAME,
            &self.billing.first_name,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::BILLING_LAST_NAME,
            &self.billing.last_name,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::BILLING_COMPANY,
            &self.billing.company,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::BILLING_ADDRESS_1,
            &self.billing.address_1,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::BILLING_ADDRESS_2,
            &self.billing.address_2,
        );
        push_if_nonempty(&mut pairs, order_keys::BILLING_CITY, &self.billing.city);
        push_if_nonempty(&mut pairs, order_keys::BILLING_STATE, &self.billing.state);
        push_if_nonempty(
            &mut pairs,
            order_keys::BILLING_POSTCODE,
            &self.billing.postcode,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::BILLING_COUNTRY,
            &self.billing.country,
        );
        push_if_nonempty(&mut pairs, order_keys::BILLING_EMAIL, &self.billing.email);
        push_if_nonempty(&mut pairs, order_keys::BILLING_PHONE, &self.billing.phone);

        // Shipping
        push_if_nonempty(
            &mut pairs,
            order_keys::SHIPPING_FIRST_NAME,
            &self.shipping.first_name,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::SHIPPING_LAST_NAME,
            &self.shipping.last_name,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::SHIPPING_COMPANY,
            &self.shipping.company,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::SHIPPING_ADDRESS_1,
            &self.shipping.address_1,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::SHIPPING_ADDRESS_2,
            &self.shipping.address_2,
        );
        push_if_nonempty(&mut pairs, order_keys::SHIPPING_CITY, &self.shipping.city);
        push_if_nonempty(&mut pairs, order_keys::SHIPPING_STATE, &self.shipping.state);
        push_if_nonempty(
            &mut pairs,
            order_keys::SHIPPING_POSTCODE,
            &self.shipping.postcode,
        );
        push_if_nonempty(
            &mut pairs,
            order_keys::SHIPPING_COUNTRY,
            &self.shipping.country,
        );

        pairs
    }

    /// Convert WooCommerce post_status to internal OrderStatus.
    pub fn to_order_status(&self) -> OrderStatus {
        wc_status_to_order_status(&self.status)
    }
}

/// Convert a WooCommerce `wc-*` post_status to an `OrderStatus`.
pub fn wc_status_to_order_status(wc_status: &str) -> OrderStatus {
    match wc_status {
        order_statuses::PENDING | "pending" => OrderStatus::Pending,
        order_statuses::PROCESSING | "processing" => OrderStatus::Processing,
        order_statuses::ON_HOLD | "on-hold" => OrderStatus::OnHold,
        order_statuses::COMPLETED | "completed" => OrderStatus::Completed,
        order_statuses::CANCELLED | "cancelled" => OrderStatus::Cancelled,
        order_statuses::REFUNDED | "refunded" => OrderStatus::Refunded,
        order_statuses::FAILED | "failed" => OrderStatus::Failed,
        _ => OrderStatus::Pending,
    }
}

/// Convert an `OrderStatus` to a WooCommerce `wc-*` post_status.
pub fn order_status_to_wc_status(status: &OrderStatus) -> &'static str {
    match status {
        OrderStatus::Pending => order_statuses::PENDING,
        OrderStatus::Processing => order_statuses::PROCESSING,
        OrderStatus::OnHold => order_statuses::ON_HOLD,
        OrderStatus::Completed => order_statuses::COMPLETED,
        OrderStatus::Cancelled => order_statuses::CANCELLED,
        OrderStatus::Refunded => order_statuses::REFUNDED,
        OrderStatus::Failed => order_statuses::FAILED,
    }
}

fn get_str(meta: &HashMap<String, String>, key: &str) -> String {
    meta.get(key).cloned().unwrap_or_default()
}

fn get_f64(meta: &HashMap<String, String>, key: &str) -> f64 {
    meta.get(key).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn non_empty_f64(val: Option<&String>) -> Option<f64> {
    val.filter(|s| !s.is_empty()).and_then(|s| s.parse().ok())
}

fn push_if_nonempty(pairs: &mut Vec<(String, String)>, key: &str, value: &str) {
    if !value.is_empty() {
        pairs.push((key.to_string(), value.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_product_meta() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("_price".into(), "29.99".into());
        m.insert("_regular_price".into(), "39.99".into());
        m.insert("_sale_price".into(), "29.99".into());
        m.insert("_sku".into(), "RUST-001".into());
        m.insert("_stock".into(), "50".into());
        m.insert("_stock_status".into(), "instock".into());
        m.insert("_manage_stock".into(), "yes".into());
        m.insert("_weight".into(), "0.5".into());
        m.insert("_length".into(), "20".into());
        m.insert("_width".into(), "15".into());
        m.insert("_height".into(), "5".into());
        m.insert("_virtual".into(), "no".into());
        m.insert("_downloadable".into(), "no".into());
        m.insert("_product_image_gallery".into(), "101,102,103".into());
        m.insert("_tax_status".into(), "taxable".into());
        m.insert("_backorders".into(), "no".into());
        m.insert("product_type".into(), "simple".into());
        m
    }

    fn sample_order_meta() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("_order_total".into(), "59.98".into());
        m.insert("_order_tax".into(), "4.80".into());
        m.insert("_order_shipping".into(), "5.00".into());
        m.insert("_order_discount".into(), "0".into());
        m.insert("_order_currency".into(), "USD".into());
        m.insert("_order_key".into(), "wc_order_abc123".into());
        m.insert("_payment_method".into(), "stripe".into());
        m.insert(
            "_payment_method_title".into(),
            "Credit Card (Stripe)".into(),
        );
        m.insert("_customer_user".into(), "42".into());
        m.insert("_billing_first_name".into(), "Alice".into());
        m.insert("_billing_last_name".into(), "Smith".into());
        m.insert("_billing_email".into(), "alice@example.com".into());
        m.insert("_billing_city".into(), "Tokyo".into());
        m.insert("_billing_country".into(), "JP".into());
        m.insert("_shipping_first_name".into(), "Alice".into());
        m.insert("_shipping_last_name".into(), "Smith".into());
        m.insert("_shipping_city".into(), "Tokyo".into());
        m.insert("_shipping_country".into(), "JP".into());
        m
    }

    #[test]
    fn test_product_from_meta() {
        let meta = sample_product_meta();
        let product = WooProductData::from_post_and_meta(
            1,
            "Rust Book",
            "rust-book",
            "A great book about Rust.",
            "Short desc",
            &meta,
        );

        assert_eq!(product.post_id, 1);
        assert_eq!(product.name, "Rust Book");
        assert_eq!(product.slug, "rust-book");
        assert_eq!(product.sku, "RUST-001");
        assert_eq!(product.price, 29.99);
        assert_eq!(product.regular_price, 39.99);
        assert_eq!(product.sale_price, Some(29.99));
        assert_eq!(product.stock_quantity, Some(50));
        assert_eq!(product.stock_status, "instock");
        assert!(product.manage_stock);
        assert_eq!(product.weight, Some(0.5));
        assert_eq!(product.image_gallery, vec![101, 102, 103]);
        assert!(!product.is_virtual);
        assert!(!product.is_downloadable);
    }

    #[test]
    fn test_product_to_meta_roundtrip() {
        let meta = sample_product_meta();
        let product = WooProductData::from_post_and_meta(1, "Test", "test", "", "", &meta);
        let pairs = product.to_meta();
        let restored: HashMap<String, String> = pairs.into_iter().collect();

        assert_eq!(restored.get("_price").unwrap(), "29.99");
        assert_eq!(restored.get("_sku").unwrap(), "RUST-001");
        assert_eq!(restored.get("_stock").unwrap(), "50");
        assert_eq!(restored.get("_stock_status").unwrap(), "instock");
        assert_eq!(restored.get("_manage_stock").unwrap(), "yes");
        assert_eq!(
            restored.get("_product_image_gallery").unwrap(),
            "101,102,103"
        );
    }

    #[test]
    fn test_product_to_internal() {
        let meta = sample_product_meta();
        let woo = WooProductData::from_post_and_meta(
            1,
            "Rust Book",
            "rust-book",
            "Description",
            "Short",
            &meta,
        );
        let product = woo.to_product();

        assert_eq!(product.id, 1);
        assert_eq!(product.name, "Rust Book");
        assert_eq!(product.sku, "RUST-001");
        assert_eq!(product.price, 29.99);
        assert_eq!(product.stock_status, StockStatus::InStock);
        assert_eq!(product.product_type, ProductType::Simple);
        assert!(product.dimensions.is_some());
    }

    #[test]
    fn test_order_from_meta() {
        let meta = sample_order_meta();
        let order = WooOrderData::from_post_and_meta(1, "wc-processing", &meta);

        assert_eq!(order.post_id, 1);
        assert_eq!(order.status, "wc-processing");
        assert_eq!(order.total, 59.98);
        assert_eq!(order.tax_total, 4.80);
        assert_eq!(order.shipping_total, 5.00);
        assert_eq!(order.payment_method, "stripe");
        assert_eq!(order.customer_id, Some(42));
        assert_eq!(order.billing.first_name, "Alice");
        assert_eq!(order.billing.email, "alice@example.com");
        assert_eq!(order.billing.country, "JP");
        assert_eq!(order.shipping.first_name, "Alice");
        assert_eq!(order.shipping.country, "JP");
    }

    #[test]
    fn test_order_to_meta_roundtrip() {
        let meta = sample_order_meta();
        let order = WooOrderData::from_post_and_meta(1, "wc-completed", &meta);
        let pairs = order.to_meta();
        let restored: HashMap<String, String> = pairs.into_iter().collect();

        assert_eq!(restored.get("_order_total").unwrap(), "59.98");
        assert_eq!(restored.get("_payment_method").unwrap(), "stripe");
        assert_eq!(restored.get("_billing_first_name").unwrap(), "Alice");
        assert_eq!(restored.get("_billing_email").unwrap(), "alice@example.com");
        assert_eq!(restored.get("_shipping_city").unwrap(), "Tokyo");
    }

    #[test]
    fn test_order_status_conversion() {
        assert_eq!(
            wc_status_to_order_status("wc-processing"),
            OrderStatus::Processing
        );
        assert_eq!(
            wc_status_to_order_status("wc-completed"),
            OrderStatus::Completed
        );
        assert_eq!(wc_status_to_order_status("wc-on-hold"), OrderStatus::OnHold);
        assert_eq!(
            wc_status_to_order_status("processing"),
            OrderStatus::Processing
        );

        assert_eq!(
            order_status_to_wc_status(&OrderStatus::Processing),
            "wc-processing"
        );
        assert_eq!(
            order_status_to_wc_status(&OrderStatus::Completed),
            "wc-completed"
        );
        assert_eq!(
            order_status_to_wc_status(&OrderStatus::OnHold),
            "wc-on-hold"
        );
    }

    #[test]
    fn test_empty_product_meta() {
        let product =
            WooProductData::from_post_and_meta(1, "Empty", "empty", "", "", &HashMap::new());
        assert_eq!(product.price, 0.0);
        assert_eq!(product.stock_status, "instock");
        assert!(!product.manage_stock);
        assert!(product.image_gallery.is_empty());
    }

    #[test]
    fn test_empty_order_meta() {
        let order = WooOrderData::from_post_and_meta(1, "wc-pending", &HashMap::new());
        assert_eq!(order.total, 0.0);
        assert_eq!(order.currency, "USD");
        assert!(order.customer_id.is_none());
    }

    #[test]
    fn test_post_type_constants() {
        assert_eq!(post_types::PRODUCT, "product");
        assert_eq!(post_types::ORDER, "shop_order");
        assert_eq!(post_types::COUPON, "shop_coupon");
    }

    #[test]
    fn test_product_keys_all() {
        assert!(product_keys::ALL.len() >= 17);
        assert!(product_keys::ALL.contains(&"_price"));
        assert!(product_keys::ALL.contains(&"_sku"));
        assert!(product_keys::ALL.contains(&"_stock_status"));
    }
}

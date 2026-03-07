use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cart::Cart;

/// The type of discount a coupon provides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscountType {
    /// A percentage off the total (e.g. 10 = 10% off).
    Percentage,
    /// A fixed amount off the cart total.
    FixedCart,
    /// A fixed amount off each applicable product.
    FixedProduct,
}

/// A coupon / discount code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coupon {
    pub code: String,
    pub discount_type: DiscountType,
    /// The discount amount: a percentage (e.g. 10.0 for 10%) or a fixed currency amount.
    pub amount: f64,
    /// Maximum number of times this coupon can be used. None means unlimited.
    pub usage_limit: Option<u32>,
    /// How many times this coupon has been used.
    pub usage_count: u32,
    /// Expiry date. None means the coupon never expires.
    pub expiry_date: Option<DateTime<Utc>>,
    /// Minimum cart subtotal required for this coupon to be valid.
    pub minimum_amount: Option<f64>,
    /// Maximum cart subtotal for this coupon to be valid.
    pub maximum_amount: Option<f64>,
    /// If non-empty, coupon only applies to these product ids.
    pub product_ids: Vec<u64>,
    /// Products that are excluded from this coupon.
    pub excluded_product_ids: Vec<u64>,
}

/// The result of applying a coupon to a cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountResult {
    pub success: bool,
    pub discount_amount: f64,
    pub message: String,
}

/// Manages coupons.
pub struct CouponManager {
    coupons: HashMap<String, Coupon>,
}

impl CouponManager {
    pub fn new() -> Self {
        Self {
            coupons: HashMap::new(),
        }
    }

    /// Create (register) a new coupon.
    pub fn create_coupon(&mut self, coupon: Coupon) {
        let code = coupon.code.to_lowercase();
        tracing::info!(coupon_code = %code, discount_type = ?coupon.discount_type, amount = coupon.amount, "Coupon created");
        self.coupons.insert(code, coupon);
    }

    /// Validate whether a coupon code can be applied to the given cart subtotal.
    /// Returns an error message if invalid, or None if valid.
    pub fn validate_coupon(&self, code: &str, cart_subtotal: f64) -> Result<&Coupon, String> {
        let code_lower = code.to_lowercase();
        let coupon = self
            .coupons
            .get(&code_lower)
            .ok_or_else(|| format!("Coupon '{code}' does not exist"))?;

        // Check usage limit
        if let Some(limit) = coupon.usage_limit {
            if coupon.usage_count >= limit {
                return Err(format!("Coupon '{code}' has reached its usage limit"));
            }
        }

        // Check expiry
        if let Some(expiry) = coupon.expiry_date {
            if Utc::now() > expiry {
                return Err(format!("Coupon '{code}' has expired"));
            }
        }

        // Check minimum amount
        if let Some(min) = coupon.minimum_amount {
            if cart_subtotal < min {
                return Err(format!(
                    "Cart subtotal ({cart_subtotal:.2}) is below the minimum ({min:.2}) for coupon '{code}'"
                ));
            }
        }

        // Check maximum amount
        if let Some(max) = coupon.maximum_amount {
            if cart_subtotal > max {
                return Err(format!(
                    "Cart subtotal ({cart_subtotal:.2}) exceeds the maximum ({max:.2}) for coupon '{code}'"
                ));
            }
        }

        Ok(coupon)
    }

    /// Apply a coupon to a cart and return the discount result. This does NOT modify the cart
    /// total directly; the caller should use the returned discount_amount.
    pub fn apply_coupon(&mut self, cart: &Cart, code: &str) -> DiscountResult {
        let subtotal = cart.get_subtotal();

        let coupon = match self.validate_coupon(code, subtotal) {
            Ok(c) => c.clone(),
            Err(msg) => {
                return DiscountResult {
                    success: false,
                    discount_amount: 0.0,
                    message: msg,
                };
            }
        };

        let discount_amount = match coupon.discount_type {
            DiscountType::Percentage => {
                let raw = subtotal * (coupon.amount / 100.0);
                // Cap at cart subtotal
                raw.min(subtotal)
            }
            DiscountType::FixedCart => coupon.amount.min(subtotal),
            DiscountType::FixedProduct => {
                // Apply fixed discount per applicable item
                let applicable_total: f64 = cart
                    .items
                    .iter()
                    .filter(|item| {
                        // If product_ids is set, only include those products
                        let included = coupon.product_ids.is_empty()
                            || coupon.product_ids.contains(&item.product_id);
                        let excluded = coupon.excluded_product_ids.contains(&item.product_id);
                        included && !excluded
                    })
                    .map(|item| {
                        (coupon.amount * item.quantity as f64)
                            .min(item.price * item.quantity as f64)
                    })
                    .sum();
                applicable_total
            }
        };

        // Increment usage count
        let code_lower = code.to_lowercase();
        if let Some(c) = self.coupons.get_mut(&code_lower) {
            c.usage_count += 1;
        }

        tracing::info!(
            coupon_code = %code,
            discount_amount = discount_amount,
            "Coupon applied"
        );

        DiscountResult {
            success: true,
            discount_amount,
            message: format!("Coupon '{code}' applied: -{discount_amount:.2}"),
        }
    }
}

impl Default for CouponManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::CartItem;

    fn make_coupon(code: &str, discount_type: DiscountType, amount: f64) -> Coupon {
        Coupon {
            code: code.to_string(),
            discount_type,
            amount,
            usage_limit: None,
            usage_count: 0,
            expiry_date: None,
            minimum_amount: None,
            maximum_amount: None,
            product_ids: Vec::new(),
            excluded_product_ids: Vec::new(),
        }
    }

    fn make_cart_with_items() -> Cart {
        let mut cart = Cart::new();
        cart.add_item(CartItem {
            product_id: 1,
            variation_id: None,
            quantity: 2,
            price: 50.0,
            name: "Widget".to_string(),
        });
        cart.add_item(CartItem {
            product_id: 2,
            variation_id: None,
            quantity: 1,
            price: 30.0,
            name: "Gadget".to_string(),
        });
        // Subtotal: 2*50 + 1*30 = 130
        cart
    }

    #[test]
    fn test_percentage_coupon() {
        let mut manager = CouponManager::new();
        manager.create_coupon(make_coupon("SAVE10", DiscountType::Percentage, 10.0));

        let cart = make_cart_with_items();
        let result = manager.apply_coupon(&cart, "save10");

        assert!(result.success);
        assert!((result.discount_amount - 13.0).abs() < f64::EPSILON); // 10% of 130
    }

    #[test]
    fn test_fixed_cart_coupon() {
        let mut manager = CouponManager::new();
        manager.create_coupon(make_coupon("FLAT20", DiscountType::FixedCart, 20.0));

        let cart = make_cart_with_items();
        let result = manager.apply_coupon(&cart, "flat20");

        assert!(result.success);
        assert!((result.discount_amount - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fixed_product_coupon() {
        let mut manager = CouponManager::new();
        let mut coupon = make_coupon("PROD5", DiscountType::FixedProduct, 5.0);
        coupon.product_ids = vec![1]; // only applies to product 1
        manager.create_coupon(coupon);

        let cart = make_cart_with_items();
        let result = manager.apply_coupon(&cart, "prod5");

        assert!(result.success);
        // $5 off per unit of product 1, quantity 2 = $10
        assert!((result.discount_amount - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_invalid_coupon_code() {
        let mut manager = CouponManager::new();
        let cart = make_cart_with_items();
        let result = manager.apply_coupon(&cart, "NONEXISTENT");

        assert!(!result.success);
        assert!((result.discount_amount - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_coupon_minimum_amount() {
        let mut manager = CouponManager::new();
        let mut coupon = make_coupon("BIGORDER", DiscountType::Percentage, 15.0);
        coupon.minimum_amount = Some(200.0);
        manager.create_coupon(coupon);

        let cart = make_cart_with_items(); // subtotal = 130
        let result = manager.apply_coupon(&cart, "bigorder");

        assert!(!result.success);
        assert!(result.message.contains("below the minimum"));
    }

    #[test]
    fn test_coupon_usage_limit() {
        let mut manager = CouponManager::new();
        let mut coupon = make_coupon("ONCE", DiscountType::FixedCart, 10.0);
        coupon.usage_limit = Some(1);
        manager.create_coupon(coupon);

        let cart = make_cart_with_items();

        let result = manager.apply_coupon(&cart, "once");
        assert!(result.success);

        let result = manager.apply_coupon(&cart, "once");
        assert!(!result.success);
        assert!(result.message.contains("usage limit"));
    }

    #[test]
    fn test_expired_coupon() {
        let mut manager = CouponManager::new();
        let mut coupon = make_coupon("EXPIRED", DiscountType::FixedCart, 10.0);
        coupon.expiry_date = Some(Utc::now() - chrono::Duration::days(1));
        manager.create_coupon(coupon);

        let cart = make_cart_with_items();
        let result = manager.apply_coupon(&cart, "expired");

        assert!(!result.success);
        assert!(result.message.contains("expired"));
    }
}

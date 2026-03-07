use serde::{Deserialize, Serialize};

use crate::cart::Cart;
use crate::order::Address;

/// A calculated shipping rate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShippingRate {
    pub method_id: String,
    pub label: String,
    pub cost: f64,
}

/// Trait that all shipping methods must implement.
pub trait ShippingMethod: Send + Sync {
    /// Unique identifier for this shipping method.
    fn id(&self) -> &str;

    /// Human-readable title.
    fn title(&self) -> &str;

    /// Calculate the shipping cost for the given cart and destination.
    fn calculate_cost(&self, cart: &Cart, destination: &Address) -> Option<ShippingRate>;
}

/// Flat-rate shipping: charges a fixed cost regardless of cart contents.
pub struct FlatRateShipping {
    pub cost: f64,
    pub title: String,
}

impl FlatRateShipping {
    pub fn new(cost: f64) -> Self {
        Self {
            cost,
            title: "Flat Rate".to_string(),
        }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }
}

impl ShippingMethod for FlatRateShipping {
    fn id(&self) -> &str {
        "flat_rate"
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn calculate_cost(&self, _cart: &Cart, _destination: &Address) -> Option<ShippingRate> {
        Some(ShippingRate {
            method_id: self.id().to_string(),
            label: self.title.clone(),
            cost: self.cost,
        })
    }
}

/// Free shipping, available when the cart subtotal meets a minimum threshold.
pub struct FreeShipping {
    /// Minimum order subtotal required for free shipping. None means always free.
    pub minimum_order_amount: Option<f64>,
}

impl FreeShipping {
    /// Create free shipping with no minimum.
    pub fn new() -> Self {
        Self {
            minimum_order_amount: None,
        }
    }

    /// Create free shipping with a minimum order threshold.
    pub fn with_minimum(minimum: f64) -> Self {
        Self {
            minimum_order_amount: Some(minimum),
        }
    }
}

impl Default for FreeShipping {
    fn default() -> Self {
        Self::new()
    }
}

impl ShippingMethod for FreeShipping {
    fn id(&self) -> &str {
        "free_shipping"
    }

    fn title(&self) -> &str {
        "Free Shipping"
    }

    fn calculate_cost(&self, cart: &Cart, _destination: &Address) -> Option<ShippingRate> {
        if let Some(min) = self.minimum_order_amount {
            if cart.get_subtotal() < min {
                return None;
            }
        }

        Some(ShippingRate {
            method_id: self.id().to_string(),
            label: "Free Shipping".to_string(),
            cost: 0.0,
        })
    }
}

/// A shipping zone defines a geographic region and its available shipping methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShippingZone {
    pub name: String,
    /// Country/region codes that belong to this zone (e.g. "US", "US:CA", "EU").
    pub regions: Vec<String>,
    /// Ids of shipping methods available in this zone.
    pub methods: Vec<String>,
}

impl ShippingZone {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            regions: Vec::new(),
            methods: Vec::new(),
        }
    }

    /// Add a region code to this zone.
    pub fn add_region(&mut self, region: &str) {
        self.regions.push(region.to_string());
    }

    /// Add a shipping method id to this zone.
    pub fn add_method(&mut self, method_id: &str) {
        self.methods.push(method_id.to_string());
    }

    /// Check if a country code belongs to this zone.
    pub fn matches_country(&self, country: &str) -> bool {
        self.regions
            .iter()
            .any(|r| r == country || r.starts_with(&format!("{}:", country)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::CartItem;

    fn make_cart(subtotal_price: f64) -> Cart {
        let mut cart = Cart::new();
        cart.add_item(CartItem {
            product_id: 1,
            variation_id: None,
            quantity: 1,
            price: subtotal_price,
            name: "Test Item".to_string(),
        });
        cart
    }

    fn make_address(country: &str) -> Address {
        Address {
            country: country.to_string(),
            ..Address::default()
        }
    }

    #[test]
    fn test_flat_rate_shipping() {
        let method = FlatRateShipping::new(9.99).with_title("Standard Shipping");
        let cart = make_cart(50.0);
        let address = make_address("US");

        let rate = method.calculate_cost(&cart, &address).unwrap();
        assert_eq!(rate.method_id, "flat_rate");
        assert_eq!(rate.label, "Standard Shipping");
        assert!((rate.cost - 9.99).abs() < f64::EPSILON);
    }

    #[test]
    fn test_free_shipping_no_minimum() {
        let method = FreeShipping::new();
        let cart = make_cart(1.0);
        let address = make_address("US");

        let rate = method.calculate_cost(&cart, &address).unwrap();
        assert!((rate.cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_free_shipping_with_minimum_met() {
        let method = FreeShipping::with_minimum(50.0);
        let cart = make_cart(75.0);
        let address = make_address("US");

        let rate = method.calculate_cost(&cart, &address);
        assert!(rate.is_some());
        assert!((rate.unwrap().cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_free_shipping_with_minimum_not_met() {
        let method = FreeShipping::with_minimum(50.0);
        let cart = make_cart(30.0);
        let address = make_address("US");

        let rate = method.calculate_cost(&cart, &address);
        assert!(rate.is_none());
    }

    #[test]
    fn test_shipping_zone() {
        let mut zone = ShippingZone::new("North America");
        zone.add_region("US");
        zone.add_region("CA");
        zone.add_method("flat_rate");
        zone.add_method("free_shipping");

        assert!(zone.matches_country("US"));
        assert!(zone.matches_country("CA"));
        assert!(!zone.matches_country("GB"));
        assert_eq!(zone.methods.len(), 2);
    }

    #[test]
    fn test_shipping_zone_with_state() {
        let mut zone = ShippingZone::new("California");
        zone.add_region("US:CA");

        // US:CA matches country "US" because it starts with "US:"
        assert!(zone.matches_country("US"));
        assert!(!zone.matches_country("CA")); // Canada != US:CA
    }
}

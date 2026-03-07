use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A single item in the shopping cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItem {
    pub product_id: u64,
    pub variation_id: Option<u64>,
    pub quantity: u32,
    pub price: f64,
    pub name: String,
}

/// A shopping cart containing items, coupons, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cart {
    pub id: String,
    pub items: Vec<CartItem>,
    pub applied_coupons: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Cart {
    /// Create a new empty cart with a generated UUID.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            items: Vec::new(),
            applied_coupons: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add an item to the cart. If an item with the same product_id and variation_id
    /// already exists, its quantity is increased instead.
    pub fn add_item(&mut self, item: CartItem) {
        if let Some(existing) = self.items.iter_mut().find(|i| {
            i.product_id == item.product_id && i.variation_id == item.variation_id
        }) {
            existing.quantity += item.quantity;
            // Update price to the latest value
            existing.price = item.price;
        } else {
            self.items.push(item);
        }
        self.updated_at = Utc::now();
    }

    /// Remove an item by product_id (and optional variation_id). Returns true if removed.
    pub fn remove_item(&mut self, product_id: u64, variation_id: Option<u64>) -> bool {
        let before = self.items.len();
        self.items.retain(|i| {
            !(i.product_id == product_id && i.variation_id == variation_id)
        });
        let removed = self.items.len() < before;
        if removed {
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Update the quantity of a specific item. If quantity is 0, the item is removed.
    /// Returns true if the item was found and updated.
    pub fn update_quantity(
        &mut self,
        product_id: u64,
        variation_id: Option<u64>,
        quantity: u32,
    ) -> bool {
        if quantity == 0 {
            return self.remove_item(product_id, variation_id);
        }
        if let Some(item) = self.items.iter_mut().find(|i| {
            i.product_id == product_id && i.variation_id == variation_id
        }) {
            item.quantity = quantity;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Calculate the subtotal (sum of price * quantity for all items).
    pub fn get_subtotal(&self) -> f64 {
        self.items
            .iter()
            .map(|item| item.price * item.quantity as f64)
            .sum()
    }

    /// Calculate the total. Currently equals the subtotal; discounts are applied
    /// externally via the coupon system.
    pub fn get_total(&self) -> f64 {
        self.get_subtotal()
    }

    /// Get the total number of items (sum of all quantities).
    pub fn get_item_count(&self) -> u32 {
        self.items.iter().map(|item| item.quantity).sum()
    }

    /// Remove all items from the cart.
    pub fn clear(&mut self) {
        self.items.clear();
        self.applied_coupons.clear();
        self.updated_at = Utc::now();
    }

    /// Apply a coupon code to the cart. Returns false if the coupon is already applied.
    pub fn apply_coupon(&mut self, code: &str) -> bool {
        let code_lower = code.to_lowercase();
        if self.applied_coupons.contains(&code_lower) {
            return false;
        }
        self.applied_coupons.push(code_lower);
        self.updated_at = Utc::now();
        true
    }

    /// Remove a coupon code from the cart. Returns true if the coupon was present.
    pub fn remove_coupon(&mut self, code: &str) -> bool {
        let code_lower = code.to_lowercase();
        let before = self.applied_coupons.len();
        self.applied_coupons.retain(|c| c != &code_lower);
        let removed = self.applied_coupons.len() < before;
        if removed {
            self.updated_at = Utc::now();
        }
        removed
    }
}

impl Default for Cart {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages carts keyed by session id.
pub struct CartManager {
    carts: HashMap<String, Cart>,
}

impl CartManager {
    pub fn new() -> Self {
        Self {
            carts: HashMap::new(),
        }
    }

    /// Get the cart for a session, if it exists.
    pub fn get_cart(&self, session_id: &str) -> Option<&Cart> {
        self.carts.get(session_id)
    }

    /// Get a mutable reference to the cart for a session, if it exists.
    pub fn get_cart_mut(&mut self, session_id: &str) -> Option<&mut Cart> {
        self.carts.get_mut(session_id)
    }

    /// Create a new cart for a session. If a cart already exists for this session,
    /// it is replaced with a new empty cart. Returns a reference to the new cart.
    pub fn create_cart(&mut self, session_id: &str) -> &Cart {
        let cart = Cart::new();
        tracing::info!(session_id = session_id, cart_id = %cart.id, "Cart created");
        self.carts.insert(session_id.to_string(), cart);
        self.carts.get(session_id).unwrap()
    }

    /// Get an existing cart or create a new one for the session.
    pub fn get_or_create_cart(&mut self, session_id: &str) -> &mut Cart {
        if !self.carts.contains_key(session_id) {
            let cart = Cart::new();
            tracing::info!(session_id = session_id, cart_id = %cart.id, "Cart created");
            self.carts.insert(session_id.to_string(), cart);
        }
        self.carts.get_mut(session_id).unwrap()
    }
}

impl Default for CartManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(product_id: u64, name: &str, price: f64, qty: u32) -> CartItem {
        CartItem {
            product_id,
            variation_id: None,
            quantity: qty,
            price,
            name: name.to_string(),
        }
    }

    #[test]
    fn test_add_items_and_totals() {
        let mut cart = Cart::new();
        cart.add_item(sample_item(1, "Widget", 10.0, 2));
        cart.add_item(sample_item(2, "Gadget", 25.0, 1));

        assert_eq!(cart.get_item_count(), 3);
        assert!((cart.get_subtotal() - 45.0).abs() < f64::EPSILON);
        assert!((cart.get_total() - 45.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_add_duplicate_increases_quantity() {
        let mut cart = Cart::new();
        cart.add_item(sample_item(1, "Widget", 10.0, 2));
        cart.add_item(sample_item(1, "Widget", 10.0, 3));

        assert_eq!(cart.items.len(), 1);
        assert_eq!(cart.items[0].quantity, 5);
    }

    #[test]
    fn test_remove_item() {
        let mut cart = Cart::new();
        cart.add_item(sample_item(1, "Widget", 10.0, 1));
        cart.add_item(sample_item(2, "Gadget", 20.0, 1));

        assert!(cart.remove_item(1, None));
        assert_eq!(cart.items.len(), 1);
        assert_eq!(cart.items[0].product_id, 2);
        assert!(!cart.remove_item(1, None)); // already removed
    }

    #[test]
    fn test_update_quantity() {
        let mut cart = Cart::new();
        cart.add_item(sample_item(1, "Widget", 10.0, 1));

        assert!(cart.update_quantity(1, None, 5));
        assert_eq!(cart.items[0].quantity, 5);

        // Setting to 0 removes the item
        assert!(cart.update_quantity(1, None, 0));
        assert!(cart.items.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut cart = Cart::new();
        cart.add_item(sample_item(1, "Widget", 10.0, 1));
        cart.apply_coupon("SAVE10");
        cart.clear();

        assert!(cart.items.is_empty());
        assert!(cart.applied_coupons.is_empty());
    }

    #[test]
    fn test_coupon_management() {
        let mut cart = Cart::new();

        assert!(cart.apply_coupon("SAVE10"));
        assert!(!cart.apply_coupon("save10")); // duplicate (case-insensitive)
        assert_eq!(cart.applied_coupons.len(), 1);

        assert!(cart.remove_coupon("SAVE10"));
        assert!(cart.applied_coupons.is_empty());
        assert!(!cart.remove_coupon("SAVE10")); // already removed
    }

    #[test]
    fn test_cart_manager() {
        let mut manager = CartManager::new();

        assert!(manager.get_cart("session1").is_none());

        manager.create_cart("session1");
        assert!(manager.get_cart("session1").is_some());

        let cart = manager.get_or_create_cart("session2");
        cart.add_item(sample_item(1, "Widget", 10.0, 1));

        let cart = manager.get_cart("session2").unwrap();
        assert_eq!(cart.get_item_count(), 1);
    }
}

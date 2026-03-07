//! Inventory management for tracking stock levels, reservations, and low-stock alerts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// Stock management mode for a product.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StockManagement {
    /// Stock is tracked and enforced.
    Managed,
    /// Stock is not tracked; product is always available.
    Unmanaged,
}

/// A stock adjustment event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockAdjustment {
    pub product_id: u64,
    pub variation_id: Option<u64>,
    pub quantity_change: i64,
    pub reason: StockAdjustmentReason,
    pub note: String,
    pub timestamp: DateTime<Utc>,
}

/// Reason for a stock adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StockAdjustmentReason {
    Sale,
    Refund,
    Restock,
    ManualAdjustment,
    Reservation,
    ReservationRelease,
}

/// A temporary stock reservation (e.g., during checkout).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockReservation {
    pub reservation_id: String,
    pub product_id: u64,
    pub variation_id: Option<u64>,
    pub quantity: u32,
    pub expires_at: DateTime<Utc>,
}

/// Stock entry for a product or variation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StockEntry {
    quantity: i64,
    reserved: u32,
    low_stock_threshold: Option<u32>,
    management: StockManagement,
    allow_backorder: bool,
}

impl StockEntry {
    fn available(&self) -> i64 {
        self.quantity - self.reserved as i64
    }
}

/// Inventory manager that tracks stock levels and reservations.
pub struct InventoryManager {
    /// Key: "product_id" or "product_id:variation_id"
    stock: HashMap<String, StockEntry>,
    reservations: Vec<StockReservation>,
    adjustment_log: Vec<StockAdjustment>,
    /// Default reservation timeout in seconds.
    reservation_timeout_secs: u64,
}

impl InventoryManager {
    pub fn new() -> Self {
        Self {
            stock: HashMap::new(),
            reservations: Vec::new(),
            adjustment_log: Vec::new(),
            reservation_timeout_secs: 900, // 15 minutes
        }
    }

    /// Set the default reservation timeout.
    pub fn set_reservation_timeout(&mut self, secs: u64) {
        self.reservation_timeout_secs = secs;
    }

    /// Register a product for stock tracking.
    pub fn register_product(
        &mut self,
        product_id: u64,
        variation_id: Option<u64>,
        quantity: i64,
        low_stock_threshold: Option<u32>,
        allow_backorder: bool,
    ) {
        let key = Self::make_key(product_id, variation_id);
        self.stock.insert(
            key,
            StockEntry {
                quantity,
                reserved: 0,
                low_stock_threshold,
                management: StockManagement::Managed,
                allow_backorder,
            },
        );
    }

    /// Get current stock quantity for a product.
    pub fn get_stock(&self, product_id: u64, variation_id: Option<u64>) -> Option<i64> {
        self.stock
            .get(&Self::make_key(product_id, variation_id))
            .map(|e| e.quantity)
    }

    /// Get available stock (quantity minus reserved).
    pub fn get_available(&self, product_id: u64, variation_id: Option<u64>) -> Option<i64> {
        self.stock
            .get(&Self::make_key(product_id, variation_id))
            .map(|e| e.available())
    }

    /// Check if a product can be purchased in the given quantity.
    pub fn can_purchase(&self, product_id: u64, variation_id: Option<u64>, quantity: u32) -> bool {
        let key = Self::make_key(product_id, variation_id);
        match self.stock.get(&key) {
            Some(entry) => {
                if entry.management == StockManagement::Unmanaged {
                    return true;
                }
                if entry.allow_backorder {
                    return true;
                }
                entry.available() >= quantity as i64
            }
            None => true, // Untracked product is always purchasable
        }
    }

    /// Reduce stock when a sale is completed. Returns the new stock quantity.
    pub fn reduce_stock(
        &mut self,
        product_id: u64,
        variation_id: Option<u64>,
        quantity: u32,
        note: &str,
    ) -> Result<i64, String> {
        let key = Self::make_key(product_id, variation_id);
        let entry = self
            .stock
            .get_mut(&key)
            .ok_or_else(|| format!("Product {} not found in inventory", product_id))?;

        if entry.management == StockManagement::Unmanaged {
            return Ok(entry.quantity);
        }

        if !entry.allow_backorder && entry.available() < quantity as i64 {
            return Err(format!(
                "Insufficient stock for product {}: available {}, requested {}",
                product_id,
                entry.available(),
                quantity
            ));
        }

        entry.quantity -= quantity as i64;

        self.adjustment_log.push(StockAdjustment {
            product_id,
            variation_id,
            quantity_change: -(quantity as i64),
            reason: StockAdjustmentReason::Sale,
            note: note.to_string(),
            timestamp: Utc::now(),
        });

        info!(
            product_id = product_id,
            quantity = quantity,
            new_stock = entry.quantity,
            "Stock reduced"
        );

        if let Some(threshold) = entry.low_stock_threshold {
            if entry.quantity <= threshold as i64 && entry.quantity > 0 {
                warn!(
                    product_id = product_id,
                    stock = entry.quantity,
                    threshold = threshold,
                    "Low stock alert"
                );
            }
        }

        Ok(entry.quantity)
    }

    /// Increase stock (e.g., restock or refund).
    pub fn increase_stock(
        &mut self,
        product_id: u64,
        variation_id: Option<u64>,
        quantity: u32,
        reason: StockAdjustmentReason,
        note: &str,
    ) -> Result<i64, String> {
        let key = Self::make_key(product_id, variation_id);
        let entry = self
            .stock
            .get_mut(&key)
            .ok_or_else(|| format!("Product {} not found in inventory", product_id))?;

        entry.quantity += quantity as i64;

        self.adjustment_log.push(StockAdjustment {
            product_id,
            variation_id,
            quantity_change: quantity as i64,
            reason,
            note: note.to_string(),
            timestamp: Utc::now(),
        });

        info!(
            product_id = product_id,
            quantity = quantity,
            new_stock = entry.quantity,
            "Stock increased"
        );

        Ok(entry.quantity)
    }

    /// Reserve stock temporarily during checkout.
    pub fn reserve_stock(
        &mut self,
        reservation_id: &str,
        product_id: u64,
        variation_id: Option<u64>,
        quantity: u32,
    ) -> Result<(), String> {
        let key = Self::make_key(product_id, variation_id);
        let entry = self
            .stock
            .get_mut(&key)
            .ok_or_else(|| format!("Product {} not found in inventory", product_id))?;

        if !entry.allow_backorder && entry.available() < quantity as i64 {
            return Err(format!(
                "Insufficient stock to reserve for product {}",
                product_id
            ));
        }

        entry.reserved += quantity;

        let expires_at =
            Utc::now() + chrono::Duration::seconds(self.reservation_timeout_secs as i64);

        self.reservations.push(StockReservation {
            reservation_id: reservation_id.to_string(),
            product_id,
            variation_id,
            quantity,
            expires_at,
        });

        info!(
            reservation_id = reservation_id,
            product_id = product_id,
            quantity = quantity,
            "Stock reserved"
        );

        Ok(())
    }

    /// Release a stock reservation (e.g., checkout abandoned or completed).
    pub fn release_reservation(&mut self, reservation_id: &str) {
        let to_release: Vec<StockReservation> = self
            .reservations
            .iter()
            .filter(|r| r.reservation_id == reservation_id)
            .cloned()
            .collect();

        for reservation in &to_release {
            let key = Self::make_key(reservation.product_id, reservation.variation_id);
            if let Some(entry) = self.stock.get_mut(&key) {
                entry.reserved = entry.reserved.saturating_sub(reservation.quantity);
            }
        }

        self.reservations
            .retain(|r| r.reservation_id != reservation_id);
    }

    /// Clean up expired reservations.
    pub fn cleanup_expired_reservations(&mut self) {
        let now = Utc::now();
        let expired: Vec<StockReservation> = self
            .reservations
            .iter()
            .filter(|r| r.expires_at < now)
            .cloned()
            .collect();

        for reservation in &expired {
            let key = Self::make_key(reservation.product_id, reservation.variation_id);
            if let Some(entry) = self.stock.get_mut(&key) {
                entry.reserved = entry.reserved.saturating_sub(reservation.quantity);
            }
            info!(
                reservation_id = %reservation.reservation_id,
                product_id = reservation.product_id,
                "Expired reservation released"
            );
        }

        self.reservations.retain(|r| r.expires_at >= now);
    }

    /// Get products with low stock levels.
    pub fn get_low_stock_products(&self) -> Vec<(u64, i64, u32)> {
        self.stock
            .iter()
            .filter_map(|(key, entry)| {
                if let Some(threshold) = entry.low_stock_threshold {
                    if entry.quantity <= threshold as i64 && entry.quantity > 0 {
                        let product_id: u64 = key
                            .split(':')
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        return Some((product_id, entry.quantity, threshold));
                    }
                }
                None
            })
            .collect()
    }

    /// Get products that are out of stock.
    pub fn get_out_of_stock_products(&self) -> Vec<u64> {
        self.stock
            .iter()
            .filter_map(|(key, entry)| {
                if entry.management == StockManagement::Managed
                    && entry.quantity <= 0
                    && !entry.allow_backorder
                {
                    key.split(':').next().and_then(|s| s.parse().ok())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get the stock adjustment history for a product.
    pub fn get_adjustment_log(
        &self,
        product_id: u64,
        variation_id: Option<u64>,
    ) -> Vec<&StockAdjustment> {
        self.adjustment_log
            .iter()
            .filter(|a| a.product_id == product_id && a.variation_id == variation_id)
            .collect()
    }

    fn make_key(product_id: u64, variation_id: Option<u64>) -> String {
        match variation_id {
            Some(vid) => format!("{}:{}", product_id, vid),
            None => product_id.to_string(),
        }
    }
}

impl Default for InventoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get_stock() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 50, Some(5), false);

        assert_eq!(inv.get_stock(1, None), Some(50));
        assert_eq!(inv.get_available(1, None), Some(50));
        assert_eq!(inv.get_stock(999, None), None);
    }

    #[test]
    fn test_reduce_stock() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 10, None, false);

        let result = inv.reduce_stock(1, None, 3, "Order #1");
        assert_eq!(result, Ok(7));
        assert_eq!(inv.get_stock(1, None), Some(7));
    }

    #[test]
    fn test_reduce_stock_insufficient() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 2, None, false);

        let result = inv.reduce_stock(1, None, 5, "Order #1");
        assert!(result.is_err());
        assert_eq!(inv.get_stock(1, None), Some(2)); // unchanged
    }

    #[test]
    fn test_backorder_allows_oversell() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 2, None, true);

        let result = inv.reduce_stock(1, None, 5, "Backorder");
        assert_eq!(result, Ok(-3));
        assert!(inv.can_purchase(1, None, 100));
    }

    #[test]
    fn test_can_purchase() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 5, None, false);

        assert!(inv.can_purchase(1, None, 5));
        assert!(!inv.can_purchase(1, None, 6));
        assert!(inv.can_purchase(999, None, 1)); // untracked = always OK
    }

    #[test]
    fn test_increase_stock() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 5, None, false);

        let result = inv.increase_stock(1, None, 10, StockAdjustmentReason::Restock, "Restock");
        assert_eq!(result, Ok(15));
    }

    #[test]
    fn test_stock_reservation() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 10, None, false);

        inv.reserve_stock("checkout-1", 1, None, 3).unwrap();
        assert_eq!(inv.get_available(1, None), Some(7)); // 10 - 3 reserved
        assert_eq!(inv.get_stock(1, None), Some(10)); // actual stock unchanged

        assert!(!inv.can_purchase(1, None, 8)); // only 7 available

        inv.release_reservation("checkout-1");
        assert_eq!(inv.get_available(1, None), Some(10)); // reservation freed
    }

    #[test]
    fn test_reservation_insufficient_stock() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 3, None, false);

        assert!(inv.reserve_stock("r1", 1, None, 2).is_ok());
        assert!(inv.reserve_stock("r2", 1, None, 2).is_err()); // only 1 available
    }

    #[test]
    fn test_low_stock_detection() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 3, Some(5), false);
        inv.register_product(2, None, 100, Some(10), false);

        let low = inv.get_low_stock_products();
        assert_eq!(low.len(), 1);
        assert_eq!(low[0].0, 1);
    }

    #[test]
    fn test_out_of_stock_detection() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 0, None, false);
        inv.register_product(2, None, 0, None, true); // backorder OK
        inv.register_product(3, None, 5, None, false);

        let oos = inv.get_out_of_stock_products();
        assert_eq!(oos.len(), 1);
        assert!(oos.contains(&1));
    }

    #[test]
    fn test_adjustment_log() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, None, 10, None, false);
        inv.reduce_stock(1, None, 2, "Sale").unwrap();
        inv.increase_stock(1, None, 5, StockAdjustmentReason::Restock, "Restock")
            .unwrap();

        let log = inv.get_adjustment_log(1, None);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].quantity_change, -2);
        assert_eq!(log[1].quantity_change, 5);
    }

    #[test]
    fn test_variation_stock() {
        let mut inv = InventoryManager::new();
        inv.register_product(1, Some(10), 20, None, false);
        inv.register_product(1, Some(11), 5, None, false);

        assert_eq!(inv.get_stock(1, Some(10)), Some(20));
        assert_eq!(inv.get_stock(1, Some(11)), Some(5));

        inv.reduce_stock(1, Some(10), 3, "Sale").unwrap();
        assert_eq!(inv.get_stock(1, Some(10)), Some(17));
        assert_eq!(inv.get_stock(1, Some(11)), Some(5)); // unchanged
    }
}

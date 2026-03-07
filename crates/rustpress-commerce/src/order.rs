use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// WooCommerce-compatible order statuses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Processing,
    OnHold,
    Completed,
    Cancelled,
    Refunded,
    Failed,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "pending"),
            OrderStatus::Processing => write!(f, "processing"),
            OrderStatus::OnHold => write!(f, "on-hold"),
            OrderStatus::Completed => write!(f, "completed"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
            OrderStatus::Refunded => write!(f, "refunded"),
            OrderStatus::Failed => write!(f, "failed"),
        }
    }
}

/// A postal / billing / shipping address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Address {
    pub first_name: String,
    pub last_name: String,
    pub company: String,
    pub address_1: String,
    pub address_2: String,
    pub city: String,
    pub state: String,
    pub postcode: String,
    pub country: String,
    pub email: String,
    pub phone: String,
}

/// A line item in an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: u64,
    pub name: String,
    pub quantity: u32,
    pub price: f64,
    pub total: f64,
}

/// An order, representing a completed or in-progress purchase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub order_number: String,
    pub status: OrderStatus,
    pub items: Vec<OrderItem>,
    pub billing_address: Address,
    pub shipping_address: Address,
    pub payment_method: String,
    pub subtotal: f64,
    pub shipping_total: f64,
    pub tax_total: f64,
    pub discount_total: f64,
    pub total: f64,
    pub customer_id: Option<u64>,
    pub customer_note: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// In-memory order manager.
pub struct OrderManager {
    orders: HashMap<u64, Order>,
    next_id: u64,
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new order from a list of items and addresses. Returns the order id.
    pub fn create_order(
        &mut self,
        items: Vec<OrderItem>,
        billing_address: Address,
        shipping_address: Address,
        payment_method: &str,
        shipping_total: f64,
        tax_total: f64,
        discount_total: f64,
        customer_id: Option<u64>,
        customer_note: &str,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let subtotal: f64 = items.iter().map(|i| i.total).sum();
        let total = subtotal + shipping_total + tax_total - discount_total;
        let now = Utc::now();

        let order = Order {
            id,
            order_number: format!("RP-{}", Uuid::new_v4().as_simple().to_string()[..8].to_uppercase()),
            status: OrderStatus::Pending,
            items,
            billing_address,
            shipping_address,
            payment_method: payment_method.to_string(),
            subtotal,
            shipping_total,
            tax_total,
            discount_total,
            total,
            customer_id,
            customer_note: customer_note.to_string(),
            created_at: now,
            updated_at: now,
        };

        tracing::info!(order_id = id, order_number = %order.order_number, total = total, "Order created");
        self.orders.insert(id, order);
        id
    }

    /// Get an order by id.
    pub fn get_order(&self, id: u64) -> Option<&Order> {
        self.orders.get(&id)
    }

    /// Update the status of an order. Returns true if the order was found.
    pub fn update_status(&mut self, id: u64, status: OrderStatus) -> bool {
        if let Some(order) = self.orders.get_mut(&id) {
            let old_status = order.status.clone();
            order.status = status.clone();
            order.updated_at = Utc::now();
            tracing::info!(order_id = id, %old_status, new_status = %status, "Order status updated");
            true
        } else {
            false
        }
    }

    /// List all orders, optionally filtered by status.
    pub fn list_orders(&self, status_filter: Option<&OrderStatus>) -> Vec<&Order> {
        let mut orders: Vec<&Order> = self
            .orders
            .values()
            .filter(|o| match status_filter {
                Some(status) => o.status == *status,
                None => true,
            })
            .collect();
        orders.sort_by_key(|o| o.id);
        orders
    }
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<OrderItem> {
        vec![
            OrderItem {
                product_id: 1,
                name: "Widget".to_string(),
                quantity: 2,
                price: 10.0,
                total: 20.0,
            },
            OrderItem {
                product_id: 2,
                name: "Gadget".to_string(),
                quantity: 1,
                price: 25.0,
                total: 25.0,
            },
        ]
    }

    #[test]
    fn test_create_and_get_order() {
        let mut manager = OrderManager::new();
        let id = manager.create_order(
            sample_items(),
            Address::default(),
            Address::default(),
            "mock",
            5.0,
            3.0,
            0.0,
            Some(42),
            "Please gift wrap",
        );

        let order = manager.get_order(id).unwrap();
        assert_eq!(order.status, OrderStatus::Pending);
        assert!((order.subtotal - 45.0).abs() < f64::EPSILON);
        assert!((order.total - 53.0).abs() < f64::EPSILON); // 45 + 5 + 3 - 0
        assert_eq!(order.customer_id, Some(42));
        assert_eq!(order.customer_note, "Please gift wrap");
        assert!(order.order_number.starts_with("RP-"));
    }

    #[test]
    fn test_update_status() {
        let mut manager = OrderManager::new();
        let id = manager.create_order(
            sample_items(),
            Address::default(),
            Address::default(),
            "mock",
            0.0,
            0.0,
            0.0,
            None,
            "",
        );

        assert!(manager.update_status(id, OrderStatus::Processing));
        assert_eq!(manager.get_order(id).unwrap().status, OrderStatus::Processing);

        assert!(manager.update_status(id, OrderStatus::Completed));
        assert_eq!(manager.get_order(id).unwrap().status, OrderStatus::Completed);

        assert!(!manager.update_status(9999, OrderStatus::Failed));
    }

    #[test]
    fn test_list_orders_with_filter() {
        let mut manager = OrderManager::new();
        let id1 = manager.create_order(
            sample_items(), Address::default(), Address::default(), "mock", 0.0, 0.0, 0.0, None, "",
        );
        let _id2 = manager.create_order(
            sample_items(), Address::default(), Address::default(), "mock", 0.0, 0.0, 0.0, None, "",
        );

        manager.update_status(id1, OrderStatus::Completed);

        let all = manager.list_orders(None);
        assert_eq!(all.len(), 2);

        let completed = manager.list_orders(Some(&OrderStatus::Completed));
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, id1);

        let pending = manager.list_orders(Some(&OrderStatus::Pending));
        assert_eq!(pending.len(), 1);
    }
}

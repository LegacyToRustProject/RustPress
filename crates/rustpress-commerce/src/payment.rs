use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::order::Order;

/// Result of processing a payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    pub success: bool,
    pub transaction_id: Option<String>,
    pub redirect_url: Option<String>,
    pub message: String,
}

/// Trait that all payment gateways must implement.
pub trait PaymentGateway: Send + Sync {
    /// Unique identifier for this gateway (e.g. "stripe", "paypal").
    fn id(&self) -> &str;

    /// Human-readable title (e.g. "Credit Card (Stripe)").
    fn title(&self) -> &str;

    /// Description shown to the customer.
    fn description(&self) -> &str;

    /// Process a payment for the given order.
    fn process_payment(&self, order: &Order) -> PaymentResult;

    /// Whether this gateway supports refunds.
    fn supports_refund(&self) -> bool;
}

/// A mock payment gateway for testing purposes. Always succeeds.
pub struct MockGateway;

impl MockGateway {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl PaymentGateway for MockGateway {
    fn id(&self) -> &str {
        "mock"
    }

    fn title(&self) -> &str {
        "Mock Payment Gateway"
    }

    fn description(&self) -> &str {
        "A test payment gateway that always succeeds. Do not use in production."
    }

    fn process_payment(&self, order: &Order) -> PaymentResult {
        tracing::info!(
            order_id = order.id,
            total = order.total,
            "Mock gateway processing payment"
        );
        PaymentResult {
            success: true,
            transaction_id: Some(format!("MOCK-TXN-{}", order.id)),
            redirect_url: None,
            message: "Payment processed successfully (mock)".to_string(),
        }
    }

    fn supports_refund(&self) -> bool {
        true
    }
}

/// Manages registered payment gateways.
pub struct PaymentManager {
    gateways: HashMap<String, Box<dyn PaymentGateway>>,
}

impl PaymentManager {
    pub fn new() -> Self {
        Self {
            gateways: HashMap::new(),
        }
    }

    /// Register a new payment gateway. Replaces any existing gateway with the same id.
    pub fn register_gateway(&mut self, gateway: Box<dyn PaymentGateway>) {
        let id = gateway.id().to_string();
        tracing::info!(gateway_id = %id, title = gateway.title(), "Payment gateway registered");
        self.gateways.insert(id, gateway);
    }

    /// Get a reference to a registered gateway by id.
    pub fn get_gateway(&self, id: &str) -> Option<&dyn PaymentGateway> {
        self.gateways.get(id).map(|g| g.as_ref())
    }

    /// List all available gateways as (id, title) pairs.
    pub fn available_gateways(&self) -> Vec<(&str, &str)> {
        let mut gateways: Vec<(&str, &str)> = self
            .gateways
            .values()
            .map(|g| (g.id(), g.title()))
            .collect();
        gateways.sort_by_key(|(id, _)| *id);
        gateways
    }
}

impl Default for PaymentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{Address, Order, OrderStatus};
    use chrono::Utc;

    fn sample_order() -> Order {
        let now = Utc::now();
        Order {
            id: 1,
            order_number: "RP-TEST001".to_string(),
            status: OrderStatus::Pending,
            items: Vec::new(),
            billing_address: Address::default(),
            shipping_address: Address::default(),
            payment_method: "mock".to_string(),
            subtotal: 100.0,
            shipping_total: 5.0,
            tax_total: 8.0,
            discount_total: 0.0,
            total: 113.0,
            customer_id: Some(1),
            customer_note: String::new(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_mock_gateway_processes_payment() {
        let gateway = MockGateway::new();
        let order = sample_order();
        let result = gateway.process_payment(&order);

        assert!(result.success);
        assert_eq!(result.transaction_id, Some("MOCK-TXN-1".to_string()));
        assert!(result.message.contains("successfully"));
        assert!(gateway.supports_refund());
    }

    #[test]
    fn test_payment_manager() {
        let mut manager = PaymentManager::new();
        manager.register_gateway(Box::new(MockGateway::new()));

        let gateways = manager.available_gateways();
        assert_eq!(gateways.len(), 1);
        assert_eq!(gateways[0], ("mock", "Mock Payment Gateway"));

        let gateway = manager.get_gateway("mock").unwrap();
        assert_eq!(gateway.id(), "mock");
        assert_eq!(gateway.description(), "A test payment gateway that always succeeds. Do not use in production.");

        assert!(manager.get_gateway("nonexistent").is_none());
    }

    #[test]
    fn test_payment_manager_register_replaces() {
        let mut manager = PaymentManager::new();
        manager.register_gateway(Box::new(MockGateway::new()));
        manager.register_gateway(Box::new(MockGateway::new()));

        // Should still only have one gateway with id "mock"
        assert_eq!(manager.available_gateways().len(), 1);
    }
}

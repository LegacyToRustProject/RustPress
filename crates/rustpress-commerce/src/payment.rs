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

/// Errors that can occur during payment processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentError {
    /// The payment was declined by the provider.
    Declined { reason: String },
    /// Network or communication error with the provider.
    NetworkError { message: String },
    /// Invalid payment details (e.g., expired card).
    InvalidDetails { message: String },
    /// The payment provider is not configured or unavailable.
    ProviderUnavailable { provider: String },
    /// Generic / unexpected error.
    InternalError { message: String },
}

impl std::fmt::Display for PaymentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentError::Declined { reason } => write!(f, "Payment declined: {}", reason),
            PaymentError::NetworkError { message } => write!(f, "Network error: {}", message),
            PaymentError::InvalidDetails { message } => write!(f, "Invalid details: {}", message),
            PaymentError::ProviderUnavailable { provider } => {
                write!(f, "Provider unavailable: {}", provider)
            }
            PaymentError::InternalError { message } => write!(f, "Internal error: {}", message),
        }
    }
}

impl std::error::Error for PaymentError {}

/// Trait for payment providers that process payments by amount and currency.
pub trait PaymentProvider: Send + Sync {
    fn process_payment(
        &self,
        amount: f64,
        currency: &str,
        metadata: HashMap<String, String>,
    ) -> Result<PaymentResult, PaymentError>;
}

/// Stub Stripe payment provider. Always returns success.
pub struct StripeProvider {
    pub api_key: String,
}

impl StripeProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
        }
    }
}

impl PaymentProvider for StripeProvider {
    fn process_payment(
        &self,
        amount: f64,
        currency: &str,
        _metadata: HashMap<String, String>,
    ) -> Result<PaymentResult, PaymentError> {
        tracing::info!(
            amount = amount,
            currency = currency,
            "Stripe stub: processing payment"
        );
        Ok(PaymentResult {
            success: true,
            transaction_id: Some(format!("stripe_ch_{}", uuid::Uuid::new_v4().as_simple())),
            redirect_url: None,
            message: format!(
                "Stripe payment of {:.2} {} processed successfully (stub)",
                amount, currency
            ),
        })
    }
}

impl PaymentGateway for StripeProvider {
    fn id(&self) -> &str {
        "stripe"
    }
    fn title(&self) -> &str {
        "Credit Card (Stripe)"
    }
    fn description(&self) -> &str {
        "Pay with your credit card via Stripe."
    }
    fn process_payment(&self, order: &Order) -> PaymentResult {
        let mut metadata = HashMap::new();
        metadata.insert("order_id".to_string(), order.id.to_string());
        metadata.insert("order_number".to_string(), order.order_number.clone());
        match PaymentProvider::process_payment(self, order.total, "USD", metadata) {
            Ok(result) => result,
            Err(e) => PaymentResult {
                success: false,
                transaction_id: None,
                redirect_url: None,
                message: e.to_string(),
            },
        }
    }
    fn supports_refund(&self) -> bool {
        true
    }
}

/// Stub PayPal payment provider. Always returns success.
pub struct PayPalProvider {
    pub client_id: String,
    pub client_secret: String,
}

impl PayPalProvider {
    pub fn new(client_id: &str, client_secret: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
        }
    }
}

impl PaymentProvider for PayPalProvider {
    fn process_payment(
        &self,
        amount: f64,
        currency: &str,
        _metadata: HashMap<String, String>,
    ) -> Result<PaymentResult, PaymentError> {
        tracing::info!(
            amount = amount,
            currency = currency,
            "PayPal stub: processing payment"
        );
        Ok(PaymentResult {
            success: true,
            transaction_id: Some(format!(
                "PAYPAL-{}",
                uuid::Uuid::new_v4().as_simple().to_string()[..12].to_uppercase()
            )),
            redirect_url: Some("https://www.sandbox.paypal.com/checkout".to_string()),
            message: format!(
                "PayPal payment of {:.2} {} processed successfully (stub)",
                amount, currency
            ),
        })
    }
}

impl PaymentGateway for PayPalProvider {
    fn id(&self) -> &str {
        "paypal"
    }
    fn title(&self) -> &str {
        "PayPal"
    }
    fn description(&self) -> &str {
        "Pay securely with your PayPal account."
    }
    fn process_payment(&self, order: &Order) -> PaymentResult {
        let mut metadata = HashMap::new();
        metadata.insert("order_id".to_string(), order.id.to_string());
        metadata.insert("order_number".to_string(), order.order_number.clone());
        match PaymentProvider::process_payment(self, order.total, "USD", metadata) {
            Ok(result) => result,
            Err(e) => PaymentResult {
                success: false,
                transaction_id: None,
                redirect_url: None,
                message: e.to_string(),
            },
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
        assert_eq!(
            gateway.description(),
            "A test payment gateway that always succeeds. Do not use in production."
        );

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

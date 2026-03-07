//! Checkout flow orchestrator that coordinates cart, inventory, tax, coupons,
//! shipping, and payment into a single purchase workflow.

use serde::{Deserialize, Serialize};

use crate::cart::Cart;
use crate::coupon::{CouponManager, DiscountResult};
use crate::inventory::InventoryManager;
use crate::order::{Address, OrderItem, OrderManager, OrderStatus};
use crate::payment::{PaymentGateway, PaymentResult};
use crate::shipping::ShippingRate;
use crate::tax::{TaxCalculation, TaxCalculator, TaxClass, TaxLocation};

/// Errors that can occur during checkout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckoutError {
    EmptyCart,
    InsufficientStock { product_id: u64, available: i64 },
    PaymentFailed { message: String },
    InvalidShipping { message: String },
    ValidationError { field: String, message: String },
}

impl std::fmt::Display for CheckoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckoutError::EmptyCart => write!(f, "Cart is empty"),
            CheckoutError::InsufficientStock {
                product_id,
                available,
            } => write!(
                f,
                "Insufficient stock for product {}: {} available",
                product_id, available
            ),
            CheckoutError::PaymentFailed { message } => {
                write!(f, "Payment failed: {}", message)
            }
            CheckoutError::InvalidShipping { message } => {
                write!(f, "Invalid shipping: {}", message)
            }
            CheckoutError::ValidationError { field, message } => {
                write!(f, "Validation error on {}: {}", field, message)
            }
        }
    }
}

/// Input data for the checkout process.
pub struct CheckoutRequest {
    pub billing_address: Address,
    pub shipping_address: Address,
    pub payment_method_id: String,
    pub shipping_rate: Option<ShippingRate>,
    pub customer_id: Option<u64>,
    pub customer_note: String,
}

/// Summary of the checkout calculation before payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSummary {
    pub subtotal: f64,
    pub discount_total: f64,
    pub shipping_total: f64,
    pub tax: TaxCalculation,
    pub total: f64,
    pub applied_coupons: Vec<DiscountResult>,
}

/// Complete result of a successful checkout.
#[derive(Debug)]
pub struct CheckoutResult {
    pub order_id: u64,
    pub order_number: String,
    pub payment_result: PaymentResult,
    pub summary: CheckoutSummary,
}

/// Orchestrates the checkout process.
pub struct CheckoutProcessor<'a> {
    order_manager: &'a mut OrderManager,
    inventory: Option<&'a mut InventoryManager>,
    tax_calculator: Option<&'a TaxCalculator>,
    coupon_manager: Option<&'a mut CouponManager>,
}

impl<'a> CheckoutProcessor<'a> {
    pub fn new(order_manager: &'a mut OrderManager) -> Self {
        Self {
            order_manager,
            inventory: None,
            tax_calculator: None,
            coupon_manager: None,
        }
    }

    pub fn with_inventory(mut self, inventory: &'a mut InventoryManager) -> Self {
        self.inventory = Some(inventory);
        self
    }

    pub fn with_tax(mut self, tax: &'a TaxCalculator) -> Self {
        self.tax_calculator = Some(tax);
        self
    }

    pub fn with_coupons(mut self, coupons: &'a mut CouponManager) -> Self {
        self.coupon_manager = Some(coupons);
        self
    }

    /// Validate the checkout request without processing payment.
    pub fn validate(
        &mut self,
        cart: &Cart,
        request: &CheckoutRequest,
    ) -> Result<(), Vec<CheckoutError>> {
        let mut errors = Vec::new();

        if cart.items.is_empty() {
            errors.push(CheckoutError::EmptyCart);
        }

        // Validate billing address
        if request.billing_address.email.is_empty() {
            errors.push(CheckoutError::ValidationError {
                field: "billing_email".into(),
                message: "Email is required".into(),
            });
        }

        // Check stock availability
        if let Some(ref inventory) = self.inventory {
            for item in &cart.items {
                if !inventory.can_purchase(item.product_id, item.variation_id, item.quantity) {
                    let available = inventory
                        .get_available(item.product_id, item.variation_id)
                        .unwrap_or(0);
                    errors.push(CheckoutError::InsufficientStock {
                        product_id: item.product_id,
                        available,
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Calculate the checkout summary without processing.
    pub fn calculate_summary(&mut self, cart: &Cart, request: &CheckoutRequest) -> CheckoutSummary {
        let subtotal = cart.get_subtotal();

        // Apply coupons
        let mut discount_total = 0.0;
        let mut applied_coupons = Vec::new();
        if let Some(ref mut coupon_mgr) = self.coupon_manager {
            for code in &cart.applied_coupons {
                let result = coupon_mgr.apply_coupon(cart, code);
                if result.success {
                    discount_total += result.discount_amount;
                }
                applied_coupons.push(result);
            }
        }

        // Shipping
        let shipping_total = request
            .shipping_rate
            .as_ref()
            .map(|r| r.cost)
            .unwrap_or(0.0);

        // Tax calculation
        let taxable_amount = subtotal - discount_total;
        let tax = if let Some(tax_calc) = self.tax_calculator {
            let location = TaxLocation {
                country: request.shipping_address.country.clone(),
                state: request.shipping_address.state.clone(),
                postcode: request.shipping_address.postcode.clone(),
                city: request.shipping_address.city.clone(),
            };
            tax_calc.calculate(taxable_amount, &location, &TaxClass::Standard)
        } else {
            TaxCalculation {
                subtotal: taxable_amount,
                tax_lines: Vec::new(),
                total_tax: 0.0,
                total_with_tax: taxable_amount,
            }
        };

        let total = subtotal - discount_total + shipping_total + tax.total_tax;

        CheckoutSummary {
            subtotal,
            discount_total,
            shipping_total,
            tax,
            total: round_cents(total),
            applied_coupons,
        }
    }

    /// Process the complete checkout: validate, calculate, pay, create order, reduce stock.
    pub fn process(
        &mut self,
        cart: &Cart,
        request: &CheckoutRequest,
        gateway: &dyn PaymentGateway,
    ) -> Result<CheckoutResult, Vec<CheckoutError>> {
        // Step 1: Validate
        self.validate(cart, request)?;

        // Step 2: Calculate summary
        let summary = self.calculate_summary(cart, request);

        // Step 3: Create order
        let order_items: Vec<OrderItem> = cart
            .items
            .iter()
            .map(|item| OrderItem {
                product_id: item.product_id,
                name: item.name.clone(),
                quantity: item.quantity,
                price: item.price,
                total: item.price * item.quantity as f64,
            })
            .collect();

        let order_id = self.order_manager.create_order(
            order_items,
            request.billing_address.clone(),
            request.shipping_address.clone(),
            &request.payment_method_id,
            summary.shipping_total,
            summary.tax.total_tax,
            summary.discount_total,
            request.customer_id,
            &request.customer_note,
        );

        let order = self.order_manager.get_order(order_id).unwrap();
        let order_number = order.order_number.clone();

        // Step 4: Process payment
        let payment_result = gateway.process_payment(order);

        if !payment_result.success {
            self.order_manager
                .update_status(order_id, OrderStatus::Failed);
            return Err(vec![CheckoutError::PaymentFailed {
                message: payment_result.message.clone(),
            }]);
        }

        // Step 5: Update order status
        self.order_manager
            .update_status(order_id, OrderStatus::Processing);

        // Step 6: Reduce stock
        if let Some(ref mut inventory) = self.inventory {
            for item in &cart.items {
                let _ = inventory.reduce_stock(
                    item.product_id,
                    item.variation_id,
                    item.quantity,
                    &format!("Order #{}", order_number),
                );
            }
        }

        Ok(CheckoutResult {
            order_id,
            order_number,
            payment_result,
            summary,
        })
    }
}

fn round_cents(amount: f64) -> f64 {
    (amount * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::CartItem;
    use crate::payment::MockGateway;

    fn make_cart() -> Cart {
        let mut cart = Cart::new();
        cart.add_item(CartItem {
            product_id: 1,
            variation_id: None,
            quantity: 2,
            price: 25.0,
            name: "Widget".to_string(),
        });
        cart.add_item(CartItem {
            product_id: 2,
            variation_id: None,
            quantity: 1,
            price: 50.0,
            name: "Gadget".to_string(),
        });
        cart
    }

    fn make_request() -> CheckoutRequest {
        CheckoutRequest {
            billing_address: Address {
                email: "test@example.com".into(),
                country: "US".into(),
                state: "CA".into(),
                ..Address::default()
            },
            shipping_address: Address {
                country: "US".into(),
                state: "CA".into(),
                ..Address::default()
            },
            payment_method_id: "mock".into(),
            shipping_rate: Some(ShippingRate {
                method_id: "flat_rate".into(),
                label: "Flat Rate".into(),
                cost: 10.0,
            }),
            customer_id: Some(1),
            customer_note: String::new(),
        }
    }

    #[test]
    fn test_validate_empty_cart() {
        let mut orders = OrderManager::new();
        let mut processor = CheckoutProcessor::new(&mut orders);
        let cart = Cart::new();
        let request = make_request();

        let result = processor.validate(&cart, &request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_missing_email() {
        let mut orders = OrderManager::new();
        let mut processor = CheckoutProcessor::new(&mut orders);
        let cart = make_cart();
        let mut request = make_request();
        request.billing_address.email = String::new();

        let result = processor.validate(&cart, &request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_insufficient_stock() {
        let mut orders = OrderManager::new();
        let mut inventory = InventoryManager::new();
        inventory.register_product(1, None, 1, None, false); // only 1 in stock, need 2
        inventory.register_product(2, None, 100, None, false);

        let mut processor = CheckoutProcessor::new(&mut orders).with_inventory(&mut inventory);
        let cart = make_cart();
        let request = make_request();

        let result = processor.validate(&cart, &request);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_summary() {
        let mut orders = OrderManager::new();
        let mut processor = CheckoutProcessor::new(&mut orders);
        let cart = make_cart(); // subtotal: 2*25 + 1*50 = 100
        let request = make_request(); // shipping: 10

        let summary = processor.calculate_summary(&cart, &request);

        assert_eq!(summary.subtotal, 100.0);
        assert_eq!(summary.shipping_total, 10.0);
        assert_eq!(summary.discount_total, 0.0);
        assert_eq!(summary.total, 110.0); // 100 + 10 shipping + 0 tax
    }

    #[test]
    fn test_full_checkout_flow() {
        let mut orders = OrderManager::new();
        let mut inventory = InventoryManager::new();
        inventory.register_product(1, None, 100, None, false);
        inventory.register_product(2, None, 100, None, false);

        let mut processor = CheckoutProcessor::new(&mut orders).with_inventory(&mut inventory);
        let cart = make_cart();
        let request = make_request();
        let gateway = MockGateway::new();

        let result = processor.process(&cart, &request, &gateway);
        assert!(result.is_ok());

        let checkout = result.unwrap();
        assert!(checkout.payment_result.success);
        assert!(checkout.order_number.starts_with("RP-"));

        // Verify order was created and is Processing
        let order = orders.get_order(checkout.order_id).unwrap();
        assert_eq!(order.status, OrderStatus::Processing);

        // Verify stock was reduced
        assert_eq!(inventory.get_stock(1, None), Some(98)); // 100 - 2
        assert_eq!(inventory.get_stock(2, None), Some(99)); // 100 - 1
    }

    #[test]
    fn test_checkout_with_tax() {
        let mut orders = OrderManager::new();
        let mut tax_calc = TaxCalculator::new();
        use crate::tax::TaxRate;
        tax_calc.add_rate(TaxRate {
            id: 0,
            country: "US".into(),
            state: "CA".into(),
            postcode: String::new(),
            city: String::new(),
            rate: 10.0,
            name: "CA Tax".into(),
            priority: 1,
            compound: false,
            tax_class: TaxClass::Standard,
        });

        let mut processor = CheckoutProcessor::new(&mut orders).with_tax(&tax_calc);
        let cart = make_cart(); // subtotal 100
        let request = make_request(); // shipping 10

        let summary = processor.calculate_summary(&cart, &request);

        assert_eq!(summary.subtotal, 100.0);
        assert_eq!(summary.tax.total_tax, 10.0); // 10% of 100
        assert_eq!(summary.total, 120.0); // 100 + 10 shipping + 10 tax
    }
}

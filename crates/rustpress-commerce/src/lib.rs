pub mod cart;
pub mod checkout;
pub mod coupon;
pub mod inventory;
pub mod order;
pub mod payment;
pub mod product;
pub mod shipping;
pub mod tax;
pub mod woo_compat;

pub use cart::{Cart, CartItem, CartManager};
pub use checkout::{CheckoutError, CheckoutProcessor, CheckoutRequest, CheckoutResult, CheckoutSummary};
pub use coupon::{Coupon, CouponManager, DiscountResult, DiscountType};
pub use inventory::{InventoryManager, StockAdjustment, StockAdjustmentReason, StockReservation};
pub use order::{Address, Order, OrderItem, OrderManager, OrderStatus};
pub use payment::{MockGateway, PaymentGateway, PaymentManager, PaymentResult};
pub use product::{
    Dimensions, Product, ProductAttribute, ProductCatalog, ProductVariation, StockStatus,
};
pub use shipping::{FlatRateShipping, FreeShipping, ShippingMethod, ShippingRate, ShippingZone};
pub use tax::{TaxCalculation, TaxCalculator, TaxClass, TaxLineItem, TaxLocation, TaxRate};
pub use woo_compat::{WooOrderData, WooProductData};

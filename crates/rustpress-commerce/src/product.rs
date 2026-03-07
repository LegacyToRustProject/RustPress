use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The type of product.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductType {
    Simple,
    Variable,
    Grouped,
    External,
}

/// Stock availability status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockStatus {
    InStock,
    OutOfStock,
    OnBackorder,
}

/// Physical dimensions of a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub unit: String,
}

/// A product attribute (e.g. color, size).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductAttribute {
    pub name: String,
    pub values: Vec<String>,
    pub visible: bool,
    pub variation: bool,
}

/// A variation of a variable product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductVariation {
    pub id: u64,
    pub product_id: u64,
    pub sku: String,
    pub price: f64,
    pub stock_quantity: Option<i64>,
    pub attributes: Vec<ProductAttribute>,
}

/// A product in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub short_description: String,
    pub sku: String,
    pub price: f64,
    pub regular_price: f64,
    pub sale_price: Option<f64>,
    pub stock_quantity: Option<i64>,
    pub stock_status: StockStatus,
    pub product_type: ProductType,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub images: Vec<String>,
    pub weight: Option<f64>,
    pub dimensions: Option<Dimensions>,
    pub attributes: Vec<ProductAttribute>,
    pub variations: Vec<ProductVariation>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// In-memory product catalog.
pub struct ProductCatalog {
    products: HashMap<u64, Product>,
    next_id: u64,
}

impl ProductCatalog {
    pub fn new() -> Self {
        Self {
            products: HashMap::new(),
            next_id: 1,
        }
    }

    /// Add a product to the catalog. The product's `id` will be set automatically.
    /// Returns the assigned product id.
    pub fn add_product(&mut self, mut product: Product) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        product.id = id;
        let now = Utc::now();
        product.created_at = now;
        product.updated_at = now;
        self.products.insert(id, product);
        tracing::info!(product_id = id, "Product added to catalog");
        id
    }

    /// Get a product by id.
    pub fn get_product(&self, id: u64) -> Option<&Product> {
        self.products.get(&id)
    }

    /// List all products in the catalog.
    pub fn list_products(&self) -> Vec<&Product> {
        let mut products: Vec<&Product> = self.products.values().collect();
        products.sort_by_key(|p| p.id);
        products
    }

    /// Search products by name or description (case-insensitive substring match).
    pub fn search_products(&self, query: &str) -> Vec<&Product> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<&Product> = self
            .products
            .values()
            .filter(|p| {
                p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
                    || p.short_description.to_lowercase().contains(&query_lower)
                    || p.sku.to_lowercase().contains(&query_lower)
            })
            .collect();
        results.sort_by_key(|p| p.id);
        results
    }

    /// Update an existing product. Returns true if the product existed and was updated.
    pub fn update_product(&mut self, id: u64, mut product: Product) -> bool {
        if let Some(existing) = self.products.get(&id) {
            product.id = id;
            product.created_at = existing.created_at;
            product.updated_at = Utc::now();
            self.products.insert(id, product);
            tracing::info!(product_id = id, "Product updated");
            true
        } else {
            false
        }
    }

    /// Delete a product by id. Returns the removed product if it existed.
    pub fn delete_product(&mut self, id: u64) -> Option<Product> {
        let removed = self.products.remove(&id);
        if removed.is_some() {
            tracing::info!(product_id = id, "Product deleted");
        }
        removed
    }
}

impl Default for ProductCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a minimal product for testing.
#[cfg(test)]
fn make_test_product(name: &str, price: f64) -> Product {
    Product {
        id: 0,
        name: name.to_string(),
        slug: name.to_lowercase().replace(' ', "-"),
        description: String::new(),
        short_description: String::new(),
        sku: String::new(),
        price,
        regular_price: price,
        sale_price: None,
        stock_quantity: None,
        stock_status: StockStatus::InStock,
        product_type: ProductType::Simple,
        categories: Vec::new(),
        tags: Vec::new(),
        images: Vec::new(),
        weight: None,
        dimensions: None,
        attributes: Vec::new(),
        variations: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_product() {
        let mut catalog = ProductCatalog::new();
        let product = make_test_product("Widget", 9.99);
        let id = catalog.add_product(product);

        let retrieved = catalog.get_product(id).unwrap();
        assert_eq!(retrieved.name, "Widget");
        assert_eq!(retrieved.price, 9.99);
        assert_eq!(retrieved.id, id);
    }

    #[test]
    fn test_search_products() {
        let mut catalog = ProductCatalog::new();

        let mut p1 = make_test_product("Blue Widget", 9.99);
        p1.description = "A wonderful blue widget".to_string();
        catalog.add_product(p1);

        let p2 = make_test_product("Red Gadget", 19.99);
        catalog.add_product(p2);

        let mut p3 = make_test_product("Green Thing", 5.0);
        p3.sku = "WIDGET-GREEN".to_string();
        catalog.add_product(p3);

        let results = catalog.search_products("widget");
        assert_eq!(results.len(), 2); // Blue Widget (name) + Green Thing (sku)

        let results = catalog.search_products("gadget");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Red Gadget");
    }

    #[test]
    fn test_update_product() {
        let mut catalog = ProductCatalog::new();
        let product = make_test_product("Old Name", 10.0);
        let id = catalog.add_product(product);

        let mut updated = make_test_product("New Name", 15.0);
        assert!(catalog.update_product(id, updated.clone()));

        let retrieved = catalog.get_product(id).unwrap();
        assert_eq!(retrieved.name, "New Name");
        assert_eq!(retrieved.price, 15.0);

        // Updating a non-existent product returns false
        updated.name = "Ghost".to_string();
        assert!(!catalog.update_product(9999, updated));
    }

    #[test]
    fn test_delete_product() {
        let mut catalog = ProductCatalog::new();
        let product = make_test_product("Doomed", 1.0);
        let id = catalog.add_product(product);

        assert!(catalog.delete_product(id).is_some());
        assert!(catalog.get_product(id).is_none());
        assert!(catalog.delete_product(id).is_none());
    }

    #[test]
    fn test_list_products_sorted() {
        let mut catalog = ProductCatalog::new();
        catalog.add_product(make_test_product("C", 3.0));
        catalog.add_product(make_test_product("A", 1.0));
        catalog.add_product(make_test_product("B", 2.0));

        let products = catalog.list_products();
        assert_eq!(products.len(), 3);
        assert_eq!(products[0].name, "C"); // id 1
        assert_eq!(products[1].name, "A"); // id 2
        assert_eq!(products[2].name, "B"); // id 3
    }
}

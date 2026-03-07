//! Tax calculation system with support for multiple tax rates and zones.
//!
//! Supports WooCommerce-compatible tax features:
//! - Tax rates by country/state
//! - Multiple tax classes (standard, reduced, zero)
//! - Compound tax (tax on tax)
//! - Tax-inclusive and tax-exclusive pricing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tax class categories.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaxClass {
    Standard,
    Reduced,
    Zero,
    Custom(String),
}

impl Default for TaxClass {
    fn default() -> Self {
        TaxClass::Standard
    }
}

/// A tax rate definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRate {
    pub id: u64,
    pub country: String,
    pub state: String,
    pub postcode: String,
    pub city: String,
    pub rate: f64,
    pub name: String,
    pub priority: u32,
    pub compound: bool,
    pub tax_class: TaxClass,
}

/// Result of a tax calculation for a single item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxLineItem {
    pub rate_id: u64,
    pub label: String,
    pub tax_amount: f64,
    pub rate_percent: f64,
    pub compound: bool,
}

/// Overall tax calculation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxCalculation {
    pub subtotal: f64,
    pub tax_lines: Vec<TaxLineItem>,
    pub total_tax: f64,
    pub total_with_tax: f64,
}

/// Location used to look up applicable tax rates.
#[derive(Debug, Clone, Default)]
pub struct TaxLocation {
    pub country: String,
    pub state: String,
    pub postcode: String,
    pub city: String,
}

/// Tax calculation engine.
pub struct TaxCalculator {
    rates: Vec<TaxRate>,
    next_id: u64,
    prices_include_tax: bool,
}

impl TaxCalculator {
    pub fn new() -> Self {
        Self {
            rates: Vec::new(),
            next_id: 1,
            prices_include_tax: false,
        }
    }

    /// Set whether product prices already include tax.
    pub fn set_prices_include_tax(&mut self, include: bool) {
        self.prices_include_tax = include;
    }

    /// Add a tax rate. Returns the assigned rate ID.
    pub fn add_rate(&mut self, mut rate: TaxRate) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        rate.id = id;
        self.rates.push(rate);
        id
    }

    /// Remove a tax rate by ID.
    pub fn remove_rate(&mut self, rate_id: u64) -> bool {
        let before = self.rates.len();
        self.rates.retain(|r| r.id != rate_id);
        self.rates.len() < before
    }

    /// Find applicable tax rates for a given location and tax class.
    pub fn find_rates(&self, location: &TaxLocation, tax_class: &TaxClass) -> Vec<&TaxRate> {
        let mut matched: Vec<&TaxRate> = self
            .rates
            .iter()
            .filter(|r| {
                r.tax_class == *tax_class && Self::location_matches(r, location)
            })
            .collect();

        // Sort by priority (lower = higher priority)
        matched.sort_by_key(|r| r.priority);

        // Group by priority — only keep the highest-priority group
        if let Some(best_priority) = matched.first().map(|r| r.priority) {
            matched.retain(|r| r.priority == best_priority);
        }

        matched
    }

    /// Calculate tax for a given amount.
    pub fn calculate(
        &self,
        amount: f64,
        location: &TaxLocation,
        tax_class: &TaxClass,
    ) -> TaxCalculation {
        let rates = self.find_rates(location, tax_class);

        if rates.is_empty() {
            return TaxCalculation {
                subtotal: amount,
                tax_lines: Vec::new(),
                total_tax: 0.0,
                total_with_tax: amount,
            };
        }

        let (non_compound, compound): (Vec<&&TaxRate>, Vec<&&TaxRate>) =
            rates.iter().partition(|r| !r.compound);

        let base_amount = if self.prices_include_tax {
            // Remove tax from the inclusive price to get net amount
            let total_rate: f64 = non_compound.iter().map(|r| r.rate).sum();
            amount / (1.0 + total_rate / 100.0)
        } else {
            amount
        };

        let mut tax_lines = Vec::new();
        let mut running_total = base_amount;

        // Apply non-compound rates first
        for rate in &non_compound {
            let tax = base_amount * rate.rate / 100.0;
            tax_lines.push(TaxLineItem {
                rate_id: rate.id,
                label: rate.name.clone(),
                tax_amount: round_tax(tax),
                rate_percent: rate.rate,
                compound: false,
            });
            running_total += tax;
        }

        // Apply compound rates on the running total (base + non-compound taxes)
        for rate in &compound {
            let tax = running_total * rate.rate / 100.0;
            tax_lines.push(TaxLineItem {
                rate_id: rate.id,
                label: rate.name.clone(),
                tax_amount: round_tax(tax),
                rate_percent: rate.rate,
                compound: true,
            });
            running_total += tax;
        }

        let total_tax: f64 = tax_lines.iter().map(|t| t.tax_amount).sum();

        TaxCalculation {
            subtotal: round_tax(base_amount),
            tax_lines,
            total_tax: round_tax(total_tax),
            total_with_tax: round_tax(base_amount + total_tax),
        }
    }

    /// Calculate tax for multiple items with different tax classes.
    pub fn calculate_cart_tax(
        &self,
        items: &[(f64, TaxClass)],
        location: &TaxLocation,
    ) -> TaxCalculation {
        let mut all_tax_lines: HashMap<u64, TaxLineItem> = HashMap::new();
        let mut total_subtotal = 0.0;

        for (amount, tax_class) in items {
            let calc = self.calculate(*amount, location, tax_class);
            total_subtotal += calc.subtotal;

            for line in calc.tax_lines {
                let entry = all_tax_lines.entry(line.rate_id).or_insert(TaxLineItem {
                    rate_id: line.rate_id,
                    label: line.label.clone(),
                    tax_amount: 0.0,
                    rate_percent: line.rate_percent,
                    compound: line.compound,
                });
                entry.tax_amount += line.tax_amount;
            }
        }

        // Round aggregated amounts
        for line in all_tax_lines.values_mut() {
            line.tax_amount = round_tax(line.tax_amount);
        }

        let mut tax_lines: Vec<TaxLineItem> = all_tax_lines.into_values().collect();
        tax_lines.sort_by_key(|t| t.rate_id);

        let total_tax: f64 = tax_lines.iter().map(|t| t.tax_amount).sum();

        TaxCalculation {
            subtotal: round_tax(total_subtotal),
            tax_lines,
            total_tax: round_tax(total_tax),
            total_with_tax: round_tax(total_subtotal + total_tax),
        }
    }

    fn location_matches(rate: &TaxRate, location: &TaxLocation) -> bool {
        // Country must match (or be wildcard "*")
        if !rate.country.is_empty() && rate.country != "*" && rate.country != location.country {
            return false;
        }

        // State match (empty means "all states")
        if !rate.state.is_empty() && rate.state != "*" && rate.state != location.state {
            return false;
        }

        // Postcode match (empty means "all postcodes")
        if !rate.postcode.is_empty() && rate.postcode != "*" {
            // Support postcode ranges like "90000...99999"
            if rate.postcode.contains("...") {
                let parts: Vec<&str> = rate.postcode.split("...").collect();
                if parts.len() == 2 {
                    let matches = location.postcode >= *parts[0]
                        && location.postcode <= *parts[1];
                    if !matches {
                        return false;
                    }
                }
            } else if rate.postcode != location.postcode {
                return false;
            }
        }

        // City match (empty means "all cities")
        if !rate.city.is_empty() && rate.city != "*" {
            if rate.city.to_lowercase() != location.city.to_lowercase() {
                return false;
            }
        }

        true
    }
}

impl Default for TaxCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Round a tax amount to 2 decimal places.
fn round_tax(amount: f64) -> f64 {
    (amount * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rate(country: &str, state: &str, rate: f64, name: &str) -> TaxRate {
        TaxRate {
            id: 0,
            country: country.to_string(),
            state: state.to_string(),
            postcode: String::new(),
            city: String::new(),
            rate,
            name: name.to_string(),
            priority: 1,
            compound: false,
            tax_class: TaxClass::Standard,
        }
    }

    fn us_ca_location() -> TaxLocation {
        TaxLocation {
            country: "US".into(),
            state: "CA".into(),
            postcode: "90210".into(),
            city: "Beverly Hills".into(),
        }
    }

    #[test]
    fn test_basic_tax_calculation() {
        let mut calc = TaxCalculator::new();
        calc.add_rate(make_rate("US", "CA", 7.25, "CA State Tax"));

        let result = calc.calculate(100.0, &us_ca_location(), &TaxClass::Standard);

        assert_eq!(result.subtotal, 100.0);
        assert_eq!(result.total_tax, 7.25);
        assert_eq!(result.total_with_tax, 107.25);
        assert_eq!(result.tax_lines.len(), 1);
        assert_eq!(result.tax_lines[0].label, "CA State Tax");
    }

    #[test]
    fn test_no_matching_rate() {
        let mut calc = TaxCalculator::new();
        calc.add_rate(make_rate("US", "CA", 7.25, "CA State Tax"));

        let ny_location = TaxLocation {
            country: "US".into(),
            state: "NY".into(),
            ..Default::default()
        };

        let result = calc.calculate(100.0, &ny_location, &TaxClass::Standard);
        assert_eq!(result.total_tax, 0.0);
        assert_eq!(result.total_with_tax, 100.0);
    }

    #[test]
    fn test_compound_tax() {
        let mut calc = TaxCalculator::new();

        // Federal tax: 5% (non-compound)
        calc.add_rate(make_rate("CA", "", 5.0, "GST"));

        // Provincial tax: 9.975% compound (tax on tax)
        let mut qst = make_rate("CA", "QC", 9.975, "QST");
        qst.compound = true;
        calc.add_rate(qst);

        let location = TaxLocation {
            country: "CA".into(),
            state: "QC".into(),
            ..Default::default()
        };

        let result = calc.calculate(100.0, &location, &TaxClass::Standard);

        // GST = 100 * 5% = 5.00
        // QST = (100 + 5) * 9.975% = 10.47 (rounded)
        assert_eq!(result.tax_lines.len(), 2);
        assert_eq!(result.tax_lines[0].tax_amount, 5.0);
        assert_eq!(result.tax_lines[1].tax_amount, 10.47);
        assert_eq!(result.total_tax, 15.47);
    }

    #[test]
    fn test_tax_inclusive_pricing() {
        let mut calc = TaxCalculator::new();
        calc.set_prices_include_tax(true);
        calc.add_rate(make_rate("US", "CA", 10.0, "Sales Tax"));

        // Price is $110 including 10% tax => net $100, tax $10
        let result = calc.calculate(110.0, &us_ca_location(), &TaxClass::Standard);

        assert_eq!(result.subtotal, 100.0);
        assert_eq!(result.total_tax, 10.0);
        assert_eq!(result.total_with_tax, 110.0);
    }

    #[test]
    fn test_multiple_rates_same_priority() {
        let mut calc = TaxCalculator::new();
        calc.add_rate(make_rate("US", "CA", 6.0, "State Tax"));
        calc.add_rate(make_rate("US", "CA", 1.25, "County Tax"));

        let result = calc.calculate(100.0, &us_ca_location(), &TaxClass::Standard);

        assert_eq!(result.tax_lines.len(), 2);
        assert_eq!(result.total_tax, 7.25);
    }

    #[test]
    fn test_priority_filtering() {
        let mut calc = TaxCalculator::new();

        let mut specific = make_rate("US", "CA", 7.25, "CA Specific");
        specific.priority = 1;
        calc.add_rate(specific);

        let mut fallback = make_rate("US", "", 5.0, "US Fallback");
        fallback.priority = 2;
        calc.add_rate(fallback);

        let result = calc.calculate(100.0, &us_ca_location(), &TaxClass::Standard);

        // Only priority-1 rate should apply
        assert_eq!(result.tax_lines.len(), 1);
        assert_eq!(result.tax_lines[0].label, "CA Specific");
        assert_eq!(result.total_tax, 7.25);
    }

    #[test]
    fn test_zero_tax_class() {
        let mut calc = TaxCalculator::new();
        calc.add_rate(make_rate("US", "CA", 7.25, "CA State Tax"));

        // Zero tax class shouldn't match standard rates
        let result = calc.calculate(100.0, &us_ca_location(), &TaxClass::Zero);
        assert_eq!(result.total_tax, 0.0);
    }

    #[test]
    fn test_cart_tax_multiple_classes() {
        let mut calc = TaxCalculator::new();
        calc.add_rate(make_rate("US", "CA", 7.25, "Standard Tax"));

        let mut reduced = make_rate("US", "CA", 3.0, "Reduced Tax");
        reduced.tax_class = TaxClass::Reduced;
        calc.add_rate(reduced);

        let items = vec![
            (100.0, TaxClass::Standard),  // 7.25
            (50.0, TaxClass::Reduced),     // 1.50
        ];

        let result = calc.calculate_cart_tax(&items, &us_ca_location());
        assert_eq!(result.subtotal, 150.0);
        assert_eq!(result.total_tax, 8.75);
        assert_eq!(result.tax_lines.len(), 2);
    }

    #[test]
    fn test_postcode_range() {
        let mut calc = TaxCalculator::new();
        let mut rate = make_rate("US", "CA", 1.0, "Local Tax");
        rate.postcode = "90000...90999".into();
        calc.add_rate(rate);

        let result = calc.calculate(100.0, &us_ca_location(), &TaxClass::Standard);
        assert_eq!(result.total_tax, 1.0);

        let out_of_range = TaxLocation {
            country: "US".into(),
            state: "CA".into(),
            postcode: "95000".into(),
            ..Default::default()
        };
        let result = calc.calculate(100.0, &out_of_range, &TaxClass::Standard);
        assert_eq!(result.total_tax, 0.0);
    }

    #[test]
    fn test_remove_rate() {
        let mut calc = TaxCalculator::new();
        let id = calc.add_rate(make_rate("US", "CA", 7.25, "CA Tax"));
        assert!(calc.remove_rate(id));
        assert!(!calc.remove_rate(id)); // already removed

        let result = calc.calculate(100.0, &us_ca_location(), &TaxClass::Standard);
        assert_eq!(result.total_tax, 0.0);
    }
}

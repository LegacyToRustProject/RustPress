use serde::{Deserialize, Serialize};

/// Field to match taxonomies against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaxField {
    TermId,
    Name,
    Slug,
}

/// Operator for taxonomy query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaxOperator {
    In,
    NotIn,
    And,
    Exists,
    NotExists,
}

/// Taxonomy query for filtering posts by categories, tags, or custom taxonomies.
///
/// Equivalent to WP_Tax_Query arguments.
#[derive(Debug, Clone)]
pub struct TaxQuery {
    pub taxonomy: String,
    pub field: TaxField,
    pub terms: Vec<String>,
    pub operator: TaxOperator,
    pub include_children: bool,
}

impl TaxQuery {
    /// Create a tax query matching by slug.
    pub fn new(taxonomy: &str, slug: &str) -> Self {
        Self {
            taxonomy: taxonomy.to_string(),
            field: TaxField::Slug,
            terms: vec![slug.to_string()],
            operator: TaxOperator::In,
            include_children: true,
        }
    }

    /// Create a tax query matching by term ID.
    pub fn by_id(taxonomy: &str, term_id: u64) -> Self {
        Self {
            taxonomy: taxonomy.to_string(),
            field: TaxField::TermId,
            terms: vec![term_id.to_string()],
            operator: TaxOperator::In,
            include_children: true,
        }
    }

    /// Match multiple terms.
    pub fn terms(mut self, terms: Vec<String>) -> Self {
        self.terms = terms;
        self
    }

    /// Set the operator (In, NotIn, And, etc.).
    pub fn operator(mut self, op: TaxOperator) -> Self {
        self.operator = op;
        self
    }

    /// Whether to include child terms (hierarchical taxonomies).
    pub fn include_children(mut self, include: bool) -> Self {
        self.include_children = include;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_by_slug() {
        let tq = TaxQuery::new("category", "news");
        assert_eq!(tq.taxonomy, "category");
        assert_eq!(tq.terms, vec!["news".to_string()]);
        assert!(matches!(tq.field, TaxField::Slug));
        assert!(tq.include_children);
    }

    #[test]
    fn test_by_id() {
        let tq = TaxQuery::by_id("category", 5);
        assert_eq!(tq.terms, vec!["5".to_string()]);
        assert!(matches!(tq.field, TaxField::TermId));
    }

    #[test]
    fn test_multiple_terms() {
        let tq = TaxQuery::new("post_tag", "rust")
            .terms(vec!["rust".to_string(), "wasm".to_string()]);
        assert_eq!(tq.terms.len(), 2);
    }

    #[test]
    fn test_not_in_operator() {
        let tq = TaxQuery::new("category", "draft")
            .operator(TaxOperator::NotIn);
        assert!(matches!(tq.operator, TaxOperator::NotIn));
    }

    #[test]
    fn test_exclude_children() {
        let tq = TaxQuery::new("category", "news")
            .include_children(false);
        assert!(!tq.include_children);
    }
}

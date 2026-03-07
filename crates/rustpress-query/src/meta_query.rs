use serde::{Deserialize, Serialize};

/// Comparison operator for meta queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetaCompare {
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
    Like,
    NotLike,
    In,
    NotIn,
    Exists,
    NotExists,
}

/// Meta query condition for filtering posts by meta values.
///
/// Equivalent to WP_Meta_Query arguments.
#[derive(Debug, Clone)]
pub struct MetaQuery {
    pub key: String,
    pub value: Option<String>,
    pub compare: MetaCompare,
    pub type_cast: MetaType,
}

/// Type casting for meta value comparison.
#[derive(Debug, Clone)]
pub enum MetaType {
    Char,
    Numeric,
    Decimal,
    Date,
    DateTime,
    Time,
    Signed,
    Unsigned,
}

impl MetaQuery {
    pub fn new(key: &str, compare: MetaCompare, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: Some(value.to_string()),
            compare,
            type_cast: MetaType::Char,
        }
    }

    pub fn exists(key: &str) -> Self {
        Self {
            key: key.to_string(),
            value: None,
            compare: MetaCompare::Exists,
            type_cast: MetaType::Char,
        }
    }

    pub fn not_exists(key: &str) -> Self {
        Self {
            key: key.to_string(),
            value: None,
            compare: MetaCompare::NotExists,
            type_cast: MetaType::Char,
        }
    }

    pub fn with_type(mut self, type_cast: MetaType) -> Self {
        self.type_cast = type_cast;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_meta_query() {
        let mq = MetaQuery::new("price", MetaCompare::Gte, "100");
        assert_eq!(mq.key, "price");
        assert_eq!(mq.value, Some("100".to_string()));
        assert!(matches!(mq.compare, MetaCompare::Gte));
    }

    #[test]
    fn test_exists_query() {
        let mq = MetaQuery::exists("thumbnail_id");
        assert_eq!(mq.key, "thumbnail_id");
        assert!(mq.value.is_none());
        assert!(matches!(mq.compare, MetaCompare::Exists));
    }

    #[test]
    fn test_not_exists_query() {
        let mq = MetaQuery::not_exists("deprecated_field");
        assert!(matches!(mq.compare, MetaCompare::NotExists));
    }

    #[test]
    fn test_with_type_cast() {
        let mq = MetaQuery::new("price", MetaCompare::Gt, "50").with_type(MetaType::Numeric);
        assert!(matches!(mq.type_cast, MetaType::Numeric));
    }
}

use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use rustpress_db::entities::wp_terms;

/// Sort field for term queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TermOrderBy {
    Name,
    Slug,
    TermId,
    TermGroup,
    Count,
}

/// WordPress WP_Term_Query equivalent — builds complex taxonomy term queries.
///
/// Corresponds to `WP_Term_Query` in `wp-includes/class-wp-term-query.php`.
#[derive(Debug, Clone)]
pub struct TermQuery {
    pub taxonomy: Option<String>,
    pub search: Option<String>,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub hide_empty: bool,
    pub include: Vec<u64>,
    pub exclude: Vec<u64>,
    pub parent: Option<u64>,
    pub orderby: TermOrderBy,
    pub order: super::post_query::Order,
    pub number: u64,
    pub offset: u64,
}

impl Default for TermQuery {
    fn default() -> Self {
        Self {
            taxonomy: None,
            search: None,
            slug: None,
            name: None,
            hide_empty: true,
            include: Vec::new(),
            exclude: Vec::new(),
            parent: None,
            orderby: TermOrderBy::Name,
            order: super::post_query::Order::Asc,
            number: 0, // 0 = no limit (matches WP)
            offset: 0,
        }
    }
}

/// Result from executing a TermQuery.
#[derive(Debug, Serialize)]
pub struct TermQueryResult {
    pub terms: Vec<wp_terms::Model>,
    pub found_terms: u64,
}

impl TermQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn taxonomy(mut self, taxonomy: &str) -> Self {
        self.taxonomy = Some(taxonomy.to_string());
        self
    }

    pub fn search(mut self, s: &str) -> Self {
        self.search = Some(s.to_string());
        self
    }

    pub fn slug(mut self, slug: &str) -> Self {
        self.slug = Some(slug.to_string());
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn hide_empty(mut self, hide: bool) -> Self {
        self.hide_empty = hide;
        self
    }

    pub fn include(mut self, ids: Vec<u64>) -> Self {
        self.include = ids;
        self
    }

    pub fn exclude(mut self, ids: Vec<u64>) -> Self {
        self.exclude = ids;
        self
    }

    pub fn parent(mut self, parent_id: u64) -> Self {
        self.parent = Some(parent_id);
        self
    }

    pub fn orderby(mut self, ob: TermOrderBy) -> Self {
        self.orderby = ob;
        self
    }

    pub fn order(mut self, o: super::post_query::Order) -> Self {
        self.order = o;
        self
    }

    pub fn number(mut self, n: u64) -> Self {
        self.number = n;
        self
    }

    pub fn offset(mut self, o: u64) -> Self {
        self.offset = o;
        self
    }

    /// Execute the query against the database.
    ///
    /// Note: Filtering by taxonomy requires a JOIN with wp_term_taxonomy,
    /// which is a simplified version here operating directly on wp_terms.
    /// For taxonomy filtering, use the taxonomy-aware variant.
    pub async fn execute(
        &self,
        db: &DatabaseConnection,
    ) -> Result<TermQueryResult, sea_orm::DbErr> {
        debug!(?self, "executing TermQuery");

        let mut query = wp_terms::Entity::find();
        let mut condition = Condition::all();

        // slug filter
        if let Some(ref slug) = self.slug {
            condition = condition.add(wp_terms::Column::Slug.eq(slug));
        }

        // name filter
        if let Some(ref name) = self.name {
            condition = condition.add(wp_terms::Column::Name.eq(name));
        }

        // search filter
        if let Some(ref s) = self.search {
            let pattern = format!("%{s}%");
            condition = condition.add(
                Condition::any()
                    .add(wp_terms::Column::Name.like(&pattern))
                    .add(wp_terms::Column::Slug.like(&pattern)),
            );
        }

        // include filter
        if !self.include.is_empty() {
            condition = condition.add(wp_terms::Column::TermId.is_in(self.include.clone()));
        }

        // exclude filter
        if !self.exclude.is_empty() {
            condition = condition.add(wp_terms::Column::TermId.is_not_in(self.exclude.clone()));
        }

        query = query.filter(condition);

        // Order
        query = match self.orderby {
            TermOrderBy::Name => match self.order {
                super::post_query::Order::Desc => query.order_by_desc(wp_terms::Column::Name),
                super::post_query::Order::Asc => query.order_by_asc(wp_terms::Column::Name),
            },
            TermOrderBy::Slug => match self.order {
                super::post_query::Order::Desc => query.order_by_desc(wp_terms::Column::Slug),
                super::post_query::Order::Asc => query.order_by_asc(wp_terms::Column::Slug),
            },
            TermOrderBy::TermId => match self.order {
                super::post_query::Order::Desc => query.order_by_desc(wp_terms::Column::TermId),
                super::post_query::Order::Asc => query.order_by_asc(wp_terms::Column::TermId),
            },
            TermOrderBy::TermGroup => match self.order {
                super::post_query::Order::Desc => query.order_by_desc(wp_terms::Column::TermGroup),
                super::post_query::Order::Asc => query.order_by_asc(wp_terms::Column::TermGroup),
            },
            TermOrderBy::Count => match self.order {
                // Count lives in wp_term_taxonomy, fall back to TermId
                super::post_query::Order::Desc => query.order_by_desc(wp_terms::Column::TermId),
                super::post_query::Order::Asc => query.order_by_asc(wp_terms::Column::TermId),
            },
        };

        // Count total
        let found_terms = query.clone().count(db).await?;

        // Pagination
        if self.offset > 0 {
            query = query.offset(self.offset);
        }
        if self.number > 0 {
            query = query.limit(self.number);
        }

        let terms = query.all(db).await?;

        Ok(TermQueryResult { terms, found_terms })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_term_query() {
        let q = TermQuery::new();
        assert!(q.taxonomy.is_none());
        assert!(q.search.is_none());
        assert!(q.hide_empty);
        assert_eq!(q.number, 0);
    }

    #[test]
    fn test_builder_taxonomy() {
        let q = TermQuery::new().taxonomy("category");
        assert_eq!(q.taxonomy, Some("category".to_string()));
    }

    #[test]
    fn test_builder_search() {
        let q = TermQuery::new().search("news");
        assert_eq!(q.search, Some("news".to_string()));
    }

    #[test]
    fn test_builder_chaining() {
        let q = TermQuery::new()
            .taxonomy("post_tag")
            .hide_empty(false)
            .orderby(TermOrderBy::TermId)
            .order(super::super::post_query::Order::Desc)
            .number(20);
        assert_eq!(q.taxonomy, Some("post_tag".to_string()));
        assert!(!q.hide_empty);
        assert_eq!(q.number, 20);
    }
}

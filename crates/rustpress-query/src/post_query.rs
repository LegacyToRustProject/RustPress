use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use rustpress_db::entities::wp_posts;

use crate::meta_query::MetaQuery;
use crate::tax_query::TaxQuery;

/// Sort field for post queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderBy {
    Date,
    Modified,
    Title,
    Id,
    Author,
    MenuOrder,
    CommentCount,
}

/// Sort direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Order {
    Asc,
    Desc,
}

/// WordPress WP_Query equivalent - builds complex post queries.
#[derive(Debug, Clone)]
#[allow(non_snake_case)]
pub struct PostQuery {
    pub post_type: Vec<String>,
    pub post_status: Vec<String>,
    pub author: Option<u64>,
    pub search: Option<String>,
    pub post_name: Option<String>,
    pub post_parent: Option<u64>,
    pub posts_per_page: u64,
    pub page: u64,
    pub orderby: OrderBy,
    pub order: Order,
    pub meta_queries: Vec<MetaQuery>,
    pub tax_queries: Vec<TaxQuery>,
    pub post__in: Vec<u64>,
    pub post__not_in: Vec<u64>,
}

impl Default for PostQuery {
    fn default() -> Self {
        Self {
            post_type: vec!["post".to_string()],
            post_status: vec!["publish".to_string()],
            author: None,
            search: None,
            post_name: None,
            post_parent: None,
            posts_per_page: 10,
            page: 1,
            orderby: OrderBy::Date,
            order: Order::Desc,
            meta_queries: Vec::new(),
            tax_queries: Vec::new(),
            post__in: Vec::new(),
            post__not_in: Vec::new(),
        }
    }
}

/// Result from executing a PostQuery.
#[derive(Debug, Serialize)]
pub struct PostQueryResult {
    pub posts: Vec<wp_posts::Model>,
    pub found_posts: u64,
    pub max_num_pages: u64,
    pub current_page: u64,
}

impl PostQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn post_type(mut self, pt: &str) -> Self {
        self.post_type = vec![pt.to_string()];
        self
    }

    pub fn post_types(mut self, types: Vec<String>) -> Self {
        self.post_type = types;
        self
    }

    pub fn status(mut self, status: &str) -> Self {
        self.post_status = vec![status.to_string()];
        self
    }

    pub fn statuses(mut self, statuses: Vec<String>) -> Self {
        self.post_status = statuses;
        self
    }

    pub fn author(mut self, author_id: u64) -> Self {
        self.author = Some(author_id);
        self
    }

    pub fn search(mut self, s: &str) -> Self {
        self.search = Some(s.to_string());
        self
    }

    pub fn slug(mut self, slug: &str) -> Self {
        self.post_name = Some(slug.to_string());
        self
    }

    pub fn parent(mut self, parent_id: u64) -> Self {
        self.post_parent = Some(parent_id);
        self
    }

    pub fn posts_per_page(mut self, n: u64) -> Self {
        self.posts_per_page = n;
        self
    }

    pub fn page(mut self, p: u64) -> Self {
        self.page = p;
        self
    }

    pub fn orderby(mut self, ob: OrderBy) -> Self {
        self.orderby = ob;
        self
    }

    pub fn order(mut self, o: Order) -> Self {
        self.order = o;
        self
    }

    pub fn meta_query(mut self, mq: MetaQuery) -> Self {
        self.meta_queries.push(mq);
        self
    }

    pub fn tax_query(mut self, tq: TaxQuery) -> Self {
        self.tax_queries.push(tq);
        self
    }

    pub fn post_in(mut self, ids: Vec<u64>) -> Self {
        self.post__in = ids;
        self
    }

    pub fn post_not_in(mut self, ids: Vec<u64>) -> Self {
        self.post__not_in = ids;
        self
    }

    /// Execute the query against the database.
    pub async fn execute(
        &self,
        db: &DatabaseConnection,
    ) -> Result<PostQueryResult, sea_orm::DbErr> {
        debug!(?self, "executing PostQuery");

        let mut query = wp_posts::Entity::find();
        let mut condition = Condition::all();

        // post_type filter
        if self.post_type.len() == 1 {
            condition = condition.add(wp_posts::Column::PostType.eq(&self.post_type[0]));
        } else {
            condition = condition.add(wp_posts::Column::PostType.is_in(&self.post_type));
        }

        // post_status filter
        if self.post_status.len() == 1 {
            condition = condition.add(wp_posts::Column::PostStatus.eq(&self.post_status[0]));
        } else {
            condition = condition.add(wp_posts::Column::PostStatus.is_in(&self.post_status));
        }

        // author filter
        if let Some(author_id) = self.author {
            condition = condition.add(wp_posts::Column::PostAuthor.eq(author_id));
        }

        // search filter
        if let Some(ref s) = self.search {
            let search_pattern = format!("%{s}%");
            condition = condition.add(
                Condition::any()
                    .add(wp_posts::Column::PostTitle.like(&search_pattern))
                    .add(wp_posts::Column::PostContent.like(&search_pattern)),
            );
        }

        // slug filter
        if let Some(ref slug) = self.post_name {
            condition = condition.add(wp_posts::Column::PostName.eq(slug));
        }

        // parent filter
        if let Some(parent_id) = self.post_parent {
            condition = condition.add(wp_posts::Column::PostParent.eq(parent_id));
        }

        // post__in filter
        if !self.post__in.is_empty() {
            condition = condition.add(wp_posts::Column::Id.is_in(self.post__in.clone()));
        }

        // post__not_in filter
        if !self.post__not_in.is_empty() {
            condition = condition.add(wp_posts::Column::Id.is_not_in(self.post__not_in.clone()));
        }

        query = query.filter(condition);

        // Order by
        query = match self.orderby {
            OrderBy::Date => match self.order {
                Order::Desc => query.order_by_desc(wp_posts::Column::PostDate),
                Order::Asc => query.order_by_asc(wp_posts::Column::PostDate),
            },
            OrderBy::Modified => match self.order {
                Order::Desc => query.order_by_desc(wp_posts::Column::PostModified),
                Order::Asc => query.order_by_asc(wp_posts::Column::PostModified),
            },
            OrderBy::Title => match self.order {
                Order::Desc => query.order_by_desc(wp_posts::Column::PostTitle),
                Order::Asc => query.order_by_asc(wp_posts::Column::PostTitle),
            },
            OrderBy::Id => match self.order {
                Order::Desc => query.order_by_desc(wp_posts::Column::Id),
                Order::Asc => query.order_by_asc(wp_posts::Column::Id),
            },
            OrderBy::Author => match self.order {
                Order::Desc => query.order_by_desc(wp_posts::Column::PostAuthor),
                Order::Asc => query.order_by_asc(wp_posts::Column::PostAuthor),
            },
            OrderBy::MenuOrder => match self.order {
                Order::Desc => query.order_by_desc(wp_posts::Column::MenuOrder),
                Order::Asc => query.order_by_asc(wp_posts::Column::MenuOrder),
            },
            OrderBy::CommentCount => match self.order {
                Order::Desc => query.order_by_desc(wp_posts::Column::CommentCount),
                Order::Asc => query.order_by_asc(wp_posts::Column::CommentCount),
            },
        };

        // Count total
        let found_posts = query.clone().count(db).await?;
        let max_num_pages = if self.posts_per_page > 0 {
            found_posts.div_ceil(self.posts_per_page)
        } else {
            1
        };

        // Pagination
        let posts = query
            .offset((self.page - 1) * self.posts_per_page)
            .limit(self.posts_per_page)
            .all(db)
            .await?;

        Ok(PostQueryResult {
            posts,
            found_posts,
            max_num_pages,
            current_page: self.page,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_query() {
        let query = PostQuery::new();
        assert_eq!(query.post_type, vec!["post".to_string()]);
        assert_eq!(query.post_status, vec!["publish".to_string()]);
        assert_eq!(query.posts_per_page, 10);
        assert_eq!(query.page, 1);
        assert!(query.author.is_none());
        assert!(query.search.is_none());
    }

    #[test]
    fn test_builder_post_type() {
        let query = PostQuery::new().post_type("page");
        assert_eq!(query.post_type, vec!["page".to_string()]);
    }

    #[test]
    fn test_builder_multiple_types() {
        let query = PostQuery::new().post_types(vec!["post".to_string(), "page".to_string()]);
        assert_eq!(query.post_type.len(), 2);
    }

    #[test]
    fn test_builder_status() {
        let query = PostQuery::new().status("draft");
        assert_eq!(query.post_status, vec!["draft".to_string()]);
    }

    #[test]
    fn test_builder_author() {
        let query = PostQuery::new().author(42);
        assert_eq!(query.author, Some(42));
    }

    #[test]
    fn test_builder_search() {
        let query = PostQuery::new().search("hello");
        assert_eq!(query.search, Some("hello".to_string()));
    }

    #[test]
    fn test_builder_slug() {
        let query = PostQuery::new().slug("my-post");
        assert_eq!(query.post_name, Some("my-post".to_string()));
    }

    #[test]
    fn test_builder_pagination() {
        let query = PostQuery::new().posts_per_page(25).page(3);
        assert_eq!(query.posts_per_page, 25);
        assert_eq!(query.page, 3);
    }

    #[test]
    fn test_builder_order() {
        let query = PostQuery::new().orderby(OrderBy::Title).order(Order::Asc);
        assert!(matches!(query.orderby, OrderBy::Title));
        assert!(matches!(query.order, Order::Asc));
    }

    #[test]
    fn test_builder_post_in() {
        let query = PostQuery::new().post_in(vec![1, 2, 3]);
        assert_eq!(query.post__in, vec![1, 2, 3]);
    }

    #[test]
    fn test_builder_post_not_in() {
        let query = PostQuery::new().post_not_in(vec![10, 20]);
        assert_eq!(query.post__not_in, vec![10, 20]);
    }

    #[test]
    fn test_builder_chaining() {
        let query = PostQuery::new()
            .post_type("post")
            .status("publish")
            .author(1)
            .search("rust")
            .posts_per_page(5)
            .page(2)
            .orderby(OrderBy::Date)
            .order(Order::Desc);

        assert_eq!(query.post_type, vec!["post".to_string()]);
        assert_eq!(query.author, Some(1));
        assert_eq!(query.search, Some("rust".to_string()));
        assert_eq!(query.posts_per_page, 5);
        assert_eq!(query.page, 2);
    }
}

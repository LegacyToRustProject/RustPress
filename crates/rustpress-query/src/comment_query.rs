use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use rustpress_db::entities::wp_comments;

/// Sort field for comment queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentOrderBy {
    CommentDateGmt,
    CommentDate,
    CommentId,
    CommentPostId,
    CommentParent,
}

/// Comment status for queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommentQueryStatus {
    Approve,
    Hold,
    Spam,
    Trash,
    All,
}

impl CommentQueryStatus {
    pub fn as_db_value(&self) -> Option<&'static str> {
        match self {
            Self::Approve => Some("1"),
            Self::Hold => Some("0"),
            Self::Spam => Some("spam"),
            Self::Trash => Some("trash"),
            Self::All => None,
        }
    }
}

/// WordPress WP_Comment_Query equivalent — builds complex comment queries.
///
/// Corresponds to `WP_Comment_Query` in `wp-includes/class-wp-comment-query.php`.
#[derive(Debug, Clone)]
pub struct CommentQuery {
    pub post_id: Option<u64>,
    pub author_email: Option<String>,
    pub status: CommentQueryStatus,
    pub comment_type: Option<String>,
    pub parent: Option<u64>,
    pub user_id: Option<u64>,
    pub search: Option<String>,
    pub orderby: CommentOrderBy,
    pub order: super::post_query::Order,
    pub number: u64,
    pub paged: u64,
    pub include: Vec<u64>,
    pub exclude: Vec<u64>,
}

impl Default for CommentQuery {
    fn default() -> Self {
        Self {
            post_id: None,
            author_email: None,
            status: CommentQueryStatus::Approve,
            comment_type: None,
            parent: None,
            user_id: None,
            search: None,
            orderby: CommentOrderBy::CommentDateGmt,
            order: super::post_query::Order::Desc,
            number: 10,
            paged: 1,
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

/// Result from executing a CommentQuery.
#[derive(Debug, Serialize)]
pub struct CommentQueryResult {
    pub comments: Vec<wp_comments::Model>,
    pub found_comments: u64,
    pub max_num_pages: u64,
    pub current_page: u64,
}

impl CommentQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn post_id(mut self, id: u64) -> Self {
        self.post_id = Some(id);
        self
    }

    pub fn author_email(mut self, email: &str) -> Self {
        self.author_email = Some(email.to_string());
        self
    }

    pub fn status(mut self, s: CommentQueryStatus) -> Self {
        self.status = s;
        self
    }

    pub fn comment_type(mut self, t: &str) -> Self {
        self.comment_type = Some(t.to_string());
        self
    }

    pub fn parent(mut self, parent_id: u64) -> Self {
        self.parent = Some(parent_id);
        self
    }

    pub fn user_id(mut self, uid: u64) -> Self {
        self.user_id = Some(uid);
        self
    }

    pub fn search(mut self, s: &str) -> Self {
        self.search = Some(s.to_string());
        self
    }

    pub fn orderby(mut self, ob: CommentOrderBy) -> Self {
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

    pub fn paged(mut self, p: u64) -> Self {
        self.paged = p;
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

    /// Execute the query against the database.
    pub async fn execute(
        &self,
        db: &DatabaseConnection,
    ) -> Result<CommentQueryResult, sea_orm::DbErr> {
        debug!(?self, "executing CommentQuery");

        let mut query = wp_comments::Entity::find();
        let mut condition = Condition::all();

        // post_id filter
        if let Some(pid) = self.post_id {
            condition = condition.add(wp_comments::Column::CommentPostId.eq(pid));
        }

        // author_email filter
        if let Some(ref email) = self.author_email {
            condition = condition.add(wp_comments::Column::CommentAuthorEmail.eq(email));
        }

        // status filter
        if let Some(status_val) = self.status.as_db_value() {
            condition = condition.add(wp_comments::Column::CommentApproved.eq(status_val));
        }

        // comment_type filter
        if let Some(ref ct) = self.comment_type {
            condition = condition.add(wp_comments::Column::CommentType.eq(ct));
        }

        // parent filter
        if let Some(parent_id) = self.parent {
            condition = condition.add(wp_comments::Column::CommentParent.eq(parent_id));
        }

        // user_id filter
        if let Some(uid) = self.user_id {
            condition = condition.add(wp_comments::Column::UserId.eq(uid));
        }

        // search filter
        if let Some(ref s) = self.search {
            let pattern = format!("%{}%", s);
            condition = condition.add(
                Condition::any()
                    .add(wp_comments::Column::CommentContent.like(&pattern))
                    .add(wp_comments::Column::CommentAuthor.like(&pattern))
                    .add(wp_comments::Column::CommentAuthorEmail.like(&pattern)),
            );
        }

        // include filter
        if !self.include.is_empty() {
            condition = condition.add(wp_comments::Column::CommentId.is_in(self.include.clone()));
        }

        // exclude filter
        if !self.exclude.is_empty() {
            condition =
                condition.add(wp_comments::Column::CommentId.is_not_in(self.exclude.clone()));
        }

        query = query.filter(condition);

        // Order
        query = match self.orderby {
            CommentOrderBy::CommentDateGmt => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_comments::Column::CommentDateGmt)
                }
                super::post_query::Order::Asc => {
                    query.order_by_asc(wp_comments::Column::CommentDateGmt)
                }
            },
            CommentOrderBy::CommentDate => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_comments::Column::CommentDate)
                }
                super::post_query::Order::Asc => {
                    query.order_by_asc(wp_comments::Column::CommentDate)
                }
            },
            CommentOrderBy::CommentId => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_comments::Column::CommentId)
                }
                super::post_query::Order::Asc => query.order_by_asc(wp_comments::Column::CommentId),
            },
            CommentOrderBy::CommentPostId => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_comments::Column::CommentPostId)
                }
                super::post_query::Order::Asc => {
                    query.order_by_asc(wp_comments::Column::CommentPostId)
                }
            },
            CommentOrderBy::CommentParent => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_comments::Column::CommentParent)
                }
                super::post_query::Order::Asc => {
                    query.order_by_asc(wp_comments::Column::CommentParent)
                }
            },
        };

        // Count total
        let found_comments = query.clone().count(db).await?;
        let max_num_pages = if self.number > 0 {
            found_comments.div_ceil(self.number)
        } else {
            1
        };

        // Pagination
        let comments = query
            .offset((self.paged - 1) * self.number)
            .limit(self.number)
            .all(db)
            .await?;

        Ok(CommentQueryResult {
            comments,
            found_comments,
            max_num_pages,
            current_page: self.paged,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_comment_query() {
        let q = CommentQuery::new();
        assert!(q.post_id.is_none());
        assert!(q.search.is_none());
        assert_eq!(q.number, 10);
        assert_eq!(q.paged, 1);
    }

    #[test]
    fn test_builder_post_id() {
        let q = CommentQuery::new().post_id(42);
        assert_eq!(q.post_id, Some(42));
    }

    #[test]
    fn test_builder_status() {
        let q = CommentQuery::new().status(CommentQueryStatus::Hold);
        assert!(matches!(q.status, CommentQueryStatus::Hold));
    }

    #[test]
    fn test_builder_chaining() {
        let q = CommentQuery::new()
            .post_id(1)
            .status(CommentQueryStatus::All)
            .search("hello")
            .number(25)
            .paged(2)
            .parent(0);
        assert_eq!(q.post_id, Some(1));
        assert_eq!(q.number, 25);
        assert_eq!(q.parent, Some(0));
    }
}

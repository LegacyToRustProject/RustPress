use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use rustpress_db::entities::wp_users;

/// Sort field for user queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserOrderBy {
    Id,
    Login,
    Nicename,
    Email,
    Registered,
    DisplayName,
}

/// WordPress WP_User_Query equivalent — builds complex user queries.
///
/// Corresponds to `WP_User_Query` in `wp-includes/class-wp-user-query.php`.
#[derive(Debug, Clone)]
pub struct UserQuery {
    pub role: Option<String>,
    pub search: Option<String>,
    pub include: Vec<u64>,
    pub exclude: Vec<u64>,
    pub orderby: UserOrderBy,
    pub order: super::post_query::Order,
    pub number: u64,
    pub paged: u64,
    pub login: Option<String>,
    pub nicename: Option<String>,
    pub email: Option<String>,
}

impl Default for UserQuery {
    fn default() -> Self {
        Self {
            role: None,
            search: None,
            include: Vec::new(),
            exclude: Vec::new(),
            orderby: UserOrderBy::Login,
            order: super::post_query::Order::Asc,
            number: 10,
            paged: 1,
            login: None,
            nicename: None,
            email: None,
        }
    }
}

/// Result from executing a UserQuery.
#[derive(Debug, Serialize)]
pub struct UserQueryResult {
    pub users: Vec<wp_users::Model>,
    pub total_users: u64,
    pub max_num_pages: u64,
    pub current_page: u64,
}

impl UserQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn role(mut self, role: &str) -> Self {
        self.role = Some(role.to_string());
        self
    }

    pub fn search(mut self, s: &str) -> Self {
        self.search = Some(s.to_string());
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

    pub fn orderby(mut self, ob: UserOrderBy) -> Self {
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

    pub fn login(mut self, login: &str) -> Self {
        self.login = Some(login.to_string());
        self
    }

    pub fn nicename(mut self, nicename: &str) -> Self {
        self.nicename = Some(nicename.to_string());
        self
    }

    pub fn email(mut self, email: &str) -> Self {
        self.email = Some(email.to_string());
        self
    }

    /// Execute the query against the database.
    pub async fn execute(
        &self,
        db: &DatabaseConnection,
    ) -> Result<UserQueryResult, sea_orm::DbErr> {
        debug!(?self, "executing UserQuery");

        let mut query = wp_users::Entity::find();
        let mut condition = Condition::all();

        // login filter
        if let Some(ref login) = self.login {
            condition = condition.add(wp_users::Column::UserLogin.eq(login));
        }

        // nicename filter
        if let Some(ref nicename) = self.nicename {
            condition = condition.add(wp_users::Column::UserNicename.eq(nicename));
        }

        // email filter
        if let Some(ref email) = self.email {
            condition = condition.add(wp_users::Column::UserEmail.eq(email));
        }

        // search filter
        if let Some(ref s) = self.search {
            let pattern = format!("%{s}%");
            condition = condition.add(
                Condition::any()
                    .add(wp_users::Column::UserLogin.like(&pattern))
                    .add(wp_users::Column::UserEmail.like(&pattern))
                    .add(wp_users::Column::DisplayName.like(&pattern))
                    .add(wp_users::Column::UserNicename.like(&pattern)),
            );
        }

        // include filter
        if !self.include.is_empty() {
            condition = condition.add(wp_users::Column::Id.is_in(self.include.clone()));
        }

        // exclude filter
        if !self.exclude.is_empty() {
            condition = condition.add(wp_users::Column::Id.is_not_in(self.exclude.clone()));
        }

        query = query.filter(condition);

        // Order
        query = match self.orderby {
            UserOrderBy::Id => match self.order {
                super::post_query::Order::Desc => query.order_by_desc(wp_users::Column::Id),
                super::post_query::Order::Asc => query.order_by_asc(wp_users::Column::Id),
            },
            UserOrderBy::Login => match self.order {
                super::post_query::Order::Desc => query.order_by_desc(wp_users::Column::UserLogin),
                super::post_query::Order::Asc => query.order_by_asc(wp_users::Column::UserLogin),
            },
            UserOrderBy::Nicename => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_users::Column::UserNicename)
                }
                super::post_query::Order::Asc => query.order_by_asc(wp_users::Column::UserNicename),
            },
            UserOrderBy::Email => match self.order {
                super::post_query::Order::Desc => query.order_by_desc(wp_users::Column::UserEmail),
                super::post_query::Order::Asc => query.order_by_asc(wp_users::Column::UserEmail),
            },
            UserOrderBy::Registered => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_users::Column::UserRegistered)
                }
                super::post_query::Order::Asc => {
                    query.order_by_asc(wp_users::Column::UserRegistered)
                }
            },
            UserOrderBy::DisplayName => match self.order {
                super::post_query::Order::Desc => {
                    query.order_by_desc(wp_users::Column::DisplayName)
                }
                super::post_query::Order::Asc => query.order_by_asc(wp_users::Column::DisplayName),
            },
        };

        // Count total
        let total_users = query.clone().count(db).await?;
        let max_num_pages = if self.number > 0 {
            total_users.div_ceil(self.number)
        } else {
            1
        };

        // Pagination
        let users = query
            .offset((self.paged - 1) * self.number)
            .limit(self.number)
            .all(db)
            .await?;

        Ok(UserQueryResult {
            users,
            total_users,
            max_num_pages,
            current_page: self.paged,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_user_query() {
        let q = UserQuery::new();
        assert!(q.role.is_none());
        assert!(q.search.is_none());
        assert_eq!(q.number, 10);
        assert_eq!(q.paged, 1);
    }

    #[test]
    fn test_builder_role() {
        let q = UserQuery::new().role("administrator");
        assert_eq!(q.role, Some("administrator".to_string()));
    }

    #[test]
    fn test_builder_search() {
        let q = UserQuery::new().search("john");
        assert_eq!(q.search, Some("john".to_string()));
    }

    #[test]
    fn test_builder_include_exclude() {
        let q = UserQuery::new().include(vec![1, 2]).exclude(vec![3]);
        assert_eq!(q.include, vec![1, 2]);
        assert_eq!(q.exclude, vec![3]);
    }

    #[test]
    fn test_builder_chaining() {
        let q = UserQuery::new()
            .role("editor")
            .search("test")
            .number(25)
            .paged(2)
            .orderby(UserOrderBy::Registered)
            .order(super::super::post_query::Order::Desc);
        assert_eq!(q.role, Some("editor".to_string()));
        assert_eq!(q.number, 25);
        assert_eq!(q.paged, 2);
    }
}

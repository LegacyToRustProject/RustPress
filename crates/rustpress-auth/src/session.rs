use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

use rustpress_db::entities::wp_options;

/// Session data stored for authenticated users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: u64,
    pub login: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Session manager with in-memory cache backed by database persistence.
///
/// Sessions are written through to the database (wp_options table) so they
/// survive server restarts. The in-memory HashMap provides fast lookups.
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    session_duration_hours: i64,
    db: Option<DatabaseConnection>,
}

impl SessionManager {
    /// Create a new session manager (in-memory only, for testing).
    pub fn new(session_duration_hours: i64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_duration_hours,
            db: None,
        }
    }

    /// Create a session manager with database persistence.
    pub fn with_db(session_duration_hours: i64, db: DatabaseConnection) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_duration_hours,
            db: Some(db),
        }
    }

    /// Load all sessions from the database into memory.
    /// Should be called once at startup.
    pub async fn load_from_db(&self) {
        let Some(ref db) = self.db else { return };

        let result = wp_options::Entity::find()
            .filter(wp_options::Column::OptionName.starts_with("_rustpress_session_"))
            .all(db)
            .await;

        match result {
            Ok(rows) => {
                let mut sessions = self.sessions.write().await;
                let now = Utc::now();
                let mut loaded = 0u32;

                for row in rows {
                    if let Ok(session) = serde_json::from_str::<Session>(&row.option_value) {
                        if session.expires_at > now {
                            sessions.insert(session.id.clone(), session);
                            loaded += 1;
                        }
                    }
                }

                debug!(count = loaded, "loaded sessions from database");
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to load sessions from database");
            }
        }
    }

    /// Create a new session for a user.
    pub async fn create_session(&self, user_id: u64, login: &str, role: &str) -> Session {
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4().to_string(),
            user_id,
            login: login.to_string(),
            role: role.to_string(),
            created_at: now,
            expires_at: now + Duration::hours(self.session_duration_hours),
        };

        // Store in memory
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session.id.clone(), session.clone());
        }

        // Persist to DB
        self.persist_session(&session).await;

        session
    }

    /// Get a session by its ID, if it exists and is not expired.
    pub async fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).and_then(|s| {
            if s.expires_at > Utc::now() {
                Some(s.clone())
            } else {
                None
            }
        })
    }

    /// Destroy a session (logout).
    pub async fn destroy_session(&self, session_id: &str) -> bool {
        let removed = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id).is_some()
        };

        if removed {
            self.delete_session_from_db(session_id).await;
        }

        removed
    }

    /// Clean up expired sessions from memory and database.
    pub async fn cleanup_expired(&self) {
        let expired_ids: Vec<String> = {
            let mut sessions = self.sessions.write().await;
            let now = Utc::now();
            let expired: Vec<String> = sessions
                .iter()
                .filter(|(_, s)| s.expires_at <= now)
                .map(|(id, _)| id.clone())
                .collect();
            for id in &expired {
                sessions.remove(id);
            }
            expired
        };

        // Remove from DB
        for id in &expired_ids {
            self.delete_session_from_db(id).await;
        }

        debug!(count = expired_ids.len(), "cleaned up expired sessions");
    }

    /// Persist a session to the database.
    async fn persist_session(&self, session: &Session) {
        let Some(ref db) = self.db else { return };

        let option_name = format!("_rustpress_session_{}", session.id);
        let value = serde_json::to_string(session).unwrap_or_default();

        use sea_orm::{ActiveModelTrait, ActiveValue::Set};
        let model = wp_options::ActiveModel {
            option_id: sea_orm::ActiveValue::NotSet,
            option_name: Set(option_name),
            option_value: Set(value),
            autoload: Set("no".to_string()),
        };

        if let Err(e) = model.insert(db).await {
            tracing::error!(error = %e, session_id = session.id, "failed to persist session");
        }
    }

    /// Delete a session from the database.
    async fn delete_session_from_db(&self, session_id: &str) {
        let Some(ref db) = self.db else { return };

        let option_name = format!("_rustpress_session_{}", session_id);
        let _ = wp_options::Entity::delete_many()
            .filter(wp_options::Column::OptionName.eq(option_name))
            .exec(db)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_session() {
        let manager = SessionManager::new(24);
        let session = manager.create_session(1, "admin", "administrator").await;

        assert_eq!(session.user_id, 1);
        assert_eq!(session.login, "admin");
        assert_eq!(session.role, "administrator");

        let retrieved = manager.get_session(&session.id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().user_id, 1);
    }

    #[tokio::test]
    async fn test_destroy_session() {
        let manager = SessionManager::new(24);
        let session = manager.create_session(1, "admin", "administrator").await;

        assert!(manager.destroy_session(&session.id).await);
        assert!(manager.get_session(&session.id).await.is_none());
    }

    #[tokio::test]
    async fn test_nonexistent_session() {
        let manager = SessionManager::new(24);
        assert!(manager.get_session("nonexistent").await.is_none());
        assert!(!manager.destroy_session("nonexistent").await);
    }

    #[tokio::test]
    async fn test_expired_session() {
        let manager = SessionManager::new(0); // 0 hours = expires immediately
        let session = manager.create_session(1, "admin", "administrator").await;

        // Session with 0h duration should already be expired
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        assert!(manager.get_session(&session.id).await.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let manager = SessionManager::new(0);
        manager.create_session(1, "user1", "subscriber").await;
        manager.create_session(2, "user2", "subscriber").await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        manager.cleanup_expired().await;

        let sessions = manager.sessions.read().await;
        assert!(sessions.is_empty());
    }
}

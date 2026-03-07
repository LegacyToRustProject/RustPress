//! Form submission storage and management.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The status of a form submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubmissionStatus {
    /// Newly received, unread.
    New,
    /// Has been read/reviewed.
    Read,
    /// Marked as spam.
    Spam,
    /// Moved to trash.
    Trash,
}

/// A single form submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSubmission {
    /// Unique submission ID.
    pub id: Uuid,
    /// The ID of the form this submission belongs to.
    pub form_id: String,
    /// Submitted field data (field name -> value).
    pub data: HashMap<String, String>,
    /// Timestamp of submission.
    pub submitted_at: DateTime<Utc>,
    /// IP address of the submitter.
    pub ip_address: Option<String>,
    /// User-Agent header of the submitter.
    pub user_agent: Option<String>,
    /// Current status of the submission.
    pub status: SubmissionStatus,
}

impl FormSubmission {
    /// Create a new submission.
    pub fn new(
        form_id: impl Into<String>,
        data: HashMap<String, String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            form_id: form_id.into(),
            data,
            submitted_at: Utc::now(),
            ip_address,
            user_agent,
            status: SubmissionStatus::New,
        }
    }
}

/// Thread-safe in-memory submission store.
#[derive(Debug, Clone)]
pub struct SubmissionStore {
    submissions: Arc<Mutex<Vec<FormSubmission>>>,
}

impl SubmissionStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            submissions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Save a submission to the store. Returns the submission ID.
    pub fn save(&self, submission: FormSubmission) -> Uuid {
        let id = submission.id;
        let mut store = self.submissions.lock().unwrap();
        store.push(submission);
        tracing::info!("Saved form submission {}", id);
        id
    }

    /// List all submissions for a given form ID, ordered by most recent first.
    pub fn list_by_form(&self, form_id: &str) -> Vec<FormSubmission> {
        let store = self.submissions.lock().unwrap();
        let mut results: Vec<FormSubmission> = store
            .iter()
            .filter(|s| s.form_id == form_id)
            .cloned()
            .collect();
        results.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));
        results
    }

    /// Get a submission by its ID.
    pub fn get_by_id(&self, id: Uuid) -> Option<FormSubmission> {
        let store = self.submissions.lock().unwrap();
        store.iter().find(|s| s.id == id).cloned()
    }

    /// Delete a submission by its ID. Returns `true` if found and removed.
    pub fn delete(&self, id: Uuid) -> bool {
        let mut store = self.submissions.lock().unwrap();
        let len_before = store.len();
        store.retain(|s| s.id != id);
        let removed = store.len() < len_before;
        if removed {
            tracing::info!("Deleted form submission {}", id);
        }
        removed
    }

    /// Update the status of a submission. Returns `true` if found and updated.
    pub fn update_status(&self, id: Uuid, status: SubmissionStatus) -> bool {
        let mut store = self.submissions.lock().unwrap();
        if let Some(sub) = store.iter_mut().find(|s| s.id == id) {
            sub.status = status;
            true
        } else {
            false
        }
    }

    /// Count submissions for a given form ID, optionally filtered by status.
    pub fn count(&self, form_id: &str, status: Option<&SubmissionStatus>) -> usize {
        let store = self.submissions.lock().unwrap();
        store
            .iter()
            .filter(|s| {
                s.form_id == form_id
                    && status.map_or(true, |st| &s.status == st)
            })
            .count()
    }
}

impl Default for SubmissionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_submission(form_id: &str) -> FormSubmission {
        let mut data = HashMap::new();
        data.insert("name".into(), "Alice".into());
        data.insert("email".into(), "alice@example.com".into());
        FormSubmission::new(
            form_id,
            data,
            Some("127.0.0.1".into()),
            Some("TestAgent/1.0".into()),
        )
    }

    #[test]
    fn test_save_and_get() {
        let store = SubmissionStore::new();
        let sub = make_submission("contact");
        let id = store.save(sub.clone());

        let retrieved = store.get_by_id(id).unwrap();
        assert_eq!(retrieved.form_id, "contact");
        assert_eq!(retrieved.data.get("name").unwrap(), "Alice");
        assert_eq!(retrieved.status, SubmissionStatus::New);
        assert_eq!(retrieved.ip_address, Some("127.0.0.1".to_string()));
    }

    #[test]
    fn test_list_by_form() {
        let store = SubmissionStore::new();
        store.save(make_submission("contact"));
        store.save(make_submission("contact"));
        store.save(make_submission("feedback"));

        let contact_subs = store.list_by_form("contact");
        assert_eq!(contact_subs.len(), 2);

        let feedback_subs = store.list_by_form("feedback");
        assert_eq!(feedback_subs.len(), 1);

        let empty = store.list_by_form("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_delete() {
        let store = SubmissionStore::new();
        let sub = make_submission("contact");
        let id = store.save(sub);

        assert!(store.get_by_id(id).is_some());
        assert!(store.delete(id));
        assert!(store.get_by_id(id).is_none());
        // Deleting again returns false
        assert!(!store.delete(id));
    }

    #[test]
    fn test_update_status() {
        let store = SubmissionStore::new();
        let sub = make_submission("contact");
        let id = store.save(sub);

        assert!(store.update_status(id, SubmissionStatus::Read));
        let updated = store.get_by_id(id).unwrap();
        assert_eq!(updated.status, SubmissionStatus::Read);

        assert!(store.update_status(id, SubmissionStatus::Spam));
        let updated = store.get_by_id(id).unwrap();
        assert_eq!(updated.status, SubmissionStatus::Spam);

        // Non-existent ID
        assert!(!store.update_status(Uuid::new_v4(), SubmissionStatus::Trash));
    }

    #[test]
    fn test_count() {
        let store = SubmissionStore::new();
        let id1 = store.save(make_submission("contact"));
        store.save(make_submission("contact"));
        store.save(make_submission("feedback"));

        assert_eq!(store.count("contact", None), 2);
        assert_eq!(store.count("contact", Some(&SubmissionStatus::New)), 2);

        store.update_status(id1, SubmissionStatus::Read);
        assert_eq!(store.count("contact", Some(&SubmissionStatus::New)), 1);
        assert_eq!(store.count("contact", Some(&SubmissionStatus::Read)), 1);
        assert_eq!(store.count("feedback", None), 1);
    }

    #[test]
    fn test_submission_new_defaults() {
        let sub = FormSubmission::new("test", HashMap::new(), None, None);
        assert_eq!(sub.status, SubmissionStatus::New);
        assert!(sub.ip_address.is_none());
        assert!(sub.user_agent.is_none());
        assert!(sub.data.is_empty());
        assert!(!sub.id.is_nil());
    }
}

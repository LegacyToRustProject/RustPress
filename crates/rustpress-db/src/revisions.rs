use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use tracing::debug;

use crate::entities::wp_posts;

/// WordPress post revision management.
///
/// Corresponds to `wp-includes/revision.php`:
/// - `wp_save_post_revision()` — saves a revision snapshot
/// - `wp_get_post_revisions()` — retrieves revisions for a post
/// - `wp_restore_post_revision()` — restores a post from a revision
///
/// Revisions are stored as posts with `post_type = 'revision'` and
/// `post_parent` pointing to the original post.
pub struct RevisionManager;

impl RevisionManager {
    /// Save a revision of the given post.
    ///
    /// Creates a new `wp_posts` row with `post_type = 'revision'`
    /// and `post_parent = post_id`.
    pub async fn save_revision(
        db: &DatabaseConnection,
        post: &wp_posts::Model,
    ) -> Result<wp_posts::Model, sea_orm::DbErr> {
        let now = Utc::now().naive_utc();

        let revision = wp_posts::ActiveModel {
            id: ActiveValue::NotSet,
            post_author: ActiveValue::Set(post.post_author),
            post_date: ActiveValue::Set(now),
            post_date_gmt: ActiveValue::Set(now),
            post_content: ActiveValue::Set(post.post_content.clone()),
            post_title: ActiveValue::Set(format!("{} — Revision", post.post_title)),
            post_excerpt: ActiveValue::Set(post.post_excerpt.clone()),
            post_status: ActiveValue::Set("inherit".to_string()),
            comment_status: ActiveValue::Set(post.comment_status.clone()),
            ping_status: ActiveValue::Set(post.ping_status.clone()),
            post_password: ActiveValue::Set(post.post_password.clone()),
            post_name: ActiveValue::Set(format!("{}-revision-v1", post.id)),
            to_ping: ActiveValue::Set(String::new()),
            pinged: ActiveValue::Set(String::new()),
            post_modified: ActiveValue::Set(now),
            post_modified_gmt: ActiveValue::Set(now),
            post_content_filtered: ActiveValue::Set(String::new()),
            post_parent: ActiveValue::Set(post.id),
            guid: ActiveValue::Set(String::new()),
            menu_order: ActiveValue::Set(0),
            post_type: ActiveValue::Set("revision".to_string()),
            post_mime_type: ActiveValue::Set(String::new()),
            comment_count: ActiveValue::Set(0),
        };

        let result = revision.insert(db).await?;
        debug!(
            post_id = post.id,
            revision_id = result.id,
            "post revision saved"
        );
        Ok(result)
    }

    /// Get all revisions for a post, ordered newest first.
    pub async fn get_revisions(
        db: &DatabaseConnection,
        post_id: u64,
    ) -> Result<Vec<wp_posts::Model>, sea_orm::DbErr> {
        let revisions = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostParent.eq(post_id))
            .filter(wp_posts::Column::PostType.eq("revision"))
            .order_by_desc(wp_posts::Column::PostDate)
            .all(db)
            .await?;
        debug!(post_id, count = revisions.len(), "revisions fetched");
        Ok(revisions)
    }

    /// Restore a post from a specific revision.
    ///
    /// Copies the revision's content/title/excerpt back to the parent post.
    pub async fn restore_revision(
        db: &DatabaseConnection,
        revision_id: u64,
    ) -> Result<Option<wp_posts::Model>, sea_orm::DbErr> {
        let Some(revision) = wp_posts::Entity::find_by_id(revision_id).one(db).await? else {
            return Ok(None);
        };

        if revision.post_type != "revision" {
            return Ok(None);
        }

        let now = Utc::now().naive_utc();
        let parent_id = revision.post_parent;

        let Some(parent) = wp_posts::Entity::find_by_id(parent_id).one(db).await? else {
            return Ok(None);
        };

        // Save current state as a revision before restoring
        Self::save_revision(db, &parent).await?;

        // Update the parent post with revision content
        let mut active: wp_posts::ActiveModel = parent.into();
        active.post_content = ActiveValue::Set(revision.post_content);
        active.post_title = ActiveValue::Set(revision.post_title.replace(" — Revision", ""));
        active.post_excerpt = ActiveValue::Set(revision.post_excerpt);
        active.post_modified = ActiveValue::Set(now);
        active.post_modified_gmt = ActiveValue::Set(now);

        let updated = active.update(db).await?;
        debug!(
            revision_id,
            post_id = updated.id,
            "post restored from revision"
        );
        Ok(Some(updated))
    }

    /// Delete all revisions for a post.
    pub async fn delete_revisions(
        db: &DatabaseConnection,
        post_id: u64,
    ) -> Result<u64, sea_orm::DbErr> {
        let result = wp_posts::Entity::delete_many()
            .filter(wp_posts::Column::PostParent.eq(post_id))
            .filter(wp_posts::Column::PostType.eq("revision"))
            .exec(db)
            .await?;
        debug!(post_id, count = result.rows_affected, "revisions deleted");
        Ok(result.rows_affected)
    }

    /// Count revisions for a post.
    pub async fn count_revisions(
        db: &DatabaseConnection,
        post_id: u64,
    ) -> Result<u64, sea_orm::DbErr> {
        use sea_orm::PaginatorTrait;
        let count = wp_posts::Entity::find()
            .filter(wp_posts::Column::PostParent.eq(post_id))
            .filter(wp_posts::Column::PostType.eq("revision"))
            .count(db)
            .await?;
        Ok(count)
    }

    /// Build a revision title from the original post title.
    ///
    /// WordPress appends " — Revision" to the original title.
    pub fn revision_title(original_title: &str) -> String {
        format!("{original_title} — Revision")
    }

    /// Build a revision post_name (slug) from the original post ID.
    ///
    /// WordPress uses the pattern `{post_id}-revision-v1`.
    pub fn revision_post_name(post_id: u64) -> String {
        format!("{post_id}-revision-v1")
    }

    /// Strip the revision suffix from a revision title to recover the original.
    ///
    /// This reverses `revision_title()`.
    pub fn strip_revision_suffix(revision_title: &str) -> String {
        revision_title.replace(" — Revision", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revision_title_format() {
        assert_eq!(
            RevisionManager::revision_title("Hello World"),
            "Hello World — Revision"
        );
    }

    #[test]
    fn test_revision_title_empty() {
        assert_eq!(RevisionManager::revision_title(""), " — Revision");
    }

    #[test]
    fn test_revision_title_with_special_chars() {
        assert_eq!(
            RevisionManager::revision_title("Post with <HTML> & \"quotes\""),
            "Post with <HTML> & \"quotes\" — Revision"
        );
    }

    #[test]
    fn test_revision_post_name() {
        assert_eq!(RevisionManager::revision_post_name(42), "42-revision-v1");
    }

    #[test]
    fn test_revision_post_name_zero() {
        assert_eq!(RevisionManager::revision_post_name(0), "0-revision-v1");
    }

    #[test]
    fn test_revision_post_name_large_id() {
        assert_eq!(
            RevisionManager::revision_post_name(999999),
            "999999-revision-v1"
        );
    }

    #[test]
    fn test_strip_revision_suffix() {
        assert_eq!(
            RevisionManager::strip_revision_suffix("Hello World — Revision"),
            "Hello World"
        );
    }

    #[test]
    fn test_strip_revision_suffix_no_match() {
        // If there's no suffix, the title is returned unchanged.
        assert_eq!(
            RevisionManager::strip_revision_suffix("Hello World"),
            "Hello World"
        );
    }

    #[test]
    fn test_strip_revision_suffix_multiple() {
        // `.replace()` strips all occurrences, matching the behavior in
        // `restore_revision`.
        assert_eq!(
            RevisionManager::strip_revision_suffix("A — Revision — Revision"),
            "A"
        );
    }

    #[test]
    fn test_roundtrip_title() {
        let original = "My Great Post";
        let rev_title = RevisionManager::revision_title(original);
        let restored = RevisionManager::strip_revision_suffix(&rev_title);
        assert_eq!(restored, original);
    }
}

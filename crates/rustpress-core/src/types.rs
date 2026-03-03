use serde::{Deserialize, Serialize};

/// WordPress post status values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostStatus {
    Publish,
    Draft,
    Pending,
    Private,
    Trash,
    AutoDraft,
    Inherit,
    Future,
}

impl PostStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Draft => "draft",
            Self::Pending => "pending",
            Self::Private => "private",
            Self::Trash => "trash",
            Self::AutoDraft => "auto-draft",
            Self::Inherit => "inherit",
            Self::Future => "future",
        }
    }
}

/// WordPress post type values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostType {
    Post,
    Page,
    Attachment,
    Revision,
    NavMenuItem,
    CustomCss,
    Changeset,
}

impl PostType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::Page => "page",
            Self::Attachment => "attachment",
            Self::Revision => "revision",
            Self::NavMenuItem => "nav_menu_item",
            Self::CustomCss => "custom_css",
            Self::Changeset => "customize_changeset",
        }
    }
}

/// WordPress comment status values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    Open,
    Closed,
}

impl CommentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

/// WordPress user role values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Administrator,
    Editor,
    Author,
    Contributor,
    Subscriber,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Administrator => "administrator",
            Self::Editor => "editor",
            Self::Author => "author",
            Self::Contributor => "contributor",
            Self::Subscriber => "subscriber",
        }
    }
}

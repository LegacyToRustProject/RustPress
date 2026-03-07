use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// WordPress user roles with associated capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Administrator,
    Editor,
    Author,
    Contributor,
    Subscriber,
}

/// WordPress capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    // Post capabilities
    EditPosts,
    EditOthersPosts,
    EditPublishedPosts,
    PublishPosts,
    DeletePosts,
    DeleteOthersPosts,
    DeletePublishedPosts,
    DeletePrivatePosts,
    EditPrivatePosts,
    ReadPrivatePosts,

    // Page capabilities
    EditPages,
    EditOthersPages,
    EditPublishedPages,
    PublishPages,
    DeletePages,
    DeleteOthersPages,
    DeletePublishedPages,
    DeletePrivatePages,
    EditPrivatePages,
    ReadPrivatePages,

    // User capabilities
    ListUsers,
    CreateUsers,
    EditUsers,
    DeleteUsers,
    PromoteUsers,

    // Theme capabilities
    EditThemeOptions,
    SwitchThemes,
    EditThemes,
    DeleteThemes,
    InstallThemes,

    // Plugin capabilities
    ActivatePlugins,
    EditPlugins,
    InstallPlugins,
    DeletePlugins,

    // Site capabilities
    ManageOptions,
    ManageLinks,
    ManageCategories,
    ModerateComments,
    UploadFiles,
    Import,
    Export,
    Unfiltered,
    Read,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Administrator => "administrator",
            Self::Editor => "editor",
            Self::Author => "author",
            Self::Contributor => "contributor",
            Self::Subscriber => "subscriber",
        }
    }

    /// Parse a role from a string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "administrator" => Some(Self::Administrator),
            "editor" => Some(Self::Editor),
            "author" => Some(Self::Author),
            "contributor" => Some(Self::Contributor),
            "subscriber" => Some(Self::Subscriber),
            _ => None,
        }
    }

    /// Get the set of capabilities for this role.
    pub fn capabilities(&self) -> HashSet<Capability> {
        use Capability::*;
        match self {
            Role::Administrator => HashSet::from([
                EditPosts,
                EditOthersPosts,
                EditPublishedPosts,
                PublishPosts,
                DeletePosts,
                DeleteOthersPosts,
                DeletePublishedPosts,
                DeletePrivatePosts,
                EditPrivatePosts,
                ReadPrivatePosts,
                EditPages,
                EditOthersPages,
                EditPublishedPages,
                PublishPages,
                DeletePages,
                DeleteOthersPages,
                DeletePublishedPages,
                DeletePrivatePages,
                EditPrivatePages,
                ReadPrivatePages,
                ListUsers,
                CreateUsers,
                EditUsers,
                DeleteUsers,
                PromoteUsers,
                EditThemeOptions,
                SwitchThemes,
                EditThemes,
                DeleteThemes,
                InstallThemes,
                ActivatePlugins,
                EditPlugins,
                InstallPlugins,
                DeletePlugins,
                ManageOptions,
                ManageLinks,
                ManageCategories,
                ModerateComments,
                UploadFiles,
                Import,
                Export,
                Unfiltered,
                Read,
            ]),
            Role::Editor => HashSet::from([
                EditPosts,
                EditOthersPosts,
                EditPublishedPosts,
                PublishPosts,
                DeletePosts,
                DeleteOthersPosts,
                DeletePublishedPosts,
                DeletePrivatePosts,
                EditPrivatePosts,
                ReadPrivatePosts,
                EditPages,
                EditOthersPages,
                EditPublishedPages,
                PublishPages,
                DeletePages,
                DeleteOthersPages,
                DeletePublishedPages,
                DeletePrivatePages,
                EditPrivatePages,
                ReadPrivatePages,
                ManageLinks,
                ManageCategories,
                ModerateComments,
                UploadFiles,
                Unfiltered,
                Read,
            ]),
            Role::Author => HashSet::from([
                EditPosts,
                EditPublishedPosts,
                PublishPosts,
                DeletePosts,
                DeletePublishedPosts,
                UploadFiles,
                Read,
            ]),
            Role::Contributor => HashSet::from([EditPosts, DeletePosts, Read]),
            Role::Subscriber => HashSet::from([Read]),
        }
    }

    /// Check if this role has a specific capability.
    pub fn can(&self, capability: &Capability) -> bool {
        self.capabilities().contains(capability)
    }
}

/// Check if a user (identified by role string) has a capability.
pub fn current_user_can(role_str: &str, capability: &Capability) -> bool {
    Role::from_str(role_str)
        .map(|r| r.can(capability))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_caps() {
        let admin = Role::Administrator;
        assert!(admin.can(&Capability::ManageOptions));
        assert!(admin.can(&Capability::EditPosts));
        assert!(admin.can(&Capability::DeleteUsers));
    }

    #[test]
    fn test_subscriber_limited() {
        let sub = Role::Subscriber;
        assert!(sub.can(&Capability::Read));
        assert!(!sub.can(&Capability::EditPosts));
        assert!(!sub.can(&Capability::ManageOptions));
    }

    #[test]
    fn test_current_user_can() {
        assert!(current_user_can(
            "administrator",
            &Capability::ManageOptions
        ));
        assert!(!current_user_can("subscriber", &Capability::EditPosts));
        assert!(!current_user_can("unknown_role", &Capability::Read));
    }
}

//! Field group management for the RustPress custom fields system.
//!
//! Field groups organize multiple field definitions together and
//! control where they appear (e.g., on specific post types, page
//! templates, or user roles).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

use crate::field_types::FieldDefinition;

/// The position where a field group is rendered in the edit screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Position {
    /// Below the content editor.
    #[default]
    Normal,
    /// In the sidebar.
    Side,
    /// Immediately after the title field.
    AcfAfterTitle,
}

/// Visual style of the field group metabox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Style {
    /// Standard metabox with a border.
    #[default]
    Default,
    /// No border, blends into the page.
    Seamless,
}

/// The parameter used to determine where a field group is displayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationParam {
    PostType,
    PageTemplate,
    PageType,
    UserRole,
    Taxonomy,
    PostStatus,
    PostFormat,
    PostCategory,
    AttachmentType,
}

/// The comparison operator for a location rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationOperator {
    Equals,
    NotEquals,
}

/// A single location rule that determines whether a field group
/// should be displayed on a given screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocationRule {
    /// What parameter to evaluate.
    pub param: LocationParam,
    /// How to compare.
    pub operator: LocationOperator,
    /// The value to compare against.
    pub value: String,
}

impl LocationRule {
    /// Creates a new location rule.
    pub fn new(param: LocationParam, operator: LocationOperator, value: &str) -> Self {
        Self {
            param,
            operator,
            value: value.to_string(),
        }
    }

    /// Checks if this rule matches the given context.
    pub fn matches(&self, param: &LocationParam, value: &str) -> bool {
        if self.param != *param {
            return false;
        }
        match self.operator {
            LocationOperator::Equals => self.value == value,
            LocationOperator::NotEquals => self.value != value,
        }
    }
}

/// A group of related field definitions, along with rules
/// controlling where the group is displayed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldGroup {
    /// Unique key for this group (e.g., "group_hero_section").
    pub key: String,
    /// Human-readable title displayed as the metabox heading.
    pub title: String,
    /// The fields contained in this group.
    pub fields: Vec<FieldDefinition>,
    /// Location rules that determine where this group is shown.
    /// Outer Vec is OR (any group can match), inner Vec is AND (all rules must match).
    pub location_rules: Vec<Vec<LocationRule>>,
    /// Where the metabox is placed on the edit screen.
    pub position: Position,
    /// Visual style of the metabox.
    pub style: Style,
    /// Controls the ordering of field groups; lower numbers appear first.
    pub menu_order: i32,
}

impl FieldGroup {
    /// Creates a new field group with the given key and title.
    pub fn new(key: &str, title: &str) -> Self {
        Self {
            key: key.to_string(),
            title: title.to_string(),
            fields: Vec::new(),
            location_rules: Vec::new(),
            position: Position::default(),
            style: Style::default(),
            menu_order: 0,
        }
    }

    /// Adds a field to this group.
    pub fn add_field(mut self, field: FieldDefinition) -> Self {
        self.fields.push(field);
        self
    }

    /// Adds a location rule group (AND group). Multiple calls create OR groups.
    pub fn add_location(mut self, rules: Vec<LocationRule>) -> Self {
        self.location_rules.push(rules);
        self
    }

    /// Sets the position of this field group.
    pub fn with_position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    /// Sets the style of this field group.
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Sets the menu order.
    pub fn with_menu_order(mut self, order: i32) -> Self {
        self.menu_order = order;
        self
    }

    /// Tests whether this field group should be shown for a given post type.
    pub fn matches_post_type(&self, post_type: &str) -> bool {
        if self.location_rules.is_empty() {
            return true;
        }
        self.location_rules.iter().any(|and_group| {
            and_group
                .iter()
                .all(|rule| rule.matches(&LocationParam::PostType, post_type))
        })
    }

    /// Tests whether this field group matches a given screen context.
    /// Context is a list of (param, value) pairs that describe the current screen.
    pub fn matches_screen(&self, context: &[(LocationParam, String)]) -> bool {
        if self.location_rules.is_empty() {
            return true;
        }
        self.location_rules.iter().any(|and_group| {
            and_group.iter().all(|rule| {
                context
                    .iter()
                    .any(|(param, value)| rule.matches(param, value))
            })
        })
    }
}

/// Registry that holds all registered field groups and provides
/// lookup methods.
#[derive(Debug, Default)]
pub struct FieldGroupRegistry {
    groups: HashMap<String, FieldGroup>,
}

impl FieldGroupRegistry {
    /// Creates a new, empty registry.
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Registers a field group. If a group with the same key already
    /// exists, it is replaced.
    pub fn register_group(&mut self, group: FieldGroup) {
        debug!(key = %group.key, title = %group.title, "Registering field group");
        self.groups.insert(group.key.clone(), group);
    }

    /// Returns a reference to a field group by key.
    pub fn get_group(&self, key: &str) -> Option<&FieldGroup> {
        self.groups.get(key)
    }

    /// Returns all field groups that should appear for a given post type,
    /// sorted by menu_order.
    pub fn get_groups_for_post_type(&self, post_type: &str) -> Vec<&FieldGroup> {
        let mut groups: Vec<&FieldGroup> = self
            .groups
            .values()
            .filter(|g| g.matches_post_type(post_type))
            .collect();
        groups.sort_by_key(|g| g.menu_order);
        groups
    }

    /// Returns all field groups that match a given screen context,
    /// sorted by menu_order.
    pub fn get_groups_for_screen(&self, context: &[(LocationParam, String)]) -> Vec<&FieldGroup> {
        let mut groups: Vec<&FieldGroup> = self
            .groups
            .values()
            .filter(|g| g.matches_screen(context))
            .collect();
        groups.sort_by_key(|g| g.menu_order);
        groups
    }

    /// Returns the total number of registered field groups.
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Returns `true` if no groups are registered.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// Returns all registered field groups, sorted by menu_order.
    pub fn all_groups(&self) -> Vec<&FieldGroup> {
        let mut groups: Vec<&FieldGroup> = self.groups.values().collect();
        groups.sort_by_key(|g| g.menu_order);
        groups
    }

    /// Removes a field group by key, returning it if it existed.
    pub fn unregister_group(&mut self, key: &str) -> Option<FieldGroup> {
        self.groups.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_types::FieldType;

    fn sample_group(key: &str, post_type: &str) -> FieldGroup {
        FieldGroup::new(key, "Test Group")
            .add_field(FieldDefinition::new(
                "field_1",
                "Title",
                "title",
                FieldType::Text,
            ))
            .add_location(vec![LocationRule::new(
                LocationParam::PostType,
                LocationOperator::Equals,
                post_type,
            )])
    }

    #[test]
    fn test_field_group_matches_post_type() {
        let group = sample_group("group_1", "post");
        assert!(group.matches_post_type("post"));
        assert!(!group.matches_post_type("page"));
    }

    #[test]
    fn test_field_group_no_rules_matches_all() {
        let group = FieldGroup::new("group_any", "Catch All");
        assert!(group.matches_post_type("post"));
        assert!(group.matches_post_type("page"));
    }

    #[test]
    fn test_registry_get_groups_for_post_type() {
        let mut registry = FieldGroupRegistry::new();
        registry.register_group(sample_group("group_posts", "post").with_menu_order(2));
        registry.register_group(sample_group("group_pages", "page").with_menu_order(1));
        registry.register_group(sample_group("group_posts_2", "post").with_menu_order(0));

        let post_groups = registry.get_groups_for_post_type("post");
        assert_eq!(post_groups.len(), 2);
        // Verify sorted by menu_order
        assert_eq!(post_groups[0].key, "group_posts_2");
        assert_eq!(post_groups[1].key, "group_posts");
    }

    #[test]
    fn test_registry_register_and_unregister() {
        let mut registry = FieldGroupRegistry::new();
        assert!(registry.is_empty());

        registry.register_group(sample_group("g1", "post"));
        assert_eq!(registry.len(), 1);

        let removed = registry.unregister_group("g1");
        assert!(removed.is_some());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_location_rule_not_equals() {
        let rule = LocationRule::new(
            LocationParam::PostType,
            LocationOperator::NotEquals,
            "attachment",
        );
        assert!(rule.matches(&LocationParam::PostType, "post"));
        assert!(rule.matches(&LocationParam::PostType, "page"));
        assert!(!rule.matches(&LocationParam::PostType, "attachment"));
    }

    #[test]
    fn test_field_group_builder() {
        let group = FieldGroup::new("group_hero", "Hero Section")
            .add_field(
                FieldDefinition::new("field_bg", "Background", "background", FieldType::Image)
                    .required(),
            )
            .add_field(FieldDefinition::new(
                "field_heading",
                "Heading",
                "heading",
                FieldType::Text,
            ))
            .with_position(Position::AcfAfterTitle)
            .with_style(Style::Seamless)
            .with_menu_order(0);

        assert_eq!(group.fields.len(), 2);
        assert_eq!(group.position, Position::AcfAfterTitle);
        assert_eq!(group.style, Style::Seamless);
    }

    #[test]
    fn test_matches_screen_context() {
        let group = FieldGroup::new("group_ctx", "Context Group").add_location(vec![
            LocationRule::new(LocationParam::PostType, LocationOperator::Equals, "page"),
            LocationRule::new(
                LocationParam::PageTemplate,
                LocationOperator::Equals,
                "template-landing.php",
            ),
        ]);

        let ctx = vec![
            (LocationParam::PostType, "page".to_string()),
            (
                LocationParam::PageTemplate,
                "template-landing.php".to_string(),
            ),
        ];
        assert!(group.matches_screen(&ctx));

        let ctx_wrong = vec![
            (LocationParam::PostType, "post".to_string()),
            (
                LocationParam::PageTemplate,
                "template-landing.php".to_string(),
            ),
        ];
        assert!(!group.matches_screen(&ctx_wrong));
    }
}

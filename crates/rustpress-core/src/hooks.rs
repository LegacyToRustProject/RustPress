use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tracing::trace;

/// Callback type for actions (side effects, no return value).
pub type ActionCallback = Arc<dyn Fn(&Value) + Send + Sync>;

/// Callback type for filters (transforms a value).
pub type FilterCallback = Arc<dyn Fn(Value) -> Value + Send + Sync>;

/// Default priority for hooks (matches WordPress default).
pub const DEFAULT_PRIORITY: i32 = 10;

struct ActionEntry {
    callback: ActionCallback,
    priority: i32,
}

struct FilterEntry {
    callback: FilterCallback,
    priority: i32,
}

/// WordPress-compatible hook registry.
///
/// Provides `add_action`/`do_action` for side effects and
/// `add_filter`/`apply_filters` for value transformation.
///
/// Uses `serde_json::Value` as the universal argument type to mirror
/// WordPress's PHP dynamic typing. Priority ordering matches WordPress
/// behavior (lower number = earlier execution).
#[derive(Clone, Default)]
pub struct HookRegistry {
    actions: Arc<RwLock<BTreeMap<String, Vec<ActionEntry>>>>,
    filters: Arc<RwLock<BTreeMap<String, Vec<FilterEntry>>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an action callback for the given tag.
    ///
    /// Equivalent to WordPress `add_action($tag, $callback, $priority)`.
    pub fn add_action(&self, tag: &str, callback: ActionCallback, priority: i32) {
        let mut actions = self.actions.write().expect("hook lock poisoned");
        let entries = actions.entry(tag.to_string()).or_default();
        entries.push(ActionEntry { callback, priority });
        entries.sort_by_key(|e| e.priority);
        trace!(tag, priority, "action registered");
    }

    /// Execute all action callbacks registered for the given tag.
    ///
    /// Equivalent to WordPress `do_action($tag, $args...)`.
    pub fn do_action(&self, tag: &str, args: &Value) {
        let actions = self.actions.read().expect("hook lock poisoned");
        if let Some(entries) = actions.get(tag) {
            trace!(tag, count = entries.len(), "executing actions");
            for entry in entries {
                (entry.callback)(args);
            }
        }
    }

    /// Register a filter callback for the given tag.
    ///
    /// Equivalent to WordPress `add_filter($tag, $callback, $priority)`.
    pub fn add_filter(&self, tag: &str, callback: FilterCallback, priority: i32) {
        let mut filters = self.filters.write().expect("hook lock poisoned");
        let entries = filters.entry(tag.to_string()).or_default();
        entries.push(FilterEntry { callback, priority });
        entries.sort_by_key(|e| e.priority);
        trace!(tag, priority, "filter registered");
    }

    /// Apply all filter callbacks registered for the given tag to the value.
    ///
    /// Each filter receives the output of the previous filter (pipeline).
    /// Equivalent to WordPress `apply_filters($tag, $value, $args...)`.
    pub fn apply_filters(&self, tag: &str, value: Value) -> Value {
        let filters = self.filters.read().expect("hook lock poisoned");
        if let Some(entries) = filters.get(tag) {
            trace!(tag, count = entries.len(), "applying filters");
            let mut result = value;
            for entry in entries {
                result = (entry.callback)(result);
            }
            result
        } else {
            value
        }
    }

    /// Check if any actions are registered for the given tag.
    pub fn has_action(&self, tag: &str) -> bool {
        let actions = self.actions.read().expect("hook lock poisoned");
        actions.get(tag).is_some_and(|e| !e.is_empty())
    }

    /// Check if any filters are registered for the given tag.
    pub fn has_filter(&self, tag: &str) -> bool {
        let filters = self.filters.read().expect("hook lock poisoned");
        filters.get(tag).is_some_and(|e| !e.is_empty())
    }

    /// Remove all actions for the given tag.
    pub fn remove_all_actions(&self, tag: &str) {
        let mut actions = self.actions.write().expect("hook lock poisoned");
        actions.remove(tag);
    }

    /// Remove all filters for the given tag.
    pub fn remove_all_filters(&self, tag: &str) {
        let mut filters = self.filters.write().expect("hook lock poisoned");
        filters.remove(tag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_action_basic() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c = counter.clone();
        registry.add_action(
            "init",
            Arc::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            }),
            DEFAULT_PRIORITY,
        );

        registry.do_action("init", &json!({}));
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        registry.do_action("init", &json!({}));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_action_priority_order() {
        let registry = HookRegistry::new();
        let order = Arc::new(RwLock::new(Vec::new()));

        let o = order.clone();
        registry.add_action(
            "init",
            Arc::new(move |_| {
                o.write().unwrap().push(20);
            }),
            20,
        );

        let o = order.clone();
        registry.add_action(
            "init",
            Arc::new(move |_| {
                o.write().unwrap().push(5);
            }),
            5,
        );

        let o = order.clone();
        registry.add_action(
            "init",
            Arc::new(move |_| {
                o.write().unwrap().push(10);
            }),
            10,
        );

        registry.do_action("init", &json!({}));
        let result = order.read().unwrap().clone();
        assert_eq!(result, vec![5, 10, 20]);
    }

    #[test]
    fn test_filter_basic() {
        let registry = HookRegistry::new();

        registry.add_filter(
            "the_title",
            Arc::new(|value| {
                if let Value::String(s) = value {
                    Value::String(format!("{} - Modified", s))
                } else {
                    value
                }
            }),
            DEFAULT_PRIORITY,
        );

        let result = registry.apply_filters("the_title", json!("Hello World"));
        assert_eq!(result, json!("Hello World - Modified"));
    }

    #[test]
    fn test_filter_pipeline() {
        let registry = HookRegistry::new();

        registry.add_filter(
            "the_content",
            Arc::new(|value| {
                if let Value::String(s) = value {
                    Value::String(format!("<p>{}</p>", s))
                } else {
                    value
                }
            }),
            10,
        );

        registry.add_filter(
            "the_content",
            Arc::new(|value| {
                if let Value::String(s) = value {
                    Value::String(format!("<div>{}</div>", s))
                } else {
                    value
                }
            }),
            20,
        );

        let result = registry.apply_filters("the_content", json!("content"));
        assert_eq!(result, json!("<div><p>content</p></div>"));
    }

    #[test]
    fn test_has_action_and_filter() {
        let registry = HookRegistry::new();
        assert!(!registry.has_action("init"));
        assert!(!registry.has_filter("the_title"));

        registry.add_action("init", Arc::new(|_| {}), DEFAULT_PRIORITY);
        registry.add_filter("the_title", Arc::new(|v| v), DEFAULT_PRIORITY);

        assert!(registry.has_action("init"));
        assert!(registry.has_filter("the_title"));
    }

    #[test]
    fn test_remove_all() {
        let registry = HookRegistry::new();
        registry.add_action("init", Arc::new(|_| {}), DEFAULT_PRIORITY);
        registry.add_filter("the_title", Arc::new(|v| v), DEFAULT_PRIORITY);

        registry.remove_all_actions("init");
        registry.remove_all_filters("the_title");

        assert!(!registry.has_action("init"));
        assert!(!registry.has_filter("the_title"));
    }

    #[test]
    fn test_do_action_nonexistent_tag() {
        let registry = HookRegistry::new();
        // Should not panic
        registry.do_action("nonexistent", &json!({}));
    }

    #[test]
    fn test_apply_filters_nonexistent_tag() {
        let registry = HookRegistry::new();
        let result = registry.apply_filters("nonexistent", json!("original"));
        assert_eq!(result, json!("original"));
    }
}

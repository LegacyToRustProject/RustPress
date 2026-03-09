use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;
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

// Thread-local storage for the currently executing filter tag.
thread_local! {
    static CURRENT_FILTER: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
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
    /// Counter tracking how many times each action has been fired.
    action_counts: Arc<RwLock<HashMap<String, usize>>>,
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
    ///
    /// Callbacks are cloned out of the registry before the read lock is
    /// released, so callbacks may safely call `add_action`/`add_filter`
    /// (write-locks) without deadlocking — matching WordPress behaviour where
    /// hooks registered during `do_action` are queued for the same tag.
    pub fn do_action(&self, tag: &str, args: &Value) {
        // Increment action fire counter
        {
            let mut counts = self.action_counts.write().expect("hook lock poisoned");
            *counts.entry(tag.to_string()).or_insert(0) += 1;
        }

        // Clone callbacks while holding the read lock, then release it before
        // invoking them. This allows callbacks to call add_action/add_filter
        // (which need a write lock) without deadlocking.
        let callbacks: Vec<ActionCallback> = {
            let actions = self.actions.read().expect("hook lock poisoned");
            match actions.get(tag) {
                Some(entries) => {
                    trace!(tag, count = entries.len(), "executing actions");
                    entries.iter().map(|e| e.callback.clone()).collect()
                }
                None => return,
            }
        };

        for cb in callbacks {
            cb(args);
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
    ///
    /// Callbacks are cloned out of the registry before the read lock is
    /// released, so callbacks may safely call `add_filter`/`add_action`
    /// without deadlocking.
    pub fn apply_filters(&self, tag: &str, value: Value) -> Value {
        // Save previous filter and set current
        let previous = CURRENT_FILTER.with(|cf| cf.borrow().clone());
        CURRENT_FILTER.with(|cf| *cf.borrow_mut() = Some(tag.to_string()));

        // Clone callbacks while holding the read lock, then release before
        // running the pipeline so nested filter/action registration is safe.
        let callbacks: Vec<FilterCallback> = {
            let filters = self.filters.read().expect("hook lock poisoned");
            match filters.get(tag) {
                Some(entries) => {
                    trace!(tag, count = entries.len(), "applying filters");
                    entries.iter().map(|e| e.callback.clone()).collect()
                }
                None => {
                    CURRENT_FILTER.with(|cf| *cf.borrow_mut() = previous);
                    return value;
                }
            }
        };

        let mut result = value;
        for cb in callbacks {
            result = cb(result);
        }

        // Restore previous filter
        CURRENT_FILTER.with(|cf| *cf.borrow_mut() = previous);

        result
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

    /// Register an action callback with DEFAULT_PRIORITY (10).
    ///
    /// Convenience wrapper around `add_action` for the common case.
    pub fn add_action_default(&self, tag: &str, callback: ActionCallback) {
        self.add_action(tag, callback, DEFAULT_PRIORITY);
    }

    /// Register a filter callback with DEFAULT_PRIORITY (10).
    ///
    /// Convenience wrapper around `add_filter` for the common case.
    pub fn add_filter_default(&self, tag: &str, callback: FilterCallback) {
        self.add_filter(tag, callback, DEFAULT_PRIORITY);
    }

    /// Returns the tag of the currently executing filter, if any.
    ///
    /// Uses thread-local storage so it works correctly even when
    /// filters are applied concurrently on different threads.
    pub fn current_filter() -> Option<String> {
        CURRENT_FILTER.with(|cf| cf.borrow().clone())
    }

    /// Returns how many times the given action has been fired.
    ///
    /// Equivalent to WordPress `did_action($tag)`.
    pub fn did_action(&self, tag: &str) -> usize {
        let counts = self.action_counts.read().expect("hook lock poisoned");
        counts.get(tag).copied().unwrap_or(0)
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
                    Value::String(format!("{s} - Modified"))
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
                    Value::String(format!("<p>{s}</p>"))
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
                    Value::String(format!("<div>{s}</div>"))
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

    /// Regression: add_action() called inside do_action() must not deadlock.
    /// WordPress plugins commonly register hooks during hook execution
    /// (e.g. add_action('init', fn() { add_action('wp_head', ...) })).
    #[test]
    fn test_add_action_inside_do_action_no_deadlock() {
        let registry = HookRegistry::new();
        let registry2 = registry.clone();

        registry.add_action(
            "init",
            Arc::new(move |_| {
                // Registering a new hook while "init" is still executing.
                // Without the fix this would deadlock (write-lock inside read-lock).
                registry2.add_action("wp_head", Arc::new(|_| {}), DEFAULT_PRIORITY);
            }),
            DEFAULT_PRIORITY,
        );

        // Must complete without deadlocking or panicking.
        registry.do_action("init", &json!({}));
        assert!(registry.has_action("wp_head"));
    }

    /// Regression: add_filter() called inside apply_filters() must not deadlock.
    #[test]
    fn test_add_filter_inside_apply_filters_no_deadlock() {
        let registry = HookRegistry::new();
        let registry2 = registry.clone();

        registry.add_filter(
            "the_title",
            Arc::new(move |v| {
                // Registering a new filter while "the_title" is still executing.
                registry2.add_filter("the_content", Arc::new(|v| v), DEFAULT_PRIORITY);
                v
            }),
            DEFAULT_PRIORITY,
        );

        let result = registry.apply_filters("the_title", json!("Hello"));
        assert_eq!(result, json!("Hello"));
        assert!(registry.has_filter("the_content"));
    }
}

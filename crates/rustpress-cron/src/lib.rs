use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tokio::time::{self, Duration};
use tracing::{debug, info, warn};

/// Callback type for cron events.
pub type CronCallback = Arc<dyn Fn() + Send + Sync>;

/// Recurrence schedule for cron events.
///
/// Equivalent to WordPress schedules registered via `wp_get_schedules()`.
#[derive(Debug, Clone)]
pub struct CronSchedule {
    pub name: String,
    pub interval: u64, // seconds
    pub display: String,
}

/// A scheduled cron event.
///
/// Equivalent to a single entry in WordPress's `cron` option.
#[derive(Debug, Clone)]
pub struct CronEvent {
    pub hook: String,
    pub timestamp: u64,
    pub schedule: Option<String>, // None = single event
    pub interval: Option<u64>,
    pub args: Vec<String>,
}

/// WordPress-compatible cron system.
///
/// Corresponds to `wp-includes/cron.php` and `wp-cron.php`.
///
/// WordPress cron is a pseudo-cron — it runs on page loads, not on a real
/// system timer. RustPress uses Tokio for background task scheduling.
#[derive(Clone)]
pub struct CronManager {
    schedules: Arc<RwLock<BTreeMap<String, CronSchedule>>>,
    events: Arc<RwLock<Vec<CronEvent>>>,
    callbacks: Arc<RwLock<BTreeMap<String, CronCallback>>>,
}

impl Default for CronManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CronManager {
    pub fn new() -> Self {
        let manager = Self {
            schedules: Arc::new(RwLock::new(BTreeMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            callbacks: Arc::new(RwLock::new(BTreeMap::new())),
        };
        manager.register_default_schedules();
        manager
    }

    fn register_default_schedules(&self) {
        let defaults = vec![
            CronSchedule {
                name: "hourly".to_string(),
                interval: 3600,
                display: "Once Hourly".to_string(),
            },
            CronSchedule {
                name: "twicedaily".to_string(),
                interval: 43200,
                display: "Twice Daily".to_string(),
            },
            CronSchedule {
                name: "daily".to_string(),
                interval: 86400,
                display: "Once Daily".to_string(),
            },
            CronSchedule {
                name: "weekly".to_string(),
                interval: 604800,
                display: "Once Weekly".to_string(),
            },
        ];

        let mut schedules = self.schedules.write().expect("cron schedule lock poisoned");
        for s in defaults {
            schedules.insert(s.name.clone(), s);
        }
    }

    /// Register a custom cron schedule.
    ///
    /// Equivalent to adding to `cron_schedules` filter in WordPress.
    pub fn add_schedule(&self, name: &str, interval: u64, display: &str) {
        let mut schedules = self.schedules.write().expect("cron schedule lock poisoned");
        schedules.insert(
            name.to_string(),
            CronSchedule {
                name: name.to_string(),
                interval,
                display: display.to_string(),
            },
        );
        debug!(name, interval, "cron schedule registered");
    }

    /// Get all registered schedules.
    pub fn get_schedules(&self) -> Vec<CronSchedule> {
        let schedules = self.schedules.read().expect("cron schedule lock poisoned");
        schedules.values().cloned().collect()
    }

    /// Register a callback for a cron hook.
    pub fn register_callback(&self, hook: &str, callback: CronCallback) {
        let mut callbacks = self.callbacks.write().expect("cron callback lock poisoned");
        callbacks.insert(hook.to_string(), callback);
    }

    /// Schedule a recurring event.
    ///
    /// Equivalent to WordPress `wp_schedule_event($timestamp, $recurrence, $hook, $args)`.
    pub fn schedule_event(&self, timestamp: u64, recurrence: &str, hook: &str, args: Vec<String>) {
        let schedules = self.schedules.read().expect("cron schedule lock poisoned");
        let interval = schedules.get(recurrence).map(|s| s.interval);

        let mut events = self.events.write().expect("cron events lock poisoned");
        events.push(CronEvent {
            hook: hook.to_string(),
            timestamp,
            schedule: Some(recurrence.to_string()),
            interval,
            args,
        });
        debug!(hook, recurrence, timestamp, "cron event scheduled");
    }

    /// Schedule a single (non-recurring) event.
    ///
    /// Equivalent to WordPress `wp_schedule_single_event($timestamp, $hook, $args)`.
    pub fn schedule_single_event(&self, timestamp: u64, hook: &str, args: Vec<String>) {
        let mut events = self.events.write().expect("cron events lock poisoned");
        events.push(CronEvent {
            hook: hook.to_string(),
            timestamp,
            schedule: None,
            interval: None,
            args,
        });
        debug!(hook, timestamp, "single cron event scheduled");
    }

    /// Unschedule all events for a hook.
    ///
    /// Equivalent to WordPress `wp_clear_scheduled_hook($hook)`.
    pub fn clear_scheduled_hook(&self, hook: &str) {
        let mut events = self.events.write().expect("cron events lock poisoned");
        events.retain(|e| e.hook != hook);
        debug!(hook, "cleared scheduled hook");
    }

    /// Check when the next scheduled event for a hook will run.
    ///
    /// Equivalent to WordPress `wp_next_scheduled($hook)`.
    pub fn next_scheduled(&self, hook: &str) -> Option<u64> {
        let events = self.events.read().expect("cron events lock poisoned");
        events
            .iter()
            .filter(|e| e.hook == hook)
            .map(|e| e.timestamp)
            .min()
    }

    /// Get all scheduled events.
    pub fn get_events(&self) -> Vec<CronEvent> {
        let events = self.events.read().expect("cron events lock poisoned");
        events.clone()
    }

    /// Run all due cron events (called periodically).
    ///
    /// Equivalent to the logic in WordPress `wp-cron.php`.
    pub fn run_due_events(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let callbacks = self.callbacks.read().expect("cron callback lock poisoned");

        let mut events = self.events.write().expect("cron events lock poisoned");
        let mut reschedule = Vec::new();
        let mut to_remove = Vec::new();

        for (i, event) in events.iter().enumerate() {
            if event.timestamp <= now {
                if let Some(callback) = callbacks.get(&event.hook) {
                    info!(hook = event.hook, "executing cron event");
                    (callback)();
                } else {
                    warn!(hook = event.hook, "no callback registered for cron hook");
                }

                if let (Some(ref schedule), Some(interval)) = (&event.schedule, event.interval) {
                    // Recurring: reschedule
                    reschedule.push(CronEvent {
                        hook: event.hook.clone(),
                        timestamp: now + interval,
                        schedule: Some(schedule.clone()),
                        interval: Some(interval),
                        args: event.args.clone(),
                    });
                }
                to_remove.push(i);
            }
        }

        // Remove executed events (reverse order to preserve indices)
        for i in to_remove.into_iter().rev() {
            events.remove(i);
        }

        // Add rescheduled events
        events.extend(reschedule);
    }

    /// Start the background cron runner (Tokio task).
    ///
    /// Unlike WordPress's pseudo-cron, this runs on a real timer.
    pub fn start_background_runner(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                self.run_due_events();
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_default_schedules() {
        let manager = CronManager::new();
        let schedules = manager.get_schedules();
        let names: Vec<&str> = schedules.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hourly"));
        assert!(names.contains(&"twicedaily"));
        assert!(names.contains(&"daily"));
        assert!(names.contains(&"weekly"));
    }

    #[test]
    fn test_schedule_event() {
        let manager = CronManager::new();
        manager.schedule_event(1000, "hourly", "my_hook", vec![]);
        assert_eq!(manager.next_scheduled("my_hook"), Some(1000));
    }

    #[test]
    fn test_clear_scheduled_hook() {
        let manager = CronManager::new();
        manager.schedule_event(1000, "daily", "cleanup", vec![]);
        assert!(manager.next_scheduled("cleanup").is_some());
        manager.clear_scheduled_hook("cleanup");
        assert!(manager.next_scheduled("cleanup").is_none());
    }

    #[test]
    fn test_run_due_events() {
        let manager = CronManager::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c = counter.clone();
        manager.register_callback(
            "test_hook",
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        );

        // Schedule in the past (immediately due)
        manager.schedule_single_event(0, "test_hook", vec![]);
        manager.run_due_events();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Single event should not repeat
        manager.run_due_events();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_add_custom_schedule() {
        let manager = CronManager::new();
        manager.add_schedule("every_5_minutes", 300, "Every 5 Minutes");
        let schedules = manager.get_schedules();
        assert!(schedules.iter().any(|s| s.name == "every_5_minutes" && s.interval == 300));
    }
}

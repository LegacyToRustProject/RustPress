use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use rustpress_core::hooks::HookRegistry;

/// Plugin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub plugin_type: PluginType,
}

/// Type of plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginType {
    /// Native Rust plugin (compiled into the binary or loaded as dylib)
    Native,
    /// WebAssembly plugin (sandboxed)
    Wasm,
}

/// Plugin status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginStatus {
    Active,
    Inactive,
    Error(String),
}

/// Registered plugin entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub meta: PluginMeta,
    pub status: PluginStatus,
    pub file_path: String,
}

/// Plugin trait that all plugins must implement.
pub trait Plugin: Send + Sync {
    fn meta(&self) -> PluginMeta;
    fn activate(&self, hooks: &HookRegistry);
    fn deactivate(&self, hooks: &HookRegistry);
}

/// Plugin registry - manages all installed plugins.
#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, PluginEntry>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a plugin.
    pub async fn register(&self, entry: PluginEntry) {
        info!(name = &entry.meta.name, "plugin registered");
        let mut plugins = self.plugins.write().await;
        plugins.insert(entry.meta.name.clone(), entry);
    }

    /// Activate a plugin by name.
    pub async fn activate(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;
        let entry = plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin '{name}' not found"))?;

        entry.status = PluginStatus::Active;
        info!(name, "plugin activated");
        Ok(())
    }

    /// Deactivate a plugin by name.
    pub async fn deactivate(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;
        let entry = plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin '{name}' not found"))?;

        entry.status = PluginStatus::Inactive;
        info!(name, "plugin deactivated");
        Ok(())
    }

    /// Get list of all plugins.
    pub async fn list(&self) -> Vec<PluginEntry> {
        let plugins = self.plugins.read().await;
        plugins.values().cloned().collect()
    }

    /// Get list of active plugins.
    pub async fn active_plugins(&self) -> Vec<PluginEntry> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|p| p.status == PluginStatus::Active)
            .cloned()
            .collect()
    }

    /// Check if a plugin is active.
    pub async fn is_active(&self, name: &str) -> bool {
        let plugins = self.plugins.read().await;
        plugins
            .get(name)
            .map(|p| p.status == PluginStatus::Active)
            .unwrap_or(false)
    }

    /// Remove a plugin from the registry.
    pub async fn remove(&self, name: &str) -> bool {
        let mut plugins = self.plugins.write().await;
        plugins.remove(name).is_some()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plugin(name: &str) -> PluginEntry {
        PluginEntry {
            meta: PluginMeta {
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: "Test plugin".to_string(),
                author: "Test Author".to_string(),
                plugin_type: PluginType::Native,
            },
            status: PluginStatus::Inactive,
            file_path: format!("/plugins/{name}"),
        }
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let registry = PluginRegistry::new();
        registry.register(test_plugin("hello")).await;
        registry.register(test_plugin("world")).await;

        let plugins = registry.list().await;
        assert_eq!(plugins.len(), 2);
    }

    #[tokio::test]
    async fn test_activate_and_deactivate() {
        let registry = PluginRegistry::new();
        registry.register(test_plugin("test-plugin")).await;

        assert!(!registry.is_active("test-plugin").await);

        registry.activate("test-plugin").await.unwrap();
        assert!(registry.is_active("test-plugin").await);

        registry.deactivate("test-plugin").await.unwrap();
        assert!(!registry.is_active("test-plugin").await);
    }

    #[tokio::test]
    async fn test_activate_nonexistent() {
        let registry = PluginRegistry::new();
        let result = registry.activate("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_active_plugins() {
        let registry = PluginRegistry::new();
        registry.register(test_plugin("active-one")).await;
        registry.register(test_plugin("inactive-one")).await;
        registry.register(test_plugin("active-two")).await;

        registry.activate("active-one").await.unwrap();
        registry.activate("active-two").await.unwrap();

        let active = registry.active_plugins().await;
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_remove_plugin() {
        let registry = PluginRegistry::new();
        registry.register(test_plugin("removable")).await;

        assert!(registry.remove("removable").await);
        assert!(!registry.remove("removable").await);
        assert!(registry.list().await.is_empty());
    }
}

use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::registry::{PluginEntry, PluginMeta, PluginRegistry, PluginStatus, PluginType};

/// Plugin loader - scans directories for plugins and loads them.
pub struct PluginLoader {
    plugins_dir: PathBuf,
}

impl PluginLoader {
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
        }
    }

    /// Scan the plugins directory and register discovered plugins.
    pub async fn scan_and_register(&self, registry: &PluginRegistry) -> Result<usize, String> {
        if !self.plugins_dir.exists() {
            info!(dir = ?self.plugins_dir, "plugins directory does not exist, creating");
            std::fs::create_dir_all(&self.plugins_dir)
                .map_err(|e| format!("Failed to create plugins dir: {}", e))?;
            return Ok(0);
        }

        let mut count = 0;
        let entries = std::fs::read_dir(&self.plugins_dir)
            .map_err(|e| format!("Failed to read plugins dir: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if let Some(plugin_entry) = self.try_load_plugin(&path).await {
                registry.register(plugin_entry).await;
                count += 1;
            }
        }

        info!(count, "plugins scanned and registered");
        Ok(count)
    }

    /// Try to load a plugin from a path.
    async fn try_load_plugin(&self, path: &Path) -> Option<PluginEntry> {
        // Check for WASM plugins (.wasm files)
        if path.extension().map(|e| e == "wasm").unwrap_or(false) {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            info!(name = &name, path = ?path, "discovered WASM plugin");

            return Some(PluginEntry {
                meta: PluginMeta {
                    name,
                    version: "0.1.0".to_string(),
                    description: "WASM Plugin".to_string(),
                    author: "Unknown".to_string(),
                    plugin_type: PluginType::Wasm,
                },
                status: PluginStatus::Inactive,
                file_path: path.to_string_lossy().to_string(),
            });
        }

        // Check for plugin directories with a plugin.json manifest
        if path.is_dir() {
            let manifest_path = path.join("plugin.json");
            if manifest_path.exists() {
                match std::fs::read_to_string(&manifest_path) {
                    Ok(content) => {
                        match serde_json::from_str::<PluginMeta>(&content) {
                            Ok(meta) => {
                                info!(name = &meta.name, "discovered plugin from manifest");
                                return Some(PluginEntry {
                                    meta,
                                    status: PluginStatus::Inactive,
                                    file_path: path.to_string_lossy().to_string(),
                                });
                            }
                            Err(e) => {
                                warn!(path = ?manifest_path, error = %e, "failed to parse plugin manifest");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(path = ?manifest_path, error = %e, "failed to read plugin manifest");
                    }
                }
            }
        }

        None
    }
}

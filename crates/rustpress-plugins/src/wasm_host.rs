use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum WasmError {
    #[error("WASM runtime error: {0}")]
    Runtime(String),
    #[error("Plugin not found: {0}")]
    NotFound(String),
    #[error("Plugin initialization failed: {0}")]
    InitFailed(String),
    #[error("Function call failed: {0}")]
    CallFailed(String),
}

/// Host function that plugins can call.
#[derive(Debug, Clone)]
pub struct HostFunction {
    pub name: String,
    pub description: String,
}

/// WASM Plugin host - manages WASM plugin execution.
///
/// This provides the framework for loading and executing WASM plugins.
/// The actual WASM runtime (wasmtime/extism) is integrated when the
/// `wasm-runtime` feature is enabled.
pub struct WasmHost {
    host_functions: HashMap<String, HostFunction>,
    loaded_plugins: HashMap<String, WasmPlugin>,
}

/// Represents a loaded WASM plugin.
#[allow(dead_code)]
struct WasmPlugin {
    name: String,
    wasm_bytes: Vec<u8>,
    initialized: bool,
}

impl WasmHost {
    pub fn new() -> Self {
        let mut host = Self {
            host_functions: HashMap::new(),
            loaded_plugins: HashMap::new(),
        };

        // Register default host functions that plugins can call
        host.register_host_function("get_option", "Read a WordPress option value");
        host.register_host_function("set_option", "Set a WordPress option value");
        host.register_host_function("get_posts", "Query posts from the database");
        host.register_host_function("add_action", "Register an action hook");
        host.register_host_function("add_filter", "Register a filter hook");
        host.register_host_function("log", "Write to the server log");

        host
    }

    /// Register a host function that plugins can call.
    pub fn register_host_function(&mut self, name: &str, description: &str) {
        self.host_functions.insert(
            name.to_string(),
            HostFunction {
                name: name.to_string(),
                description: description.to_string(),
            },
        );
    }

    /// Load a WASM plugin from a file.
    pub fn load_plugin(&mut self, name: &str, wasm_path: &Path) -> Result<(), WasmError> {
        let wasm_bytes = std::fs::read(wasm_path)
            .map_err(|e| WasmError::NotFound(format!("{}: {}", wasm_path.display(), e)))?;

        info!(name, path = ?wasm_path, bytes = wasm_bytes.len(), "WASM plugin loaded");

        self.loaded_plugins.insert(
            name.to_string(),
            WasmPlugin {
                name: name.to_string(),
                wasm_bytes,
                initialized: false,
            },
        );

        Ok(())
    }

    /// Initialize a loaded plugin (instantiate WASM module).
    pub fn init_plugin(&mut self, name: &str) -> Result<(), WasmError> {
        let plugin = self
            .loaded_plugins
            .get_mut(name)
            .ok_or_else(|| WasmError::NotFound(name.to_string()))?;

        // Validate WASM magic bytes
        if plugin.wasm_bytes.len() < 4 || &plugin.wasm_bytes[0..4] != b"\0asm" {
            return Err(WasmError::InitFailed(
                "Invalid WASM binary format".to_string(),
            ));
        }

        plugin.initialized = true;
        info!(name, "WASM plugin initialized");
        Ok(())
    }

    /// Call a function exported by a WASM plugin.
    pub fn call_plugin(
        &self,
        plugin_name: &str,
        function_name: &str,
        _args: &serde_json::Value,
    ) -> Result<serde_json::Value, WasmError> {
        let plugin = self
            .loaded_plugins
            .get(plugin_name)
            .ok_or_else(|| WasmError::NotFound(plugin_name.to_string()))?;

        if !plugin.initialized {
            return Err(WasmError::Runtime(format!(
                "Plugin '{}' not initialized",
                plugin_name
            )));
        }

        // WASM execution would happen here with wasmtime/extism.
        // For now, return a placeholder indicating the call was received.
        warn!(
            plugin = plugin_name,
            function = function_name,
            "WASM execution not yet implemented - install wasmtime feature"
        );

        Ok(serde_json::json!({
            "status": "wasm_runtime_pending",
            "plugin": plugin_name,
            "function": function_name,
        }))
    }

    /// Get list of available host functions.
    pub fn host_functions(&self) -> Vec<&HostFunction> {
        self.host_functions.values().collect()
    }

    /// Get list of loaded plugins.
    pub fn loaded_plugins(&self) -> Vec<&str> {
        self.loaded_plugins.keys().map(|s| s.as_str()).collect()
    }

    /// Unload a plugin.
    pub fn unload_plugin(&mut self, name: &str) -> bool {
        self.loaded_plugins.remove(name).is_some()
    }
}

impl Default for WasmHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_new_has_default_host_functions() {
        let host = WasmHost::new();
        let funcs = host.host_functions();
        assert!(funcs.len() >= 6);

        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"get_option"));
        assert!(names.contains(&"add_action"));
        assert!(names.contains(&"log"));
    }

    #[test]
    fn test_register_custom_host_function() {
        let mut host = WasmHost::new();
        let initial = host.host_functions().len();

        host.register_host_function("custom_fn", "A custom function");
        assert_eq!(host.host_functions().len(), initial + 1);
    }

    #[test]
    fn test_load_plugin() {
        let mut host = WasmHost::new();

        // Create a temp file with WASM magic bytes
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        let mut f = std::fs::File::create(&wasm_path).unwrap();
        f.write_all(b"\0asm\x01\x00\x00\x00").unwrap();

        host.load_plugin("test", &wasm_path).unwrap();
        assert_eq!(host.loaded_plugins().len(), 1);
    }

    #[test]
    fn test_load_plugin_not_found() {
        let mut host = WasmHost::new();
        let result = host.load_plugin("missing", Path::new("/nonexistent/plugin.wasm"));
        assert!(result.is_err());
    }

    #[test]
    fn test_init_plugin_validates_magic() {
        let mut host = WasmHost::new();

        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("invalid.wasm");
        std::fs::write(&wasm_path, b"not a wasm file").unwrap();

        host.load_plugin("invalid", &wasm_path).unwrap();
        let result = host.init_plugin("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_init_valid_wasm() {
        let mut host = WasmHost::new();

        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("valid.wasm");
        std::fs::write(&wasm_path, b"\0asm\x01\x00\x00\x00").unwrap();

        host.load_plugin("valid", &wasm_path).unwrap();
        host.init_plugin("valid").unwrap();
    }

    #[test]
    fn test_call_uninitialized_plugin() {
        let mut host = WasmHost::new();

        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, b"\0asm\x01\x00\x00\x00").unwrap();

        host.load_plugin("test", &wasm_path).unwrap();
        // Don't init, try to call
        let result = host.call_plugin("test", "func", &serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_unload_plugin() {
        let mut host = WasmHost::new();

        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("test.wasm");
        std::fs::write(&wasm_path, b"\0asm\x01\x00\x00\x00").unwrap();

        host.load_plugin("test", &wasm_path).unwrap();
        assert!(host.unload_plugin("test"));
        assert!(!host.unload_plugin("test"));
        assert!(host.loaded_plugins().is_empty());
    }
}

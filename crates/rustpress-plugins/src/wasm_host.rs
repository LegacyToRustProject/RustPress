use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
#[cfg(feature = "wasm-runtime")]
use tracing::info;
#[cfg(not(feature = "wasm-runtime"))]
use tracing::{info, warn};

#[cfg(feature = "wasm-runtime")]
use wasmtime::{Engine, Module};

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
/// The actual WASM runtime (wasmtime) is integrated when the
/// `wasm-runtime` feature is enabled.
pub struct WasmHost {
    host_functions: HashMap<String, HostFunction>,
    loaded_plugins: HashMap<String, WasmPlugin>,
    #[cfg(feature = "wasm-runtime")]
    engine: Engine,
    #[cfg(feature = "wasm-runtime")]
    compiled_modules: HashMap<String, Module>,
}

/// Represents a loaded WASM plugin.
#[allow(dead_code)]
struct WasmPlugin {
    name: String,
    wasm_bytes: Vec<u8>,
    initialized: bool,
}

/// State accessible by WASM host functions.
#[cfg(feature = "wasm-runtime")]
struct HostState {
    /// Linear memory exported by the WASM module (set after instantiation).
    memory: Option<wasmtime::Memory>,
    /// Log messages captured from the plugin.
    log_messages: Vec<String>,
}

impl WasmHost {
    pub fn new() -> Self {
        let mut host = Self {
            host_functions: HashMap::new(),
            loaded_plugins: HashMap::new(),
            #[cfg(feature = "wasm-runtime")]
            engine: Engine::default(),
            #[cfg(feature = "wasm-runtime")]
            compiled_modules: HashMap::new(),
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

    /// Load a WASM plugin from raw bytes (useful for testing).
    pub fn load_plugin_bytes(&mut self, name: &str, wasm_bytes: Vec<u8>) -> Result<(), WasmError> {
        info!(
            name,
            bytes = wasm_bytes.len(),
            "WASM plugin loaded from bytes"
        );

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

        #[cfg(feature = "wasm-runtime")]
        {
            let module = Module::new(&self.engine, &plugin.wasm_bytes).map_err(|e| {
                WasmError::InitFailed(format!("Failed to compile WASM module: {}", e))
            })?;
            self.compiled_modules.insert(name.to_string(), module);
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
        args: &serde_json::Value,
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

        #[cfg(feature = "wasm-runtime")]
        {
            return self.call_plugin_wasm(plugin_name, function_name, args);
        }

        #[cfg(not(feature = "wasm-runtime"))]
        {
            let _ = args;
            // WASM execution would happen here with wasmtime.
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
    }

    /// Actually execute a WASM function via wasmtime.
    #[cfg(feature = "wasm-runtime")]
    fn call_plugin_wasm(
        &self,
        plugin_name: &str,
        function_name: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, WasmError> {
        use wasmtime::{Caller, Linker, Store};

        let module = self.compiled_modules.get(plugin_name).ok_or_else(|| {
            WasmError::Runtime(format!("No compiled module for '{}'", plugin_name))
        })?;

        let mut store = Store::new(
            &self.engine,
            HostState {
                memory: None,
                log_messages: Vec::new(),
            },
        );

        let mut linker = Linker::new(&self.engine);

        // Host function: log(ptr: i32, len: i32)
        // Reads a UTF-8 string from the plugin's linear memory and logs it.
        linker
            .func_wrap(
                "env",
                "log",
                |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
                    if let Some(memory) = caller.data().memory {
                        let start = ptr as usize;
                        let end = start + len as usize;
                        let msg = {
                            let data = memory.data(&caller);
                            if end <= data.len() {
                                std::str::from_utf8(&data[start..end])
                                    .ok()
                                    .map(|s| s.to_string())
                            } else {
                                None
                            }
                        };
                        if let Some(msg) = msg {
                            info!(plugin_msg = %msg, "WASM plugin log");
                            caller.data_mut().log_messages.push(msg);
                        }
                    }
                },
            )
            .map_err(|e| WasmError::Runtime(format!("Failed to define 'log': {}", e)))?;

        // Host function: get_option(ptr: i32, len: i32) -> i64
        // Placeholder - reads the option name but always returns 0.
        // Will be wired to the database layer later.
        linker
            .func_wrap(
                "env",
                "get_option",
                |caller: Caller<'_, HostState>, ptr: i32, len: i32| -> i64 {
                    if let Some(memory) = caller.data().memory {
                        let data = memory.data(&caller);
                        let start = ptr as usize;
                        let end = start + len as usize;
                        if end <= data.len() {
                            if let Ok(option_name) = std::str::from_utf8(&data[start..end]) {
                                info!(option = option_name, "WASM plugin get_option (stub)");
                            }
                        }
                    }
                    0i64
                },
            )
            .map_err(|e| WasmError::Runtime(format!("Failed to define 'get_option': {}", e)))?;

        // Host function: set_option(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32)
        // Placeholder - reads key and value but does nothing.
        linker
            .func_wrap(
                "env",
                "set_option",
                |caller: Caller<'_, HostState>,
                 key_ptr: i32,
                 key_len: i32,
                 val_ptr: i32,
                 val_len: i32| {
                    if let Some(memory) = caller.data().memory {
                        let data = memory.data(&caller);
                        let ks = key_ptr as usize;
                        let ke = ks + key_len as usize;
                        let vs = val_ptr as usize;
                        let ve = vs + val_len as usize;
                        if ke <= data.len() && ve <= data.len() {
                            let key = std::str::from_utf8(&data[ks..ke]).unwrap_or("<invalid>");
                            let val = std::str::from_utf8(&data[vs..ve]).unwrap_or("<invalid>");
                            info!(key, val, "WASM plugin set_option (stub)");
                        }
                    }
                },
            )
            .map_err(|e| WasmError::Runtime(format!("Failed to define 'set_option': {}", e)))?;

        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|e| WasmError::CallFailed(format!("Failed to instantiate module: {}", e)))?;

        // Capture the module's exported memory so host functions can read it.
        if let Some(memory) = instance.get_memory(&mut store, "memory") {
            store.data_mut().memory = Some(memory);
        }

        // Try to find the exported function by name.
        // Strategy 1: JSON-based calling convention (ptr, len) -> i32
        //   We write the JSON args into the module's memory via an exported `alloc` function,
        //   call the target, then read the result back.
        // Strategy 2: Simple () -> i32 export
        // Strategy 3: Simple (i32, i32) -> i32 export

        // First, try as a simple () -> i32 export
        if let Ok(func) = instance.get_typed_func::<(), i32>(&mut store, function_name) {
            let result = func.call(&mut store, ()).map_err(|e| {
                WasmError::CallFailed(format!("Function '{}' trapped: {}", function_name, e))
            })?;
            return Ok(serde_json::json!({
                "status": "ok",
                "result": result,
                "log_messages": store.data().log_messages,
            }));
        }

        // Try as (i32, i32) -> i32 (pointer + length -> result)
        if let Ok(func) = instance.get_typed_func::<(i32, i32), i32>(&mut store, function_name) {
            // Serialize args to JSON, write into WASM memory
            let args_json = serde_json::to_string(args)
                .map_err(|e| WasmError::CallFailed(format!("Failed to serialize args: {}", e)))?;

            let (ptr, len) =
                self.write_to_wasm_memory(&instance, &mut store, args_json.as_bytes())?;

            let result = func.call(&mut store, (ptr, len)).map_err(|e| {
                WasmError::CallFailed(format!("Function '{}' trapped: {}", function_name, e))
            })?;

            // Try to read result string from WASM memory if the return value looks like a pointer
            // For now, just return the i32 result
            return Ok(serde_json::json!({
                "status": "ok",
                "result": result,
                "log_messages": store.data().log_messages,
            }));
        }

        // Try as () -> () (void function)
        if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, function_name) {
            func.call(&mut store, ()).map_err(|e| {
                WasmError::CallFailed(format!("Function '{}' trapped: {}", function_name, e))
            })?;
            return Ok(serde_json::json!({
                "status": "ok",
                "result": null,
                "log_messages": store.data().log_messages,
            }));
        }

        Err(WasmError::CallFailed(format!(
            "Function '{}' not found or has unsupported signature in plugin '{}'",
            function_name, plugin_name
        )))
    }

    /// Write bytes into a WASM module's linear memory.
    /// Tries to use an exported `alloc(size: i32) -> i32` function first,
    /// otherwise writes to a fixed offset after the data section.
    #[cfg(feature = "wasm-runtime")]
    fn write_to_wasm_memory(
        &self,
        instance: &wasmtime::Instance,
        store: &mut wasmtime::Store<HostState>,
        bytes: &[u8],
    ) -> Result<(i32, i32), WasmError> {
        let memory = instance
            .get_memory(&mut *store, "memory")
            .ok_or_else(|| WasmError::CallFailed("Module has no exported memory".to_string()))?;

        let len = bytes.len() as i32;

        // Try using an exported alloc function
        if let Ok(alloc) = instance.get_typed_func::<i32, i32>(&mut *store, "alloc") {
            let ptr = alloc
                .call(&mut *store, len)
                .map_err(|e| WasmError::CallFailed(format!("alloc failed: {}", e)))?;

            let mem_data = memory.data_mut(&mut *store);
            let start = ptr as usize;
            let end = start + bytes.len();
            if end > mem_data.len() {
                return Err(WasmError::CallFailed(
                    "alloc returned out-of-bounds pointer".to_string(),
                ));
            }
            mem_data[start..end].copy_from_slice(bytes);
            return Ok((ptr, len));
        }

        // Fallback: write at a fixed offset (1024) - suitable for simple plugins
        let offset = 1024i32;
        let mem_data = memory.data_mut(&mut *store);
        let start = offset as usize;
        let end = start + bytes.len();
        if end > mem_data.len() {
            // Try to grow memory
            let pages_needed = ((end - mem_data.len()) / 65536) + 1;
            memory
                .grow(&mut *store, pages_needed as u64)
                .map_err(|e| WasmError::CallFailed(format!("Failed to grow memory: {}", e)))?;
            let mem_data = memory.data_mut(&mut *store);
            mem_data[start..end].copy_from_slice(bytes);
        } else {
            mem_data[start..end].copy_from_slice(bytes);
        }

        Ok((offset, len))
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
        #[cfg(feature = "wasm-runtime")]
        {
            self.compiled_modules.remove(name);
        }
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
        // The 8-byte WASM header (magic + version 1) is a valid empty module
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

    // =======================================================================
    // Tests that require the wasm-runtime feature and actual WASM modules
    // =======================================================================

    /// Build a minimal valid WASM module (WAT text format).
    #[cfg(feature = "wasm-runtime")]
    fn minimal_wasm_returning(value: i32) -> Vec<u8> {
        let wat_src = format!(
            r#"(module
                (memory (export "memory") 1)
                (func (export "get_value") (result i32)
                    i32.const {value}
                )
            )"#,
        );
        wat::parse_str(&wat_src).expect("failed to parse WAT")
    }

    /// Build a WASM module with an (i32, i32) -> i32 function that returns ptr+len.
    #[cfg(feature = "wasm-runtime")]
    fn wasm_with_ptr_len_func() -> Vec<u8> {
        let wat = r#"(module
            (memory (export "memory") 1)
            (func (export "process") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add
            )
        )"#;
        wat::parse_str(wat).expect("failed to parse WAT")
    }

    /// Build a WASM module that imports and calls the host `log` function.
    #[cfg(feature = "wasm-runtime")]
    fn wasm_with_log_call() -> Vec<u8> {
        // This module:
        // 1. Has a memory with "hello" at offset 0
        // 2. Exports run() which calls env.log(0, 5) to log "hello"
        // 3. Returns 42
        let wat = r#"(module
            (import "env" "log" (func $log (param i32 i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "hello")
            (func (export "run") (result i32)
                i32.const 0
                i32.const 5
                call $log
                i32.const 42
            )
        )"#;
        wat::parse_str(wat).expect("failed to parse WAT")
    }

    /// Build a WASM module that imports get_option.
    #[cfg(feature = "wasm-runtime")]
    fn wasm_with_get_option() -> Vec<u8> {
        let wat = r#"(module
            (import "env" "get_option" (func $get_option (param i32 i32) (result i64)))
            (memory (export "memory") 1)
            (data (i32.const 0) "site_name")
            (func (export "check_option") (result i32)
                i32.const 0
                i32.const 9
                call $get_option
                i32.wrap_i64
            )
        )"#;
        wat::parse_str(wat).expect("failed to parse WAT")
    }

    /// Build a WASM module that imports set_option.
    #[cfg(feature = "wasm-runtime")]
    fn wasm_with_set_option() -> Vec<u8> {
        let wat = r#"(module
            (import "env" "set_option" (func $set_option (param i32 i32 i32 i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "mykey")
            (data (i32.const 16) "myval")
            (func (export "write_option") (result i32)
                i32.const 0
                i32.const 5
                i32.const 16
                i32.const 5
                call $set_option
                i32.const 1
            )
        )"#;
        wat::parse_str(wat).expect("failed to parse WAT")
    }

    #[cfg(feature = "wasm-runtime")]
    #[test]
    fn test_wasm_simple_call() {
        let mut host = WasmHost::new();
        let wasm_bytes = minimal_wasm_returning(99);

        host.load_plugin_bytes("test", wasm_bytes).unwrap();
        host.init_plugin("test").unwrap();

        let result = host
            .call_plugin("test", "get_value", &serde_json::json!({}))
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["result"], 99);
    }

    #[cfg(feature = "wasm-runtime")]
    #[test]
    fn test_wasm_ptr_len_call() {
        let mut host = WasmHost::new();
        let wasm_bytes = wasm_with_ptr_len_func();

        host.load_plugin_bytes("test", wasm_bytes).unwrap();
        host.init_plugin("test").unwrap();

        let args = serde_json::json!({"key": "value"});
        let result = host.call_plugin("test", "process", &args).unwrap();

        assert_eq!(result["status"], "ok");
        // The function returns ptr + len, so the result depends on JSON serialization length
        let json_str = serde_json::to_string(&args).unwrap();
        // Fallback offset is 1024 since there's no alloc export
        let expected = 1024 + json_str.len() as i64;
        assert_eq!(result["result"], expected);
    }

    #[cfg(feature = "wasm-runtime")]
    #[test]
    fn test_wasm_host_log_function() {
        let mut host = WasmHost::new();
        let wasm_bytes = wasm_with_log_call();

        host.load_plugin_bytes("test", wasm_bytes).unwrap();
        host.init_plugin("test").unwrap();

        let result = host
            .call_plugin("test", "run", &serde_json::json!({}))
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["result"], 42);

        // Verify the log message was captured
        let log_messages = result["log_messages"].as_array().unwrap();
        assert_eq!(log_messages.len(), 1);
        assert_eq!(log_messages[0], "hello");
    }

    #[cfg(feature = "wasm-runtime")]
    #[test]
    fn test_wasm_get_option_host_function() {
        let mut host = WasmHost::new();
        let wasm_bytes = wasm_with_get_option();

        host.load_plugin_bytes("test", wasm_bytes).unwrap();
        host.init_plugin("test").unwrap();

        let result = host
            .call_plugin("test", "check_option", &serde_json::json!({}))
            .unwrap();

        assert_eq!(result["status"], "ok");
        // get_option stub returns 0
        assert_eq!(result["result"], 0);
    }

    #[cfg(feature = "wasm-runtime")]
    #[test]
    fn test_wasm_set_option_host_function() {
        let mut host = WasmHost::new();
        let wasm_bytes = wasm_with_set_option();

        host.load_plugin_bytes("test", wasm_bytes).unwrap();
        host.init_plugin("test").unwrap();

        let result = host
            .call_plugin("test", "write_option", &serde_json::json!({}))
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["result"], 1);
    }

    #[cfg(feature = "wasm-runtime")]
    #[test]
    fn test_wasm_function_not_found() {
        let mut host = WasmHost::new();
        let wasm_bytes = minimal_wasm_returning(0);

        host.load_plugin_bytes("test", wasm_bytes).unwrap();
        host.init_plugin("test").unwrap();

        let result = host.call_plugin("test", "nonexistent", &serde_json::json!({}));
        assert!(result.is_err());
        match result.unwrap_err() {
            WasmError::CallFailed(msg) => {
                assert!(msg.contains("nonexistent"));
            }
            other => panic!("Expected CallFailed, got: {:?}", other),
        }
    }

    #[cfg(feature = "wasm-runtime")]
    #[test]
    fn test_wasm_unload_removes_compiled_module() {
        let mut host = WasmHost::new();
        let wasm_bytes = minimal_wasm_returning(1);

        host.load_plugin_bytes("test", wasm_bytes).unwrap();
        host.init_plugin("test").unwrap();
        assert!(host.compiled_modules.contains_key("test"));

        host.unload_plugin("test");
        assert!(!host.compiled_modules.contains_key("test"));
    }
}

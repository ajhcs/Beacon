use wasmtime::{Engine, ExternType, Linker, Module, Store, Val};

use crate::config::SandboxConfig;

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("WASM engine error: {0}")]
    Engine(#[from] wasmtime::Error),

    #[error("Export not found: {name}")]
    ExportNotFound { name: String },

    #[error("Export '{name}' is not a function")]
    ExportNotFunction { name: String },

    #[error("Fuel exhausted during execution")]
    FuelExhausted,

    #[error("Type mismatch: {details}")]
    TypeMismatch { details: String },
}

/// Store data that implements resource limiting.
pub(crate) struct StoreData {
    memory_limit_bytes: u64,
}

impl wasmtime::ResourceLimiter for StoreData {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        Ok((desired as u64) <= self.memory_limit_bytes)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        Ok(desired <= 10_000)
    }
}

/// The WASM sandbox — loads and contains DUT modules with full isolation.
pub struct Sandbox {
    engine: Engine,
    config: SandboxConfig,
}

/// A loaded (but not yet instantiated) WASM module.
pub struct LoadedModule {
    module: Module,
}

/// A live WASM instance ready for function calls.
pub struct SandboxInstance {
    pub(crate) store: Store<StoreData>,
    pub(crate) instance: wasmtime::Instance,
    pub(crate) fuel_per_action: Option<u64>,
}

/// Wrapper around wasmtime::Val for a cleaner interface.
#[derive(Debug, Clone)]
pub struct WasmVal(Val);

impl WasmVal {
    pub fn i32(&self) -> Option<i32> {
        match &self.0 {
            Val::I32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn i64(&self) -> Option<i64> {
        match &self.0 {
            Val::I64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn f32_bits(&self) -> Option<u32> {
        match &self.0 {
            Val::F32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn f64_bits(&self) -> Option<u64> {
        match &self.0 {
            Val::F64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn into_val(self) -> Val {
        self.0
    }
}

impl From<i32> for WasmVal {
    fn from(v: i32) -> Self {
        WasmVal(Val::I32(v))
    }
}

impl From<i64> for WasmVal {
    fn from(v: i64) -> Self {
        WasmVal(Val::I64(v))
    }
}

impl From<Val> for WasmVal {
    fn from(v: Val) -> Self {
        WasmVal(v)
    }
}

impl Sandbox {
    /// Create a new sandbox with the given configuration.
    pub fn new(config: &SandboxConfig) -> Result<Self, SandboxError> {
        let mut engine_config = wasmtime::Config::new();

        // Enable fuel metering if configured
        if config.fuel_per_action.is_some() {
            engine_config.consume_fuel(true);
        }

        // Disable threading — pure computation only
        engine_config.wasm_threads(false);

        let engine = Engine::new(&engine_config)?;
        Ok(Self {
            engine,
            config: config.clone(),
        })
    }

    /// Load a WASM module from bytes. Validates the module.
    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<LoadedModule, SandboxError> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        Ok(LoadedModule { module })
    }

    /// Instantiate a loaded module with no imports (fully isolated).
    pub fn instantiate(&self, loaded: &LoadedModule) -> Result<SandboxInstance, SandboxError> {
        let data = StoreData {
            memory_limit_bytes: self.config.memory_limit_bytes,
        };
        let mut store = Store::new(&self.engine, data);

        // Enable resource limiter
        store.limiter(|data| data);

        // Add initial fuel if metering is enabled
        let fuel_per_action = self.config.fuel_per_action;
        if let Some(fuel) = fuel_per_action {
            store.set_fuel(fuel)?;
        }

        // Create a linker with NO imports — full isolation
        let linker = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, &loaded.module)?;

        Ok(SandboxInstance {
            store,
            instance,
            fuel_per_action,
        })
    }

    /// List all exports from a module as (name, kind) pairs.
    pub fn list_exports(&self, loaded: &LoadedModule) -> Vec<(String, String)> {
        loaded
            .module
            .exports()
            .map(|export| {
                let kind = match export.ty() {
                    ExternType::Func(_) => "func",
                    ExternType::Global(_) => "global",
                    ExternType::Memory(_) => "memory",
                    ExternType::Table(_) => "table",
                };
                (export.name().to_string(), kind.to_string())
            })
            .collect()
    }

    /// Get a reference to the engine (needed for snapshot/restore).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Get the sandbox configuration.
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }
}

impl SandboxInstance {
    /// Call an exported function by name with the given arguments.
    /// Returns the result values, or an error if the call fails.
    pub fn call_func(
        &mut self,
        name: &str,
        args: &[WasmVal],
    ) -> Result<Vec<WasmVal>, SandboxError> {
        // Reset fuel before each action
        if let Some(fuel) = self.fuel_per_action {
            self.store.set_fuel(fuel)?;
        }

        let func = self
            .instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| SandboxError::ExportNotFound {
                name: name.to_string(),
            })?;

        let func_ty = func.ty(&self.store);
        let result_count = func_ty.results().len();

        // Convert WasmVal args to wasmtime Val
        let wasm_args: Vec<Val> = args.iter().map(|a| a.0).collect();

        // Prepare result buffer
        let mut results = vec![Val::I32(0); result_count];

        // Call the function
        match func.call(&mut self.store, &wasm_args, &mut results) {
            Ok(()) => Ok(results.into_iter().map(WasmVal).collect()),
            Err(trap) => {
                // Check entire error chain for fuel exhaustion
                let full_msg = format!("{:?}", trap);
                if full_msg.contains("fuel") || full_msg.contains("Fuel") {
                    Err(SandboxError::FuelExhausted)
                } else {
                    Err(SandboxError::Engine(trap))
                }
            }
        }
    }

    /// Get the remaining fuel in the store.
    pub fn remaining_fuel(&self) -> Option<u64> {
        self.store.get_fuel().ok()
    }

    /// Get the configured fuel budget per action.
    pub fn fuel_budget(&self) -> Option<u64> {
        self.fuel_per_action
    }
}

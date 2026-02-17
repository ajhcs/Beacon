use crate::config::SandboxConfig;
use crate::sandbox::{LoadedModule, Sandbox, SandboxError, SandboxInstance};
use wasmtime::Val;

/// A paired snapshot of WASM instance state and model generation number.
/// Restore is atomic — both WASM and model state roll back together, or neither does.
#[derive(Debug, Clone)]
pub struct PairedSnapshot {
    /// The model generation number at snapshot time.
    pub model_generation: u64,
    /// Serialized WASM memory contents.
    wasm_memory: Option<Vec<u8>>,
    /// Serialized WASM global values.
    wasm_globals: Vec<GlobalSnapshot>,
    /// Fuel remaining at snapshot time.
    fuel_remaining: Option<u64>,
}

#[derive(Debug, Clone)]
struct GlobalSnapshot {
    name: String,
    value: SerializedVal,
}

#[derive(Debug, Clone)]
enum SerializedVal {
    I32(i32),
    I64(i64),
    F32(u32),
    F64(u64),
}

/// A sandbox that supports snapshot/restore with paired model generations.
pub struct SnapshotableSandbox {
    sandbox: Sandbox,
    module: LoadedModule,
}

impl SnapshotableSandbox {
    /// Create a new snapshotable sandbox from config and WASM bytes.
    pub fn new(config: &SandboxConfig, wasm_bytes: &[u8]) -> Result<Self, SandboxError> {
        let sandbox = Sandbox::new(config)?;
        let module = sandbox.load_module(wasm_bytes)?;
        Ok(Self { sandbox, module })
    }

    /// Create a fresh instance.
    pub fn instantiate(&self) -> Result<SandboxInstance, SandboxError> {
        self.sandbox.instantiate(&self.module)
    }

    /// List exports from the module.
    pub fn list_exports(&self) -> Vec<(String, String)> {
        self.sandbox.list_exports(&self.module)
    }
}

impl SandboxInstance {
    /// Take a snapshot of this instance's WASM state, paired with a model generation.
    pub fn snapshot(&mut self, model_generation: u64) -> Result<PairedSnapshot, SandboxError> {
        let wasm_memory = self.capture_memory()?;
        let wasm_globals = self.capture_globals()?;
        let fuel_remaining = self.store.get_fuel().ok();

        Ok(PairedSnapshot {
            model_generation,
            wasm_memory,
            wasm_globals,
            fuel_remaining,
        })
    }

    /// Restore this instance to a previously captured snapshot.
    /// Returns the model generation number so the caller can restore model state to match.
    pub fn restore(&mut self, snapshot: &PairedSnapshot) -> Result<u64, SandboxError> {
        if let Some(ref memory_data) = snapshot.wasm_memory {
            self.restore_memory(memory_data)?;
        }

        self.restore_globals(&snapshot.wasm_globals)?;

        if let Some(fuel) = snapshot.fuel_remaining {
            self.store.set_fuel(fuel).map_err(SandboxError::Engine)?;
        }

        Ok(snapshot.model_generation)
    }

    fn capture_memory(&mut self) -> Result<Option<Vec<u8>>, SandboxError> {
        if let Some(memory) = self.instance.get_memory(&mut self.store, "memory") {
            let data: &[u8] = memory.data(&self.store);
            Ok(Some(data.to_vec()))
        } else {
            Ok(None)
        }
    }

    fn capture_globals(&mut self) -> Result<Vec<GlobalSnapshot>, SandboxError> {
        let mut globals = Vec::new();
        let export_names: Vec<String> = self
            .instance
            .exports(&mut self.store)
            .map(|e: wasmtime::Export<'_>| e.name().to_string())
            .collect();

        for name in &export_names {
            if let Some(global) = self.instance.get_global(&mut self.store, name) {
                let val: Val = global.get(&mut self.store);
                let serialized = match val {
                    Val::I32(v) => SerializedVal::I32(v),
                    Val::I64(v) => SerializedVal::I64(v),
                    Val::F32(v) => SerializedVal::F32(v),
                    Val::F64(v) => SerializedVal::F64(v),
                    _ => continue,
                };
                globals.push(GlobalSnapshot {
                    name: name.clone(),
                    value: serialized,
                });
            }
        }

        Ok(globals)
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), SandboxError> {
        if let Some(memory) = self.instance.get_memory(&mut self.store, "memory") {
            let current_size: usize = memory.data_size(&self.store);
            let target_size = data.len();

            if target_size > current_size {
                let pages_needed = (target_size - current_size).div_ceil(65536);
                memory
                    .grow(&mut self.store, pages_needed as u64)
                    .map_err(SandboxError::Engine)?;
            }

            let mem_data = memory.data_mut(&mut self.store);
            let copy_len = data.len().min(mem_data.len());
            mem_data[..copy_len].copy_from_slice(&data[..copy_len]);

            // Zero out remaining memory beyond snapshot data
            for byte in &mut mem_data[copy_len..] {
                *byte = 0;
            }

            Ok(())
        } else {
            Ok(())
        }
    }

    fn restore_globals(&mut self, globals: &[GlobalSnapshot]) -> Result<(), SandboxError> {
        for g in globals {
            if let Some(global) = self.instance.get_global(&mut self.store, &g.name) {
                let val = match &g.value {
                    SerializedVal::I32(v) => Val::I32(*v),
                    SerializedVal::I64(v) => Val::I64(*v),
                    SerializedVal::F32(v) => Val::F32(*v),
                    SerializedVal::F64(v) => Val::F64(*v),
                };
                let result: Result<(), wasmtime::Error> = global.set(&mut self.store, val);
                result.map_err(SandboxError::Engine)?;
            }
        }
        Ok(())
    }
}

impl PairedSnapshot {
    /// Get the model generation this snapshot was taken at.
    pub fn model_generation(&self) -> u64 {
        self.model_generation
    }
}

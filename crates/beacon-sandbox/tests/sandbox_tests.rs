use beacon_sandbox::config::SandboxConfig;
use beacon_sandbox::sandbox::{Sandbox, SandboxError};

/// Helper: compile WAT to WASM bytes.
fn wat_to_wasm(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("valid WAT")
}

/// Minimal DUT that exports a few functions matching a document lifecycle binding.
const TEST_DUT_WAT: &str = r#"
(module
  (func (export "create_document") (param i32) (result i32)
    local.get 0)
  (func (export "get_document") (param i32 i32) (result i32)
    local.get 1)
  (func (export "delete_document") (param i32 i32)
    nop)
)
"#;

#[test]
fn test_load_valid_wasm_module() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let result = sandbox.load_module(&wasm);
    assert!(result.is_ok(), "should load valid WASM: {:?}", result.err());
}

#[test]
fn test_load_invalid_wasm_fails() {
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let result = sandbox.load_module(b"not wasm at all");
    assert!(result.is_err());
}

#[test]
fn test_instantiate_module() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let result = sandbox.instantiate(&module);
    assert!(result.is_ok(), "should instantiate: {:?}", result.err());
}

#[test]
fn test_call_exported_function() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    // create_document(42) should return 42 (echo function)
    let results = instance
        .call_func("create_document", &[42i32.into()])
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].i32(), Some(42));
}

#[test]
fn test_call_void_function() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    // delete_document(1, 2) returns void
    let results = instance
        .call_func("delete_document", &[1i32.into(), 2i32.into()])
        .unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_call_nonexistent_function_fails() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let result = instance.call_func("nonexistent", &[]);
    assert!(result.is_err());
    match result.unwrap_err() {
        SandboxError::ExportNotFound { name } => assert_eq!(name, "nonexistent"),
        other => panic!("expected ExportNotFound, got: {:?}", other),
    }
}

#[test]
fn test_fuel_exhaustion() {
    // A WASM module with an infinite loop
    let infinite_loop_wat = r#"
    (module
      (func (export "spin") (result i32)
        (local i32)
        (loop $loop
          (local.set 0 (i32.add (local.get 0) (i32.const 1)))
          (br $loop)
        )
        local.get 0
      )
    )
    "#;
    let wasm = wat_to_wasm(infinite_loop_wat);
    let config = SandboxConfig {
        fuel_per_action: Some(10_000), // very low fuel
        ..Default::default()
    };
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let result = instance.call_func("spin", &[]);
    assert!(result.is_err(), "should fail with fuel exhaustion");
    match result.unwrap_err() {
        SandboxError::FuelExhausted => {}
        other => panic!("expected FuelExhausted, got: {:?}", other),
    }
}

#[test]
fn test_no_wasi_access() {
    // A module that tries to import WASI functions should fail to instantiate
    // because we don't provide WASI.
    let wasi_module_wat = r#"
    (module
      (import "wasi_snapshot_preview1" "fd_write"
        (func (param i32 i32 i32 i32) (result i32)))
      (func (export "do_write")
        nop)
    )
    "#;
    let wasm = wat_to_wasm(wasi_module_wat);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();

    // Instantiation should fail because we don't link WASI imports
    let result = sandbox.instantiate(&module);
    assert!(result.is_err(), "should fail without WASI");
}

#[test]
fn test_memory_limit() {
    // A module that tries to allocate a lot of memory
    let big_memory_wat = r#"
    (module
      (memory (export "memory") 1)
      (func (export "grow_memory") (param i32) (result i32)
        (memory.grow (local.get 0))
      )
    )
    "#;
    let wasm = wat_to_wasm(big_memory_wat);
    // Set a small memory limit (2MB)
    let config = SandboxConfig {
        memory_limit_bytes: 2 * 1024 * 1024,
        ..Default::default()
    };
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    // Try to grow by 100 pages (6.4MB) — should fail (return -1)
    let results = instance.call_func("grow_memory", &[100i32.into()]).unwrap();
    assert_eq!(
        results[0].i32(),
        Some(-1),
        "memory growth beyond limit should return -1"
    );
}

#[test]
fn test_list_exports() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();

    let exports = sandbox.list_exports(&module);
    assert!(exports.contains(&("create_document".to_string(), "func".to_string())));
    assert!(exports.contains(&("get_document".to_string(), "func".to_string())));
    assert!(exports.contains(&("delete_document".to_string(), "func".to_string())));
}

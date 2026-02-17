use std::collections::HashMap;

use beacon_ir::types::{ActionBinding, Bindings, EventHooks};
use beacon_sandbox::config::SandboxConfig;
use beacon_sandbox::sandbox::Sandbox;
use beacon_vif::adapter::{ObserverResult, VerificationAdapter};

fn wat_to_wasm(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("valid WAT")
}

/// Test DUT: document lifecycle with observers.
const TEST_DUT_WAT: &str = r#"
(module
  (memory (export "memory") 1)

  ;; Simple state: store doc count at offset 0, last actor at offset 4
  (global $doc_count (mut i32) (i32.const 0))

  ;; create_document(actor_id) -> doc_id
  (func (export "create_document") (param i32) (result i32)
    (global.set $doc_count (i32.add (global.get $doc_count) (i32.const 1)))
    (i32.store (i32.const 4) (local.get 0))
    (global.get $doc_count)
  )

  ;; get_document(actor_id, doc_id) -> doc_id (echo)
  (func (export "get_document") (param i32 i32) (result i32)
    (i32.store (i32.const 4) (local.get 0))
    local.get 1
  )

  ;; delete_document(actor_id, doc_id) -> void
  (func (export "delete_document") (param i32 i32)
    (i32.store (i32.const 4) (local.get 0))
    nop
  )

  ;; Observer: get_doc_count() -> i32
  (func (export "get_doc_count") (result i32)
    (global.get $doc_count)
  )

  ;; Observer: check_access(user_id, doc_id) -> bool (1=true, 0=false)
  (func (export "check_access") (param i32 i32) (result i32)
    ;; Simple: always return 1 (accessible)
    (i32.const 1)
  )
)
"#;

fn make_test_bindings() -> Bindings {
    let mut actions = HashMap::new();
    actions.insert(
        "create_document".to_string(),
        ActionBinding {
            function: "create_document".to_string(),
            args: vec!["actor_id".to_string()],
            returns: serde_json::json!({ "type": "Document" }),
            mutates: true,
            idempotent: false,
            reads: vec![],
            writes: vec!["Document".to_string()],
        },
    );
    actions.insert(
        "read".to_string(),
        ActionBinding {
            function: "get_document".to_string(),
            args: vec!["actor_id".to_string(), "doc_id".to_string()],
            returns: serde_json::json!({ "type": "Document" }),
            mutates: false,
            idempotent: true,
            reads: vec!["Document".to_string()],
            writes: vec![],
        },
    );
    actions.insert(
        "delete".to_string(),
        ActionBinding {
            function: "delete_document".to_string(),
            args: vec!["actor_id".to_string(), "doc_id".to_string()],
            returns: serde_json::json!({ "type": "void" }),
            mutates: true,
            idempotent: false,
            reads: vec![],
            writes: vec!["Document".to_string()],
        },
    );

    Bindings {
        runtime: "wasm".to_string(),
        entry: "main.wasm".to_string(),
        actions,
        event_hooks: EventHooks {
            mode: "function_intercept".to_string(),
            observe: vec!["create_document".to_string(), "delete".to_string()],
            capture: vec!["args".to_string(), "return_value".to_string()],
        },
    }
}

#[test]
fn test_adapter_from_bindings() {
    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    assert!(adapter.has_action("create_document"));
    assert!(adapter.has_action("read"));
    assert!(adapter.has_action("delete"));
    assert!(!adapter.has_action("nonexistent"));

    assert_eq!(adapter.function_for_action("read"), Some("get_document"));
}

#[test]
fn test_execute_action_create() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    // Execute create_document with actor_id=42
    let result = adapter.execute_action(&mut instance, "create_document", &[42]);

    assert_eq!(result.action, "create_document");
    assert_eq!(result.function, "create_document");
    assert_eq!(result.args, vec![42]);
    assert_eq!(result.return_value, Some(1)); // First doc created = 1
    assert!(!result.trapped);
    assert!(result.error.is_none());
}

#[test]
fn test_execute_action_with_fuel_tracking() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig {
        fuel_per_action: Some(1_000_000),
        ..Default::default()
    };
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    let result = adapter.execute_action(&mut instance, "create_document", &[1]);
    assert!(result.fuel_consumed.is_some());
    assert!(result.fuel_consumed.unwrap() > 0);
}

#[test]
fn test_execute_void_action() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    let result = adapter.execute_action(&mut instance, "delete", &[1, 1]);
    assert_eq!(result.action, "delete");
    assert_eq!(result.return_value, None); // void
    assert!(!result.trapped);
}

#[test]
fn test_execute_missing_action() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    let result = adapter.execute_action(&mut instance, "nonexistent", &[]);
    assert!(result.error.is_some());
    assert!(result.error.unwrap().contains("No binding"));
}

#[test]
fn test_sequential_actions() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    // Create 3 documents
    let r1 = adapter.execute_action(&mut instance, "create_document", &[1]);
    let r2 = adapter.execute_action(&mut instance, "create_document", &[2]);
    let r3 = adapter.execute_action(&mut instance, "create_document", &[3]);

    assert_eq!(r1.return_value, Some(1));
    assert_eq!(r2.return_value, Some(2));
    assert_eq!(r3.return_value, Some(3));
}

#[test]
fn test_observer_query() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let mut adapter = VerificationAdapter::from_bindings(&bindings);

    // Register observers
    adapter.register_observer("get_doc_count", "get_doc_count", &[]);
    adapter.register_observer(
        "check_access",
        "check_access",
        &["user_id".to_string(), "doc_id".to_string()],
    );

    // Create a document
    adapter.execute_action(&mut instance, "create_document", &[1]);

    // Query observer: doc count should be 1
    let obs = adapter.query_observer(&mut instance, "get_doc_count", &[]);
    assert_eq!(obs.value, Some(1));
    assert!(obs.error.is_none());

    // Query observer: check_access should return 1 (true)
    let obs = adapter.query_observer(&mut instance, "check_access", &[1, 1]);
    assert_eq!(obs.value, Some(1));
    assert!(obs.error.is_none());
}

#[test]
fn test_observer_result_is_tagged() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let mut adapter = VerificationAdapter::from_bindings(&bindings);
    adapter.register_observer("get_doc_count", "get_doc_count", &[]);

    let obs: ObserverResult = adapter.query_observer(&mut instance, "get_doc_count", &[]);

    // The result is an ObserverResult, not a raw value — type system ensures
    // observer results can't be confused with model truth.
    assert_eq!(obs.observer, "get_doc_count");
    assert_eq!(obs.function, "get_doc_count");
}

#[test]
fn test_missing_observer_returns_error() {
    let wasm = wat_to_wasm(TEST_DUT_WAT);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();

    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    let obs = adapter.query_observer(&mut instance, "nonexistent", &[]);
    assert!(obs.error.is_some());
    assert!(obs.error.unwrap().contains("No observer binding"));
}

#[test]
fn test_action_names() {
    let bindings = make_test_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    let names = adapter.action_names();
    assert!(names.contains(&"create_document"));
    assert!(names.contains(&"read"));
    assert!(names.contains(&"delete"));
    assert_eq!(names.len(), 3);
}

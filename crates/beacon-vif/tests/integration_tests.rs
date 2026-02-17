//! End-to-end integration test: load DUT -> validate interface -> execute actions
//! -> query observers -> snapshot/restore. Exercises the full puppet-master
//! relationship between Beacon and the DUT.

use std::collections::HashMap;

use beacon_ir::types::{ActionBinding, Bindings, EventHooks};
use beacon_sandbox::config::SandboxConfig;
use beacon_sandbox::sandbox::Sandbox;
use beacon_vif::adapter::VerificationAdapter;
use beacon_vif::validate::{validate_interface, validate_signatures};

fn wat_to_wasm(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("valid WAT")
}

/// A more realistic DUT with stateful behavior.
const DOCUMENT_DUT_WAT: &str = r#"
(module
  (memory (export "memory") 1)

  ;; State: doc_count at offset 0, per-doc visibility at offset 100+id*4
  (global $next_id (mut i32) (i32.const 0))

  ;; create_document(actor_id) -> doc_id
  (func (export "create_document") (param $actor i32) (result i32)
    (local $id i32)
    ;; Allocate new ID
    (local.set $id (global.get $next_id))
    (global.set $next_id (i32.add (global.get $next_id) (i32.const 1)))
    ;; Store doc count
    (i32.store (i32.const 0) (global.get $next_id))
    ;; Store visibility = 0 (private) at offset 100 + id*4
    (i32.store
      (i32.add (i32.const 100) (i32.mul (local.get $id) (i32.const 4)))
      (i32.const 0))
    ;; Store owner at offset 200 + id*4
    (i32.store
      (i32.add (i32.const 200) (i32.mul (local.get $id) (i32.const 4)))
      (local.get $actor))
    (local.get $id)
  )

  ;; get_document(actor_id, doc_id) -> visibility
  (func (export "get_document") (param $actor i32) (param $doc_id i32) (result i32)
    (i32.load
      (i32.add (i32.const 100) (i32.mul (local.get $doc_id) (i32.const 4))))
  )

  ;; delete_document(actor_id, doc_id) -> void
  ;; Sets visibility to -1 (deleted)
  (func (export "delete_document") (param $actor i32) (param $doc_id i32)
    (i32.store
      (i32.add (i32.const 100) (i32.mul (local.get $doc_id) (i32.const 4)))
      (i32.const -1))
  )

  ;; Observer: get_doc_count() -> i32
  (func (export "get_doc_count") (result i32)
    (i32.load (i32.const 0))
  )

  ;; Observer: get_visibility(doc_id) -> visibility
  (func (export "get_visibility") (param $doc_id i32) (result i32)
    (i32.load
      (i32.add (i32.const 100) (i32.mul (local.get $doc_id) (i32.const 4))))
  )

  ;; Observer: get_owner(doc_id) -> owner_id
  (func (export "get_owner") (param $doc_id i32) (result i32)
    (i32.load
      (i32.add (i32.const 200) (i32.mul (local.get $doc_id) (i32.const 4))))
  )
)
"#;

fn make_document_bindings() -> Bindings {
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
            observe: vec![
                "create_document".to_string(),
                "delete".to_string(),
            ],
            capture: vec!["args".to_string(), "return_value".to_string()],
        },
    }
}

#[test]
fn test_full_dut_lifecycle() {
    let wasm = wat_to_wasm(DOCUMENT_DUT_WAT);
    let config = SandboxConfig {
        fuel_per_action: Some(1_000_000),
        ..Default::default()
    };

    // Step 1: Load module
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();

    // Step 2: Validate interface
    let exports = sandbox.list_exports(&module);
    let bindings = make_document_bindings();
    validate_interface(&exports, &bindings).expect("interface should validate");

    // Step 3: Validate signatures
    let mut sigs = HashMap::new();
    sigs.insert("create_document".to_string(), (1usize, 1usize));
    sigs.insert("get_document".to_string(), (2, 1));
    sigs.insert("delete_document".to_string(), (2, 0));
    validate_signatures(&sigs, &bindings).expect("signatures should match");

    // Step 4: Instantiate and set up adapter
    let mut instance = sandbox.instantiate(&module).unwrap();
    let mut adapter = VerificationAdapter::from_bindings(&bindings);
    adapter.register_observer("get_doc_count", "get_doc_count", &[]);
    adapter.register_observer("get_visibility", "get_visibility", &["doc_id".to_string()]);
    adapter.register_observer("get_owner", "get_owner", &["doc_id".to_string()]);

    // Step 5: Execute actions — create two documents
    let r1 = adapter.execute_action(&mut instance, "create_document", &[100]);
    assert_eq!(r1.return_value, Some(0)); // first doc ID = 0
    assert!(!r1.trapped);
    assert!(r1.fuel_consumed.unwrap() > 0);

    let r2 = adapter.execute_action(&mut instance, "create_document", &[200]);
    assert_eq!(r2.return_value, Some(1)); // second doc ID = 1

    // Step 6: Query observers — verify DUT state
    let obs_count = adapter.query_observer(&mut instance, "get_doc_count", &[]);
    assert_eq!(obs_count.value, Some(2)); // 2 documents created

    let obs_vis = adapter.query_observer(&mut instance, "get_visibility", &[0]);
    assert_eq!(obs_vis.value, Some(0)); // doc 0 is private (0)

    let obs_owner = adapter.query_observer(&mut instance, "get_owner", &[0]);
    assert_eq!(obs_owner.value, Some(100)); // doc 0 owned by actor 100

    let obs_owner2 = adapter.query_observer(&mut instance, "get_owner", &[1]);
    assert_eq!(obs_owner2.value, Some(200)); // doc 1 owned by actor 200

    // Step 7: Delete a document
    let r3 = adapter.execute_action(&mut instance, "delete", &[100, 0]);
    assert!(r3.error.is_none());
    assert!(!r3.trapped);

    // Verify deletion via observer
    let obs_deleted = adapter.query_observer(&mut instance, "get_visibility", &[0]);
    assert_eq!(obs_deleted.value, Some(-1)); // deleted = -1
}

#[test]
fn test_snapshot_restore_with_adapter() {
    let wasm = wat_to_wasm(DOCUMENT_DUT_WAT);
    let config = SandboxConfig {
        fuel_per_action: Some(1_000_000),
        ..Default::default()
    };

    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();
    let bindings = make_document_bindings();
    let mut adapter = VerificationAdapter::from_bindings(&bindings);
    adapter.register_observer("get_doc_count", "get_doc_count", &[]);
    adapter.register_observer("get_visibility", "get_visibility", &["doc_id".to_string()]);

    // Create a document
    adapter.execute_action(&mut instance, "create_document", &[100]);

    // Snapshot at model generation 1
    let snap = instance.snapshot(1).unwrap();

    // Create another document and delete the first
    adapter.execute_action(&mut instance, "create_document", &[200]);
    adapter.execute_action(&mut instance, "delete", &[100, 0]);

    // Verify current state
    let obs = adapter.query_observer(&mut instance, "get_doc_count", &[]);
    assert_eq!(obs.value, Some(2)); // 2 documents
    let obs = adapter.query_observer(&mut instance, "get_visibility", &[0]);
    assert_eq!(obs.value, Some(-1)); // doc 0 deleted

    // Restore to snapshot
    let gen = instance.restore(&snap).unwrap();
    assert_eq!(gen, 1);

    // Verify restored state
    let obs = adapter.query_observer(&mut instance, "get_doc_count", &[]);
    assert_eq!(obs.value, Some(1)); // back to 1 document
    let obs = adapter.query_observer(&mut instance, "get_visibility", &[0]);
    assert_eq!(obs.value, Some(0)); // doc 0 is private again, not deleted
}

#[test]
fn test_interface_validation_catches_bad_dut() {
    // Module that's missing some expected functions
    let incomplete_wat = r#"
    (module
      (func (export "create_document") (param i32) (result i32)
        local.get 0)
      ;; Missing get_document and delete_document!
    )
    "#;
    let wasm = wat_to_wasm(incomplete_wat);
    let config = SandboxConfig::default();
    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();

    let exports = sandbox.list_exports(&module);
    let bindings = make_document_bindings();
    let result = validate_interface(&exports, &bindings);

    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should flag the 2 missing functions
    assert_eq!(errors.len(), 2);
}

#[test]
fn test_fuel_metering_reports_consumption() {
    let wasm = wat_to_wasm(DOCUMENT_DUT_WAT);
    let config = SandboxConfig {
        fuel_per_action: Some(1_000_000),
        ..Default::default()
    };

    let sandbox = Sandbox::new(&config).unwrap();
    let module = sandbox.load_module(&wasm).unwrap();
    let mut instance = sandbox.instantiate(&module).unwrap();
    let bindings = make_document_bindings();
    let adapter = VerificationAdapter::from_bindings(&bindings);

    // Execute multiple actions, verify fuel consumption is tracked
    let r1 = adapter.execute_action(&mut instance, "create_document", &[1]);
    let r2 = adapter.execute_action(&mut instance, "read", &[1, 0]);
    let r3 = adapter.execute_action(&mut instance, "delete", &[1, 0]);

    // All should have fuel consumed > 0
    assert!(r1.fuel_consumed.unwrap() > 0);
    assert!(r2.fuel_consumed.unwrap() > 0);
    assert!(r3.fuel_consumed.unwrap() > 0);

    // Reads should generally consume less fuel than writes
    // (but this depends on the specific WASM, so just check they're all positive)
    assert!(r1.fuel_consumed.unwrap() < 1_000_000);
    assert!(r2.fuel_consumed.unwrap() < 1_000_000);
    assert!(r3.fuel_consumed.unwrap() < 1_000_000);
}

use beacon_sandbox::config::SandboxConfig;
use beacon_sandbox::snapshot::SnapshotableSandbox;

fn wat_to_wasm(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("valid WAT")
}

/// A stateful WASM module with mutable memory and a global counter.
const STATEFUL_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (global $counter (mut i32) (i32.const 0))
  (global (export "counter") (mut i32) (i32.const 0))

  ;; Increment counter and write value at offset 0
  (func (export "increment") (result i32)
    (global.set $counter (i32.add (global.get $counter) (i32.const 1)))
    (global.set 1 (global.get $counter))
    (i32.store (i32.const 0) (global.get $counter))
    (global.get $counter)
  )

  ;; Read counter from memory at offset 0
  (func (export "read_counter") (result i32)
    (i32.load (i32.const 0))
  )

  ;; Store a value at a given memory offset
  (func (export "store_at") (param i32 i32)
    (i32.store (local.get 0) (local.get 1))
  )

  ;; Load a value from a given memory offset
  (func (export "load_from") (param i32) (result i32)
    (i32.load (local.get 0))
  )
)
"#;

#[test]
fn test_snapshot_preserves_model_generation() {
    let wasm = wat_to_wasm(STATEFUL_WAT);
    let config = SandboxConfig::default();
    let ssb = SnapshotableSandbox::new(&config, &wasm).unwrap();
    let mut instance = ssb.instantiate().unwrap();

    let snap = instance.snapshot(42).unwrap();
    assert_eq!(snap.model_generation(), 42);
}

#[test]
fn test_snapshot_and_restore_memory() {
    let wasm = wat_to_wasm(STATEFUL_WAT);
    let config = SandboxConfig::default();
    let ssb = SnapshotableSandbox::new(&config, &wasm).unwrap();
    let mut instance = ssb.instantiate().unwrap();

    // Write some data
    instance
        .call_func("store_at", &[0i32.into(), 999i32.into()])
        .unwrap();
    instance
        .call_func("store_at", &[4i32.into(), 888i32.into()])
        .unwrap();

    // Take snapshot at generation 5
    let snap = instance.snapshot(5).unwrap();

    // Mutate more
    instance
        .call_func("store_at", &[0i32.into(), 111i32.into()])
        .unwrap();
    instance
        .call_func("store_at", &[4i32.into(), 222i32.into()])
        .unwrap();

    // Verify mutation happened
    let val = instance.call_func("load_from", &[0i32.into()]).unwrap();
    assert_eq!(val[0].i32(), Some(111));

    // Restore snapshot
    let gen = instance.restore(&snap).unwrap();
    assert_eq!(gen, 5);

    // Verify memory was restored
    let val = instance.call_func("load_from", &[0i32.into()]).unwrap();
    assert_eq!(val[0].i32(), Some(999));
    let val = instance.call_func("load_from", &[4i32.into()]).unwrap();
    assert_eq!(val[0].i32(), Some(888));
}

#[test]
fn test_snapshot_and_restore_globals() {
    let wasm = wat_to_wasm(STATEFUL_WAT);
    let config = SandboxConfig::default();
    let ssb = SnapshotableSandbox::new(&config, &wasm).unwrap();
    let mut instance = ssb.instantiate().unwrap();

    // Increment counter 3 times
    instance.call_func("increment", &[]).unwrap();
    instance.call_func("increment", &[]).unwrap();
    instance.call_func("increment", &[]).unwrap();

    // Take snapshot (counter should be 3)
    let snap = instance.snapshot(10).unwrap();

    // Increment more
    instance.call_func("increment", &[]).unwrap();
    instance.call_func("increment", &[]).unwrap();

    // Counter is now 5 in memory
    let val = instance.call_func("read_counter", &[]).unwrap();
    assert_eq!(val[0].i32(), Some(5));

    // Restore
    let gen = instance.restore(&snap).unwrap();
    assert_eq!(gen, 10);

    // Memory should show 3 again
    let val = instance.call_func("read_counter", &[]).unwrap();
    assert_eq!(val[0].i32(), Some(3));
}

#[test]
fn test_snapshot_no_memory_module() {
    // Module without memory
    let no_mem_wat = r#"
    (module
      (func (export "add") (param i32 i32) (result i32)
        (i32.add (local.get 0) (local.get 1)))
    )
    "#;
    let wasm = wat_to_wasm(no_mem_wat);
    let config = SandboxConfig::default();
    let ssb = SnapshotableSandbox::new(&config, &wasm).unwrap();
    let mut instance = ssb.instantiate().unwrap();

    // Snapshot should work even without memory
    let snap = instance.snapshot(0).unwrap();

    // Call function
    let val = instance
        .call_func("add", &[3i32.into(), 4i32.into()])
        .unwrap();
    assert_eq!(val[0].i32(), Some(7));

    // Restore should succeed
    let gen = instance.restore(&snap).unwrap();
    assert_eq!(gen, 0);

    // Function should still work
    let val = instance
        .call_func("add", &[10i32.into(), 20i32.into()])
        .unwrap();
    assert_eq!(val[0].i32(), Some(30));
}

#[test]
fn test_multiple_snapshots() {
    let wasm = wat_to_wasm(STATEFUL_WAT);
    let config = SandboxConfig::default();
    let ssb = SnapshotableSandbox::new(&config, &wasm).unwrap();
    let mut instance = ssb.instantiate().unwrap();

    // State 1: counter = 1
    instance.call_func("increment", &[]).unwrap();
    let snap1 = instance.snapshot(1).unwrap();

    // State 2: counter = 3
    instance.call_func("increment", &[]).unwrap();
    instance.call_func("increment", &[]).unwrap();
    let snap2 = instance.snapshot(3).unwrap();

    // State 3: counter = 5
    instance.call_func("increment", &[]).unwrap();
    instance.call_func("increment", &[]).unwrap();

    // Restore to snap1 (counter=1)
    instance.restore(&snap1).unwrap();
    let val = instance.call_func("read_counter", &[]).unwrap();
    assert_eq!(val[0].i32(), Some(1));

    // Restore to snap2 (counter=3)
    instance.restore(&snap2).unwrap();
    let val = instance.call_func("read_counter", &[]).unwrap();
    assert_eq!(val[0].i32(), Some(3));
}

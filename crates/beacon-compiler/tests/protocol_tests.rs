use beacon_compiler::graph::GraphNode;
use beacon_compiler::predicate::TypeContext;
use beacon_compiler::protocol::compile_protocol;
use beacon_ir::parse::parse_ir;
use beacon_ir::types::Protocol;

fn make_test_context() -> TypeContext {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    TypeContext::from_ir(&ir)
}

fn parse_protocol(json: serde_json::Value) -> Protocol {
    serde_json::from_value(json).unwrap()
}

fn get_protocols() -> std::collections::HashMap<String, Protocol> {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    ir.protocols
}

#[test]
fn test_single_call_produces_start_terminal_end() {
    let ctx = make_test_context();
    let protocols = std::collections::HashMap::new();
    let proto = parse_protocol(serde_json::json!({
        "root": { "type": "call", "action": "read" }
    }));
    let graph = compile_protocol(&proto, &ctx, &protocols).unwrap();
    // Should have: Start -> Terminal(read) -> End
    assert!(graph.nodes.len() >= 3);
    assert!(matches!(graph.nodes[graph.entry as usize], GraphNode::Start));
    assert!(matches!(graph.nodes[graph.exit as usize], GraphNode::End));
    // Find the terminal node
    let terminals: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| matches!(n, GraphNode::Terminal { .. }))
        .collect();
    assert_eq!(terminals.len(), 1);
    if let GraphNode::Terminal { action, .. } = &terminals[0] {
        assert_eq!(action, "read");
    }
}

#[test]
fn test_seq_chains_nodes() {
    let ctx = make_test_context();
    let protocols = std::collections::HashMap::new();
    let proto = parse_protocol(serde_json::json!({
        "root": {
            "type": "seq",
            "children": [
                { "type": "call", "action": "create" },
                { "type": "call", "action": "read" }
            ]
        }
    }));
    let graph = compile_protocol(&proto, &ctx, &protocols).unwrap();
    // Should have 2 terminal nodes in sequence
    let terminals: Vec<_> = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, GraphNode::Terminal { .. }))
        .collect();
    assert_eq!(terminals.len(), 2);
    // Verify they're connected in sequence via edges
    let (create_id, _) = terminals[0];
    let (read_id, _) = terminals[1];
    assert!(graph.edges.iter().any(|(from, to)| *from == create_id as u32 && *to == read_id as u32));
}

#[test]
fn test_alt_produces_branch_node() {
    let ctx = make_test_context();
    let protocols = std::collections::HashMap::new();
    let proto = parse_protocol(serde_json::json!({
        "root": {
            "type": "alt",
            "branches": [
                { "id": "a", "weight": 60, "body": { "type": "call", "action": "read" } },
                { "id": "b", "weight": 40, "body": { "type": "call", "action": "write" } }
            ]
        }
    }));
    let graph = compile_protocol(&proto, &ctx, &protocols).unwrap();
    // Should have a Branch node
    let branches: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| matches!(n, GraphNode::Branch { .. }))
        .collect();
    assert_eq!(branches.len(), 1);
    if let GraphNode::Branch { alternatives } = &branches[0] {
        assert_eq!(alternatives.len(), 2);
        assert_eq!(alternatives[0].id, "a");
        assert!((alternatives[0].weight - 60.0).abs() < f64::EPSILON);
        assert_eq!(alternatives[1].id, "b");
        assert!((alternatives[1].weight - 40.0).abs() < f64::EPSILON);
    }
}

#[test]
fn test_repeat_produces_loop_structure() {
    let ctx = make_test_context();
    let protocols = std::collections::HashMap::new();
    let proto = parse_protocol(serde_json::json!({
        "root": {
            "type": "repeat",
            "min": 1,
            "max": 5,
            "body": { "type": "call", "action": "read" }
        }
    }));
    let graph = compile_protocol(&proto, &ctx, &protocols).unwrap();
    // Should have LoopEntry and LoopExit nodes
    let loop_entries: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| matches!(n, GraphNode::LoopEntry { .. }))
        .collect();
    assert_eq!(loop_entries.len(), 1);
    if let GraphNode::LoopEntry { min, max, .. } = &loop_entries[0] {
        assert_eq!(*min, 1);
        assert_eq!(*max, 5);
    }
    let loop_exits: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| matches!(n, GraphNode::LoopExit))
        .collect();
    assert_eq!(loop_exits.len(), 1);
}

#[test]
fn test_ref_inlines_referenced_protocol() {
    let ctx = make_test_context();
    let protocols = get_protocols();
    // The "idle" protocol is just a single "read" call
    let proto = parse_protocol(serde_json::json!({
        "root": { "type": "ref", "protocol": "idle" }
    }));
    let graph = compile_protocol(&proto, &ctx, &protocols).unwrap();
    // Should have inlined the "read" terminal from the idle protocol
    let terminals: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| matches!(n, GraphNode::Terminal { .. }))
        .collect();
    assert_eq!(terminals.len(), 1);
    if let GraphNode::Terminal { action, .. } = &terminals[0] {
        assert_eq!(action, "read");
    }
}

#[test]
fn test_full_document_lifecycle_compiles() {
    let ctx = make_test_context();
    let protocols = get_protocols();
    let proto = protocols.get("document_lifecycle").unwrap();
    let graph = compile_protocol(proto, &ctx, &protocols).unwrap();
    // Sanity checks on the compiled graph
    assert!(graph.nodes.len() > 5);
    assert!(!graph.edges.is_empty());
    assert!(matches!(graph.nodes[graph.entry as usize], GraphNode::Start));
    assert!(matches!(graph.nodes[graph.exit as usize], GraphNode::End));
}

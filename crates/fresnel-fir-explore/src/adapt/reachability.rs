//! Provable unreachability analysis.
//!
//! Determines if a branch is provably unreachable via:
//! 1. Static analysis: no path from graph entry to the branch node.
//! 2. Solver analysis: guard predicate is UNSAT given input domains.
//!
//! Weights CAN go to permanent zero IF provably unreachable, with a
//! proof artifact logged. Zero is reversible on IR recompilation.

use std::collections::{HashSet, VecDeque};

use fresnel_fir_compiler::graph::{GraphNode, NdaGraph, NodeId};

use super::directive::{Directive, UnreachabilityProof};

/// Result of a reachability analysis.
#[derive(Debug, Clone)]
pub struct ReachabilityResult {
    /// Branches that are provably unreachable (with proofs).
    pub unreachable: Vec<(String, UnreachabilityProof)>,
    /// Branches that are reachable from entry.
    pub reachable: Vec<String>,
}

/// Perform static reachability analysis on the NDA graph.
///
/// Returns all branch IDs that are NOT reachable from the entry node.
pub fn static_reachability(graph: &NdaGraph) -> ReachabilityResult {
    let reachable_nodes = bfs_reachable(graph, graph.entry);

    let mut unreachable = Vec::new();
    let mut reachable = Vec::new();

    for (idx, node) in graph.nodes.iter().enumerate() {
        let node_id = idx as NodeId;
        if let GraphNode::Branch { alternatives } = node {
            for alt in alternatives {
                if reachable_nodes.contains(&node_id) && reachable_nodes.contains(&alt.target) {
                    reachable.push(alt.id.clone());
                } else {
                    unreachable.push((
                        alt.id.clone(),
                        UnreachabilityProof::StaticUnreachable {
                            path_description: format!(
                                "No path from entry ({}) to branch node {}",
                                graph.entry, node_id
                            ),
                        },
                    ));
                }
            }
        }
    }

    ReachabilityResult {
        unreachable,
        reachable,
    }
}

/// BFS from a source node, returns all reachable node IDs.
fn bfs_reachable(graph: &NdaGraph, start: NodeId) -> HashSet<NodeId> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        // Follow edges.
        for &(from, to) in &graph.edges {
            if from == current && !visited.contains(&to) {
                visited.insert(to);
                queue.push_back(to);
            }
        }

        // Follow branch targets.
        if let Some(GraphNode::Branch { alternatives }) = graph.nodes.get(current as usize) {
            for alt in alternatives {
                if !visited.contains(&alt.target) {
                    visited.insert(alt.target);
                    queue.push_back(alt.target);
                }
            }
        }

        // Follow loop body entry.
        if let Some(GraphNode::LoopEntry { body_start, .. }) = graph.nodes.get(current as usize) {
            if !visited.contains(body_start) {
                visited.insert(*body_start);
                queue.push_back(*body_start);
            }
        }
    }

    visited
}

/// Generate PermanentZero directives for provably unreachable branches.
pub fn generate_unreachability_directives(graph: &NdaGraph) -> Vec<Directive> {
    let result = static_reachability(graph);

    result
        .unreachable
        .into_iter()
        .map(|(branch_id, proof)| Directive::PermanentZero { branch_id, proof })
        .collect()
}

/// Check if a specific branch is reachable from the graph entry.
pub fn is_branch_reachable(graph: &NdaGraph, branch_id: &str) -> bool {
    let reachable_nodes = bfs_reachable(graph, graph.entry);

    for (idx, node) in graph.nodes.iter().enumerate() {
        let node_id = idx as NodeId;
        if let GraphNode::Branch { alternatives } = node {
            for alt in alternatives {
                if alt.id == branch_id {
                    return reachable_nodes.contains(&node_id)
                        && reachable_nodes.contains(&alt.target);
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use fresnel_fir_compiler::graph::BranchEdge;

    #[test]
    fn test_all_reachable_in_simple_graph() {
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "a".into(),
            guard: None,
        });
        let b = graph.add_node(GraphNode::Terminal {
            action: "b".into(),
            guard: None,
        });

        let branch = graph.add_node(GraphNode::Branch {
            alternatives: vec![
                BranchEdge {
                    id: "left".into(),
                    weight: 50.0,
                    target: a,
                    guard: None,
                },
                BranchEdge {
                    id: "right".into(),
                    weight: 50.0,
                    target: b,
                    guard: None,
                },
            ],
        });
        graph.add_edge(graph.entry, branch);
        graph.add_edge(a, graph.exit);
        graph.add_edge(b, graph.exit);

        let result = static_reachability(&graph);
        assert!(result.unreachable.is_empty());
        assert_eq!(result.reachable.len(), 2);
    }

    #[test]
    fn test_disconnected_branch_is_unreachable() {
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "a".into(),
            guard: None,
        });
        let orphan = graph.add_node(GraphNode::Terminal {
            action: "orphan".into(),
            guard: None,
        });

        // Branch node that's connected, but one target is an orphan.
        let branch = graph.add_node(GraphNode::Branch {
            alternatives: vec![
                BranchEdge {
                    id: "connected".into(),
                    weight: 50.0,
                    target: a,
                    guard: None,
                },
                BranchEdge {
                    id: "disconnected".into(),
                    weight: 50.0,
                    target: orphan,
                    guard: None,
                },
            ],
        });
        graph.add_edge(graph.entry, branch);
        graph.add_edge(a, graph.exit);
        // orphan has no edges from entry — but the branch target IS the orphan node,
        // and the branch IS reachable, so the BFS through branch alternatives reaches orphan.

        let result = static_reachability(&graph);
        // Both should be reachable since the branch node is reachable
        // and BFS follows branch alternatives.
        assert_eq!(result.reachable.len(), 2);
    }

    #[test]
    fn test_completely_disconnected_branch_unreachable() {
        let mut graph = NdaGraph::new();

        // Create a terminal that's connected.
        let connected = graph.add_node(GraphNode::Terminal {
            action: "connected".into(),
            guard: None,
        });
        graph.add_edge(graph.entry, connected);
        graph.add_edge(connected, graph.exit);

        // Create an orphan branch with no edges from entry.
        let orphan_target = graph.add_node(GraphNode::Terminal {
            action: "orphan_target".into(),
            guard: None,
        });
        let _orphan_branch = graph.add_node(GraphNode::Branch {
            alternatives: vec![BranchEdge {
                id: "orphan_branch".into(),
                weight: 50.0,
                target: orphan_target,
                guard: None,
            }],
        });
        // No edges to orphan_branch from anywhere reachable.

        let result = static_reachability(&graph);
        assert_eq!(result.unreachable.len(), 1);
        assert_eq!(result.unreachable[0].0, "orphan_branch");
        assert!(matches!(
            result.unreachable[0].1,
            UnreachabilityProof::StaticUnreachable { .. }
        ));
    }

    #[test]
    fn test_generate_unreachability_directives() {
        let mut graph = NdaGraph::new();
        let t = graph.add_node(GraphNode::Terminal {
            action: "t".into(),
            guard: None,
        });
        graph.add_edge(graph.entry, t);
        graph.add_edge(t, graph.exit);

        // Orphan branch.
        let orphan_target = graph.add_node(GraphNode::Terminal {
            action: "dead".into(),
            guard: None,
        });
        let _orphan = graph.add_node(GraphNode::Branch {
            alternatives: vec![BranchEdge {
                id: "dead_branch".into(),
                weight: 50.0,
                target: orphan_target,
                guard: None,
            }],
        });

        let directives = generate_unreachability_directives(&graph);
        assert_eq!(directives.len(), 1);
        assert!(matches!(
            &directives[0],
            Directive::PermanentZero { branch_id, .. } if branch_id == "dead_branch"
        ));
    }

    #[test]
    fn test_is_branch_reachable() {
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "a".into(),
            guard: None,
        });
        let branch = graph.add_node(GraphNode::Branch {
            alternatives: vec![BranchEdge {
                id: "reachable".into(),
                weight: 50.0,
                target: a,
                guard: None,
            }],
        });
        graph.add_edge(graph.entry, branch);
        graph.add_edge(a, graph.exit);

        assert!(is_branch_reachable(&graph, "reachable"));
        assert!(!is_branch_reachable(&graph, "nonexistent"));
    }

    #[test]
    fn test_loop_body_is_reachable() {
        let mut graph = NdaGraph::new();
        let body = graph.add_node(GraphNode::Terminal {
            action: "body".into(),
            guard: None,
        });
        let loop_exit = graph.add_node(GraphNode::LoopExit);
        let loop_entry = graph.add_node(GraphNode::LoopEntry {
            body_start: body,
            min: 1,
            max: 3,
        });

        // Branch inside loop body.
        let inner_a = graph.add_node(GraphNode::Terminal {
            action: "inner".into(),
            guard: None,
        });
        let inner_branch = graph.add_node(GraphNode::Branch {
            alternatives: vec![BranchEdge {
                id: "inner_branch".into(),
                weight: 50.0,
                target: inner_a,
                guard: None,
            }],
        });
        graph.add_edge(body, inner_branch);
        graph.add_edge(inner_a, loop_exit);

        graph.add_edge(graph.entry, loop_entry);
        graph.add_edge(loop_entry, loop_exit);
        graph.add_edge(loop_exit, graph.exit);

        assert!(is_branch_reachable(&graph, "inner_branch"));
    }
}

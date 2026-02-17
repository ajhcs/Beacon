use std::collections::HashMap;

use beacon_ir::types::{Protocol, ProtocolNode};

use crate::graph::{BranchEdge, GraphNode, NdaGraph, NodeId};
use crate::predicate::{compile_expr, TypeContext};

#[derive(Debug, thiserror::Error)]
pub enum ProtocolCompileError {
    #[error("Unknown protocol reference: '{name}'")]
    UnknownProtocolRef { name: String },

    #[error("Guard compilation error: {0}")]
    GuardCompile(#[from] crate::predicate::CompileError),
}

/// Compile a protocol into an NDA graph.
pub fn compile_protocol(
    protocol: &Protocol,
    ctx: &TypeContext,
    all_protocols: &HashMap<String, Protocol>,
) -> Result<NdaGraph, ProtocolCompileError> {
    let mut graph = NdaGraph::new();
    let (body_entry, body_exit) = compile_node(&protocol.root, ctx, all_protocols, &mut graph)?;
    graph.add_edge(graph.entry, body_entry);
    graph.add_edge(body_exit, graph.exit);
    Ok(graph)
}

/// Compile a protocol node, returning (entry_node_id, exit_node_id) for the subgraph.
fn compile_node(
    node: &ProtocolNode,
    ctx: &TypeContext,
    all_protocols: &HashMap<String, Protocol>,
    graph: &mut NdaGraph,
) -> Result<(NodeId, NodeId), ProtocolCompileError> {
    match node {
        ProtocolNode::Call { action } => {
            let id = graph.add_node(GraphNode::Terminal {
                action: action.clone(),
                guard: None,
            });
            Ok((id, id))
        }

        ProtocolNode::Seq { children } => {
            if children.is_empty() {
                // Empty seq: create a passthrough
                let id = graph.add_node(GraphNode::Start); // placeholder
                return Ok((id, id));
            }

            let mut first_entry = None;
            let mut prev_exit = None;

            for child in children {
                let (entry, exit) = compile_node(child, ctx, all_protocols, graph)?;
                if first_entry.is_none() {
                    first_entry = Some(entry);
                }
                if let Some(pe) = prev_exit {
                    graph.add_edge(pe, entry);
                }
                prev_exit = Some(exit);
            }

            Ok((first_entry.unwrap(), prev_exit.unwrap()))
        }

        ProtocolNode::Alt { branches } => {
            // Create a join node for all branches to converge on
            let join = graph.add_node(GraphNode::Start); // placeholder join

            let mut alternatives = Vec::new();
            for branch in branches {
                let (body_entry, body_exit) =
                    compile_node(&branch.body, ctx, all_protocols, graph)?;
                graph.add_edge(body_exit, join);

                let guard = if let Some(guard_expr) = &branch.guard {
                    Some(compile_expr(guard_expr, ctx)?)
                } else {
                    None
                };

                alternatives.push(BranchEdge {
                    id: branch.id.clone(),
                    weight: branch.weight as f64,
                    target: body_entry,
                    guard,
                });
            }

            let branch_id = graph.add_node(GraphNode::Branch { alternatives });
            Ok((branch_id, join))
        }

        ProtocolNode::Repeat { min, max, body } => {
            let (body_entry, body_exit) = compile_node(body, ctx, all_protocols, graph)?;
            let loop_exit = graph.add_node(GraphNode::LoopExit);
            let loop_entry = graph.add_node(GraphNode::LoopEntry {
                body_start: body_entry,
                min: *min,
                max: *max,
            });

            graph.add_edge(loop_entry, body_entry);
            graph.add_edge(body_exit, loop_entry); // back edge for repeat
            graph.add_edge(loop_entry, loop_exit); // exit edge

            Ok((loop_entry, loop_exit))
        }

        ProtocolNode::Ref { protocol } => {
            let referenced = all_protocols.get(protocol).ok_or_else(|| {
                ProtocolCompileError::UnknownProtocolRef {
                    name: protocol.clone(),
                }
            })?;
            compile_node(&referenced.root, ctx, all_protocols, graph)
        }
    }
}

use crate::predicate::CompiledExpr;

pub type NodeId = u32;

#[derive(Debug, Clone)]
pub enum GraphNode {
    Terminal {
        action: String,
        guard: Option<CompiledExpr>,
    },
    Branch {
        alternatives: Vec<BranchEdge>,
    },
    LoopEntry {
        body_start: NodeId,
        min: u32,
        max: u32,
    },
    LoopExit,
    Start,
    End,
}

#[derive(Debug, Clone)]
pub struct BranchEdge {
    pub id: String,
    pub weight: f64,
    pub target: NodeId,
    pub guard: Option<CompiledExpr>,
}

#[derive(Debug, Clone)]
pub struct NdaGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<(NodeId, NodeId)>,
    pub entry: NodeId,
    pub exit: NodeId,
}

impl NdaGraph {
    pub fn new() -> Self {
        let mut graph = NdaGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            entry: 0,
            exit: 0,
        };
        let start = graph.add_node(GraphNode::Start);
        let end = graph.add_node(GraphNode::End);
        graph.entry = start;
        graph.exit = end;
        graph
    }

    pub fn add_node(&mut self, node: GraphNode) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(node);
        id
    }

    pub fn add_edge(&mut self, from: NodeId, to: NodeId) {
        self.edges.push((from, to));
    }
}

impl Default for NdaGraph {
    fn default() -> Self {
        Self::new()
    }
}

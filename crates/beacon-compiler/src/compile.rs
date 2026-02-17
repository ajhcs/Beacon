use std::collections::HashMap;

use beacon_ir::types::BeaconIR;

use crate::graph::NdaGraph;
use crate::predicate::{compile_expr, CompiledExpr, TypeContext};
use crate::protocol::compile_protocol;
use crate::validate::{validate_ir, ValidationError};

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("Validation errors: {}", .0.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "))]
    Validation(Vec<ValidationError>),

    #[error("Predicate compilation error: {0}")]
    Predicate(#[from] crate::predicate::CompileError),

    #[error("Protocol compilation error: {0}")]
    Protocol(#[from] crate::protocol::ProtocolCompileError),
}

#[derive(Debug)]
pub struct CompiledIR {
    pub graphs: HashMap<String, NdaGraph>,
    pub predicates: HashMap<String, CompiledExpr>,
    pub type_context: TypeContext,
}

pub fn compile(ir: &BeaconIR) -> Result<CompiledIR, CompileError> {
    // 1. Validate
    validate_ir(ir).map_err(CompileError::Validation)?;

    // 2. Build type context
    let ctx = TypeContext::from_ir(ir);

    // 3. Compile predicates (refinements + invariant properties)
    let mut predicates = HashMap::new();

    for (name, refinement) in &ir.refinements {
        let compiled = compile_expr(&refinement.predicate, &ctx)?;
        predicates.insert(name.clone(), compiled);
    }

    for (name, property) in &ir.properties {
        if let Some(pred) = &property.predicate {
            let compiled = compile_expr(pred, &ctx)?;
            predicates.insert(format!("property:{name}"), compiled);
        }
    }

    // 4. Compile protocols into NDA graphs
    let mut graphs = HashMap::new();
    for (name, protocol) in &ir.protocols {
        let graph = compile_protocol(protocol, &ctx, &ir.protocols)?;
        graphs.insert(name.clone(), graph);
    }

    Ok(CompiledIR {
        graphs,
        predicates,
        type_context: ctx,
    })
}

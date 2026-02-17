use std::collections::HashMap;

use beacon_ir::expr::{Expr, FnClassification, Literal, OpKind, QuantifierKind};
use beacon_ir::types::BeaconIR;

// ── Values ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    String(String),
}

// ── Value Environment ────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ValueEnv {
    /// Fields keyed by (entity_var, field_name).
    fields: HashMap<(String, String), Value>,
}

impl ValueEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_field(&mut self, entity: &str, field: &str, value: Value) {
        self.fields
            .insert((entity.to_string(), field.to_string()), value);
    }

    pub fn get_field(&self, entity: &str, field: &str) -> Option<&Value> {
        self.fields.get(&(entity.to_string(), field.to_string()))
    }
}

// ── Type Context ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypeContext {
    pub entities: HashMap<String, HashMap<String, FieldInfo>>,
    pub refinements: HashMap<String, String>, // name -> base entity
    pub functions: HashMap<String, FunctionInfo>,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub field_type: String, // "string", "bool", "enum", "int", "ref"
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub classification: String, // "derived" or "observer"
    pub params: Vec<String>,
    pub returns: String,
}

impl TypeContext {
    pub fn from_ir(ir: &BeaconIR) -> Self {
        let mut entities = HashMap::new();
        for (name, entity) in &ir.entities {
            let mut fields = HashMap::new();
            for (fname, fdef) in &entity.fields {
                let field_type = match &fdef.field_type {
                    beacon_ir::types::FieldType::String { .. } => "string",
                    beacon_ir::types::FieldType::Bool { .. } => "bool",
                    beacon_ir::types::FieldType::Int { .. } => "int",
                    beacon_ir::types::FieldType::Enum { .. } => "enum",
                    beacon_ir::types::FieldType::Ref { .. } => "ref",
                };
                fields.insert(
                    fname.clone(),
                    FieldInfo {
                        field_type: field_type.to_string(),
                    },
                );
            }
            entities.insert(name.clone(), fields);
        }

        let mut refinements = HashMap::new();
        for (name, refinement) in &ir.refinements {
            refinements.insert(name.clone(), refinement.base.clone());
        }

        let mut functions = HashMap::new();
        for (name, func) in &ir.functions {
            let classification = match func.classification {
                beacon_ir::types::FnClassification::Derived => "derived",
                beacon_ir::types::FnClassification::Observer => "observer",
            };
            functions.insert(
                name.clone(),
                FunctionInfo {
                    classification: classification.to_string(),
                    params: func.params.iter().map(|p| p.param_type.clone()).collect(),
                    returns: func.returns.clone(),
                },
            );
        }

        TypeContext {
            entities,
            refinements,
            functions,
        }
    }
}

// ── Compiled Expression ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CompiledExpr {
    Literal(Value),
    Field {
        entity: String,
        field: String,
    },
    Op {
        op: OpKind,
        args: Vec<CompiledExpr>,
    },
    Quantifier {
        kind: QuantifierKind,
        var: String,
        domain: String,
        body: Box<CompiledExpr>,
    },
    FnCall {
        classification: FnClassification,
        name: String,
        args: Vec<String>,
    },
    Is {
        entity: String,
        refinement: String,
        params: HashMap<String, String>,
    },
}

// ── Errors ───────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("Unknown function '{name}'")]
    UnknownFunction { name: String },
}

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("Field not found: {entity}.{field}")]
    FieldNotFound { entity: String, field: String },

    #[error("Type error: expected {expected}, got {actual}")]
    TypeError { expected: String, actual: String },

    #[error("Cannot evaluate: {reason}")]
    Unsupported { reason: String },
}

// ── Compilation ──────────────────────────────────────────────────────

pub fn compile_expr(expr: &Expr, _ctx: &TypeContext) -> Result<CompiledExpr, CompileError> {
    match expr {
        Expr::Literal(lit) => Ok(CompiledExpr::Literal(match lit {
            Literal::Bool(b) => Value::Bool(*b),
            Literal::Int(i) => Value::Int(*i),
            Literal::String(s) => Value::String(s.clone()),
        })),
        Expr::Field { entity, field } => Ok(CompiledExpr::Field {
            entity: entity.clone(),
            field: field.clone(),
        }),
        Expr::Op { op, args } => {
            let compiled_args = args
                .iter()
                .map(|a| compile_expr(a, _ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CompiledExpr::Op {
                op: op.clone(),
                args: compiled_args,
            })
        }
        Expr::Quantifier {
            kind,
            var,
            domain,
            body,
        } => {
            let compiled_body = compile_expr(body, _ctx)?;
            Ok(CompiledExpr::Quantifier {
                kind: kind.clone(),
                var: var.clone(),
                domain: domain.clone(),
                body: Box::new(compiled_body),
            })
        }
        Expr::FnCall {
            classification,
            name,
            args,
        } => Ok(CompiledExpr::FnCall {
            classification: classification.clone(),
            name: name.clone(),
            args: args.clone(),
        }),
        Expr::Is {
            entity,
            refinement,
            params,
        } => Ok(CompiledExpr::Is {
            entity: entity.clone(),
            refinement: refinement.clone(),
            params: params.clone(),
        }),
    }
}

// ── Evaluation ───────────────────────────────────────────────────────

pub fn eval_expr(expr: &CompiledExpr, env: &ValueEnv) -> Result<Value, EvalError> {
    match expr {
        CompiledExpr::Literal(v) => Ok(v.clone()),
        CompiledExpr::Field { entity, field } => env
            .get_field(entity, field)
            .cloned()
            .ok_or_else(|| EvalError::FieldNotFound {
                entity: entity.clone(),
                field: field.clone(),
            }),
        CompiledExpr::Op { op, args } => eval_op(op, args, env),
        CompiledExpr::Quantifier { .. } => Err(EvalError::Unsupported {
            reason: "Quantifier evaluation requires model state iteration".to_string(),
        }),
        CompiledExpr::FnCall { .. } => Err(EvalError::Unsupported {
            reason: "Function call evaluation requires model state resolution".to_string(),
        }),
        CompiledExpr::Is { .. } => Err(EvalError::Unsupported {
            reason: "Is expression evaluation requires refinement predicate resolution".to_string(),
        }),
    }
}

fn eval_op(op: &OpKind, args: &[CompiledExpr], env: &ValueEnv) -> Result<Value, EvalError> {
    match op {
        OpKind::Eq => {
            let left = eval_expr(&args[0], env)?;
            let right = eval_expr(&args[1], env)?;
            Ok(Value::Bool(left == right))
        }
        OpKind::Neq => {
            let left = eval_expr(&args[0], env)?;
            let right = eval_expr(&args[1], env)?;
            Ok(Value::Bool(left != right))
        }
        OpKind::And => {
            for arg in args {
                let val = eval_expr(arg, env)?;
                if val == Value::Bool(false) {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        OpKind::Or => {
            for arg in args {
                let val = eval_expr(arg, env)?;
                if val == Value::Bool(true) {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        OpKind::Not => {
            let val = eval_expr(&args[0], env)?;
            match val {
                Value::Bool(b) => Ok(Value::Bool(!b)),
                other => Err(EvalError::TypeError {
                    expected: "bool".to_string(),
                    actual: format!("{other:?}"),
                }),
            }
        }
        OpKind::Implies => {
            let antecedent = eval_expr(&args[0], env)?;
            match antecedent {
                Value::Bool(false) => Ok(Value::Bool(true)),
                Value::Bool(true) => eval_expr(&args[1], env),
                other => Err(EvalError::TypeError {
                    expected: "bool".to_string(),
                    actual: format!("{other:?}"),
                }),
            }
        }
        OpKind::Lt => eval_int_compare(args, env, |a, b| a < b),
        OpKind::Lte => eval_int_compare(args, env, |a, b| a <= b),
        OpKind::Gt => eval_int_compare(args, env, |a, b| a > b),
        OpKind::Gte => eval_int_compare(args, env, |a, b| a >= b),
    }
}

fn eval_int_compare(
    args: &[CompiledExpr],
    env: &ValueEnv,
    cmp: fn(i64, i64) -> bool,
) -> Result<Value, EvalError> {
    let left = eval_expr(&args[0], env)?;
    let right = eval_expr(&args[1], env)?;
    match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(cmp(*a, *b))),
        _ => Err(EvalError::TypeError {
            expected: "int".to_string(),
            actual: format!("{left:?}, {right:?}"),
        }),
    }
}

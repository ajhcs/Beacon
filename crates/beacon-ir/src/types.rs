use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::expr::Expr;

/// Top-level Beacon IR — all 9 sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconIR {
    pub entities: HashMap<String, Entity>,
    pub refinements: HashMap<String, Refinement>,
    pub functions: HashMap<String, FunctionDef>,
    pub protocols: HashMap<String, Protocol>,
    pub effects: HashMap<String, Effect>,
    pub properties: HashMap<String, Property>,
    pub generators: HashMap<String, Generator>,
    pub exploration: ExplorationConfig,
    pub inputs: InputSpace,
    pub bindings: Bindings,
}

// ── Section 1: Entities ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub fields: HashMap<String, FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    #[serde(flatten)]
    pub field_type: FieldType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    String {
        #[serde(default)]
        format: Option<String>,
    },
    Bool {
        #[serde(default)]
        default: Option<bool>,
    },
    Int {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },
    Enum {
        values: Vec<String>,
    },
    Ref {
        entity: String,
    },
}

// ── Section 2: Refinement Types ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refinement {
    pub base: String,
    #[serde(default)]
    pub params: Vec<ParamDef>,
    pub predicate: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub classification: FnClassification,
    pub params: Vec<ParamDef>,
    #[serde(default)]
    pub body: Option<Expr>,
    #[serde(default)]
    pub binding: Option<String>,
    pub returns: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FnClassification {
    Derived,
    Observer,
}

// ── Section 3: Protocols ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Protocol {
    pub root: ProtocolNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProtocolNode {
    Seq {
        children: Vec<ProtocolNode>,
    },
    Alt {
        branches: Vec<AltBranch>,
    },
    Repeat {
        min: u32,
        max: u32,
        body: Box<ProtocolNode>,
    },
    Call {
        action: String,
    },
    Ref {
        protocol: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AltBranch {
    pub id: String,
    pub weight: u32,
    #[serde(default)]
    pub guard: Option<Expr>,
    pub body: ProtocolNode,
}

// ── Section 4: Effects ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    #[serde(default)]
    pub creates: Option<CreateEffect>,
    #[serde(default)]
    pub sets: Vec<EffectSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEffect {
    pub entity: String,
    pub assign: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectSet {
    pub target: Vec<String>,
    pub value: serde_json::Value,
}

// ── Section 5: Properties ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    #[serde(rename = "type")]
    pub property_type: PropertyType,
    #[serde(default)]
    pub predicate: Option<Expr>,
    #[serde(default)]
    pub rule: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    Invariant,
    Temporal,
}

// ── Section 6: Generators ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generator {
    #[serde(default)]
    pub description: Option<String>,
    pub sequence: Vec<GeneratorStep>,
    #[serde(default)]
    pub postcondition: Option<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorStep {
    pub action: String,
    #[serde(default)]
    pub with: Option<serde_json::Value>,
}

// ── Section 7: Exploration ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationConfig {
    pub weights: WeightConfig,
    pub directives_allowed: Vec<DirectiveConfig>,
    pub adaptation_signals: Vec<AdaptationSignal>,
    pub strategy: StrategyConfig,
    pub epoch_size: u32,
    pub coverage_floor_threshold: f64,
    pub concurrency: ConcurrencyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightConfig {
    pub scope: String,
    pub initial: String,
    pub decay: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveConfig {
    #[serde(rename = "type")]
    pub directive_type: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationSignal {
    pub signal: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub initial: String,
    pub fallback: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    pub mode: String,
    pub threads: u32,
}

// ── Section 8: Inputs ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSpace {
    pub domains: HashMap<String, Domain>,
    pub constraints: Vec<InputConstraint>,
    pub coverage: CoverageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    #[serde(flatten)]
    pub domain_type: DomainType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainType {
    Enum { values: Vec<String> },
    Bool,
    Int { min: i64, max: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConstraint {
    pub name: String,
    pub rule: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageConfig {
    pub targets: Vec<CoverageTarget>,
    pub seed: u64,
    pub reproducible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoverageTarget {
    AllPairs {
        over: Vec<String>,
    },
    EachTransition {
        machine: String,
    },
    Boundary {
        domain: String,
        values: Vec<serde_json::Value>,
    },
}

// ── Section 9: Bindings ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bindings {
    pub runtime: String,
    pub entry: String,
    pub actions: HashMap<String, ActionBinding>,
    pub event_hooks: EventHooks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionBinding {
    pub function: String,
    pub args: Vec<String>,
    pub returns: serde_json::Value,
    pub mutates: bool,
    pub idempotent: bool,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventHooks {
    pub mode: String,
    pub observe: Vec<String>,
    pub capture: Vec<String>,
}

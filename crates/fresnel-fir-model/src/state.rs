use std::collections::HashMap;
use std::sync::Arc;

/// Runtime values in the model.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    String(String),
}

/// Unique identifier for an entity instance in the model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceId {
    pub entity_type: String,
    pub index: u64,
}

/// A single entity instance with typed fields.
#[derive(Debug, Clone)]
pub struct EntityInstance {
    pub entity_type: String,
    pub id: InstanceId,
    fields: HashMap<String, Value>,
}

impl EntityInstance {
    pub fn new(entity_type: String, id: InstanceId) -> Self {
        Self {
            entity_type,
            id,
            fields: HashMap::new(),
        }
    }

    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    pub fn set_field(&mut self, name: &str, value: Value) {
        self.fields.insert(name.to_string(), value);
    }
}

/// A trace entry recording an action that was executed.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    pub action: String,
    pub args: Vec<(String, String)>,
    pub generation: u64,
}

/// Snapshot token for rollback.
#[derive(Debug, Clone)]
pub struct Snapshot {
    instances: HashMap<String, Arc<Vec<EntityInstance>>>,
    trace: Arc<Vec<TraceEntry>>,
    generation: u64,
    next_instance_id: u64,
}

/// Copy-on-Write model state.
///
/// Uses Arc for efficient forking — data is shared until mutation.
#[derive(Debug, Clone)]
pub struct ModelState {
    /// Instances grouped by entity type. Arc for CoW sharing.
    instances: HashMap<String, Arc<Vec<EntityInstance>>>,
    /// Action trace.
    trace: Arc<Vec<TraceEntry>>,
    /// Monotonic generation counter, incremented on every mutation.
    generation: u64,
    /// Next unique instance ID.
    next_instance_id: u64,
}

impl ModelState {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            trace: Arc::new(Vec::new()),
            generation: 0,
            next_instance_id: 0,
        }
    }

    /// Current generation number.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Create a new entity instance, returning its ID.
    pub fn create_instance(&mut self, entity_type: &str) -> InstanceId {
        let id = InstanceId {
            entity_type: entity_type.to_string(),
            index: self.next_instance_id,
        };
        self.next_instance_id += 1;
        self.generation += 1;

        let instance = EntityInstance::new(entity_type.to_string(), id.clone());
        let instances = self
            .instances
            .entry(entity_type.to_string())
            .or_insert_with(|| Arc::new(Vec::new()));
        Arc::make_mut(instances).push(instance);

        id
    }

    /// Get an entity instance by ID (read-only).
    pub fn get_instance(&self, id: &InstanceId) -> Option<&EntityInstance> {
        self.instances
            .get(&id.entity_type)?
            .iter()
            .find(|inst| inst.id == *id)
    }

    /// Set a field value on an entity instance.
    pub fn set_field(&mut self, id: &InstanceId, field: &str, value: Value) {
        if let Some(instances) = self.instances.get_mut(&id.entity_type) {
            let instances = Arc::make_mut(instances);
            if let Some(inst) = instances.iter_mut().find(|inst| inst.id == *id) {
                inst.set_field(field, value);
                self.generation += 1;
            }
        }
    }

    /// Get all known entity type names.
    pub fn entity_types(&self) -> Vec<String> {
        self.instances.keys().cloned().collect()
    }

    /// Get all instances of a given entity type.
    pub fn all_instances(&self, entity_type: &str) -> &[EntityInstance] {
        self.instances
            .get(entity_type)
            .map(|arc| arc.as_slice())
            .unwrap_or(&[])
    }

    /// Fork this state — creates a cheap CoW clone.
    pub fn fork(&self) -> Self {
        self.clone()
    }

    /// Take a snapshot for later rollback.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            instances: self.instances.clone(),
            trace: self.trace.clone(),
            generation: self.generation,
            next_instance_id: self.next_instance_id,
        }
    }

    /// Rollback to a previously captured snapshot.
    pub fn rollback(&mut self, snapshot: Snapshot) {
        self.instances = snapshot.instances;
        self.trace = snapshot.trace;
        self.generation = snapshot.generation;
        self.next_instance_id = snapshot.next_instance_id;
    }

    /// Record an action in the trace.
    pub fn record_action(&mut self, action: &str, args: &[(&str, &str)]) {
        let entry = TraceEntry {
            action: action.to_string(),
            args: args
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            generation: self.generation,
        };
        Arc::make_mut(&mut self.trace).push(entry);
    }

    /// Get the action trace.
    pub fn trace(&self) -> &[TraceEntry] {
        &self.trace
    }
}

impl Default for ModelState {
    fn default() -> Self {
        Self::new()
    }
}

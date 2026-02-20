//! Execution context providing controlled access to external dependencies

use chrono::{DateTime, Utc};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::any::Any;
use crate::error::ProcessingError;

/// Deterministic time provider with frozen time values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterministicTime {
    current_time: DateTime<Utc>,
}

impl DeterministicTime {
    /// Create a new deterministic time with a specific timestamp
    pub fn new(time: DateTime<Utc>) -> Self {
        Self {
            current_time: time,
        }
    }
    
    /// Get the current frozen time
    pub fn current(&self) -> DateTime<Utc> {
        self.current_time
    }
    
    /// Create a new DeterministicTime with an updated time value
    pub fn with_time(&self, time: DateTime<Utc>) -> Self {
        Self {
            current_time: time,
        }
    }
}

/// Seeded random number generator for reproducible randomness
#[derive(Debug)]
pub struct SeededRandom {
    rng: ChaCha8Rng,
    seed: u64,
}

impl SeededRandom {
    /// Create a new seeded random number generator
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seed,
        }
    }
    
    /// Get the seed used for this random number generator
    pub fn seed(&self) -> u64 {
        self.seed
    }
    
    /// Generate a random u64
    pub fn next_u64(&mut self) -> u64 {
        self.rng.gen()
    }
    
    /// Generate a random u32
    pub fn next_u32(&mut self) -> u32 {
        self.rng.gen()
    }
    
    /// Generate a random value in a range
    pub fn gen_range<T, R>(&mut self, range: R) -> T
    where
        T: rand::distributions::uniform::SampleUniform,
        R: rand::distributions::uniform::SampleRange<T>,
    {
        self.rng.gen_range(range)
    }
    
    /// Generate a random boolean
    pub fn gen_bool(&mut self, p: f64) -> bool {
        self.rng.gen_bool(p)
    }
}

impl Clone for SeededRandom {
    fn clone(&self) -> Self {
        // Create a new RNG with the same seed to ensure reproducibility
        Self::new(self.seed)
    }
}

/// Container for immutable external data
pub struct ExternalFacts {
    facts: HashMap<String, FactWrapper>,
}

struct FactWrapper {
    value: Box<dyn ExternalFact>,
    type_id: std::any::TypeId,
}

impl Clone for FactWrapper {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone_box(),
            type_id: self.type_id,
        }
    }
}

impl Clone for ExternalFacts {
    fn clone(&self) -> Self {
        let mut new_facts = HashMap::new();
        for (key, wrapper) in &self.facts {
            new_facts.insert(key.clone(), wrapper.clone());
        }
        Self {
            facts: new_facts,
        }
    }
}

impl std::fmt::Debug for ExternalFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalFacts")
            .field("count", &self.facts.len())
            .finish()
    }
}

impl ExternalFacts {
    /// Create a new empty external facts container
    pub fn new() -> Self {
        Self {
            facts: HashMap::new(),
        }
    }
    
    /// Add an external fact with a key
    pub fn insert<T: ExternalFact>(&mut self, key: String, value: T) {
        let type_id = std::any::TypeId::of::<T>();
        self.facts.insert(key, FactWrapper {
            value: Box::new(value),
            type_id,
        });
    }
    
    /// Get an external fact by key
    pub fn get<T: ExternalFact>(&self, key: &str) -> Option<&T> {
        let requested_type_id = std::any::TypeId::of::<T>();
        self.facts.get(key).and_then(|wrapper| {
            if wrapper.type_id == requested_type_id {
                // Cast the trait object to &dyn Any, then downcast to concrete type
                let any_ref: &dyn Any = &*wrapper.value;
                any_ref.downcast_ref::<T>()
            } else {
                None
            }
        })
    }
    
    /// Check if a key exists
    pub fn contains_key(&self, key: &str) -> bool {
        self.facts.contains_key(key)
    }
    
    /// Get the number of facts stored
    pub fn len(&self) -> usize {
        self.facts.len()
    }
    
    /// Check if the container is empty
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }
}

/// External entity resolver for deterministic entity lookup
#[derive(Debug, Clone)]
pub struct ExternalEntityResolver {
    entities: HashMap<String, EntityWrapper>,
}

struct EntityWrapper {
    value: Box<dyn ExternalEntity>,
    type_id: std::any::TypeId,
}

impl std::fmt::Debug for EntityWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityWrapper")
            .field("type_id", &self.type_id)
            .finish()
    }
}

impl Clone for EntityWrapper {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone_box(),
            type_id: self.type_id,
        }
    }
}

impl ExternalEntityResolver {
    /// Create a new empty entity resolver
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }
    
    /// Register an external entity with a unique identifier
    pub fn register<T: ExternalEntity>(&mut self, entity_id: String, entity: T) {
        let type_id = std::any::TypeId::of::<T>();
        self.entities.insert(entity_id, EntityWrapper {
            value: Box::new(entity),
            type_id,
        });
    }
    
    /// Resolve an external entity by its identifier
    pub fn resolve<T: ExternalEntity>(&self, entity_id: &str) -> Result<&T, ProcessingError> {
        let requested_type_id = std::any::TypeId::of::<T>();
        
        self.entities.get(entity_id)
            .ok_or_else(|| ProcessingError::ExternalEntityNotFound {
                entity_id: entity_id.to_string(),
            })
            .and_then(|wrapper| {
                if wrapper.type_id == requested_type_id {
                    let any_ref: &dyn Any = &*wrapper.value;
                    any_ref.downcast_ref::<T>()
                        .ok_or_else(|| ProcessingError::ExternalEntityTypeMismatch {
                            entity_id: entity_id.to_string(),
                            expected_type: std::any::type_name::<T>().to_string(),
                        })
                } else {
                    Err(ProcessingError::ExternalEntityTypeMismatch {
                        entity_id: entity_id.to_string(),
                        expected_type: std::any::type_name::<T>().to_string(),
                    })
                }
            })
    }
    
    /// Check if an entity is registered
    pub fn contains(&self, entity_id: &str) -> bool {
        self.entities.contains_key(entity_id)
    }
    
    /// Get the number of registered entities
    pub fn len(&self) -> usize {
        self.entities.len()
    }
    
    /// Check if the resolver is empty
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

impl Default for ExternalEntityResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for external entities that can be resolved
pub trait ExternalEntity: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn ExternalEntity>;
}

// Blanket implementation for all types that are Clone + Send + Sync + 'static
impl<T> ExternalEntity for T
where
    T: Any + Clone + Send + Sync,
{
    fn clone_box(&self) -> Box<dyn ExternalEntity> {
        Box::new(self.clone())
    }
}

/// Ordering rules for deterministic collection iteration
#[derive(Debug, Clone)]
pub struct OrderingRules {
    /// Enforce stable ordering for all collections
    enforce_stable_ordering: bool,
    /// Custom ordering keys for specific entity types
    custom_orderings: HashMap<String, Vec<String>>,
}

impl OrderingRules {
    /// Create new ordering rules with stable ordering enforced
    pub fn new() -> Self {
        Self {
            enforce_stable_ordering: true,
            custom_orderings: HashMap::new(),
        }
    }
    
    /// Create ordering rules without enforcement (for testing)
    pub fn permissive() -> Self {
        Self {
            enforce_stable_ordering: false,
            custom_orderings: HashMap::new(),
        }
    }
    
    /// Add a custom ordering for a specific entity type
    pub fn add_ordering(&mut self, entity_type: String, ordered_ids: Vec<String>) {
        self.custom_orderings.insert(entity_type, ordered_ids);
    }
    
    /// Get the custom ordering for an entity type
    pub fn get_ordering(&self, entity_type: &str) -> Option<&Vec<String>> {
        self.custom_orderings.get(entity_type)
    }
    
    /// Check if stable ordering is enforced
    pub fn is_stable_ordering_enforced(&self) -> bool {
        self.enforce_stable_ordering
    }
    
    /// Validate that a collection follows the required ordering
    pub fn validate_ordering<T>(&self, entity_type: &str, items: &[T], get_id: impl Fn(&T) -> &str) -> Result<(), ProcessingError> {
        if !self.enforce_stable_ordering {
            return Ok(());
        }
        
        if let Some(expected_order) = self.custom_orderings.get(entity_type) {
            let actual_order: Vec<String> = items.iter().map(|item| get_id(item).to_string()).collect();
            
            // Check if the actual order matches the expected order
            if actual_order != *expected_order {
                return Err(ProcessingError::OrderingViolation {
                    entity_type: entity_type.to_string(),
                    expected_order: expected_order.clone(),
                    actual_order,
                });
            }
        }
        
        Ok(())
    }
    
    /// Sort a collection according to the defined ordering rules
    pub fn sort_by_ordering<T>(&self, entity_type: &str, items: &mut [T], get_id: impl Fn(&T) -> &str) {
        if let Some(expected_order) = self.custom_orderings.get(entity_type) {
            // Create a map of id to position in expected order
            let position_map: HashMap<&str, usize> = expected_order
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();
            
            // Sort items based on their position in the expected order
            items.sort_by_key(|item| {
                position_map.get(get_id(item)).copied().unwrap_or(usize::MAX)
            });
        } else {
            // Default to lexicographic ordering for determinism
            items.sort_by_key(|item| get_id(item).to_string());
        }
    }
}

impl Default for OrderingRules {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ExternalFacts {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for external facts that can be stored
pub trait ExternalFact: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn ExternalFact>;
}

// Blanket implementation for all types that are Clone + Send + Sync + 'static
impl<T> ExternalFact for T
where
    T: Any + Clone + Send + Sync,
{
    fn clone_box(&self) -> Box<dyn ExternalFact> {
        Box::new(self.clone())
    }
}

/// Execution context providing controlled access to external dependencies
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    deterministic_time: DeterministicTime,
    seeded_random: SeededRandom,
    external_facts: ExternalFacts,
    entity_resolver: ExternalEntityResolver,
    ordering_rules: OrderingRules,
}

impl ExecutionContext {
    /// Create a new execution context with specified time and random seed
    pub fn new(time: DateTime<Utc>, random_seed: u64) -> Self {
        Self {
            deterministic_time: DeterministicTime::new(time),
            seeded_random: SeededRandom::new(random_seed),
            external_facts: ExternalFacts::new(),
            entity_resolver: ExternalEntityResolver::new(),
            ordering_rules: OrderingRules::new(),
        }
    }
    
    /// Create a builder for constructing an execution context
    pub fn builder() -> ExecutionContextBuilder {
        ExecutionContextBuilder::new()
    }
    
    /// Get the current deterministic time
    pub fn now(&self) -> DateTime<Utc> {
        self.deterministic_time.current()
    }
    
    /// Get mutable access to the random number generator
    pub fn random(&mut self) -> &mut SeededRandom {
        &mut self.seeded_random
    }
    
    /// Get an external fact by key
    pub fn get_external_fact<T: ExternalFact>(&self, key: &str) -> Option<&T> {
        self.external_facts.get(key)
    }
    
    /// Get the external facts container
    pub fn external_facts(&self) -> &ExternalFacts {
        &self.external_facts
    }
    
    /// Resolve an external entity by its identifier
    pub fn resolve_entity<T: ExternalEntity>(&self, entity_id: &str) -> Result<&T, ProcessingError> {
        self.entity_resolver.resolve(entity_id)
    }
    
    /// Get the entity resolver
    pub fn entity_resolver(&self) -> &ExternalEntityResolver {
        &self.entity_resolver
    }
    
    /// Get the ordering rules
    pub fn ordering_rules(&self) -> &OrderingRules {
        &self.ordering_rules
    }
    
    /// Validate ordering for a collection
    pub fn validate_ordering<T>(&self, entity_type: &str, items: &[T], get_id: impl Fn(&T) -> &str) -> Result<(), ProcessingError> {
        self.ordering_rules.validate_ordering(entity_type, items, get_id)
    }
    
    /// Sort a collection according to ordering rules
    pub fn sort_by_ordering<T>(&self, entity_type: &str, items: &mut [T], get_id: impl Fn(&T) -> &str) {
        self.ordering_rules.sort_by_ordering(entity_type, items, get_id)
    }
    
    /// Create a new context with updated time
    pub fn with_time(&self, time: DateTime<Utc>) -> Self {
        Self {
            deterministic_time: self.deterministic_time.with_time(time),
            seeded_random: self.seeded_random.clone(),
            external_facts: self.external_facts.clone(),
            entity_resolver: self.entity_resolver.clone(),
            ordering_rules: self.ordering_rules.clone(),
        }
    }
}

/// Builder for constructing execution contexts
pub struct ExecutionContextBuilder {
    time: Option<DateTime<Utc>>,
    random_seed: Option<u64>,
    external_facts: ExternalFacts,
    entity_resolver: ExternalEntityResolver,
    ordering_rules: OrderingRules,
}

impl ExecutionContextBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            time: None,
            random_seed: None,
            external_facts: ExternalFacts::new(),
            entity_resolver: ExternalEntityResolver::new(),
            ordering_rules: OrderingRules::new(),
        }
    }
    
    /// Set the deterministic time
    pub fn with_time(mut self, time: DateTime<Utc>) -> Self {
        self.time = Some(time);
        self
    }
    
    /// Set the random seed
    pub fn with_random_seed(mut self, seed: u64) -> Self {
        self.random_seed = Some(seed);
        self
    }
    
    /// Add an external fact
    pub fn with_external_fact<T: ExternalFact>(mut self, key: String, value: T) -> Self {
        self.external_facts.insert(key, value);
        self
    }
    
    /// Register an external entity
    pub fn with_external_entity<T: ExternalEntity>(mut self, entity_id: String, entity: T) -> Self {
        self.entity_resolver.register(entity_id, entity);
        self
    }
    
    /// Add a custom ordering rule
    pub fn with_ordering(mut self, entity_type: String, ordered_ids: Vec<String>) -> Self {
        self.ordering_rules.add_ordering(entity_type, ordered_ids);
        self
    }
    
    /// Set ordering rules
    pub fn with_ordering_rules(mut self, ordering_rules: OrderingRules) -> Self {
        self.ordering_rules = ordering_rules;
        self
    }
    
    /// Build the execution context
    pub fn build(self) -> ExecutionContext {
        let time = self.time.unwrap_or_else(|| Utc::now());
        let random_seed = self.random_seed.unwrap_or(0);
        
        ExecutionContext {
            deterministic_time: DeterministicTime::new(time),
            seeded_random: SeededRandom::new(random_seed),
            external_facts: self.external_facts,
            entity_resolver: self.entity_resolver,
            ordering_rules: self.ordering_rules,
        }
    }
}

impl Default for ExecutionContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Types of operations that can be checked for determinism
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Access to system time
    SystemTime,
    /// Random number generation without a seed
    RandomWithoutSeed,
    /// Network access
    NetworkAccess,
    /// File system access (reading)
    FileSystemRead,
    /// File system access (writing)
    FileSystemWrite,
    /// Environment variable access
    EnvironmentVariable,
    /// Thread spawning
    ThreadSpawn,
    /// Process spawning
    ProcessSpawn,
}

/// Guard to detect and prevent non-deterministic operations
#[derive(Debug, Clone)]
pub struct NonDeterminismGuard {
    strict_mode: bool,
}

impl NonDeterminismGuard {
    /// Create a new non-determinism guard in strict mode
    pub fn new() -> Self {
        Self {
            strict_mode: true,
        }
    }
    
    /// Create a guard with custom strictness settings
    pub fn with_strict_mode(strict: bool) -> Self {
        Self {
            strict_mode: strict,
        }
    }
    
    /// Check if an operation is allowed
    pub fn check_operation(&self, op: &Operation) -> Result<(), ProcessingError> {
        if !self.strict_mode {
            return Ok(());
        }
        
        match op {
            Operation::SystemTime => Err(ProcessingError::NonDeterministicOperation {
                operation: "system_time".to_string(),
                location: "time access".to_string(),
            }),
            Operation::RandomWithoutSeed => Err(ProcessingError::NonDeterministicOperation {
                operation: "unseeded_random".to_string(),
                location: "random generation".to_string(),
            }),
            Operation::NetworkAccess => Err(ProcessingError::NonDeterministicOperation {
                operation: "network_access".to_string(),
                location: "external dependency".to_string(),
            }),
            Operation::FileSystemRead => Err(ProcessingError::NonDeterministicOperation {
                operation: "file_system_read".to_string(),
                location: "file system access".to_string(),
            }),
            Operation::FileSystemWrite => Err(ProcessingError::NonDeterministicOperation {
                operation: "file_system_write".to_string(),
                location: "file system access".to_string(),
            }),
            Operation::EnvironmentVariable => Err(ProcessingError::NonDeterministicOperation {
                operation: "environment_variable".to_string(),
                location: "environment access".to_string(),
            }),
            Operation::ThreadSpawn => Err(ProcessingError::NonDeterministicOperation {
                operation: "thread_spawn".to_string(),
                location: "concurrency".to_string(),
            }),
            Operation::ProcessSpawn => Err(ProcessingError::NonDeterministicOperation {
                operation: "process_spawn".to_string(),
                location: "process management".to_string(),
            }),
        }
    }
    
    /// Validate that an operation is deterministic, returning the operation if valid
    pub fn validate<T, F>(&self, op: &Operation, f: F) -> Result<T, ProcessingError>
    where
        F: FnOnce() -> T,
    {
        self.check_operation(op)?;
        Ok(f())
    }
}

impl Default for NonDeterminismGuard {
    fn default() -> Self {
        Self::new()
    }
}

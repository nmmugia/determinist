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
}

impl ExecutionContext {
    /// Create a new execution context with specified time and random seed
    pub fn new(time: DateTime<Utc>, random_seed: u64) -> Self {
        Self {
            deterministic_time: DeterministicTime::new(time),
            seeded_random: SeededRandom::new(random_seed),
            external_facts: ExternalFacts::new(),
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
    
    /// Create a new context with updated time
    pub fn with_time(&self, time: DateTime<Utc>) -> Self {
        Self {
            deterministic_time: self.deterministic_time.with_time(time),
            seeded_random: self.seeded_random.clone(),
            external_facts: self.external_facts.clone(),
        }
    }
}

/// Builder for constructing execution contexts
pub struct ExecutionContextBuilder {
    time: Option<DateTime<Utc>>,
    random_seed: Option<u64>,
    external_facts: ExternalFacts,
}

impl ExecutionContextBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            time: None,
            random_seed: None,
            external_facts: ExternalFacts::new(),
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
    
    /// Build the execution context
    pub fn build(self) -> ExecutionContext {
        let time = self.time.unwrap_or_else(|| Utc::now());
        let random_seed = self.random_seed.unwrap_or(0);
        
        ExecutionContext {
            deterministic_time: DeterministicTime::new(time),
            seeded_random: SeededRandom::new(random_seed),
            external_facts: self.external_facts,
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

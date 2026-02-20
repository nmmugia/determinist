//! Deterministic Transaction Replay Engine (DTRE)
//!
//! A library for deterministic execution of financial transactions through pure functional programming.

pub mod context;
pub mod error;
pub mod hasher;
pub mod replay_engine;
pub mod rule_set;
pub mod state_manager;
pub mod traits;
pub mod transaction_processor;
pub mod types;

// Re-export core types and traits
pub use context::{
    ExecutionContext, DeterministicTime, SeededRandom, ExternalFacts, ExternalFact, 
    ExternalEntityResolver, ExternalEntity, OrderingRules, NonDeterminismGuard, Operation
};
pub use error::{DTREError, ProcessingError, ValidationError, StateError, RuleError, SerializationError};
pub use hasher::StateHasher;
pub use replay_engine::{ReplayEngine, ReplayEngineBuilder};
pub use rule_set::{VersionedRuleSet, RuleSetRegistry, RuleSetMetadata};
pub use state_manager::{StateManager, Checkpoint, StateDiff};
pub use traits::{State, Transaction, RuleSet};
pub use transaction_processor::TransactionProcessor;
pub use types::{Version, StateHash, ReplayResult, ExecutionTrace, StateTransition};

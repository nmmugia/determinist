//! Deterministic Transaction Replay Engine (DTRE)
//!
//! A library for deterministic execution of financial transactions through pure functional programming.

pub mod context;
pub mod error;
pub mod traits;
pub mod types;

// Re-export core types and traits
pub use context::{ExecutionContext, DeterministicTime, SeededRandom, ExternalFacts, ExternalFact};
pub use error::{DTREError, ProcessingError, ValidationError, StateError, RuleError, SerializationError};
pub use traits::{State, Transaction, RuleSet};
pub use types::{Version, StateHash, ReplayResult, ExecutionTrace, StateTransition};

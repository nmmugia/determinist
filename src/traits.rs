//! Core traits for the DTRE

use std::hash::Hash;
use serde::{Serialize, de::DeserializeOwned};
use chrono::{DateTime, Utc};
use crate::error::{ValidationError, ProcessingError};
use crate::types::Version;
use crate::context::ExecutionContext;

/// Trait for state objects that can be replayed deterministically
pub trait State: Clone + Serialize + DeserializeOwned + Hash {
    /// Validate the state for consistency and correctness
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Trait for transaction events that can be processed
pub trait Transaction: Clone + Serialize + DeserializeOwned {
    /// Get the unique identifier for this transaction
    fn id(&self) -> &str;
    
    /// Get the timestamp of this transaction
    fn timestamp(&self) -> DateTime<Utc>;
    
    /// Validate the transaction for completeness and correctness
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Trait for rule sets that process transactions
pub trait RuleSet<S, T>
where
    S: State,
    T: Transaction,
{
    /// Get the version of this rule set
    fn version(&self) -> Version;
    
    /// Apply this rule set to a state and transaction, producing a new state
    fn apply(&self, state: &S, transaction: &T, context: &ExecutionContext) -> Result<S, ProcessingError>;
}


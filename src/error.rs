//! Error types for the DTRE

use thiserror::Error;
use crate::types::Version;

#[derive(Debug, Error)]
pub enum DTREError {
    #[error("Processing error: {0}")]
    Processing(#[from] ProcessingError),
    
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
    
    #[error("State error: {0}")]
    State(#[from] StateError),
    
    #[error("Rule error: {0}")]
    Rule(#[from] RuleError),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),
}

#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("Non-deterministic operation detected: {operation} at {location}")]
    NonDeterministicOperation { operation: String, location: String },
    
    #[error("Transaction processing failed: {transaction_id} - {reason}")]
    TransactionFailed { transaction_id: String, reason: String },
    
    #[error("Rule application failed: {rule_version} - {details}")]
    RuleApplicationFailed { rule_version: Version, details: String },
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid state: {reason}")]
    InvalidState { reason: String },
    
    #[error("Invalid transaction: {reason}")]
    InvalidTransaction { reason: String },
    
    #[error("Validation rule violated: {rule}")]
    RuleViolated { rule: String },
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("State transition failed: {reason}")]
    TransitionFailed { reason: String },
    
    #[error("State mismatch: expected {expected}, got {actual}")]
    Mismatch { expected: String, actual: String },
    
    #[error("Checkpoint error: {reason}")]
    CheckpointError { reason: String },
}

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("Rule not found: version {version}")]
    NotFound { version: Version },
    
    #[error("Rule version conflict: {reason}")]
    VersionConflict { reason: String },
    
    #[error("Rule registration failed: {reason}")]
    RegistrationFailed { reason: String },
}

#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("Serialization failed: {reason}")]
    SerializationFailed { reason: String },
    
    #[error("Deserialization failed: {reason}")]
    DeserializationFailed { reason: String },
}

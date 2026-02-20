//! Error types for the DTRE

use thiserror::Error;
use crate::types::{Version, StateHash};
use serde::{Serialize, Deserialize};

/// Comprehensive error context for debugging and diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Transaction ID if applicable
    pub transaction_id: Option<String>,
    /// Transaction index in the sequence
    pub transaction_index: Option<usize>,
    /// Rule version being applied
    pub rule_version: Option<Version>,
    /// State hash before the operation
    pub state_hash_before: Option<StateHash>,
    /// State hash after the operation (if available)
    pub state_hash_after: Option<StateHash>,
    /// Additional contextual information
    pub additional_info: Vec<(String, String)>,
}

impl ErrorContext {
    /// Create a new empty error context
    pub fn new() -> Self {
        Self {
            transaction_id: None,
            transaction_index: None,
            rule_version: None,
            state_hash_before: None,
            state_hash_after: None,
            additional_info: Vec::new(),
        }
    }
    
    /// Add transaction context
    pub fn with_transaction(mut self, id: String, index: usize) -> Self {
        self.transaction_id = Some(id);
        self.transaction_index = Some(index);
        self
    }
    
    /// Add rule context
    pub fn with_rule(mut self, version: Version) -> Self {
        self.rule_version = Some(version);
        self
    }
    
    /// Add state context
    pub fn with_state_hashes(mut self, before: StateHash, after: Option<StateHash>) -> Self {
        self.state_hash_before = Some(before);
        self.state_hash_after = after;
        self
    }
    
    /// Add additional information
    pub fn with_info(mut self, key: String, value: String) -> Self {
        self.additional_info.push((key, value));
        self
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Detailed information about a state mismatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMismatchDetail {
    /// Expected state hash
    pub expected_hash: StateHash,
    /// Actual state hash
    pub actual_hash: StateHash,
    /// Field-level differences
    pub field_diffs: Vec<FieldDiff>,
    /// Transaction where mismatch occurred
    pub transaction_id: Option<String>,
    /// Transaction index where mismatch occurred
    pub transaction_index: Option<usize>,
}

/// Difference in a specific field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiff {
    /// Field path (e.g., "account.balance")
    pub field_path: String,
    /// Expected value (serialized)
    pub expected_value: String,
    /// Actual value (serialized)
    pub actual_value: String,
}

/// Detailed validation error with rule violations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationDetail {
    /// Validation rules that were violated
    pub violated_rules: Vec<String>,
    /// Field that failed validation
    pub field: Option<String>,
    /// Expected constraint
    pub expected_constraint: Option<String>,
    /// Actual value that violated the constraint
    pub actual_value: Option<String>,
    /// Additional context
    pub context: ErrorContext,
}

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
    
    #[error("External entity not found: {entity_id}")]
    ExternalEntityNotFound { entity_id: String },
    
    #[error("External entity type mismatch: {entity_id} - expected {expected_type}")]
    ExternalEntityTypeMismatch { entity_id: String, expected_type: String },
    
    #[error("Ordering violation for {entity_type}: expected {expected_order:?}, got {actual_order:?}")]
    OrderingViolation { 
        entity_type: String, 
        expected_order: Vec<String>, 
        actual_order: Vec<String> 
    },
    
    #[error("Processing failed with context: {message}")]
    WithContext {
        message: String,
        context: ErrorContext,
    },
}

impl ProcessingError {
    /// Create a processing error with full context
    pub fn with_context(message: String, context: ErrorContext) -> Self {
        Self::WithContext { message, context }
    }
    
    /// Get the error context if available
    pub fn context(&self) -> Option<&ErrorContext> {
        match self {
            Self::WithContext { context, .. } => Some(context),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid state: {reason}")]
    InvalidState { reason: String },
    
    #[error("Invalid transaction: {reason}")]
    InvalidTransaction { reason: String },
    
    #[error("Validation rule violated: {rule}")]
    RuleViolated { rule: String },
    
    #[error("Validation failed with details")]
    WithDetails {
        details: ValidationDetail,
    },
}

impl ValidationError {
    /// Create a validation error with detailed information
    pub fn with_details(details: ValidationDetail) -> Self {
        Self::WithDetails { details }
    }
    
    /// Get the validation details if available
    pub fn details(&self) -> Option<&ValidationDetail> {
        match self {
            Self::WithDetails { details } => Some(details),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("State transition failed: {reason}")]
    TransitionFailed { reason: String },
    
    #[error("State mismatch: expected {expected}, got {actual}")]
    Mismatch { expected: String, actual: String },
    
    #[error("Checkpoint error: {reason}")]
    CheckpointError { reason: String },
    
    #[error("State mismatch with detailed diff")]
    MismatchWithDetail {
        detail: StateMismatchDetail,
    },
}

impl StateError {
    /// Create a state mismatch error with detailed diff information
    pub fn mismatch_with_detail(detail: StateMismatchDetail) -> Self {
        Self::MismatchWithDetail { detail }
    }
    
    /// Get the mismatch details if available
    pub fn mismatch_detail(&self) -> Option<&StateMismatchDetail> {
        match self {
            Self::MismatchWithDetail { detail } => Some(detail),
            _ => None,
        }
    }
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

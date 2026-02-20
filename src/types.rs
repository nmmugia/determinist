//! Core data types for the DTRE

use serde::{Serialize, Deserialize};
use std::fmt;

/// Semantic version for rule sets
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Create a new version
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    
    /// Check if this version is compatible with another version
    pub fn is_compatible_with(&self, other: &Version) -> bool {
        self.major == other.major
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Cryptographic hash of a state
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateHash(pub [u8; 32]);

impl fmt::Display for StateHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

/// Result of a replay operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult<S> {
    pub final_state: S,
    pub final_hash: StateHash,
    pub execution_trace: ExecutionTrace,
    pub performance_metrics: PerformanceMetrics,
}

/// Trace of execution for audit purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub transactions_processed: usize,
    pub state_transitions: Vec<StateTransitionInfo>,
    pub rule_applications: Vec<RuleApplication>,
}

/// Information about a state transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransitionInfo {
    pub from_hash: StateHash,
    pub to_hash: StateHash,
    pub transaction_id: String,
}

/// State transition with full state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition<S> {
    pub from_state: S,
    pub to_state: S,
    pub from_hash: StateHash,
    pub to_hash: StateHash,
    pub transaction_id: String,
}

/// Information about a rule application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleApplication {
    pub rule_version: Version,
    pub transaction_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Performance metrics for a replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_duration_ms: u64,
    pub transactions_per_second: f64,
    pub average_transaction_time_ms: f64,
}

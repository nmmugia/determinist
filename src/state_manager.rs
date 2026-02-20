//! State management and transition tracking

use crate::context::ExecutionContext;
use crate::error::{ProcessingError, StateError};
use crate::hasher::StateHasher;
use crate::traits::{RuleSet, State, Transaction};
use crate::types::{StateHash, StateTransition};
use serde::{Deserialize, Serialize};

/// Checkpoint representing a state at a specific point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint<S> {
    pub state: S,
    pub hash: StateHash,
    pub transaction_index: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Difference between two states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff<S> {
    pub from_state: S,
    pub to_state: S,
    pub from_hash: StateHash,
    pub to_hash: StateHash,
}

/// StateManager manages state transitions and checkpoints
#[derive(Debug, Clone)]
pub struct StateManager<S: State> {
    current_state: S,
    hasher: StateHasher,
    checkpoints: Vec<Checkpoint<S>>,
    transaction_count: usize,
}

impl<S: State> StateManager<S> {
    /// Create a new StateManager with an initial state
    pub fn new(initial_state: S) -> Result<Self, StateError> {
        // Validate the initial state
        initial_state.validate().map_err(|e| StateError::TransitionFailed {
            reason: format!("Initial state validation failed: {}", e),
        })?;
        
        Ok(Self {
            current_state: initial_state,
            hasher: StateHasher::new(),
            checkpoints: Vec::new(),
            transaction_count: 0,
        })
    }
    
    /// Get the current state
    pub fn current_state(&self) -> &S {
        &self.current_state
    }
    
    /// Get the current state hash
    pub fn current_hash(&self) -> StateHash {
        self.hasher.hash(&self.current_state)
    }
    
    /// Apply a transaction to the current state using the provided rule set
    pub fn apply_transaction<T, R>(
        &mut self,
        transaction: &T,
        rules: &R,
        context: &ExecutionContext,
    ) -> Result<StateTransition<S>, ProcessingError>
    where
        T: Transaction,
        R: RuleSet<S, T>,
    {
        // Validate the transaction
        transaction.validate().map_err(|e| ProcessingError::TransactionFailed {
            transaction_id: transaction.id().to_string(),
            reason: format!("Transaction validation failed: {}", e),
        })?;
        
        // Store the old state and hash
        let from_state = self.current_state.clone();
        let from_hash = self.hasher.hash(&from_state);
        
        // Apply the rule set to get the new state
        let new_state = rules.apply(&self.current_state, transaction, context)?;
        
        // Validate the new state
        new_state.validate().map_err(|e| ProcessingError::TransactionFailed {
            transaction_id: transaction.id().to_string(),
            reason: format!("New state validation failed: {}", e),
        })?;
        
        // Compute the new hash
        let to_hash = self.hasher.hash(&new_state);
        
        // Update the current state
        self.current_state = new_state.clone();
        self.transaction_count += 1;
        
        // Create and return the transition
        Ok(StateTransition {
            from_state,
            to_state: new_state,
            from_hash,
            to_hash,
            transaction_id: transaction.id().to_string(),
        })
    }
    
    /// Create a checkpoint at the current state
    pub fn create_checkpoint(&mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Checkpoint<S> {
        let checkpoint = Checkpoint {
            state: self.current_state.clone(),
            hash: self.current_hash(),
            transaction_index: self.transaction_count,
            timestamp,
        };
        
        self.checkpoints.push(checkpoint.clone());
        checkpoint
    }
    
    /// Restore state from a checkpoint
    pub fn restore_checkpoint(&mut self, checkpoint: &Checkpoint<S>) -> Result<(), StateError> {
        // Validate the checkpoint state
        checkpoint.state.validate().map_err(|e| StateError::CheckpointError {
            reason: format!("Checkpoint state validation failed: {}", e),
        })?;
        
        // Verify the checkpoint hash matches
        let computed_hash = self.hasher.hash(&checkpoint.state);
        if computed_hash != checkpoint.hash {
            return Err(StateError::CheckpointError {
                reason: format!(
                    "Checkpoint hash mismatch: expected {}, got {}",
                    checkpoint.hash, computed_hash
                ),
            });
        }
        
        // Restore the state
        self.current_state = checkpoint.state.clone();
        self.transaction_count = checkpoint.transaction_index;
        
        Ok(())
    }
    
    /// Get all checkpoints
    pub fn checkpoints(&self) -> &[Checkpoint<S>] {
        &self.checkpoints
    }
    
    /// Calculate the difference between two states
    pub fn calculate_diff(&self, from_state: &S, to_state: &S) -> StateDiff<S> {
        let from_hash = self.hasher.hash(from_state);
        let to_hash = self.hasher.hash(to_state);
        
        StateDiff {
            from_state: from_state.clone(),
            to_state: to_state.clone(),
            from_hash,
            to_hash,
        }
    }
    
    /// Compare two states and return whether they are identical
    pub fn compare_states(&self, state1: &S, state2: &S) -> bool {
        let hash1 = self.hasher.hash(state1);
        let hash2 = self.hasher.hash(state2);
        hash1 == hash2
    }
    
    /// Get the number of transactions processed
    pub fn transaction_count(&self) -> usize {
        self.transaction_count
    }
    
    /// Clear all checkpoints
    pub fn clear_checkpoints(&mut self) {
        self.checkpoints.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Version;
    use crate::error::ValidationError;
    use chrono::Utc;
    use serde::{Deserialize, Serialize};
    use std::hash::{Hash, Hasher};
    
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        balance: i64,
    }
    
    impl Hash for TestState {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.balance.hash(state);
        }
    }
    
    impl State for TestState {
        fn validate(&self) -> Result<(), ValidationError> {
            if self.balance < 0 {
                return Err(ValidationError::InvalidState {
                    reason: "Balance cannot be negative".to_string(),
                });
            }
            Ok(())
        }
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestTransaction {
        id: String,
        amount: i64,
        timestamp: chrono::DateTime<Utc>,
    }
    
    impl Transaction for TestTransaction {
        fn id(&self) -> &str {
            &self.id
        }
        
        fn timestamp(&self) -> chrono::DateTime<Utc> {
            self.timestamp
        }
        
        fn validate(&self) -> Result<(), ValidationError> {
            Ok(())
        }
    }
    
    struct TestRuleSet;
    
    impl RuleSet<TestState, TestTransaction> for TestRuleSet {
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        
        fn apply(
            &self,
            state: &TestState,
            transaction: &TestTransaction,
            _context: &ExecutionContext,
        ) -> Result<TestState, ProcessingError> {
            Ok(TestState {
                balance: state.balance + transaction.amount,
            })
        }
    }
    
    #[test]
    fn test_state_manager_creation() {
        let state = TestState { balance: 100 };
        let manager = StateManager::new(state.clone());
        assert!(manager.is_ok());
        
        let manager = manager.unwrap();
        assert_eq!(manager.current_state().balance, 100);
    }
    
    #[test]
    fn test_state_manager_rejects_invalid_initial_state() {
        let state = TestState { balance: -100 };
        let manager = StateManager::new(state);
        assert!(manager.is_err());
    }
    
    #[test]
    fn test_apply_transaction() {
        let state = TestState { balance: 100 };
        let mut manager = StateManager::new(state).unwrap();
        
        let transaction = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc::now(),
        };
        
        let context = ExecutionContext::new(Utc::now(), 42);
        let rules = TestRuleSet;
        
        let transition = manager.apply_transaction(&transaction, &rules, &context);
        assert!(transition.is_ok());
        
        let transition = transition.unwrap();
        assert_eq!(transition.from_state.balance, 100);
        assert_eq!(transition.to_state.balance, 150);
        assert_eq!(manager.current_state().balance, 150);
    }
    
    #[test]
    fn test_checkpoint_creation_and_restoration() {
        let state = TestState { balance: 100 };
        let mut manager = StateManager::new(state).unwrap();
        
        let checkpoint1 = manager.create_checkpoint(Utc::now());
        assert_eq!(checkpoint1.state.balance, 100);
        
        // Apply a transaction
        let transaction = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc::now(),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        let rules = TestRuleSet;
        manager.apply_transaction(&transaction, &rules, &context).unwrap();
        
        assert_eq!(manager.current_state().balance, 150);
        
        // Restore checkpoint
        let result = manager.restore_checkpoint(&checkpoint1);
        assert!(result.is_ok());
        assert_eq!(manager.current_state().balance, 100);
    }
    
    #[test]
    fn test_calculate_diff() {
        let state1 = TestState { balance: 100 };
        let state2 = TestState { balance: 150 };
        
        let manager = StateManager::new(state1.clone()).unwrap();
        let diff = manager.calculate_diff(&state1, &state2);
        
        assert_eq!(diff.from_state.balance, 100);
        assert_eq!(diff.to_state.balance, 150);
        assert_ne!(diff.from_hash, diff.to_hash);
    }
    
    #[test]
    fn test_compare_states() {
        let state1 = TestState { balance: 100 };
        let state2 = TestState { balance: 100 };
        let state3 = TestState { balance: 150 };
        
        let manager = StateManager::new(state1.clone()).unwrap();
        
        assert!(manager.compare_states(&state1, &state2));
        assert!(!manager.compare_states(&state1, &state3));
    }
}

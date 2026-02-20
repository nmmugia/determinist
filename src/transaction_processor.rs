//! Transaction processing engine with rule application and execution tracing

use crate::context::ExecutionContext;
use crate::error::ProcessingError;
use crate::state_manager::StateManager;
use crate::traits::{RuleSet, State, Transaction};
use crate::types::{ExecutionTrace, RuleApplication, StateTransition, StateTransitionInfo};
use chrono::{DateTime, Utc};

/// Transaction processor that applies rules and generates execution traces
#[derive(Debug)]
pub struct TransactionProcessor<S: State> {
    state_manager: StateManager<S>,
    execution_trace: ExecutionTrace,
}

impl<S: State> TransactionProcessor<S> {
    /// Create a new transaction processor with an initial state
    pub fn new(initial_state: S) -> Result<Self, ProcessingError> {
        let state_manager = StateManager::new(initial_state)
            .map_err(|e| ProcessingError::TransactionFailed {
                transaction_id: "initial".to_string(),
                reason: format!("Failed to initialize state manager: {}", e),
            })?;
        
        Ok(Self {
            state_manager,
            execution_trace: ExecutionTrace {
                transactions_processed: 0,
                state_transitions: Vec::new(),
                rule_applications: Vec::new(),
                checkpoints: Vec::new(),
            },
        })
    }
    
    
    /// Create a transaction processor from a checkpoint
    pub fn from_checkpoint(checkpoint: &crate::state_manager::Checkpoint<S>) -> Result<Self, ProcessingError> {
        let mut state_manager = StateManager::new(checkpoint.state.clone())
            .map_err(|e| ProcessingError::TransactionFailed {
                transaction_id: "checkpoint".to_string(),
                reason: format!("Failed to initialize state manager from checkpoint: {}", e),
            })?;
        
        // Restore the checkpoint to set the transaction count
        state_manager.restore_checkpoint(checkpoint)
            .map_err(|e| ProcessingError::TransactionFailed {
                transaction_id: "checkpoint".to_string(),
                reason: format!("Failed to restore checkpoint: {}", e),
            })?;
        
        Ok(Self {
            state_manager,
            execution_trace: ExecutionTrace {
                transactions_processed: checkpoint.transaction_index,
                state_transitions: Vec::new(),
                rule_applications: Vec::new(),
                checkpoints: Vec::new(),
            },
        })
    }
    /// Process a single transaction with the given rule set and context
    pub fn process_transaction<T, R>(
        &mut self,
        transaction: &T,
        rule_set: &R,
        context: &ExecutionContext,
    ) -> Result<StateTransition<S>, ProcessingError>
    where
        T: Transaction,
        R: RuleSet<S, T>,
    {
        // Validate the transaction before processing
        transaction.validate().map_err(|e| ProcessingError::TransactionFailed {
            transaction_id: transaction.id().to_string(),
            reason: format!("Transaction validation failed: {}", e),
        })?;
        
        // Apply the transaction through the state manager
        let transition = self.state_manager.apply_transaction(transaction, rule_set, context)?;
        
        // Record the state transition in the execution trace
        self.execution_trace.state_transitions.push(StateTransitionInfo {
            from_hash: transition.from_hash,
            to_hash: transition.to_hash,
            transaction_id: transition.transaction_id.clone(),
        });
        
        // Record the rule application in the execution trace
        self.execution_trace.rule_applications.push(RuleApplication {
            rule_version: rule_set.version(),
            transaction_id: transaction.id().to_string(),
            timestamp: transaction.timestamp(),
        });
        
        // Increment the transaction count
        self.execution_trace.transactions_processed += 1;
        
        Ok(transition)
    }
    
    /// Process a sequence of transactions
    pub fn process_transactions<T, R>(
        &mut self,
        transactions: &[T],
        rule_set: &R,
        context: &ExecutionContext,
    ) -> Result<Vec<StateTransition<S>>, ProcessingError>
    where
        T: Transaction,
        R: RuleSet<S, T>,
    {
        let mut transitions = Vec::with_capacity(transactions.len());
        
        for transaction in transactions {
            let transition = self.process_transaction(transaction, rule_set, context)?;
            transitions.push(transition);
        }
        
        Ok(transitions)
    }
    
    
    /// Process a sequence of transactions with automatic checkpointing at specified intervals
    pub fn process_transactions_with_checkpoints<T, R>(
        &mut self,
        transactions: &[T],
        rule_set: &R,
        context: &ExecutionContext,
        checkpoint_interval: usize,
    ) -> Result<Vec<StateTransition<S>>, ProcessingError>
    where
        T: Transaction,
        R: RuleSet<S, T>,
    {
        let mut transitions = Vec::with_capacity(transactions.len());
        
        for (index, transaction) in transactions.iter().enumerate() {
            let transition = self.process_transaction(transaction, rule_set, context)?;
            transitions.push(transition);
            
            // Create checkpoint at specified intervals
            if checkpoint_interval > 0 && (index + 1) % checkpoint_interval == 0 {
                let checkpoint = self.create_checkpoint(transaction.timestamp());
                
                // Record checkpoint info in execution trace
                self.execution_trace.checkpoints.push(crate::types::CheckpointInfo {
                    transaction_index: checkpoint.transaction_index,
                    hash: checkpoint.hash,
                    timestamp: checkpoint.timestamp,
                });
            }
        }
        
        Ok(transitions)
    }
    /// Get the current state
    pub fn current_state(&self) -> &S {
        self.state_manager.current_state()
    }
    
    /// Get the current state hash
    pub fn current_hash(&self) -> crate::types::StateHash {
        self.state_manager.current_hash()
    }
    
    /// Get the execution trace
    pub fn execution_trace(&self) -> &ExecutionTrace {
        &self.execution_trace
    }
    
    /// Consume the processor and return the final state and execution trace
    pub fn into_result(self) -> (S, ExecutionTrace) {
        (self.state_manager.current_state().clone(), self.execution_trace)
    }
    
    /// Get the number of transactions processed
    pub fn transactions_processed(&self) -> usize {
        self.execution_trace.transactions_processed
    }
    
    /// Create a checkpoint at the current state
    pub fn create_checkpoint(&mut self, timestamp: DateTime<Utc>) -> crate::state_manager::Checkpoint<S> {
        self.state_manager.create_checkpoint(timestamp)
    }
    
    /// Get access to the underlying state manager
    pub fn state_manager(&self) -> &StateManager<S> {
        &self.state_manager
    }
    
    /// Get mutable access to the underlying state manager
    pub fn state_manager_mut(&mut self) -> &mut StateManager<S> {
        &mut self.state_manager
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
        timestamp: DateTime<Utc>,
    }
    
    impl Transaction for TestTransaction {
        fn id(&self) -> &str {
            &self.id
        }
        
        fn timestamp(&self) -> DateTime<Utc> {
            self.timestamp
        }
        
        fn validate(&self) -> Result<(), ValidationError> {
            Ok(())
        }
    }
    
    struct TestRuleSet {
        version: Version,
    }
    
    impl RuleSet<TestState, TestTransaction> for TestRuleSet {
        fn version(&self) -> Version {
            self.version.clone()
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
    fn test_processor_creation() {
        let state = TestState { balance: 100 };
        let processor = TransactionProcessor::new(state);
        assert!(processor.is_ok());
        
        let processor = processor.unwrap();
        assert_eq!(processor.current_state().balance, 100);
        assert_eq!(processor.transactions_processed(), 0);
    }
    
    #[test]
    fn test_process_single_transaction() {
        let state = TestState { balance: 100 };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let transaction = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc::now(),
        };
        
        let context = ExecutionContext::new(Utc::now(), 42);
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        let result = processor.process_transaction(&transaction, &rule_set, &context);
        assert!(result.is_ok());
        
        let transition = result.unwrap();
        assert_eq!(transition.from_state.balance, 100);
        assert_eq!(transition.to_state.balance, 150);
        assert_eq!(processor.current_state().balance, 150);
        assert_eq!(processor.transactions_processed(), 1);
    }
    
    #[test]
    fn test_execution_trace_recording() {
        let state = TestState { balance: 100 };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let transaction = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc::now(),
        };
        
        let context = ExecutionContext::new(Utc::now(), 42);
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        processor.process_transaction(&transaction, &rule_set, &context).unwrap();
        
        let trace = processor.execution_trace();
        assert_eq!(trace.transactions_processed, 1);
        assert_eq!(trace.state_transitions.len(), 1);
        assert_eq!(trace.rule_applications.len(), 1);
        
        assert_eq!(trace.state_transitions[0].transaction_id, "tx1");
        assert_eq!(trace.rule_applications[0].transaction_id, "tx1");
        assert_eq!(trace.rule_applications[0].rule_version, Version::new(1, 0, 0));
    }
    
    #[test]
    fn test_process_multiple_transactions() {
        let state = TestState { balance: 100 };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc::now(),
            },
            TestTransaction {
                id: "tx2".to_string(),
                amount: 30,
                timestamp: Utc::now(),
            },
            TestTransaction {
                id: "tx3".to_string(),
                amount: 20,
                timestamp: Utc::now(),
            },
        ];
        
        let context = ExecutionContext::new(Utc::now(), 42);
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        let result = processor.process_transactions(&transactions, &rule_set, &context);
        assert!(result.is_ok());
        
        let transitions = result.unwrap();
        assert_eq!(transitions.len(), 3);
        assert_eq!(processor.current_state().balance, 200);
        assert_eq!(processor.transactions_processed(), 3);
        
        let trace = processor.execution_trace();
        assert_eq!(trace.state_transitions.len(), 3);
        assert_eq!(trace.rule_applications.len(), 3);
    }
    
    #[test]
    fn test_rule_version_tracking() {
        let state = TestState { balance: 100 };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let transaction1 = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc::now(),
        };
        
        let transaction2 = TestTransaction {
            id: "tx2".to_string(),
            amount: 30,
            timestamp: Utc::now(),
        };
        
        let context = ExecutionContext::new(Utc::now(), 42);
        let rule_set_v1 = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let rule_set_v2 = TestRuleSet {
            version: Version::new(2, 0, 0),
        };
        
        processor.process_transaction(&transaction1, &rule_set_v1, &context).unwrap();
        processor.process_transaction(&transaction2, &rule_set_v2, &context).unwrap();
        
        let trace = processor.execution_trace();
        assert_eq!(trace.rule_applications[0].rule_version, Version::new(1, 0, 0));
        assert_eq!(trace.rule_applications[1].rule_version, Version::new(2, 0, 0));
    }
    
    #[test]
    fn test_transaction_validation_failure() {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct InvalidTransaction {
            id: String,
            timestamp: DateTime<Utc>,
        }
        
        impl Transaction for InvalidTransaction {
            fn id(&self) -> &str {
                &self.id
            }
            
            fn timestamp(&self) -> DateTime<Utc> {
                self.timestamp
            }
            
            fn validate(&self) -> Result<(), ValidationError> {
                Err(ValidationError::InvalidTransaction {
                    reason: "Invalid transaction".to_string(),
                })
            }
        }
        
        struct InvalidRuleSet;
        
        impl RuleSet<TestState, InvalidTransaction> for InvalidRuleSet {
            fn version(&self) -> Version {
                Version::new(1, 0, 0)
            }
            
            fn apply(
                &self,
                state: &TestState,
                _transaction: &InvalidTransaction,
                _context: &ExecutionContext,
            ) -> Result<TestState, ProcessingError> {
                Ok(state.clone())
            }
        }
        
        let state = TestState { balance: 100 };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let transaction = InvalidTransaction {
            id: "tx1".to_string(),
            timestamp: Utc::now(),
        };
        
        let context = ExecutionContext::new(Utc::now(), 42);
        let rule_set = InvalidRuleSet;
        
        let result = processor.process_transaction(&transaction, &rule_set, &context);
        assert!(result.is_err());
        
        // State should remain unchanged
        assert_eq!(processor.current_state().balance, 100);
        assert_eq!(processor.transactions_processed(), 0);
    }
}

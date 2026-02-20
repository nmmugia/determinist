//! Core replay engine with builder pattern for deterministic transaction replay

use crate::context::ExecutionContext;
use crate::error::ProcessingError;
use crate::transaction_processor::TransactionProcessor;
use crate::traits::{RuleSet, State, Transaction};
use crate::types::{PerformanceMetrics, ReplayResult};
use chrono::Utc;
use std::marker::PhantomData;
use std::time::Instant;

/// Core replay engine for deterministic transaction processing
#[derive(Debug)]
pub struct ReplayEngine<S, T, R>
where
    S: State,
    T: Transaction,
    R: RuleSet<S, T>,
{
    initial_state: S,
    rule_set: R,
    context: ExecutionContext,
    _phantom_t: PhantomData<T>,
}

impl<S, T, R> ReplayEngine<S, T, R>
where
    S: State,
    T: Transaction,
    R: RuleSet<S, T>,
{
    /// Create a new replay engine with the specified initial state, rule set, and context
    pub fn new(initial_state: S, rule_set: R, context: ExecutionContext) -> Self {
        Self {
            initial_state,
            rule_set,
            context,
            _phantom_t: PhantomData,
        }
    }
    
    /// Create a builder for constructing a replay engine
    pub fn builder() -> ReplayEngineBuilder<S, T, R> {
        ReplayEngineBuilder::new()
    }
    
    /// Replay a sequence of transactions and return the comprehensive result
    pub fn replay(&self, transactions: &[T]) -> Result<ReplayResult<S>, ProcessingError> {
        let start_time = Instant::now();
        
        // Create a transaction processor with the initial state
        let mut processor = TransactionProcessor::new(self.initial_state.clone())?;
        
        // Process all transactions in order
        processor.process_transactions(transactions, &self.rule_set, &self.context)?;
        
        // Calculate performance metrics
        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;
        let transactions_per_second = if duration_ms > 0 {
            (transactions.len() as f64) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };
        let average_transaction_time_ms = if !transactions.is_empty() {
            duration_ms as f64 / transactions.len() as f64
        } else {
            0.0
        };
        
        let performance_metrics = PerformanceMetrics {
            total_duration_ms: duration_ms,
            transactions_per_second,
            average_transaction_time_ms,
        };
        
        // Get the final hash before consuming the processor
        let final_hash = processor.current_hash();
        
        // Get the final state and execution trace
        let (final_state, execution_trace) = processor.into_result();
        
        Ok(ReplayResult {
            final_state,
            final_hash,
            execution_trace,
            performance_metrics,
        })
    }
    
    /// Get the initial state
    pub fn initial_state(&self) -> &S {
        &self.initial_state
    }
    
    /// Get the rule set
    pub fn rule_set(&self) -> &R {
        &self.rule_set
    }
    
    /// Get the execution context
    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }
}

/// Builder for constructing replay engines with a fluent API
pub struct ReplayEngineBuilder<S, T, R>
where
    S: State,
    T: Transaction,
    R: RuleSet<S, T>,
{
    initial_state: Option<S>,
    rule_set: Option<R>,
    context: Option<ExecutionContext>,
    _phantom_t: PhantomData<T>,
}

impl<S, T, R> ReplayEngineBuilder<S, T, R>
where
    S: State,
    T: Transaction,
    R: RuleSet<S, T>,
{
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            initial_state: None,
            rule_set: None,
            context: None,
            _phantom_t: PhantomData,
        }
    }
    
    /// Set the initial state
    pub fn with_initial_state(mut self, state: S) -> Self {
        self.initial_state = Some(state);
        self
    }
    
    /// Set the rule set
    pub fn with_rule_set(mut self, rule_set: R) -> Self {
        self.rule_set = Some(rule_set);
        self
    }
    
    /// Set the execution context
    pub fn with_context(mut self, context: ExecutionContext) -> Self {
        self.context = Some(context);
        self
    }
    
    /// Set the execution context using a time and random seed
    pub fn with_time_and_seed(mut self, time: chrono::DateTime<Utc>, seed: u64) -> Self {
        self.context = Some(ExecutionContext::new(time, seed));
        self
    }
    
    /// Build the replay engine
    pub fn build(self) -> Result<ReplayEngine<S, T, R>, String> {
        let initial_state = self.initial_state.ok_or("Initial state is required")?;
        let rule_set = self.rule_set.ok_or("Rule set is required")?;
        let context = self.context.ok_or("Execution context is required")?;
        
        Ok(ReplayEngine::new(initial_state, rule_set, context))
    }
}

impl<S, T, R> Default for ReplayEngineBuilder<S, T, R>
where
    S: State,
    T: Transaction,
    R: RuleSet<S, T>,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ValidationError;
    use crate::types::Version;
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
    
    #[derive(Clone, Debug)]
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
    fn test_replay_engine_creation() {
        let state = TestState { balance: 100 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state.clone(), rule_set, context);
        assert_eq!(engine.initial_state().balance, 100);
    }
    
    #[test]
    fn test_replay_engine_builder() {
        let state = TestState { balance: 100 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::builder()
            .with_initial_state(state)
            .with_rule_set(rule_set)
            .with_context(context)
            .build();
        
        assert!(engine.is_ok());
        let engine = engine.unwrap();
        assert_eq!(engine.initial_state().balance, 100);
    }
    
    #[test]
    fn test_replay_engine_builder_with_time_and_seed() {
        let state = TestState { balance: 100 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let time = Utc::now();
        
        let engine = ReplayEngine::builder()
            .with_initial_state(state)
            .with_rule_set(rule_set)
            .with_time_and_seed(time, 42)
            .build();
        
        assert!(engine.is_ok());
    }
    
    #[test]
    fn test_replay_engine_builder_missing_fields() {
        let state = TestState { balance: 100 };
        
        let result: Result<ReplayEngine<TestState, TestTransaction, TestRuleSet>, String> = 
            ReplayEngine::builder()
                .with_initial_state(state)
                .build();
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Rule set is required"));
    }
    
    #[test]
    fn test_replay_empty_transaction_sequence() {
        let state = TestState { balance: 100 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set, context);
        let transactions: Vec<TestTransaction> = vec![];
        
        let result = engine.replay(&transactions);
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.final_state.balance, 100);
        assert_eq!(result.execution_trace.transactions_processed, 0);
    }
    
    #[test]
    fn test_replay_single_transaction() {
        let state = TestState { balance: 100 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set, context);
        let transactions = vec![TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc::now(),
        }];
        
        let result = engine.replay(&transactions);
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.final_state.balance, 150);
        assert_eq!(result.execution_trace.transactions_processed, 1);
        assert_eq!(result.execution_trace.state_transitions.len(), 1);
        assert_eq!(result.execution_trace.rule_applications.len(), 1);
    }
    
    #[test]
    fn test_replay_multiple_transactions() {
        let state = TestState { balance: 100 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set, context);
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
        
        let result = engine.replay(&transactions);
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.final_state.balance, 200);
        assert_eq!(result.execution_trace.transactions_processed, 3);
        assert_eq!(result.execution_trace.state_transitions.len(), 3);
        assert_eq!(result.execution_trace.rule_applications.len(), 3);
    }
    
    #[test]
    fn test_replay_result_contains_all_metadata() {
        let state = TestState { balance: 100 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set, context);
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc::now(),
            },
        ];
        
        let result = engine.replay(&transactions).unwrap();
        
        // Check that all expected metadata is present
        assert!(result.performance_metrics.transactions_per_second >= 0.0);
        assert!(result.performance_metrics.average_transaction_time_ms >= 0.0);
        assert_eq!(result.execution_trace.transactions_processed, 1);
        assert_eq!(result.execution_trace.state_transitions.len(), 1);
        assert_eq!(result.execution_trace.rule_applications.len(), 1);
        assert_eq!(result.execution_trace.state_transitions[0].transaction_id, "tx1");
        assert_eq!(result.execution_trace.rule_applications[0].transaction_id, "tx1");
    }
    
    #[test]
    fn test_deterministic_ordering_maintained() {
        let state = TestState { balance: 0 };
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set, context);
        
        // Create transactions with specific ordering
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 10,
                timestamp: Utc::now(),
            },
            TestTransaction {
                id: "tx2".to_string(),
                amount: 20,
                timestamp: Utc::now(),
            },
            TestTransaction {
                id: "tx3".to_string(),
                amount: 30,
                timestamp: Utc::now(),
            },
        ];
        
        let result = engine.replay(&transactions).unwrap();
        
        // Verify that transactions were processed in order
        assert_eq!(result.execution_trace.state_transitions[0].transaction_id, "tx1");
        assert_eq!(result.execution_trace.state_transitions[1].transaction_id, "tx2");
        assert_eq!(result.execution_trace.state_transitions[2].transaction_id, "tx3");
        
        // Verify final state reflects ordered processing
        assert_eq!(result.final_state.balance, 60);
    }
}

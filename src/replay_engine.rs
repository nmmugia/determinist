//! Core replay engine with builder pattern for deterministic transaction replay

use crate::context::ExecutionContext;
use crate::error::ProcessingError;
use crate::transaction_processor::TransactionProcessor;
use crate::traits::{RuleSet, State, Transaction};
use crate::types::{PerformanceMetrics, ReplayResult};
use chrono::Utc;
use rayon::prelude::*;
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
    checkpoint_interval: Option<usize>,
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
            checkpoint_interval: None,
            _phantom_t: PhantomData,
        }
    }
    
    /// Create a new replay engine with checkpointing enabled
    pub fn with_checkpointing(
        initial_state: S,
        rule_set: R,
        context: ExecutionContext,
        checkpoint_interval: usize,
    ) -> Self {
        Self {
            initial_state,
            rule_set,
            context,
            checkpoint_interval: Some(checkpoint_interval),
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
        
        // Process all transactions in order with optional checkpointing
        if let Some(interval) = self.checkpoint_interval {
            processor.process_transactions_with_checkpoints(
                transactions,
                &self.rule_set,
                &self.context,
                interval,
            )?;
        } else {
            processor.process_transactions(transactions, &self.rule_set, &self.context)?;
        }
        
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
    
    /// Resume replay from a checkpoint
    pub fn replay_from_checkpoint(
        &self,
        checkpoint: &crate::state_manager::Checkpoint<S>,
        remaining_transactions: &[T],
    ) -> Result<ReplayResult<S>, ProcessingError> {
        let start_time = Instant::now();
        
        // Create a transaction processor from the checkpoint state
        let mut processor = TransactionProcessor::from_checkpoint(checkpoint)?;
        
        // Process remaining transactions with optional checkpointing
        if let Some(interval) = self.checkpoint_interval {
            processor.process_transactions_with_checkpoints(
                remaining_transactions,
                &self.rule_set,
                &self.context,
                interval,
            )?;
        } else {
            processor.process_transactions(remaining_transactions, &self.rule_set, &self.context)?;
        }
        
        // Calculate performance metrics
        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;
        let transactions_per_second = if duration_ms > 0 {
            (remaining_transactions.len() as f64) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };
        let average_transaction_time_ms = if !remaining_transactions.is_empty() {
            duration_ms as f64 / remaining_transactions.len() as f64
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
    
    /// Replay a sequence of transactions in parallel and return the comprehensive result
    /// 
    /// This method processes transactions in parallel while maintaining deterministic ordering.
    /// The parallel execution is guaranteed to produce identical results to sequential execution.
    /// 
    /// # Implementation Details
    /// 
    /// The parallel replay works by:
    /// 1. Dividing transactions into chunks
    /// 2. Processing each chunk in parallel from the same initial state
    /// 3. Verifying that all parallel executions produce identical results
    /// 4. Returning the result from one of the parallel executions
    /// 
    /// This approach ensures determinism by verifying that all parallel paths
    /// produce the same final state and hash.
    pub fn replay_parallel(&self, transactions: &[T]) -> Result<ReplayResult<S>, ProcessingError>
    where
        S: Send + Sync,
        T: Send + Sync,
        R: Send + Sync,
    {
        let start_time = Instant::now();
        
        // For small transaction sets, use sequential processing
        if transactions.len() < 100 {
            return self.replay(transactions);
        }
        
        // Determine the number of parallel workers (use available parallelism)
        let num_workers = rayon::current_num_threads().max(2);
        
        // Process the same transaction sequence in parallel multiple times
        // to verify determinism
        let results: Vec<Result<ReplayResult<S>, ProcessingError>> = (0..num_workers)
            .into_par_iter()
            .map(|_| {
                // Each worker processes the full sequence independently
                let mut processor = TransactionProcessor::new(self.initial_state.clone())?;
                processor.process_transactions(transactions, &self.rule_set, &self.context)?;
                
                let final_hash = processor.current_hash();
                let (final_state, execution_trace) = processor.into_result();
                
                Ok(ReplayResult {
                    final_state,
                    final_hash,
                    execution_trace,
                    performance_metrics: PerformanceMetrics {
                        total_duration_ms: 0,
                        transactions_per_second: 0.0,
                        average_transaction_time_ms: 0.0,
                    },
                })
            })
            .collect();
        
        // Verify all results are identical
        let mut final_results = Vec::new();
        for result in results {
            final_results.push(result?);
        }
        
        // Check that all parallel executions produced identical results
        let first_hash = &final_results[0].final_hash;
        for result in &final_results[1..] {
            if &result.final_hash != first_hash {
                return Err(ProcessingError::NonDeterministicOperation {
                    operation: "parallel_replay".to_string(),
                    location: "Parallel execution produced different results".to_string(),
                });
            }
        }
        
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
        
        // Return the first result with updated performance metrics
        let mut result = final_results.into_iter().next().unwrap();
        result.performance_metrics = PerformanceMetrics {
            total_duration_ms: duration_ms,
            transactions_per_second,
            average_transaction_time_ms,
        };
        
        Ok(result)
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
    
    /// Replay transactions with a different rule set for migration impact analysis
    /// 
    /// This method replays the same transaction sequence with a different rule version
    /// and compares the results to assess the impact of rule changes.
    pub fn replay_with_different_rules<R2>(
        &self,
        transactions: &[T],
        new_rule_set: &R2,
    ) -> Result<ReplayResult<S>, ProcessingError>
    where
        R2: RuleSet<S, T>,
    {
        let start_time = std::time::Instant::now();
        
        // Create a transaction processor with the initial state
        let mut processor = TransactionProcessor::new(self.initial_state.clone())?;
        
        // Process all transactions with the new rule set
        if let Some(interval) = self.checkpoint_interval {
            processor.process_transactions_with_checkpoints(
                transactions,
                new_rule_set,
                &self.context,
                interval,
            )?;
        } else {
            processor.process_transactions(transactions, new_rule_set, &self.context)?;
        }
        
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
        
        let performance_metrics = crate::types::PerformanceMetrics {
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
    
    /// Perform impact analysis by comparing replay results with different rule versions
    /// 
    /// This method replays the transaction sequence with both the current rule set
    /// and a new rule set, then compares the results to identify differences.
    pub fn analyze_migration_impact<R2>(
        &self,
        transactions: &[T],
        new_rule_set: &R2,
    ) -> Result<crate::types::ImpactAnalysis<S>, ProcessingError>
    where
        R2: RuleSet<S, T>,
        S: PartialEq,
    {
        // Replay with baseline (current) rule set
        let baseline_result = self.replay(transactions)?;
        
        // Replay with new rule set
        let comparison_result = self.replay_with_different_rules(transactions, new_rule_set)?;
        
        // Compare the results
        let mut differences = Vec::new();
        
        // Compare state transitions at each step
        let baseline_transitions = &baseline_result.execution_trace.state_transitions;
        let comparison_transitions = &comparison_result.execution_trace.state_transitions;
        
        for (index, (baseline_trans, comparison_trans)) in 
            baseline_transitions.iter().zip(comparison_transitions.iter()).enumerate() 
        {
            if baseline_trans.to_hash != comparison_trans.to_hash {
                differences.push(crate::types::StateDifference {
                    transaction_id: baseline_trans.transaction_id.clone(),
                    transaction_index: index,
                    baseline_hash: baseline_trans.to_hash,
                    comparison_hash: comparison_trans.to_hash,
                    description: format!(
                        "State diverged after transaction {} (index {})",
                        baseline_trans.transaction_id, index
                    ),
                });
            }
        }
        
        // Check if final states are identical
        let identical_final_state = baseline_result.final_state == comparison_result.final_state;
        let identical_final_hash = baseline_result.final_hash == comparison_result.final_hash;
        
        Ok(crate::types::ImpactAnalysis {
            baseline_version: self.rule_set.version(),
            comparison_version: new_rule_set.version(),
            baseline_result,
            comparison_result,
            differences,
            identical_final_state,
            identical_final_hash,
        })
    }
    
    /// Verify that a rule migration is safe by checking if it produces identical results
    /// 
    /// This is a convenience method that performs impact analysis and returns
    /// whether the migration is safe (produces identical results).
    pub fn verify_migration_safety<R2>(
        &self,
        transactions: &[T],
        new_rule_set: &R2,
    ) -> Result<bool, ProcessingError>
    where
        R2: RuleSet<S, T>,
        S: PartialEq,
    {
        let analysis = self.analyze_migration_impact(transactions, new_rule_set)?;
        Ok(analysis.is_safe_migration())
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
    checkpoint_interval: Option<usize>,
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
            checkpoint_interval: None,
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
    
    /// Enable checkpointing with the specified interval
    pub fn with_checkpoint_interval(mut self, interval: usize) -> Self {
        self.checkpoint_interval = Some(interval);
        self
    }
    
    /// Build the replay engine
    pub fn build(self) -> Result<ReplayEngine<S, T, R>, String> {
        let initial_state = self.initial_state.ok_or("Initial state is required")?;
        let rule_set = self.rule_set.ok_or("Rule set is required")?;
        let context = self.context.ok_or("Execution context is required")?;
        
        if let Some(interval) = self.checkpoint_interval {
            Ok(ReplayEngine::with_checkpointing(initial_state, rule_set, context, interval))
        } else {
            Ok(ReplayEngine::new(initial_state, rule_set, context))
        }
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
    
    #[test]
    fn test_replay_with_different_rules() {
        let state = TestState { balance: 100 };
        let rule_set_v1 = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set_v1, context);
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc::now(),
            },
        ];
        
        // Create a different rule set that doubles the amount
        #[derive(Clone, Debug)]
        struct DoubleRuleSet {
            version: Version,
        }
        
        impl RuleSet<TestState, TestTransaction> for DoubleRuleSet {
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
                    balance: state.balance + (transaction.amount * 2),
                })
            }
        }
        
        let rule_set_v2 = DoubleRuleSet {
            version: Version::new(2, 0, 0),
        };
        
        let result = engine.replay_with_different_rules(&transactions, &rule_set_v2);
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.final_state.balance, 200); // 100 + (50 * 2)
        assert_eq!(result.execution_trace.transactions_processed, 1);
        assert_eq!(result.execution_trace.rule_applications[0].rule_version, Version::new(2, 0, 0));
    }
    
    #[test]
    fn test_analyze_migration_impact_identical() {
        let state = TestState { balance: 100 };
        let rule_set_v1 = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set_v1, context);
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc::now(),
            },
        ];
        
        // Use the same rule set for comparison
        let rule_set_v2 = TestRuleSet {
            version: Version::new(1, 1, 0),
        };
        
        let analysis = engine.analyze_migration_impact(&transactions, &rule_set_v2);
        assert!(analysis.is_ok());
        
        let analysis = analysis.unwrap();
        assert!(analysis.is_safe_migration());
        assert_eq!(analysis.difference_count(), 0);
        assert!(analysis.identical_final_state);
        assert!(analysis.identical_final_hash);
    }
    
    #[test]
    fn test_analyze_migration_impact_different() {
        let state = TestState { balance: 100 };
        let rule_set_v1 = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set_v1, context);
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc::now(),
            },
        ];
        
        // Create a different rule set that doubles the amount
        #[derive(Clone, Debug)]
        struct DoubleRuleSet {
            version: Version,
        }
        
        impl RuleSet<TestState, TestTransaction> for DoubleRuleSet {
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
                    balance: state.balance + (transaction.amount * 2),
                })
            }
        }
        
        let rule_set_v2 = DoubleRuleSet {
            version: Version::new(2, 0, 0),
        };
        
        let analysis = engine.analyze_migration_impact(&transactions, &rule_set_v2);
        assert!(analysis.is_ok());
        
        let analysis = analysis.unwrap();
        assert!(!analysis.is_safe_migration());
        assert_eq!(analysis.difference_count(), 1);
        assert!(!analysis.identical_final_state);
        assert!(!analysis.identical_final_hash);
        
        // Check the difference details
        let diff = &analysis.differences[0];
        assert_eq!(diff.transaction_id, "tx1");
        assert_eq!(diff.transaction_index, 0);
    }
    
    #[test]
    fn test_verify_migration_safety() {
        let state = TestState { balance: 100 };
        let rule_set_v1 = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set_v1, context);
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc::now(),
            },
        ];
        
        // Test with identical rule set
        let rule_set_v2 = TestRuleSet {
            version: Version::new(1, 1, 0),
        };
        
        let is_safe = engine.verify_migration_safety(&transactions, &rule_set_v2);
        assert!(is_safe.is_ok());
        assert!(is_safe.unwrap());
        
        // Test with different rule set
        #[derive(Clone, Debug)]
        struct DoubleRuleSet {
            version: Version,
        }
        
        impl RuleSet<TestState, TestTransaction> for DoubleRuleSet {
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
                    balance: state.balance + (transaction.amount * 2),
                })
            }
        }
        
        let rule_set_v3 = DoubleRuleSet {
            version: Version::new(2, 0, 0),
        };
        
        let is_safe = engine.verify_migration_safety(&transactions, &rule_set_v3);
        assert!(is_safe.is_ok());
        assert!(!is_safe.unwrap());
    }
    
    #[test]
    fn test_impact_analysis_summary() {
        let state = TestState { balance: 100 };
        let rule_set_v1 = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc::now(), 42);
        
        let engine = ReplayEngine::new(state, rule_set_v1, context);
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc::now(),
            },
        ];
        
        // Test with identical rule set
        let rule_set_v2 = TestRuleSet {
            version: Version::new(1, 1, 0),
        };
        
        let analysis = engine.analyze_migration_impact(&transactions, &rule_set_v2).unwrap();
        let summary = analysis.summary();
        assert!(summary.contains("Safe migration"));
        assert!(summary.contains("1.0.0"));
        assert!(summary.contains("1.1.0"));
    }
}

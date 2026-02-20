use dtre::{ExecutionContext, ReplayEngine, State, Transaction, RuleSet, Version};
use dtre::error::{ValidationError, ProcessingError};
use chrono::{DateTime, Utc, TimeZone};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

// Test state implementation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    balance: i64,
    transaction_count: usize,
}

impl Hash for TestState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.balance.hash(state);
        self.transaction_count.hash(state);
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

// Test transaction implementation
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

// Test rule set implementation
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
            transaction_count: state.transaction_count + 1,
        })
    }
}

// Helper to create arbitrary DateTime
fn arbitrary_datetime() -> impl Strategy<Value = DateTime<Utc>> {
    (0i64..2_000_000_000).prop_map(|secs| {
        Utc.timestamp_opt(secs, 0).unwrap()
    })
}

// Helper to create arbitrary test state
fn arbitrary_test_state() -> impl Strategy<Value = TestState> {
    (0i64..1_000_000, 0usize..1000).prop_map(|(balance, count)| {
        TestState {
            balance,
            transaction_count: count,
        }
    })
}

// Helper to create arbitrary test transaction
fn arbitrary_test_transaction() -> impl Strategy<Value = TestTransaction> {
    (
        "[a-z0-9]{3,10}",
        0i64..1000,  // Only positive amounts to avoid negative balances
        arbitrary_datetime()
    ).prop_map(|(id, amount, timestamp)| {
        TestTransaction {
            id,
            amount,
            timestamp,
        }
    })
}

// Helper to create arbitrary seed
fn arbitrary_seed() -> impl Strategy<Value = u64> {
    any::<u64>()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 1: Core Determinism Guarantee**
    /// **Validates: Requirements 1.1, 1.2**
    /// 
    /// For any transaction sequence, initial state, and rule set, replaying the sequence 
    /// multiple times (including on different machines or at different times) should produce 
    /// byte-for-byte identical final states and identical state hashes.
    #[test]
    fn property_core_determinism_guarantee(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 0..20),
        time in arbitrary_datetime(),
        seed in arbitrary_seed()
    ) {
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(time, seed);
        
        // Create two engines with identical configuration
        let engine1 = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        let engine2 = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        
        // Replay the same transaction sequence on both engines
        let result1 = engine1.replay(&transactions);
        let result2 = engine2.replay(&transactions);
        
        // Both replays should succeed
        prop_assert!(result1.is_ok(), "First replay failed: {:?}", result1.err());
        prop_assert!(result2.is_ok(), "Second replay failed: {:?}", result2.err());
        
        let result1 = result1.unwrap();
        let result2 = result2.unwrap();
        
        // Final states should be byte-for-byte identical
        prop_assert_eq!(&result1.final_state, &result2.final_state,
            "Final states differ");
        
        // State hashes should be identical
        prop_assert_eq!(result1.final_hash, result2.final_hash,
            "State hashes differ");
        
        // Execution traces should be identical
        prop_assert_eq!(result1.execution_trace.transactions_processed,
            result2.execution_trace.transactions_processed,
            "Transaction counts differ");
        prop_assert_eq!(result1.execution_trace.state_transitions.len(),
            result2.execution_trace.state_transitions.len(),
            "State transition counts differ");
        
        // Verify that the final state matches the expected result
        let expected_balance = initial_state.balance + 
            transactions.iter().map(|t| t.amount).sum::<i64>();
        prop_assert_eq!(result1.final_state.balance, expected_balance,
            "Final balance doesn't match expected");
        prop_assert_eq!(result1.final_state.transaction_count,
            initial_state.transaction_count + transactions.len(),
            "Transaction count doesn't match expected");
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 2: Deterministic Ordering**
    /// **Validates: Requirements 1.4, 3.4**
    /// 
    /// For any collection of transactions or data structures, iteration and processing order 
    /// should be stable and reproducible across all executions.
    #[test]
    fn property_deterministic_ordering(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 1..20),
        time in arbitrary_datetime(),
        seed in arbitrary_seed()
    ) {
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(time, seed);
        
        // Create engine and replay
        let engine = ReplayEngine::new(initial_state.clone(), rule_set, context);
        let result = engine.replay(&transactions);
        
        prop_assert!(result.is_ok(), "Replay failed: {:?}", result.err());
        let result = result.unwrap();
        
        // Verify that transactions were processed in the exact order provided
        prop_assert_eq!(result.execution_trace.state_transitions.len(), transactions.len(),
            "Number of state transitions doesn't match number of transactions");
        
        for (i, transaction) in transactions.iter().enumerate() {
            prop_assert_eq!(
                &result.execution_trace.state_transitions[i].transaction_id,
                transaction.id(),
                "Transaction at index {} has wrong ID", i
            );
            prop_assert_eq!(
                &result.execution_trace.rule_applications[i].transaction_id,
                transaction.id(),
                "Rule application at index {} has wrong transaction ID", i
            );
        }
        
        // Verify that the final state reflects ordered processing
        // Each transaction should have been applied in sequence
        let mut expected_balance = initial_state.balance;
        for transaction in &transactions {
            expected_balance += transaction.amount;
        }
        prop_assert_eq!(result.final_state.balance, expected_balance,
            "Final balance doesn't reflect ordered processing");
        
        // Verify that replaying in a different order produces a different result
        // (unless all transactions have the same amount)
        if transactions.len() >= 2 && transactions[0].amount != transactions[1].amount {
            let mut reversed_transactions = transactions.clone();
            reversed_transactions.reverse();
            
            let engine2 = ReplayEngine::new(
                initial_state.clone(),
                TestRuleSet { version: Version::new(1, 0, 0) },
                ExecutionContext::new(time, seed)
            );
            let result2 = engine2.replay(&reversed_transactions).unwrap();
            
            // The final balance should be the same (addition is commutative)
            // but the execution trace should show different ordering
            prop_assert_eq!(result.final_state.balance, result2.final_state.balance,
                "Final balance should be same regardless of order (addition is commutative)");
            
            // But the transaction IDs in the trace should be in different order
            if transactions.len() >= 2 {
                prop_assert_ne!(
                    &result.execution_trace.state_transitions[0].transaction_id,
                    &result2.execution_trace.state_transitions[0].transaction_id,
                    "First transaction ID should differ when order is reversed"
                );
            }
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 17: Parallel Execution Determinism**
    /// **Validates: Requirements 5.5, 9.3**
    /// 
    /// For any transaction sequence, parallel execution should produce identical results 
    /// to sequential execution.
    #[test]
    fn property_parallel_execution_determinism(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 1..50),
        time in arbitrary_datetime(),
        seed in arbitrary_seed()
    ) {
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(time, seed);
        
        // Create engine for sequential execution
        let engine_seq = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        
        // Create engine for parallel execution
        let engine_par = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        
        // Execute sequentially
        let result_seq = engine_seq.replay(&transactions);
        prop_assert!(result_seq.is_ok(), "Sequential replay failed: {:?}", result_seq.err());
        let result_seq = result_seq.unwrap();
        
        // Execute in parallel
        let result_par = engine_par.replay_parallel(&transactions);
        prop_assert!(result_par.is_ok(), "Parallel replay failed: {:?}", result_par.err());
        let result_par = result_par.unwrap();
        
        // Final states should be identical
        prop_assert_eq!(&result_seq.final_state, &result_par.final_state,
            "Sequential and parallel final states differ");
        
        // State hashes should be identical
        prop_assert_eq!(result_seq.final_hash, result_par.final_hash,
            "Sequential and parallel state hashes differ");
        
        // Transaction counts should match
        prop_assert_eq!(
            result_seq.execution_trace.transactions_processed,
            result_par.execution_trace.transactions_processed,
            "Transaction counts differ between sequential and parallel"
        );
        
        // State transition counts should match
        prop_assert_eq!(
            result_seq.execution_trace.state_transitions.len(),
            result_par.execution_trace.state_transitions.len(),
            "State transition counts differ between sequential and parallel"
        );
        
        // Verify that all state transitions have matching transaction IDs
        for i in 0..result_seq.execution_trace.state_transitions.len() {
            prop_assert_eq!(
                &result_seq.execution_trace.state_transitions[i].transaction_id,
                &result_par.execution_trace.state_transitions[i].transaction_id,
                "Transaction ID at index {} differs between sequential and parallel", i
            );
        }
        
        // Verify that the final balance is correct
        let expected_balance = initial_state.balance + 
            transactions.iter().map(|t| t.amount).sum::<i64>();
        prop_assert_eq!(result_par.final_state.balance, expected_balance,
            "Parallel execution final balance doesn't match expected");
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_determinism_across_multiple_replays() {
        let initial_state = TestState {
            balance: 1000,
            transaction_count: 0,
        };
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 100,
                timestamp: Utc.timestamp_opt(1000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx2".to_string(),
                amount: 200,
                timestamp: Utc.timestamp_opt(2000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx3".to_string(),
                amount: 300,
                timestamp: Utc.timestamp_opt(3000, 0).unwrap(),
            },
        ];
        
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, 42);
        
        // Perform multiple replays
        let mut results = Vec::new();
        for _ in 0..5 {
            let engine = ReplayEngine::new(
                initial_state.clone(),
                rule_set.clone(),
                context.clone()
            );
            let result = engine.replay(&transactions).unwrap();
            results.push(result);
        }
        
        // All results should be identical
        for i in 1..results.len() {
            assert_eq!(results[0].final_state, results[i].final_state);
            assert_eq!(results[0].final_hash, results[i].final_hash);
            assert_eq!(
                results[0].execution_trace.transactions_processed,
                results[i].execution_trace.transactions_processed
            );
        }
    }
    
    #[test]
    fn test_ordering_is_preserved() {
        let initial_state = TestState {
            balance: 0,
            transaction_count: 0,
        };
        
        let transactions = vec![
            TestTransaction {
                id: "first".to_string(),
                amount: 10,
                timestamp: Utc.timestamp_opt(1000, 0).unwrap(),
            },
            TestTransaction {
                id: "second".to_string(),
                amount: 20,
                timestamp: Utc.timestamp_opt(2000, 0).unwrap(),
            },
            TestTransaction {
                id: "third".to_string(),
                amount: 30,
                timestamp: Utc.timestamp_opt(3000, 0).unwrap(),
            },
        ];
        
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc.timestamp_opt(1000000, 0).unwrap(), 42);
        
        let engine = ReplayEngine::new(initial_state, rule_set, context);
        let result = engine.replay(&transactions).unwrap();
        
        // Verify ordering in execution trace
        assert_eq!(result.execution_trace.state_transitions[0].transaction_id, "first");
        assert_eq!(result.execution_trace.state_transitions[1].transaction_id, "second");
        assert_eq!(result.execution_trace.state_transitions[2].transaction_id, "third");
        
        // Verify final state reflects ordered processing
        assert_eq!(result.final_state.balance, 60);
        assert_eq!(result.final_state.transaction_count, 3);
    }
    
    #[test]
    fn test_empty_transaction_sequence_is_deterministic() {
        let initial_state = TestState {
            balance: 100,
            transaction_count: 0,
        };
        
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc.timestamp_opt(1000000, 0).unwrap(), 42);
        
        let engine1 = ReplayEngine::new(initial_state.clone(), rule_set.clone(), context.clone());
        let engine2 = ReplayEngine::new(initial_state.clone(), rule_set.clone(), context.clone());
        
        let transactions: Vec<TestTransaction> = vec![];
        
        let result1 = engine1.replay(&transactions).unwrap();
        let result2 = engine2.replay(&transactions).unwrap();
        
        assert_eq!(result1.final_state, result2.final_state);
        assert_eq!(result1.final_hash, result2.final_hash);
        assert_eq!(result1.final_state.balance, 100);
        assert_eq!(result1.final_state.transaction_count, 0);
    }
    
    #[test]
    fn test_parallel_execution_matches_sequential() {
        let initial_state = TestState {
            balance: 1000,
            transaction_count: 0,
        };
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 100,
                timestamp: Utc.timestamp_opt(1000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx2".to_string(),
                amount: 200,
                timestamp: Utc.timestamp_opt(2000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx3".to_string(),
                amount: 300,
                timestamp: Utc.timestamp_opt(3000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx4".to_string(),
                amount: 400,
                timestamp: Utc.timestamp_opt(4000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx5".to_string(),
                amount: 500,
                timestamp: Utc.timestamp_opt(5000, 0).unwrap(),
            },
        ];
        
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, 42);
        
        // Sequential execution
        let engine_seq = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        let result_seq = engine_seq.replay(&transactions).unwrap();
        
        // Parallel execution
        let engine_par = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        let result_par = engine_par.replay_parallel(&transactions).unwrap();
        
        // Results should be identical
        assert_eq!(result_seq.final_state, result_par.final_state);
        assert_eq!(result_seq.final_hash, result_par.final_hash);
        assert_eq!(result_seq.final_state.balance, 2500);
        assert_eq!(result_seq.final_state.transaction_count, 5);
        assert_eq!(
            result_seq.execution_trace.transactions_processed,
            result_par.execution_trace.transactions_processed
        );
    }
    
    #[test]
    fn test_parallel_execution_with_large_transaction_set() {
        let initial_state = TestState {
            balance: 0,
            transaction_count: 0,
        };
        
        // Create a large set of transactions to trigger parallel processing
        let mut transactions = Vec::new();
        for i in 0..150 {
            transactions.push(TestTransaction {
                id: format!("tx{}", i),
                amount: i as i64,
                timestamp: Utc.timestamp_opt(1000 + i as i64, 0).unwrap(),
            });
        }
        
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc.timestamp_opt(1000000, 0).unwrap(), 42);
        
        // Sequential execution
        let engine_seq = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        let result_seq = engine_seq.replay(&transactions).unwrap();
        
        // Parallel execution
        let engine_par = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone()
        );
        let result_par = engine_par.replay_parallel(&transactions).unwrap();
        
        // Results should be identical
        assert_eq!(result_seq.final_state, result_par.final_state);
        assert_eq!(result_seq.final_hash, result_par.final_hash);
        
        // Verify the expected final balance (sum of 0..149)
        let expected_balance: i64 = (0..150).sum();
        assert_eq!(result_par.final_state.balance, expected_balance);
        assert_eq!(result_par.final_state.transaction_count, 150);
    }
    
    #[test]
    fn test_parallel_execution_with_small_transaction_set_uses_sequential() {
        let initial_state = TestState {
            balance: 100,
            transaction_count: 0,
        };
        
        // Small transaction set (< 100) should use sequential processing
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 50,
                timestamp: Utc.timestamp_opt(1000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx2".to_string(),
                amount: 30,
                timestamp: Utc.timestamp_opt(2000, 0).unwrap(),
            },
        ];
        
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let context = ExecutionContext::new(Utc.timestamp_opt(1000000, 0).unwrap(), 42);
        
        let engine = ReplayEngine::new(initial_state, rule_set, context);
        let result = engine.replay_parallel(&transactions).unwrap();
        
        // Should still produce correct results
        assert_eq!(result.final_state.balance, 180);
        assert_eq!(result.final_state.transaction_count, 2);
    }
}


// **Feature: deterministic-transaction-replay-engine, Property 33: Incremental Checkpointing**
// **Validates: Requirements 9.1**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    fn property_incremental_checkpointing(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 10..100),
        checkpoint_interval in 5usize..20,
        seed in any::<u64>(),
    ) {
        let time = Utc::now();
        let context = ExecutionContext::new(time, seed);
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        // Create engine with checkpointing enabled
        let engine = ReplayEngine::with_checkpointing(
            initial_state.clone(),
            rule_set.clone(),
            context.clone(),
            checkpoint_interval,
        );
        
        // Replay with checkpointing
        let result = engine.replay(&transactions).unwrap();
        
        // Verify checkpoints were created at the expected intervals
        let expected_checkpoint_count = transactions.len() / checkpoint_interval;
        if expected_checkpoint_count > 0 {
            prop_assert!(result.execution_trace.checkpoints.len() >= expected_checkpoint_count - 1);
        }
        
        // Verify final state is correct
        prop_assert!(result.final_state.validate().is_ok());
        prop_assert_eq!(result.execution_trace.transactions_processed, transactions.len());
    }
}

// **Feature: deterministic-transaction-replay-engine, Property 34: Replay Resumption**
// **Validates: Requirements 9.4**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    fn property_replay_resumption(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 20..50),
        checkpoint_interval in 5usize..10,
        seed in any::<u64>(),
    ) {
        let time = Utc::now();
        let context = ExecutionContext::new(time, seed);
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        // First, do a complete replay without interruption
        let engine_complete = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone(),
        );
        let complete_result = engine_complete.replay(&transactions).unwrap();
        
        // Now, simulate an interrupted replay by splitting transactions
        let split_point = transactions.len() / 2;
        let (first_half, second_half) = transactions.split_at(split_point);
        
        // Process first half with checkpointing
        let engine_partial = ReplayEngine::with_checkpointing(
            initial_state.clone(),
            rule_set.clone(),
            context.clone(),
            checkpoint_interval,
        );
        let partial_result = engine_partial.replay(first_half).unwrap();
        
        // Get the last checkpoint
        prop_assume!(!partial_result.execution_trace.checkpoints.is_empty());
        let last_checkpoint_info = partial_result.execution_trace.checkpoints.last().unwrap();
        
        // Create a checkpoint from the partial result's final state
        let checkpoint = dtre::Checkpoint {
            state: partial_result.final_state.clone(),
            hash: last_checkpoint_info.hash,
            transaction_index: last_checkpoint_info.transaction_index,
            timestamp: last_checkpoint_info.timestamp,
        };
        
        // Resume from checkpoint with remaining transactions
        let engine_resumed = ReplayEngine::new(
            initial_state.clone(),
            rule_set.clone(),
            context.clone(),
        );
        let resumed_result = engine_resumed.replay_from_checkpoint(&checkpoint, second_half).unwrap();
        
        // The final state from resumed replay should match the complete replay
        prop_assert_eq!(resumed_result.final_hash, complete_result.final_hash);
        prop_assert_eq!(resumed_result.final_state.balance, complete_result.final_state.balance);
    }
}

// **Feature: deterministic-transaction-replay-engine, Property 25: Rule Migration Impact Analysis**
// **Validates: Requirements 7.3**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    fn property_rule_migration_impact_analysis(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 1..30),
        seed in any::<u64>(),
        multiplier in 1i64..5,
    ) {
        let time = Utc::now();
        let context = ExecutionContext::new(time, seed);
        
        // Create baseline rule set
        let baseline_rules = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        // Create a modified rule set that applies a multiplier to amounts
        #[derive(Clone, Debug)]
        struct ModifiedRuleSet {
            version: Version,
            multiplier: i64,
        }
        
        impl RuleSet<TestState, TestTransaction> for ModifiedRuleSet {
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
                    balance: state.balance + (transaction.amount * self.multiplier),
                    transaction_count: state.transaction_count + 1,
                })
            }
        }
        
        let modified_rules = ModifiedRuleSet {
            version: Version::new(2, 0, 0),
            multiplier,
        };
        
        // Create engine with baseline rules
        let engine = ReplayEngine::new(
            initial_state.clone(),
            baseline_rules.clone(),
            context.clone(),
        );
        
        // Perform impact analysis
        let analysis = engine.analyze_migration_impact(&transactions, &modified_rules);
        prop_assert!(analysis.is_ok(), "Impact analysis failed: {:?}", analysis.err());
        let analysis = analysis.unwrap();
        
        // Verify analysis structure
        prop_assert_eq!(&analysis.baseline_version, &Version::new(1, 0, 0));
        prop_assert_eq!(&analysis.comparison_version, &Version::new(2, 0, 0));
        
        // Verify both replays succeeded
        prop_assert_eq!(
            analysis.baseline_result.execution_trace.transactions_processed,
            transactions.len()
        );
        prop_assert_eq!(
            analysis.comparison_result.execution_trace.transactions_processed,
            transactions.len()
        );
        
        // If multiplier is 1, results should be identical (safe migration)
        if multiplier == 1 {
            prop_assert!(analysis.is_safe_migration(),
                "Migration with multiplier=1 should be safe");
            prop_assert_eq!(analysis.difference_count(), 0,
                "Migration with multiplier=1 should have no differences");
            prop_assert!(analysis.identical_final_state,
                "Final states should be identical with multiplier=1");
            prop_assert!(analysis.identical_final_hash,
                "Final hashes should be identical with multiplier=1");
        } else {
            // If multiplier is not 1, results should differ (unsafe migration)
            prop_assert!(!analysis.is_safe_migration(),
                "Migration with multiplier={} should not be safe", multiplier);
            
            // If there are transactions, there should be differences
            if !transactions.is_empty() {
                prop_assert!(analysis.difference_count() > 0,
                    "Migration with multiplier={} should have differences", multiplier);
                prop_assert!(!analysis.identical_final_state,
                    "Final states should differ with multiplier={}", multiplier);
                prop_assert!(!analysis.identical_final_hash,
                    "Final hashes should differ with multiplier={}", multiplier);
            }
        }
        
        // Verify that the baseline result matches expected calculation
        let expected_baseline_balance = initial_state.balance + 
            transactions.iter().map(|t| t.amount).sum::<i64>();
        prop_assert_eq!(
            analysis.baseline_result.final_state.balance,
            expected_baseline_balance,
            "Baseline balance doesn't match expected"
        );
        
        // Verify that the comparison result matches expected calculation with multiplier
        let expected_comparison_balance = initial_state.balance + 
            transactions.iter().map(|t| t.amount * multiplier).sum::<i64>();
        prop_assert_eq!(
            analysis.comparison_result.final_state.balance,
            expected_comparison_balance,
            "Comparison balance doesn't match expected"
        );
        
        // Verify that differences are correctly identified
        if multiplier != 1 && !transactions.is_empty() {
            // Each transaction should create a difference
            for (index, diff) in analysis.differences.iter().enumerate() {
                prop_assert_eq!(diff.transaction_index, index,
                    "Difference index should match transaction index");
                prop_assert_eq!(&diff.transaction_id, transactions[index].id(),
                    "Difference transaction ID should match");
                prop_assert_ne!(diff.baseline_hash, diff.comparison_hash,
                    "Baseline and comparison hashes should differ");
            }
        }
        
        // Verify summary contains expected information
        let summary = analysis.summary();
        prop_assert!(summary.contains("1.0.0"), "Summary should contain baseline version");
        prop_assert!(summary.contains("2.0.0"), "Summary should contain comparison version");
        
        if multiplier == 1 {
            prop_assert!(summary.contains("Safe migration"),
                "Summary should indicate safe migration");
        } else if !transactions.is_empty() {
            prop_assert!(summary.contains("Migration impact"),
                "Summary should indicate migration impact");
        }
    }
}

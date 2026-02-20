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
        -1000i64..1000,
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
}

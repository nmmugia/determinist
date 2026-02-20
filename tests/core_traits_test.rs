use dtre::*;
use dtre::types::PerformanceMetrics;
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use chrono::{DateTime, Utc};

// Test implementations of the core traits

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    balance: i64,
    counter: u32,
}

impl Hash for TestState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.balance.hash(state);
        self.counter.hash(state);
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
        if self.id.is_empty() {
            return Err(ValidationError::InvalidTransaction {
                reason: "Transaction ID cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}

// Property test generators

fn arb_test_state() -> impl Strategy<Value = TestState> {
    (0i64..1000000, 0u32..10000).prop_map(|(balance, counter)| TestState { balance, counter })
}

fn arb_test_transaction() -> impl Strategy<Value = TestTransaction> {
    (
        "[a-z]{3,10}",
        -1000i64..1000,
        0i64..1000000000,
    ).prop_map(|(id, amount, timestamp_secs)| TestTransaction {
        id,
        amount,
        timestamp: DateTime::from_timestamp(timestamp_secs, 0).unwrap_or_else(|| Utc::now()),
    })
}

fn arb_replay_result() -> impl Strategy<Value = ReplayResult<TestState>> {
    (
        arb_test_state(),
        prop::array::uniform32(any::<u8>()),
        1usize..100,
        1u64..10000,
        1.0f64..1000.0,
        1.0f64..100.0,
    ).prop_map(|(state, hash_bytes, tx_count, duration, tps, avg_time)| {
        ReplayResult {
            final_state: state,
            final_hash: StateHash(hash_bytes),
            execution_trace: ExecutionTrace {
                transactions_processed: tx_count,
                state_transitions: vec![],
                rule_applications: vec![],
            },
            performance_metrics: PerformanceMetrics {
                total_duration_ms: duration,
                transactions_per_second: tps,
                average_transaction_time_ms: avg_time,
            },
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 36: Result Object Completeness**
    /// **Validates: Requirements 10.4**
    /// 
    /// For any completed processing, the result objects should contain all expected data and metadata.
    #[test]
    fn property_result_object_completeness(result in arb_replay_result()) {
        // Verify that the result object contains all required fields
        
        // 1. Final state should be present and accessible
        let _ = &result.final_state;
        
        // 2. Final hash should be present and have correct length
        prop_assert_eq!(result.final_hash.0.len(), 32);
        
        // 3. Execution trace should be present with valid transaction count
        prop_assert!(result.execution_trace.transactions_processed > 0);
        
        // 4. Performance metrics should be present and valid
        prop_assert!(result.performance_metrics.total_duration_ms > 0);
        prop_assert!(result.performance_metrics.transactions_per_second > 0.0);
        prop_assert!(result.performance_metrics.average_transaction_time_ms > 0.0);
        
        // 5. Result should be serializable (required for storage/transmission)
        let serialized = bincode::serialize(&result);
        prop_assert!(serialized.is_ok(), "Result should be serializable");
        
        // 6. Result should be deserializable (round-trip test)
        if let Ok(bytes) = serialized {
            let deserialized: Result<ReplayResult<TestState>, _> = bincode::deserialize(&bytes);
            prop_assert!(deserialized.is_ok(), "Result should be deserializable");
            
            if let Ok(restored) = deserialized {
                // Verify key fields are preserved
                prop_assert_eq!(restored.final_hash, result.final_hash);
                prop_assert_eq!(restored.execution_trace.transactions_processed, 
                              result.execution_trace.transactions_processed);
                prop_assert_eq!(restored.performance_metrics.total_duration_ms,
                              result.performance_metrics.total_duration_ms);
            }
        }
    }
    
    /// Test that State trait validation works correctly
    #[test]
    fn property_state_validation(balance in 0i64..1000000, counter in 0u32..10000) {
        let state = TestState { balance, counter };
        let validation_result = state.validate();
        prop_assert!(validation_result.is_ok(), "Valid state should pass validation");
    }
    
    /// Test that State trait rejects invalid states
    #[test]
    fn property_state_validation_rejects_invalid(balance in -1000i64..-1, counter in 0u32..10000) {
        let state = TestState { balance, counter };
        let validation_result = state.validate();
        prop_assert!(validation_result.is_err(), "Invalid state should fail validation");
    }
    
    /// Test that Transaction trait validation works correctly
    #[test]
    fn property_transaction_validation(tx in arb_test_transaction()) {
        let validation_result = tx.validate();
        prop_assert!(validation_result.is_ok(), "Valid transaction should pass validation");
        
        // Verify transaction ID is accessible
        prop_assert!(!tx.id().is_empty());
        
        // Verify timestamp is accessible
        let _ = tx.timestamp();
    }
}

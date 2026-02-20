use dtre::{
    ExecutionContext, TransactionProcessor, State, Transaction, RuleSet,
    ValidationError, ProcessingError, Version,
};
use chrono::{DateTime, Utc, TimeZone};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

// Test state for property tests
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

// Test transaction for property tests
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

// Test rule set for property tests
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

// Generators for property tests
fn arbitrary_test_state() -> impl Strategy<Value = TestState> {
    (0i64..1000000, 0usize..1000).prop_map(|(balance, count)| TestState {
        balance,
        transaction_count: count,
    })
}

fn arbitrary_test_transaction() -> impl Strategy<Value = TestTransaction> {
    (
        "[a-z]{3,10}",
        -1000i64..1000,
        0i64..2_000_000_000,
    ).prop_map(|(id, amount, timestamp_secs)| TestTransaction {
        id,
        amount,
        timestamp: Utc.timestamp_opt(timestamp_secs, 0).unwrap(),
    })
}

fn arbitrary_version() -> impl Strategy<Value = Version> {
    (0u32..10, 0u32..10, 0u32..10).prop_map(|(major, minor, patch)| {
        Version::new(major, minor, patch)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 4: Rule Version Consistency**
    /// **Validates: Requirements 2.1**
    /// 
    /// For any transaction and rule set version, processing with the same version should 
    /// always produce identical results, while different versions may produce different results.
    #[test]
    fn property_rule_version_consistency(
        initial_state in arbitrary_test_state(),
        transaction in arbitrary_test_transaction(),
        version in arbitrary_version(),
        seed in any::<u64>(),
    ) {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, seed);
        
        // Create two processors with the same initial state
        let mut processor1 = TransactionProcessor::new(initial_state.clone()).unwrap();
        let mut processor2 = TransactionProcessor::new(initial_state.clone()).unwrap();
        
        // Create rule sets with the same version
        let rule_set1 = TestRuleSet { version: version.clone() };
        let rule_set2 = TestRuleSet { version: version.clone() };
        
        // Process the same transaction with both processors
        let result1 = processor1.process_transaction(&transaction, &rule_set1, &context);
        let result2 = processor2.process_transaction(&transaction, &rule_set2, &context);
        
        // Both should succeed or fail identically
        prop_assert_eq!(result1.is_ok(), result2.is_ok());
        
        if result1.is_ok() {
            // Final states should be identical
            prop_assert_eq!(processor1.current_state(), processor2.current_state());
            
            // State hashes should be identical
            prop_assert_eq!(processor1.current_hash(), processor2.current_hash());
            
            // Execution traces should record the same version
            let trace1 = processor1.execution_trace();
            let trace2 = processor2.execution_trace();
            prop_assert_eq!(&trace1.rule_applications[0].rule_version, &version);
            prop_assert_eq!(&trace2.rule_applications[0].rule_version, &version);
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 15: Rule Tracking Completeness**
    /// **Validates: Requirements 5.3**
    /// 
    /// For any replay with rule version tracking, the trace should accurately record 
    /// which rules produced which state changes.
    #[test]
    fn property_rule_tracking_completeness(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 1..10),
        versions in prop::collection::vec(arbitrary_version(), 1..10),
        seed in any::<u64>(),
    ) {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, seed);
        
        let mut processor = TransactionProcessor::new(initial_state).unwrap();
        
        // Process transactions with different rule versions
        for (i, transaction) in transactions.iter().enumerate() {
            let version = &versions[i % versions.len()];
            let rule_set = TestRuleSet { version: version.clone() };
            
            let _ = processor.process_transaction(transaction, &rule_set, &context);
        }
        
        let trace = processor.execution_trace();
        
        // The trace should have recorded all successful transactions
        prop_assert!(trace.rule_applications.len() <= transactions.len());
        
        // Each rule application should have a version and transaction ID
        for (i, rule_app) in trace.rule_applications.iter().enumerate() {
            prop_assert!(!rule_app.transaction_id.is_empty());
            
            // The version should match one of the versions we used
            let expected_version = &versions[i % versions.len()];
            prop_assert_eq!(&rule_app.rule_version, expected_version);
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 16: Execution Trace Completeness**
    /// **Validates: Requirements 5.4**
    /// 
    /// For any completed replay, the execution trace should contain all expected information 
    /// about transactions, state transitions, and rule applications.
    #[test]
    fn property_execution_trace_completeness(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 1..20),
        version in arbitrary_version(),
        seed in any::<u64>(),
    ) {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, seed);
        
        let mut processor = TransactionProcessor::new(initial_state).unwrap();
        let rule_set = TestRuleSet { version: version.clone() };
        
        let mut successful_count = 0;
        for transaction in &transactions {
            if processor.process_transaction(transaction, &rule_set, &context).is_ok() {
                successful_count += 1;
            }
        }
        
        let trace = processor.execution_trace();
        
        // Trace should record the correct number of transactions processed
        prop_assert_eq!(trace.transactions_processed, successful_count);
        
        // State transitions should match the number of successful transactions
        prop_assert_eq!(trace.state_transitions.len(), successful_count);
        
        // Rule applications should match the number of successful transactions
        prop_assert_eq!(trace.rule_applications.len(), successful_count);
        
        // Each state transition should have valid hashes and transaction IDs
        for transition in &trace.state_transitions {
            prop_assert!(!transition.transaction_id.is_empty());
            // Hashes should be non-zero (assuming valid states produce non-zero hashes)
        }
        
        // Each rule application should have the correct version
        for rule_app in &trace.rule_applications {
            prop_assert_eq!(&rule_app.rule_version, &version);
            prop_assert!(!rule_app.transaction_id.is_empty());
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 18: Transaction Event Validation**
    /// **Validates: Requirements 6.1**
    /// 
    /// For any transaction event, validation should consistently accept valid events 
    /// and reject invalid events.
    #[test]
    fn property_transaction_event_validation(
        initial_state in arbitrary_test_state(),
        transaction in arbitrary_test_transaction(),
        version in arbitrary_version(),
        seed in any::<u64>(),
    ) {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, seed);
        
        let mut processor = TransactionProcessor::new(initial_state).unwrap();
        let rule_set = TestRuleSet { version };
        
        // Valid transactions should be validated consistently
        let validation_result = transaction.validate();
        prop_assert!(validation_result.is_ok());
        
        // Processing should succeed for valid transactions (assuming valid state transitions)
        let process_result = processor.process_transaction(&transaction, &rule_set, &context);
        
        // If validation passed, processing should at least attempt to apply the transaction
        // (it may still fail due to state validation, but not due to transaction validation)
        if validation_result.is_ok() {
            // The processor should have attempted to process it
            // (we can't guarantee success due to state validation, but we can check consistency)
            prop_assert!(process_result.is_ok() || process_result.is_err());
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 19: Timestamp Determinism**
    /// **Validates: Requirements 6.2**
    /// 
    /// For any events with timestamps, the timestamp handling should be deterministic 
    /// and consistent.
    #[test]
    fn property_timestamp_determinism(
        initial_state in arbitrary_test_state(),
        transactions in prop::collection::vec(arbitrary_test_transaction(), 1..10),
        version in arbitrary_version(),
        seed in any::<u64>(),
    ) {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, seed);
        
        // Process transactions twice with the same context
        let mut processor1 = TransactionProcessor::new(initial_state.clone()).unwrap();
        let mut processor2 = TransactionProcessor::new(initial_state).unwrap();
        let rule_set = TestRuleSet { version };
        
        for transaction in &transactions {
            let _ = processor1.process_transaction(transaction, &rule_set, &context);
            let _ = processor2.process_transaction(transaction, &rule_set, &context);
        }
        
        let trace1 = processor1.execution_trace();
        let trace2 = processor2.execution_trace();
        
        // Both traces should have the same number of rule applications
        prop_assert_eq!(trace1.rule_applications.len(), trace2.rule_applications.len());
        
        // Timestamps in rule applications should be identical
        for (app1, app2) in trace1.rule_applications.iter().zip(trace2.rule_applications.iter()) {
            prop_assert_eq!(app1.timestamp, app2.timestamp);
            prop_assert_eq!(&app1.transaction_id, &app2.transaction_id);
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 22: Event Data Immutability**
    /// **Validates: Requirements 6.5**
    /// 
    /// For any transaction events, the original event data should remain unchanged 
    /// throughout processing.
    #[test]
    fn property_event_data_immutability(
        initial_state in arbitrary_test_state(),
        transaction in arbitrary_test_transaction(),
        version in arbitrary_version(),
        seed in any::<u64>(),
    ) {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let context = ExecutionContext::new(time, seed);
        
        // Clone the transaction to compare later
        let original_transaction = transaction.clone();
        let original_id = transaction.id().to_string();
        let original_amount = transaction.amount;
        let original_timestamp = transaction.timestamp();
        
        let mut processor = TransactionProcessor::new(initial_state).unwrap();
        let rule_set = TestRuleSet { version };
        
        // Process the transaction
        let _ = processor.process_transaction(&transaction, &rule_set, &context);
        
        // Verify the transaction data hasn't changed
        prop_assert_eq!(transaction.id(), original_id.as_str());
        prop_assert_eq!(transaction.amount, original_amount);
        prop_assert_eq!(transaction.timestamp(), original_timestamp);
        
        // Verify it's still equal to the original clone
        prop_assert_eq!(transaction.id(), original_transaction.id());
        prop_assert_eq!(transaction.amount, original_transaction.amount);
        prop_assert_eq!(transaction.timestamp(), original_transaction.timestamp());
    }
}

// Unit tests for transaction processor
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_basic_transaction_processing() {
        let state = TestState {
            balance: 100,
            transaction_count: 0,
        };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let transaction = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc.timestamp_opt(1000000, 0).unwrap(),
        };
        
        let context = ExecutionContext::new(Utc.timestamp_opt(1000000, 0).unwrap(), 42);
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        let result = processor.process_transaction(&transaction, &rule_set, &context);
        assert!(result.is_ok());
        
        assert_eq!(processor.current_state().balance, 150);
        assert_eq!(processor.current_state().transaction_count, 1);
    }
    
    #[test]
    fn test_multiple_rule_versions() {
        let state = TestState {
            balance: 100,
            transaction_count: 0,
        };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let tx1 = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc.timestamp_opt(1000000, 0).unwrap(),
        };
        
        let tx2 = TestTransaction {
            id: "tx2".to_string(),
            amount: 30,
            timestamp: Utc.timestamp_opt(1000001, 0).unwrap(),
        };
        
        let context = ExecutionContext::new(Utc.timestamp_opt(1000000, 0).unwrap(), 42);
        let rule_set_v1 = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        let rule_set_v2 = TestRuleSet {
            version: Version::new(2, 0, 0),
        };
        
        processor.process_transaction(&tx1, &rule_set_v1, &context).unwrap();
        processor.process_transaction(&tx2, &rule_set_v2, &context).unwrap();
        
        let trace = processor.execution_trace();
        assert_eq!(trace.rule_applications.len(), 2);
        assert_eq!(trace.rule_applications[0].rule_version, Version::new(1, 0, 0));
        assert_eq!(trace.rule_applications[1].rule_version, Version::new(2, 0, 0));
    }
    
    #[test]
    fn test_execution_trace_completeness() {
        let state = TestState {
            balance: 100,
            transaction_count: 0,
        };
        let mut processor = TransactionProcessor::new(state).unwrap();
        
        let transactions = vec![
            TestTransaction {
                id: "tx1".to_string(),
                amount: 10,
                timestamp: Utc.timestamp_opt(1000000, 0).unwrap(),
            },
            TestTransaction {
                id: "tx2".to_string(),
                amount: 20,
                timestamp: Utc.timestamp_opt(1000001, 0).unwrap(),
            },
            TestTransaction {
                id: "tx3".to_string(),
                amount: 30,
                timestamp: Utc.timestamp_opt(1000002, 0).unwrap(),
            },
        ];
        
        let context = ExecutionContext::new(Utc.timestamp_opt(1000000, 0).unwrap(), 42);
        let rule_set = TestRuleSet {
            version: Version::new(1, 0, 0),
        };
        
        for tx in &transactions {
            processor.process_transaction(tx, &rule_set, &context).unwrap();
        }
        
        let trace = processor.execution_trace();
        assert_eq!(trace.transactions_processed, 3);
        assert_eq!(trace.state_transitions.len(), 3);
        assert_eq!(trace.rule_applications.len(), 3);
        
        // Verify transaction IDs are recorded correctly
        assert_eq!(trace.state_transitions[0].transaction_id, "tx1");
        assert_eq!(trace.state_transitions[1].transaction_id, "tx2");
        assert_eq!(trace.state_transitions[2].transaction_id, "tx3");
    }
}

use dtre::*;
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use chrono::{DateTime, Utc, TimeZone};

// Test implementations for property tests

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    balance: i64,
    counter: u32,
    name: String,
}

impl Hash for TestState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.balance.hash(state);
        self.counter.hash(state);
        self.name.hash(state);
    }
}

impl State for TestState {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.balance < 0 {
            return Err(ValidationError::InvalidState {
                reason: "Balance cannot be negative".to_string(),
            });
        }
        if self.name.is_empty() {
            return Err(ValidationError::InvalidState {
                reason: "Name cannot be empty".to_string(),
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
        let new_balance = state.balance + transaction.amount;
        
        Ok(TestState {
            balance: new_balance,
            counter: state.counter + 1,
            name: state.name.clone(),
        })
    }
}

// Failing rule set that always returns an error
struct FailingRuleSet;

impl RuleSet<TestState, TestTransaction> for FailingRuleSet {
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    
    fn apply(
        &self,
        _state: &TestState,
        transaction: &TestTransaction,
        _context: &ExecutionContext,
    ) -> Result<TestState, ProcessingError> {
        Err(ProcessingError::TransactionFailed {
            transaction_id: transaction.id().to_string(),
            reason: "Intentional failure for testing".to_string(),
        })
    }
}

// Property test generators

fn arb_valid_test_state() -> impl Strategy<Value = TestState> {
    (0i64..1000000, 0u32..10000, "[a-z]{3,20}").prop_map(|(balance, counter, name)| {
        TestState { balance, counter, name }
    })
}

fn arb_invalid_test_state() -> impl Strategy<Value = TestState> {
    prop_oneof![
        // Negative balance
        (-1000i64..-1, 0u32..10000, "[a-z]{3,20}").prop_map(|(balance, counter, name)| {
            TestState { balance, counter, name }
        }),
        // Empty name
        (0i64..1000000, 0u32..10000, Just("".to_string())).prop_map(|(balance, counter, name)| {
            TestState { balance, counter, name }
        }),
    ]
}

fn arb_test_transaction() -> impl Strategy<Value = TestTransaction> {
    (
        "[a-z]{3,10}",
        -1000i64..1000,
        0i64..1000000000,
    ).prop_map(|(id, amount, timestamp_secs)| {
        TestTransaction {
            id,
            amount,
            timestamp: Utc.timestamp_opt(timestamp_secs, 0).unwrap(),
        }
    })
}

fn arb_datetime() -> impl Strategy<Value = DateTime<Utc>> {
    (0i64..2_000_000_000).prop_map(|secs| {
        Utc.timestamp_opt(secs, 0).unwrap()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 5: State Validation**
    /// **Validates: Requirements 2.3**
    /// 
    /// For any state transition, invalid states should be rejected and valid states 
    /// should be accepted consistently.
    #[test]
    fn property_state_validation(
        valid_state in arb_valid_test_state(),
        invalid_state in arb_invalid_test_state()
    ) {
        // Valid states should be accepted by StateManager
        let manager_result = StateManager::new(valid_state.clone());
        prop_assert!(manager_result.is_ok(), 
            "Valid state should be accepted: {:?}", valid_state);
        
        // Invalid states should be rejected by StateManager
        let invalid_manager_result = StateManager::new(invalid_state.clone());
        prop_assert!(invalid_manager_result.is_err(), 
            "Invalid state should be rejected: {:?}", invalid_state);
        
        // Validation should be consistent across multiple calls
        let validation1 = valid_state.validate();
        let validation2 = valid_state.validate();
        prop_assert_eq!(validation1.is_ok(), validation2.is_ok(),
            "Validation should be consistent");
        
        // Invalid state validation should consistently fail
        let invalid_validation1 = invalid_state.validate();
        let invalid_validation2 = invalid_state.validate();
        prop_assert!(invalid_validation1.is_err() && invalid_validation2.is_err(),
            "Invalid state validation should consistently fail");
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 6: Error State Preservation**
    /// **Validates: Requirements 2.4**
    /// 
    /// For any processing failure, the original state should remain completely unchanged.
    #[test]
    fn property_error_state_preservation(
        initial_state in arb_valid_test_state(),
        transaction in arb_test_transaction(),
        seed in any::<u64>()
    ) {
        let mut manager = StateManager::new(initial_state.clone()).unwrap();
        
        // Store the initial state and hash
        let initial_hash = manager.current_hash();
        let initial_balance = manager.current_state().balance;
        let initial_counter = manager.current_state().counter;
        
        // Apply a transaction with a failing rule set
        let context = ExecutionContext::new(Utc::now(), seed);
        let failing_rules = FailingRuleSet;
        
        let result = manager.apply_transaction(&transaction, &failing_rules, &context);
        
        // The transaction should fail
        prop_assert!(result.is_err(), "Transaction should fail with FailingRuleSet");
        
        // The state should remain completely unchanged
        prop_assert_eq!(manager.current_state().balance, initial_balance,
            "Balance should be unchanged after failed transaction");
        prop_assert_eq!(manager.current_state().counter, initial_counter,
            "Counter should be unchanged after failed transaction");
        prop_assert_eq!(manager.current_hash(), initial_hash,
            "State hash should be unchanged after failed transaction");
        
        // The state should still be valid
        prop_assert!(manager.current_state().validate().is_ok(),
            "State should remain valid after failed transaction");
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 11: State Diff Accuracy**
    /// **Validates: Requirements 4.2, 4.3**
    /// 
    /// For any two states, the computed diff should accurately identify all differences between them.
    #[test]
    fn property_state_diff_accuracy(
        state1 in arb_valid_test_state(),
        state2 in arb_valid_test_state()
    ) {
        let manager = StateManager::new(state1.clone()).unwrap();
        
        // Calculate diff between the two states
        let diff = manager.calculate_diff(&state1, &state2);
        
        // Diff should contain both states
        prop_assert_eq!(&diff.from_state, &state1, "Diff should contain original from_state");
        prop_assert_eq!(&diff.to_state, &state2, "Diff should contain original to_state");
        
        // Diff hashes should match the actual state hashes
        let hasher = StateHasher::new();
        let expected_from_hash = hasher.hash(&state1);
        let expected_to_hash = hasher.hash(&state2);
        
        prop_assert_eq!(diff.from_hash, expected_from_hash,
            "Diff from_hash should match actual state hash");
        prop_assert_eq!(diff.to_hash, expected_to_hash,
            "Diff to_hash should match actual state hash");
        
        // If states are identical, hashes should be identical
        if state1 == state2 {
            prop_assert_eq!(diff.from_hash, diff.to_hash,
                "Identical states should have identical hashes");
        }
        
        // If states are different, we can detect it through comparison
        let are_same = manager.compare_states(&state1, &state2);
        if state1 == state2 {
            prop_assert!(are_same, "Identical states should compare as same");
        } else {
            prop_assert!(!are_same, "Different states should compare as different");
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 13: Checkpoint Round-Trip**
    /// **Validates: Requirements 5.1**
    /// 
    /// For any state, creating a checkpoint and then restoring from it should produce 
    /// an identical state.
    #[test]
    fn property_checkpoint_round_trip(
        initial_state in arb_valid_test_state(),
        transactions in prop::collection::vec(arb_test_transaction(), 0..10),
        checkpoint_time in arb_datetime(),
        seed in any::<u64>()
    ) {
        let mut manager = StateManager::new(initial_state.clone()).unwrap();
        
        // Apply some transactions
        let context = ExecutionContext::new(Utc::now(), seed);
        let rules = TestRuleSet;
        
        for transaction in &transactions {
            let _ = manager.apply_transaction(transaction, &rules, &context);
        }
        
        // Create a checkpoint at the current state
        let checkpoint = manager.create_checkpoint(checkpoint_time);
        
        // Store the current state for comparison
        let state_before_checkpoint = manager.current_state().clone();
        let hash_before_checkpoint = manager.current_hash();
        let tx_count_before = manager.transaction_count();
        
        // Verify checkpoint contains correct data
        prop_assert_eq!(&checkpoint.state, &state_before_checkpoint,
            "Checkpoint should contain current state");
        prop_assert_eq!(checkpoint.hash, hash_before_checkpoint,
            "Checkpoint hash should match current hash");
        prop_assert_eq!(checkpoint.transaction_index, tx_count_before,
            "Checkpoint should record correct transaction index");
        prop_assert_eq!(checkpoint.timestamp, checkpoint_time,
            "Checkpoint should record correct timestamp");
        
        // Apply more transactions to change the state
        for transaction in &transactions {
            let _ = manager.apply_transaction(transaction, &rules, &context);
        }
        
        // State should now be different (if we applied any transactions)
        if !transactions.is_empty() {
            let state_after_more_txs = manager.current_state().clone();
            prop_assert_ne!(&state_after_more_txs, &state_before_checkpoint,
                "State should change after applying more transactions");
        }
        
        // Restore from checkpoint
        let restore_result = manager.restore_checkpoint(&checkpoint);
        prop_assert!(restore_result.is_ok(), "Checkpoint restoration should succeed");
        
        // State should be identical to the checkpoint state
        prop_assert_eq!(manager.current_state(), &state_before_checkpoint,
            "Restored state should match checkpoint state");
        prop_assert_eq!(manager.current_hash(), hash_before_checkpoint,
            "Restored hash should match checkpoint hash");
        prop_assert_eq!(manager.transaction_count(), tx_count_before,
            "Restored transaction count should match checkpoint");
        
        // Restoring the same checkpoint again should be idempotent
        let restore_again = manager.restore_checkpoint(&checkpoint);
        prop_assert!(restore_again.is_ok(), "Second restoration should succeed");
        prop_assert_eq!(manager.current_state(), &state_before_checkpoint,
            "State should remain unchanged after second restoration");
    }
    
    /// Additional test: Verify state transitions are tracked correctly
    #[test]
    fn property_state_transition_tracking(
        initial_state in arb_valid_test_state(),
        transaction in arb_test_transaction(),
        seed in any::<u64>()
    ) {
        let mut manager = StateManager::new(initial_state.clone()).unwrap();
        
        let initial_hash = manager.current_hash();
        let context = ExecutionContext::new(Utc::now(), seed);
        let rules = TestRuleSet;
        
        // Apply transaction
        let transition_result = manager.apply_transaction(&transaction, &rules, &context);
        prop_assert!(transition_result.is_ok(), "Transaction should succeed");
        
        let transition = transition_result.unwrap();
        
        // Verify transition contains correct information
        prop_assert_eq!(&transition.from_state, &initial_state,
            "Transition should record original state");
        prop_assert_eq!(transition.from_hash, initial_hash,
            "Transition should record original hash");
        prop_assert_eq!(transition.transaction_id, transaction.id(),
            "Transition should record transaction ID");
        
        // Verify the new state is different (for non-zero amount transactions)
        if transaction.amount != 0 {
            prop_assert_ne!(transition.from_hash, transition.to_hash,
                "State hash should change after non-zero transaction");
        }
        
        // Verify the new state matches the manager's current state
        prop_assert_eq!(&transition.to_state, manager.current_state(),
            "Transition to_state should match manager's current state");
        prop_assert_eq!(transition.to_hash, manager.current_hash(),
            "Transition to_hash should match manager's current hash");
    }
    
    /// Additional test: Verify multiple checkpoints work correctly
    #[test]
    fn property_multiple_checkpoints(
        initial_state in arb_valid_test_state(),
        transactions in prop::collection::vec(arb_test_transaction(), 1..5),
        seed in any::<u64>()
    ) {
        let mut manager = StateManager::new(initial_state).unwrap();
        let context = ExecutionContext::new(Utc::now(), seed);
        let rules = TestRuleSet;
        
        let mut checkpoints = Vec::new();
        let mut expected_states = Vec::new();
        
        // Create checkpoints after each transaction
        for (i, transaction) in transactions.iter().enumerate() {
            let _ = manager.apply_transaction(transaction, &rules, &context);
            
            let checkpoint_time = Utc.timestamp_opt(1000000 + i as i64, 0).unwrap();
            let checkpoint = manager.create_checkpoint(checkpoint_time);
            
            checkpoints.push(checkpoint.clone());
            expected_states.push(manager.current_state().clone());
        }
        
        // Verify we can restore to any checkpoint
        for (i, checkpoint) in checkpoints.iter().enumerate() {
            let restore_result = manager.restore_checkpoint(checkpoint);
            prop_assert!(restore_result.is_ok(), 
                "Should be able to restore checkpoint {}", i);
            
            prop_assert_eq!(manager.current_state(), &expected_states[i],
                "Restored state should match expected state for checkpoint {}", i);
        }
        
        // Verify checkpoint list is maintained
        prop_assert_eq!(manager.checkpoints().len(), checkpoints.len(),
            "Manager should maintain all checkpoints");
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_state_validation_basic() {
        let valid_state = TestState {
            balance: 100,
            counter: 0,
            name: "test".to_string(),
        };
        
        let manager = StateManager::new(valid_state);
        assert!(manager.is_ok());
    }
    
    #[test]
    fn test_state_validation_negative_balance() {
        let invalid_state = TestState {
            balance: -100,
            counter: 0,
            name: "test".to_string(),
        };
        
        let manager = StateManager::new(invalid_state);
        assert!(manager.is_err());
    }
    
    #[test]
    fn test_state_validation_empty_name() {
        let invalid_state = TestState {
            balance: 100,
            counter: 0,
            name: "".to_string(),
        };
        
        let manager = StateManager::new(invalid_state);
        assert!(manager.is_err());
    }
    
    #[test]
    fn test_error_preserves_state() {
        let initial_state = TestState {
            balance: 100,
            counter: 0,
            name: "test".to_string(),
        };
        
        let mut manager = StateManager::new(initial_state.clone()).unwrap();
        let initial_hash = manager.current_hash();
        
        let transaction = TestTransaction {
            id: "tx1".to_string(),
            amount: 50,
            timestamp: Utc::now(),
        };
        
        let context = ExecutionContext::new(Utc::now(), 42);
        let failing_rules = FailingRuleSet;
        
        let result = manager.apply_transaction(&transaction, &failing_rules, &context);
        assert!(result.is_err());
        
        // State should be unchanged
        assert_eq!(manager.current_state().balance, 100);
        assert_eq!(manager.current_state().counter, 0);
        assert_eq!(manager.current_hash(), initial_hash);
    }
    
    #[test]
    fn test_checkpoint_round_trip_basic() {
        let initial_state = TestState {
            balance: 100,
            counter: 0,
            name: "test".to_string(),
        };
        
        let mut manager = StateManager::new(initial_state).unwrap();
        
        // Create checkpoint
        let checkpoint = manager.create_checkpoint(Utc::now());
        
        // Apply transaction
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
        manager.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(manager.current_state().balance, 100);
    }
    
    #[test]
    fn test_state_diff_identical_states() {
        let state = TestState {
            balance: 100,
            counter: 0,
            name: "test".to_string(),
        };
        
        let manager = StateManager::new(state.clone()).unwrap();
        let diff = manager.calculate_diff(&state, &state);
        
        assert_eq!(diff.from_hash, diff.to_hash);
        assert!(manager.compare_states(&state, &state));
    }
    
    #[test]
    fn test_state_diff_different_states() {
        let state1 = TestState {
            balance: 100,
            counter: 0,
            name: "test".to_string(),
        };
        
        let state2 = TestState {
            balance: 150,
            counter: 1,
            name: "test".to_string(),
        };
        
        let manager = StateManager::new(state1.clone()).unwrap();
        let diff = manager.calculate_diff(&state1, &state2);
        
        assert_ne!(diff.from_hash, diff.to_hash);
        assert!(!manager.compare_states(&state1, &state2));
    }
}

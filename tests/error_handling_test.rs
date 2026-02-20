use dtre::{
    ErrorContext, StateMismatchDetail, FieldDiff, ValidationDetail,
    ProcessingError, ValidationError, StateError, StateHash, Version
};
use proptest::prelude::*;

// Helper to create arbitrary Version
fn arbitrary_version() -> impl Strategy<Value = Version> {
    (0u32..10, 0u32..20, 0u32..100).prop_map(|(major, minor, patch)| {
        Version::new(major, minor, patch)
    })
}

// Helper to create arbitrary StateHash
fn arbitrary_state_hash() -> impl Strategy<Value = StateHash> {
    prop::array::uniform32(any::<u8>()).prop_map(StateHash)
}

// Helper to create arbitrary ErrorContext
fn arbitrary_error_context() -> impl Strategy<Value = ErrorContext> {
    (
        prop::option::of("[a-z0-9]{8,16}"),
        prop::option::of(0usize..1000),
        prop::option::of(arbitrary_version()),
        prop::option::of(arbitrary_state_hash()),
        prop::option::of(arbitrary_state_hash()),
        prop::collection::vec(("[a-z_]{3,10}", "[a-z0-9 ]{5,20}"), 0..5)
    ).prop_map(|(tx_id, tx_idx, rule_ver, hash_before, hash_after, info)| {
        let mut ctx = ErrorContext::new();
        if let (Some(id), Some(idx)) = (tx_id, tx_idx) {
            ctx = ctx.with_transaction(id, idx);
        }
        if let Some(ver) = rule_ver {
            ctx = ctx.with_rule(ver);
        }
        if let Some(before) = hash_before {
            ctx = ctx.with_state_hashes(before, hash_after);
        }
        for (k, v) in info {
            ctx = ctx.with_info(k, v);
        }
        ctx
    })
}

// Helper to create arbitrary FieldDiff
fn arbitrary_field_diff() -> impl Strategy<Value = FieldDiff> {
    (
        "[a-z_]{3,10}(\\.[a-z_]{3,10}){0,3}",
        "[a-z0-9 ]{5,20}",
        "[a-z0-9 ]{5,20}"
    ).prop_map(|(path, expected, actual)| {
        FieldDiff {
            field_path: path,
            expected_value: expected,
            actual_value: actual,
        }
    })
}

// Helper to create arbitrary StateMismatchDetail
fn arbitrary_state_mismatch_detail() -> impl Strategy<Value = StateMismatchDetail> {
    (
        arbitrary_state_hash(),
        arbitrary_state_hash(),
        prop::collection::vec(arbitrary_field_diff(), 0..10),
        prop::option::of("[a-z0-9]{8,16}"),
        prop::option::of(0usize..1000)
    ).prop_map(|(expected, actual, diffs, tx_id, tx_idx)| {
        StateMismatchDetail {
            expected_hash: expected,
            actual_hash: actual,
            field_diffs: diffs,
            transaction_id: tx_id,
            transaction_index: tx_idx,
        }
    })
}

// Helper to create arbitrary ValidationDetail
fn arbitrary_validation_detail() -> impl Strategy<Value = ValidationDetail> {
    (
        prop::collection::vec("[a-z_]{5,15}", 1..5),
        prop::option::of("[a-z_]{3,10}"),
        prop::option::of("[a-z0-9 ]{5,20}"),
        prop::option::of("[a-z0-9 ]{5,20}"),
        arbitrary_error_context()
    ).prop_map(|(rules, field, constraint, value, ctx)| {
        ValidationDetail {
            violated_rules: rules,
            field,
            expected_constraint: constraint,
            actual_value: value,
            context: ctx,
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 28: Error Context Completeness**
    /// **Validates: Requirements 8.1**
    /// 
    /// For any processing failure, the error should contain all expected context information 
    /// including transaction, rule, and state details.
    #[test]
    fn property_error_context_completeness(
        tx_id in "[a-z0-9]{8,16}",
        tx_idx in 0usize..1000,
        rule_ver in arbitrary_version(),
        hash_before in arbitrary_state_hash(),
        hash_after in arbitrary_state_hash(),
        message in "[a-z ]{10,50}"
    ) {
        // Create a comprehensive error context
        let context = ErrorContext::new()
            .with_transaction(tx_id.clone(), tx_idx)
            .with_rule(rule_ver.clone())
            .with_state_hashes(hash_before, Some(hash_after))
            .with_info("operation".to_string(), "test_operation".to_string());
        
        // Create a processing error with context
        let error = ProcessingError::with_context(message.clone(), context.clone());
        
        // Verify the error contains the context
        let retrieved_context = error.context();
        prop_assert!(retrieved_context.is_some());
        
        let ctx = retrieved_context.unwrap();
        
        // Verify all context fields are present
        prop_assert_eq!(ctx.transaction_id.as_ref(), Some(&tx_id));
        prop_assert_eq!(ctx.transaction_index, Some(tx_idx));
        prop_assert_eq!(ctx.rule_version.as_ref(), Some(&rule_ver));
        prop_assert_eq!(ctx.state_hash_before, Some(hash_before));
        prop_assert_eq!(ctx.state_hash_after, Some(hash_after));
        prop_assert!(!ctx.additional_info.is_empty());
        
        // Verify the error message is preserved
        let error_string = format!("{}", error);
        prop_assert!(error_string.contains(&message));
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 30: State Mismatch Reporting**
    /// **Validates: Requirements 8.3**
    /// 
    /// For any state mismatches, the system should provide precise and accurate diff information.
    #[test]
    fn property_state_mismatch_reporting(
        detail in arbitrary_state_mismatch_detail()
    ) {
        // Create a state error with detailed mismatch information
        let error = StateError::mismatch_with_detail(detail.clone());
        
        // Verify the error contains the mismatch details
        let retrieved_detail = error.mismatch_detail();
        prop_assert!(retrieved_detail.is_some());
        
        let mismatch = retrieved_detail.unwrap();
        
        // Verify all mismatch fields are present and correct
        prop_assert_eq!(mismatch.expected_hash, detail.expected_hash);
        prop_assert_eq!(mismatch.actual_hash, detail.actual_hash);
        prop_assert_eq!(mismatch.field_diffs.len(), detail.field_diffs.len());
        prop_assert_eq!(&mismatch.transaction_id, &detail.transaction_id);
        prop_assert_eq!(mismatch.transaction_index, detail.transaction_index);
        
        // Verify field diffs are preserved
        for (i, diff) in mismatch.field_diffs.iter().enumerate() {
            prop_assert_eq!(&diff.field_path, &detail.field_diffs[i].field_path);
            prop_assert_eq!(&diff.expected_value, &detail.field_diffs[i].expected_value);
            prop_assert_eq!(&diff.actual_value, &detail.field_diffs[i].actual_value);
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 31: Validation Error Reporting**
    /// **Validates: Requirements 8.4**
    /// 
    /// For any validation failures, the system should accurately indicate which validation 
    /// rules were violated.
    #[test]
    fn property_validation_error_reporting(
        detail in arbitrary_validation_detail()
    ) {
        // Create a validation error with detailed information
        let error = ValidationError::with_details(detail.clone());
        
        // Verify the error contains the validation details
        let retrieved_detail = error.details();
        prop_assert!(retrieved_detail.is_some());
        
        let validation = retrieved_detail.unwrap();
        
        // Verify all violated rules are reported
        prop_assert_eq!(validation.violated_rules.len(), detail.violated_rules.len());
        for (i, rule) in validation.violated_rules.iter().enumerate() {
            prop_assert_eq!(rule, &detail.violated_rules[i]);
        }
        
        // Verify field information is preserved
        prop_assert_eq!(&validation.field, &detail.field);
        prop_assert_eq!(&validation.expected_constraint, &detail.expected_constraint);
        prop_assert_eq!(&validation.actual_value, &detail.actual_value);
        
        // Verify context is preserved
        prop_assert_eq!(
            &validation.context.transaction_id, 
            &detail.context.transaction_id
        );
        prop_assert_eq!(
            validation.context.transaction_index, 
            detail.context.transaction_index
        );
        prop_assert_eq!(
            &validation.context.rule_version, 
            &detail.context.rule_version
        );
    }
}

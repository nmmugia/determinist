use dtre::{ExecutionContext, DeterministicTime, SeededRandom, ExternalFacts};
use chrono::{DateTime, Utc, TimeZone};
use proptest::prelude::*;

// Helper to create arbitrary DateTime
fn arbitrary_datetime() -> impl Strategy<Value = DateTime<Utc>> {
    (0i64..2_000_000_000).prop_map(|secs| {
        Utc.timestamp_opt(secs, 0).unwrap()
    })
}

// Helper to create arbitrary seed
fn arbitrary_seed() -> impl Strategy<Value = u64> {
    any::<u64>()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 7: Deterministic Time Context**
    /// **Validates: Requirements 3.1**
    /// 
    /// For any transaction sequence with identical time contexts, the results should be 
    /// identical regardless of when the replay occurs.
    #[test]
    fn property_deterministic_time_context(
        time in arbitrary_datetime(),
        operations in prop::collection::vec(0u32..100, 1..20)
    ) {
        // Create two execution contexts with the same time
        let ctx1 = ExecutionContext::new(time, 12345);
        let ctx2 = ExecutionContext::new(time, 12345);
        
        // Both contexts should return the same time
        prop_assert_eq!(ctx1.now(), ctx2.now());
        prop_assert_eq!(ctx1.now(), time);
        prop_assert_eq!(ctx2.now(), time);
        
        // Time should remain frozen across multiple accesses
        for _ in &operations {
            prop_assert_eq!(ctx1.now(), time);
        }
        
        // Creating a new context with updated time should work correctly
        let new_time = Utc.timestamp_opt(time.timestamp() + 3600, 0).unwrap();
        let ctx3 = ctx1.with_time(new_time);
        prop_assert_eq!(ctx3.now(), new_time);
        
        // Original context should be unchanged
        prop_assert_eq!(ctx1.now(), time);
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 8: Seeded Randomness Reproducibility**
    /// **Validates: Requirements 3.2**
    /// 
    /// For any random operations with the same seed, the sequence of random values and 
    /// final results should be identical across all executions.
    #[test]
    fn property_seeded_randomness_reproducibility(
        seed in arbitrary_seed(),
        num_operations in 1usize..50
    ) {
        // Create two execution contexts with the same seed
        let mut ctx1 = ExecutionContext::new(Utc::now(), seed);
        let mut ctx2 = ExecutionContext::new(Utc::now(), seed);
        
        // Generate random numbers from both contexts
        let mut values1 = Vec::new();
        let mut values2 = Vec::new();
        
        for _ in 0..num_operations {
            values1.push(ctx1.random().next_u64());
        }
        
        for _ in 0..num_operations {
            values2.push(ctx2.random().next_u64());
        }
        
        // Both sequences should be identical
        prop_assert_eq!(&values1, &values2);
        
        // Cloning the context should preserve the seed and restart the sequence
        let mut ctx3 = ctx1.clone();
        let mut values3 = Vec::new();
        for _ in 0..num_operations {
            values3.push(ctx3.random().next_u64());
        }
        
        // The cloned context should produce the same sequence as the original
        prop_assert_eq!(&values1, &values3);
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 9: External Facts Consistency**
    /// **Validates: Requirements 3.3**
    /// 
    /// For any transaction sequence with identical external facts, the results should be 
    /// identical regardless of execution environment.
    #[test]
    fn property_external_facts_consistency(
        _seed in arbitrary_seed(),
        fact_values in prop::collection::vec(any::<i64>(), 1..10),
        fact_strings in prop::collection::vec("[a-z]{3,10}", 1..10)
    ) {
        // Create execution contexts with external facts
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let mut ctx1 = ExecutionContext::builder()
            .with_time(time)
            .with_random_seed(42);
        let mut ctx2 = ExecutionContext::builder()
            .with_time(time)
            .with_random_seed(42);
        
        // Add the same external facts to both contexts
        for (i, value) in fact_values.iter().enumerate() {
            let key = format!("fact_int_{}", i);
            ctx1 = ctx1.with_external_fact(key.clone(), *value);
            ctx2 = ctx2.with_external_fact(key, *value);
        }
        
        for (i, value) in fact_strings.iter().enumerate() {
            let key = format!("fact_str_{}", i);
            ctx1 = ctx1.with_external_fact(key.clone(), value.clone());
            ctx2 = ctx2.with_external_fact(key, value.clone());
        }
        
        let ctx1 = ctx1.build();
        let ctx2 = ctx2.build();
        
        // Both contexts should have the same number of facts
        prop_assert_eq!(ctx1.external_facts().len(), ctx2.external_facts().len());
        prop_assert_eq!(ctx1.external_facts().len(), fact_values.len() + fact_strings.len());
        
        // Retrieving facts should return the same values
        for (i, expected_value) in fact_values.iter().enumerate() {
            let key = format!("fact_int_{}", i);
            let value1: Option<&i64> = ctx1.get_external_fact(&key);
            let value2: Option<&i64> = ctx2.get_external_fact(&key);
            
            prop_assert_eq!(value1, Some(expected_value));
            prop_assert_eq!(value2, Some(expected_value));
            prop_assert_eq!(value1, value2);
        }
        
        for (i, expected_value) in fact_strings.iter().enumerate() {
            let key = format!("fact_str_{}", i);
            let value1: Option<&String> = ctx1.get_external_fact(&key);
            let value2: Option<&String> = ctx2.get_external_fact(&key);
            
            prop_assert_eq!(value1, Some(expected_value));
            prop_assert_eq!(value2, Some(expected_value));
            prop_assert_eq!(value1, value2);
        }
        
        // Cloning context should preserve all facts
        let ctx3 = ctx1.clone();
        prop_assert_eq!(ctx3.external_facts().len(), ctx1.external_facts().len());
        
        for (i, expected_value) in fact_values.iter().enumerate() {
            let key = format!("fact_int_{}", i);
            let value3: Option<&i64> = ctx3.get_external_fact(&key);
            prop_assert_eq!(value3, Some(expected_value));
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_deterministic_time_basic() {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let dt = DeterministicTime::new(time);
        
        assert_eq!(dt.current(), time);
        
        // Time should remain frozen
        assert_eq!(dt.current(), time);
        assert_eq!(dt.current(), time);
    }
    
    #[test]
    fn test_seeded_random_basic() {
        let mut rng1 = SeededRandom::new(42);
        let mut rng2 = SeededRandom::new(42);
        
        // Same seed should produce same sequence
        assert_eq!(rng1.next_u64(), rng2.next_u64());
        assert_eq!(rng1.next_u64(), rng2.next_u64());
        assert_eq!(rng1.next_u64(), rng2.next_u64());
    }
    
    #[test]
    fn test_external_facts_basic() {
        let mut facts = ExternalFacts::new();
        
        facts.insert("key1".to_string(), 42i64);
        facts.insert("key2".to_string(), "value".to_string());
        
        assert_eq!(facts.get::<i64>("key1"), Some(&42));
        assert_eq!(facts.get::<String>("key2"), Some(&"value".to_string()));
        assert_eq!(facts.get::<i64>("nonexistent"), None);
        
        assert!(facts.contains_key("key1"));
        assert!(!facts.contains_key("nonexistent"));
        assert_eq!(facts.len(), 2);
    }
    
    #[test]
    fn test_execution_context_builder() {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let ctx = ExecutionContext::builder()
            .with_time(time)
            .with_random_seed(42)
            .with_external_fact("test".to_string(), 123i64)
            .build();
        
        assert_eq!(ctx.now(), time);
        assert_eq!(ctx.get_external_fact::<i64>("test"), Some(&123));
    }
}

// Property tests for non-determinism detection
use dtre::{NonDeterminismGuard, Operation};

// Helper to create arbitrary operations
fn arbitrary_operation() -> impl Strategy<Value = Operation> {
    prop_oneof![
        Just(Operation::SystemTime),
        Just(Operation::RandomWithoutSeed),
        Just(Operation::NetworkAccess),
        Just(Operation::FileSystemRead),
        Just(Operation::FileSystemWrite),
        Just(Operation::EnvironmentVariable),
        Just(Operation::ThreadSpawn),
        Just(Operation::ProcessSpawn),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 3: Non-Deterministic Operation Rejection**
    /// **Validates: Requirements 1.5**
    /// 
    /// For any operation that could introduce non-deterministic behavior, the DTRE should 
    /// reject it with an appropriate error.
    #[test]
    fn property_non_deterministic_operation_rejection(
        op in arbitrary_operation()
    ) {
        let guard = NonDeterminismGuard::new();
        
        // All non-deterministic operations should be rejected in strict mode
        let result = guard.check_operation(&op);
        prop_assert!(result.is_err(), "Operation {:?} should be rejected", op);
        
        // The error should contain information about the operation
        if let Err(e) = result {
            let error_msg = format!("{}", e);
            prop_assert!(
                error_msg.contains("Non-deterministic operation"),
                "Error message should indicate non-deterministic operation: {}",
                error_msg
            );
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 29: Non-Determinism Detection**
    /// **Validates: Requirements 8.2**
    /// 
    /// For any non-deterministic behavior, the system should accurately detect and report 
    /// the specific source.
    #[test]
    fn property_non_determinism_detection(
        op in arbitrary_operation()
    ) {
        let guard = NonDeterminismGuard::new();
        
        // Check that the guard detects the operation
        let result = guard.check_operation(&op);
        prop_assert!(result.is_err(), "Non-deterministic operation {:?} should be detected", op);
        
        // Verify the error contains specific information about the source
        if let Err(e) = result {
            let error_msg = format!("{}", e);
            
            // The error should identify the specific operation type
            match op {
                Operation::SystemTime => {
                    prop_assert!(error_msg.contains("system_time"), 
                        "Error should identify system_time: {}", error_msg);
                }
                Operation::RandomWithoutSeed => {
                    prop_assert!(error_msg.contains("unseeded_random"), 
                        "Error should identify unseeded_random: {}", error_msg);
                }
                Operation::NetworkAccess => {
                    prop_assert!(error_msg.contains("network_access"), 
                        "Error should identify network_access: {}", error_msg);
                }
                Operation::FileSystemRead => {
                    prop_assert!(error_msg.contains("file_system_read"), 
                        "Error should identify file_system_read: {}", error_msg);
                }
                Operation::FileSystemWrite => {
                    prop_assert!(error_msg.contains("file_system_write"), 
                        "Error should identify file_system_write: {}", error_msg);
                }
                Operation::EnvironmentVariable => {
                    prop_assert!(error_msg.contains("environment_variable"), 
                        "Error should identify environment_variable: {}", error_msg);
                }
                Operation::ThreadSpawn => {
                    prop_assert!(error_msg.contains("thread_spawn"), 
                        "Error should identify thread_spawn: {}", error_msg);
                }
                Operation::ProcessSpawn => {
                    prop_assert!(error_msg.contains("process_spawn"), 
                        "Error should identify process_spawn: {}", error_msg);
                }
            }
        }
    }
}

#[cfg(test)]
mod non_determinism_tests {
    use super::*;
    
    #[test]
    fn test_guard_strict_mode() {
        let guard = NonDeterminismGuard::new();
        
        // All operations should be rejected in strict mode
        assert!(guard.check_operation(&Operation::SystemTime).is_err());
        assert!(guard.check_operation(&Operation::NetworkAccess).is_err());
        assert!(guard.check_operation(&Operation::RandomWithoutSeed).is_err());
    }
    
    #[test]
    fn test_guard_non_strict_mode() {
        let guard = NonDeterminismGuard::with_strict_mode(false);
        
        // All operations should be allowed in non-strict mode
        assert!(guard.check_operation(&Operation::SystemTime).is_ok());
        assert!(guard.check_operation(&Operation::NetworkAccess).is_ok());
        assert!(guard.check_operation(&Operation::RandomWithoutSeed).is_ok());
    }
    
    #[test]
    fn test_guard_validate() {
        let guard = NonDeterminismGuard::new();
        
        // Validate should reject non-deterministic operations
        let result = guard.validate(&Operation::SystemTime, || 42);
        assert!(result.is_err());
        
        // Non-strict mode should allow operations
        let guard_permissive = NonDeterminismGuard::with_strict_mode(false);
        let result = guard_permissive.validate(&Operation::SystemTime, || 42);
        assert_eq!(result.unwrap(), 42);
    }
}

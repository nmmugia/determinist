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

// Property tests for external entity resolution and ordering

use dtre::{ExternalEntityResolver, OrderingRules};

// Test entity types
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestEntity {
    id: String,
    value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestAccount {
    account_id: String,
    balance: i64,
}

// Helper to create arbitrary test entities
fn arbitrary_test_entity() -> impl Strategy<Value = TestEntity> {
    ("[a-z]{3,10}", any::<i64>()).prop_map(|(id, value)| TestEntity { id, value })
}

// Helper to create arbitrary test accounts
fn arbitrary_test_account() -> impl Strategy<Value = TestAccount> {
    ("[A-Z]{3}[0-9]{3}", 0i64..1_000_000).prop_map(|(account_id, balance)| {
        TestAccount { account_id, balance }
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 20: External Entity Resolution**
    /// **Validates: Requirements 6.3**
    /// 
    /// For any events referencing external entities, resolution should be deterministic 
    /// through the provided context.
    #[test]
    fn property_external_entity_resolution(
        entities in prop::collection::vec(arbitrary_test_entity(), 1..20)
    ) {
        // Create two entity resolvers with the same entities
        let mut resolver1 = ExternalEntityResolver::new();
        let mut resolver2 = ExternalEntityResolver::new();
        
        // Register all entities in both resolvers
        for entity in &entities {
            resolver1.register(entity.id.clone(), entity.clone());
            resolver2.register(entity.id.clone(), entity.clone());
        }
        
        // Both resolvers should have the same number of entities
        prop_assert_eq!(resolver1.len(), resolver2.len());
        prop_assert_eq!(resolver1.len(), entities.len());
        
        // Resolving entities should return the same values from both resolvers
        for entity in &entities {
            let resolved1: Result<&TestEntity, _> = resolver1.resolve(&entity.id);
            let resolved2: Result<&TestEntity, _> = resolver2.resolve(&entity.id);
            
            prop_assert!(resolved1.is_ok(), "Entity {} should be resolvable", entity.id);
            prop_assert!(resolved2.is_ok(), "Entity {} should be resolvable", entity.id);
            
            let e1 = resolved1.unwrap();
            let e2 = resolved2.unwrap();
            
            prop_assert_eq!(e1, e2);
            prop_assert_eq!(e1, entity);
        }
        
        // Attempting to resolve non-existent entities should fail consistently
        let non_existent_id = "nonexistent_entity_xyz";
        let result1: Result<&TestEntity, _> = resolver1.resolve(non_existent_id);
        let result2: Result<&TestEntity, _> = resolver2.resolve(non_existent_id);
        
        prop_assert!(result1.is_err(), "Non-existent entity should not be resolvable");
        prop_assert!(result2.is_err(), "Non-existent entity should not be resolvable");
        
        // Type mismatches should be detected
        if !entities.is_empty() {
            let first_id = &entities[0].id;
            let wrong_type_result: Result<&TestAccount, _> = resolver1.resolve(first_id);
            prop_assert!(wrong_type_result.is_err(), "Type mismatch should be detected");
        }
        
        // Test with execution context
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let mut ctx_builder = ExecutionContext::builder()
            .with_time(time)
            .with_random_seed(42);
        
        for entity in &entities {
            ctx_builder = ctx_builder.with_external_entity(entity.id.clone(), entity.clone());
        }
        
        let ctx = ctx_builder.build();
        
        // Resolving through context should work the same way
        for entity in &entities {
            let resolved: Result<&TestEntity, _> = ctx.resolve_entity(&entity.id);
            prop_assert!(resolved.is_ok(), "Entity {} should be resolvable through context", entity.id);
            prop_assert_eq!(resolved.unwrap(), entity);
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 21: Event Ordering Enforcement**
    /// **Validates: Requirements 6.4**
    /// 
    /// For any ambiguous event ordering, the system should detect and enforce explicit 
    /// ordering requirements.
    #[test]
    fn property_event_ordering_enforcement(
        accounts in prop::collection::vec(arbitrary_test_account(), 2..15)
    ) {
        // Create ordering rules with explicit ordering
        let mut ordering_rules = OrderingRules::new();
        
        // Define the expected order based on account IDs
        let mut expected_order: Vec<String> = accounts.iter()
            .map(|a| a.account_id.clone())
            .collect();
        expected_order.sort(); // Lexicographic ordering for determinism
        
        ordering_rules.add_ordering("accounts".to_string(), expected_order.clone());
        
        // Create a correctly ordered collection
        let mut ordered_accounts = accounts.clone();
        ordered_accounts.sort_by_key(|a| a.account_id.clone());
        
        // Validation should pass for correctly ordered collections
        let result = ordering_rules.validate_ordering(
            "accounts",
            &ordered_accounts,
            |a| &a.account_id
        );
        prop_assert!(result.is_ok(), "Correctly ordered collection should pass validation");
        
        // Create an incorrectly ordered collection (if possible)
        if accounts.len() >= 2 {
            let mut unordered_accounts = ordered_accounts.clone();
            // Swap first two elements to create disorder
            unordered_accounts.swap(0, 1);
            
            // Check if the swap actually created a different order
            let first_id = &unordered_accounts[0].account_id;
            let second_id = &unordered_accounts[1].account_id;
            
            if first_id != second_id {
                // Validation should fail for incorrectly ordered collections
                let result = ordering_rules.validate_ordering(
                    "accounts",
                    &unordered_accounts,
                    |a| &a.account_id
                );
                prop_assert!(result.is_err(), "Incorrectly ordered collection should fail validation");
                
                // The error should contain information about the ordering violation
                if let Err(e) = result {
                    let error_msg = format!("{}", e);
                    prop_assert!(
                        error_msg.contains("Ordering violation"),
                        "Error should indicate ordering violation: {}",
                        error_msg
                    );
                }
            }
        }
        
        // Test sorting functionality
        let mut unsorted_accounts = accounts.clone();
        ordering_rules.sort_by_ordering("accounts", &mut unsorted_accounts, |a| &a.account_id);
        
        // After sorting, the collection should match the expected order
        let sorted_ids: Vec<String> = unsorted_accounts.iter()
            .map(|a| a.account_id.clone())
            .collect();
        prop_assert_eq!(&sorted_ids, &expected_order, "Sorted collection should match expected order");
        
        // Test with execution context
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        let ctx = ExecutionContext::builder()
            .with_time(time)
            .with_random_seed(42)
            .with_ordering_rules(ordering_rules.clone())
            .build();
        
        // Validation through context should work the same way
        let result = ctx.validate_ordering("accounts", &ordered_accounts, |a| &a.account_id);
        prop_assert!(result.is_ok(), "Context validation should pass for correctly ordered collection");
        
        // Test default lexicographic ordering when no custom ordering is defined
        let default_rules = OrderingRules::new();
        let mut test_accounts = accounts.clone();
        default_rules.sort_by_ordering("unspecified_type", &mut test_accounts, |a| &a.account_id);
        
        // Should be sorted lexicographically
        let sorted_ids: Vec<String> = test_accounts.iter()
            .map(|a| a.account_id.clone())
            .collect();
        let mut expected_lex_order = sorted_ids.clone();
        expected_lex_order.sort();
        prop_assert_eq!(&sorted_ids, &expected_lex_order, "Default ordering should be lexicographic");
    }
}

#[cfg(test)]
mod external_entity_tests {
    use super::*;
    
    #[test]
    fn test_entity_resolver_basic() {
        let mut resolver = ExternalEntityResolver::new();
        
        let entity = TestEntity {
            id: "test1".to_string(),
            value: 42,
        };
        
        resolver.register(entity.id.clone(), entity.clone());
        
        let resolved: Result<&TestEntity, _> = resolver.resolve("test1");
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), &entity);
        
        // Non-existent entity should fail
        let result: Result<&TestEntity, _> = resolver.resolve("nonexistent");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_entity_resolver_type_mismatch() {
        let mut resolver = ExternalEntityResolver::new();
        
        let entity = TestEntity {
            id: "test1".to_string(),
            value: 42,
        };
        
        resolver.register(entity.id.clone(), entity.clone());
        
        // Attempting to resolve with wrong type should fail
        let result: Result<&TestAccount, _> = resolver.resolve("test1");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_ordering_rules_basic() {
        let mut rules = OrderingRules::new();
        
        let expected_order = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        rules.add_ordering("test_type".to_string(), expected_order.clone());
        
        assert_eq!(rules.get_ordering("test_type"), Some(&expected_order));
        assert_eq!(rules.get_ordering("nonexistent"), None);
    }
    
    #[test]
    fn test_ordering_validation() {
        let mut rules = OrderingRules::new();
        
        let expected_order = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        rules.add_ordering("items".to_string(), expected_order);
        
        let items = vec!["a", "b", "c"];
        let result = rules.validate_ordering("items", &items, |s| s);
        assert!(result.is_ok());
        
        let wrong_order = vec!["c", "b", "a"];
        let result = rules.validate_ordering("items", &wrong_order, |s| s);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_ordering_sort() {
        let mut rules = OrderingRules::new();
        
        let expected_order = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        rules.add_ordering("items".to_string(), expected_order.clone());
        
        let mut items = vec!["a", "b", "c"];
        rules.sort_by_ordering("items", &mut items, |s| s);
        
        assert_eq!(items, vec!["c", "a", "b"]);
    }
    
    #[test]
    fn test_ordering_default_lexicographic() {
        let rules = OrderingRules::new();
        
        let mut items = vec!["zebra", "apple", "banana"];
        rules.sort_by_ordering("unspecified", &mut items, |s| s);
        
        assert_eq!(items, vec!["apple", "banana", "zebra"]);
    }
    
    #[test]
    fn test_execution_context_with_entities() {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        
        let entity = TestEntity {
            id: "test1".to_string(),
            value: 42,
        };
        
        let ctx = ExecutionContext::builder()
            .with_time(time)
            .with_random_seed(42)
            .with_external_entity(entity.id.clone(), entity.clone())
            .build();
        
        let resolved: Result<&TestEntity, _> = ctx.resolve_entity("test1");
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), &entity);
    }
    
    #[test]
    fn test_execution_context_with_ordering() {
        let time = Utc.timestamp_opt(1000000, 0).unwrap();
        
        let mut ordering_rules = OrderingRules::new();
        ordering_rules.add_ordering(
            "accounts".to_string(),
            vec!["ACC001".to_string(), "ACC002".to_string(), "ACC003".to_string()]
        );
        
        let ctx = ExecutionContext::builder()
            .with_time(time)
            .with_random_seed(42)
            .with_ordering_rules(ordering_rules)
            .build();
        
        let accounts = vec!["ACC001", "ACC002", "ACC003"];
        let result = ctx.validate_ordering("accounts", &accounts, |s| s);
        assert!(result.is_ok());
        
        let wrong_order = vec!["ACC003", "ACC001", "ACC002"];
        let result = ctx.validate_ordering("accounts", &wrong_order, |s| s);
        assert!(result.is_err());
    }
}

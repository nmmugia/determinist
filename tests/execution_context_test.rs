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
        seed in arbitrary_seed(),
        fact_values in prop::collection::vec(any::<i64>(), 1..20),
        fact_strings in prop::collection::vec("[a-z]{3,10}", 1..20)
    ) {
        // Create external facts containers
        let mut facts1 = ExternalFacts::new();
        let mut facts2 = ExternalFacts::new();
        
        // Add the same external facts to both contexts
        for (i, value) in fact_values.iter().enumerate() {
            let key = format!("fact_int_{}", i);
            facts1.insert(key.clone(), *value);
            facts2.insert(key, *value);
        }
        
        for (i, value) in fact_strings.iter().enumerate() {
            let key = format!("fact_str_{}", i);
            facts1.insert(key.clone(), value.clone());
            facts2.insert(key, value.clone());
        }
        
        // Both fact containers should have the same number of facts
        prop_assert_eq!(facts1.len(), facts2.len());
        prop_assert_eq!(facts1.len(), fact_values.len() + fact_strings.len());
        
        // Retrieving facts should return the same values
        for (i, expected_value) in fact_values.iter().enumerate() {
            let key = format!("fact_int_{}", i);
            let value1: Option<&i64> = facts1.get(&key);
            let value2: Option<&i64> = facts2.get(&key);
            
            prop_assert_eq!(value1, Some(expected_value));
            prop_assert_eq!(value2, Some(expected_value));
            prop_assert_eq!(value1, value2);
        }
        
        for (i, expected_value) in fact_strings.iter().enumerate() {
            let key = format!("fact_str_{}", i);
            let value1: Option<&String> = facts1.get(&key);
            let value2: Option<&String> = facts2.get(&key);
            
            prop_assert_eq!(value1, Some(expected_value));
            prop_assert_eq!(value2, Some(expected_value));
            prop_assert_eq!(value1, value2);
        }
        
        // Cloning should preserve all facts
        let facts3 = facts1.clone();
        prop_assert_eq!(facts3.len(), facts1.len());
        
        for (i, expected_value) in fact_values.iter().enumerate() {
            let key = format!("fact_int_{}", i);
            let value3: Option<&i64> = facts3.get(&key);
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

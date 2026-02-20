use dtre::{
    RuleSetRegistry, VersionedRuleSet, RuleSetMetadata, Version,
    State, Transaction, RuleSet, ExecutionContext,
    ProcessingError, ValidationError,
};
use proptest::prelude::*;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, TimeZone};

// Mock state for testing
#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
struct TestState {
    value: i32,
}

impl State for TestState {
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

// Mock transaction for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestTransaction {
    id: String,
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

// Mock rule set for testing
struct TestRuleSet {
    version: Version,
    increment_by: i32,
}

impl RuleSet<TestState, TestTransaction> for TestRuleSet {
    fn version(&self) -> Version {
        self.version.clone()
    }
    
    fn apply(
        &self,
        state: &TestState,
        _transaction: &TestTransaction,
        _context: &ExecutionContext,
    ) -> Result<TestState, ProcessingError> {
        Ok(TestState { value: state.value + self.increment_by })
    }
}

// Proptest strategies
fn arbitrary_version() -> impl Strategy<Value = Version> {
    (0u32..10, 0u32..10, 0u32..10).prop_map(|(major, minor, patch)| {
        Version::new(major, minor, patch)
    })
}

fn arbitrary_metadata() -> impl Strategy<Value = RuleSetMetadata> {
    ("[a-z]{3,10}", "[a-z ]{10,30}").prop_map(|(name, desc)| {
        RuleSetMetadata::new(name, desc)
    })
}

// Helper function to create a versioned rule set
fn create_versioned_rule_set(
    version: Version,
    metadata: RuleSetMetadata,
    increment_by: i32,
) -> VersionedRuleSet<TestState, TestTransaction> {
    let rules = Box::new(TestRuleSet {
        version: version.clone(),
        increment_by,
    });
    VersionedRuleSet::new(version, rules, metadata)
}

#[test]
fn test_simple() {
    assert_eq!(1 + 1, 2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 23: Rule Set Version Identification**
    /// **Validates: Requirements 7.1**
    #[test]
    fn property_rule_set_version_identification(
        version in arbitrary_version(),
        metadata in arbitrary_metadata(),
        increment in 1i32..10,
    ) {
        // Create a versioned rule set
        let versioned = create_versioned_rule_set(version.clone(), metadata, increment);
        
        // The version should be correctly identified
        prop_assert_eq!(versioned.version(), &version);
        
        // Register it in a registry
        let mut registry = RuleSetRegistry::new();
        registry.register(versioned).unwrap();
        
        // Should be able to retrieve by version
        prop_assert!(registry.contains(&version));
        let retrieved = registry.get(&version);
        prop_assert!(retrieved.is_some());
        prop_assert_eq!(retrieved.unwrap().version(), &version);
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 24: Rule Version Coexistence**
    /// **Validates: Requirements 7.2**
    #[test]
    fn property_rule_version_coexistence(
        versions in prop::collection::vec(arbitrary_version(), 2..5),
        metadata in arbitrary_metadata(),
    ) {
        // Filter to ensure unique versions
        let mut unique_versions: Vec<Version> = versions.into_iter().collect();
        unique_versions.sort_by(|a, b| {
            a.major.cmp(&b.major)
                .then(a.minor.cmp(&b.minor))
                .then(a.patch.cmp(&b.patch))
        });
        unique_versions.dedup();
        
        if unique_versions.len() < 2 {
            return Ok(());
        }
        
        let mut registry = RuleSetRegistry::new();
        
        // Register multiple versions
        for (i, version) in unique_versions.iter().enumerate() {
            let meta = RuleSetMetadata::new(
                format!("test_{}", i),
                metadata.description.clone(),
            );
            let versioned = create_versioned_rule_set(version.clone(), meta, i as i32 + 1);
            registry.register(versioned).unwrap();
        }
        
        // All versions should coexist
        for version in &unique_versions {
            prop_assert!(registry.contains(version));
        }
        
        // Each version should be retrievable and distinct
        let state = TestState { value: 0 };
        let tx = TestTransaction {
            id: "test".to_string(),
            timestamp: Utc.timestamp_opt(0, 0).unwrap(),
        };
        let context = ExecutionContext::new(
            Utc.timestamp_opt(0, 0).unwrap(),
            0,
        );
        
        let mut results = Vec::new();
        for version in &unique_versions {
            let rule_set = registry.get(version).unwrap();
            let result = rule_set.rules().apply(&state, &tx, &context).unwrap();
            results.push(result.value);
        }
        
        // Different versions should produce different results (since we use different increments)
        if unique_versions.len() >= 2 {
            prop_assert_ne!(results[0], results[1]);
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 26: Rule Conflict Detection**
    /// **Validates: Requirements 7.4**
    #[test]
    fn property_rule_conflict_detection(
        version in arbitrary_version(),
        metadata1 in arbitrary_metadata(),
        metadata2 in arbitrary_metadata(),
    ) {
        let mut registry = RuleSetRegistry::new();
        
        // Register first version
        let versioned1 = create_versioned_rule_set(version.clone(), metadata1, 1);
        let result1 = registry.register(versioned1);
        prop_assert!(result1.is_ok());
        
        // Attempt to register the same version again should fail
        let versioned2 = create_versioned_rule_set(version.clone(), metadata2, 2);
        let result2 = registry.register(versioned2);
        prop_assert!(result2.is_err());
        
        // The error should indicate a conflict
        match result2 {
            Err(e) => {
                let error_msg = format!("{}", e);
                prop_assert!(error_msg.contains("already exists") || error_msg.contains("conflict"));
            }
            Ok(_) => prop_assert!(false, "Expected error for duplicate version"),
        }
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 27: Rule Set Immutability**
    /// **Validates: Requirements 7.5**
    #[test]
    fn property_rule_set_immutability(
        version in arbitrary_version(),
        metadata in arbitrary_metadata(),
        increment in 1i32..10,
    ) {
        let mut registry = RuleSetRegistry::new();
        
        // Register a rule set
        let versioned = create_versioned_rule_set(version.clone(), metadata.clone(), increment);
        let original_name = metadata.name.clone();
        registry.register(versioned).unwrap();
        
        // Get the rule set and verify its metadata
        let retrieved = registry.get(&version).unwrap();
        prop_assert_eq!(&retrieved.metadata().name, &original_name);
        
        // Apply the rule set to verify behavior
        let state = TestState { value: 10 };
        let tx = TestTransaction {
            id: "test".to_string(),
            timestamp: Utc.timestamp_opt(0, 0).unwrap(),
        };
        let context = ExecutionContext::new(
            Utc.timestamp_opt(0, 0).unwrap(),
            0,
        );
        
        let result1 = retrieved.rules().apply(&state, &tx, &context).unwrap();
        let expected_value = 10 + increment;
        prop_assert_eq!(result1.value, expected_value);
        
        // Get the rule set again and verify it produces the same result
        let retrieved2 = registry.get(&version).unwrap();
        let result2 = retrieved2.rules().apply(&state, &tx, &context).unwrap();
        prop_assert_eq!(result2.value, expected_value);
        
        // The metadata should still be the same
        prop_assert_eq!(&retrieved2.metadata().name, &original_name);
        
        // Multiple retrievals should give consistent results
        for _ in 0..5 {
            let retrieved_n = registry.get(&version).unwrap();
            let result_n = retrieved_n.rules().apply(&state, &tx, &context).unwrap();
            prop_assert_eq!(result_n.value, expected_value);
            prop_assert_eq!(&retrieved_n.metadata().name, &original_name);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_version_identification_basic() {
        let version = Version::new(1, 0, 0);
        let metadata = RuleSetMetadata::new("Test".to_string(), "Test rule set".to_string());
        let versioned = create_versioned_rule_set(version.clone(), metadata, 1);
        
        assert_eq!(versioned.version(), &version);
    }
    
    #[test]
    fn test_multiple_versions_coexist() {
        let mut registry = RuleSetRegistry::new();
        
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(2, 0, 0);
        
        let meta1 = RuleSetMetadata::new("v1".to_string(), "Version 1".to_string());
        let meta2 = RuleSetMetadata::new("v2".to_string(), "Version 2".to_string());
        let meta3 = RuleSetMetadata::new("v3".to_string(), "Version 3".to_string());
        
        registry.register(create_versioned_rule_set(v1.clone(), meta1, 1)).unwrap();
        registry.register(create_versioned_rule_set(v2.clone(), meta2, 2)).unwrap();
        registry.register(create_versioned_rule_set(v3.clone(), meta3, 3)).unwrap();
        
        assert!(registry.contains(&v1));
        assert!(registry.contains(&v2));
        assert!(registry.contains(&v3));
    }
    
    #[test]
    fn test_duplicate_version_conflict() {
        let mut registry = RuleSetRegistry::new();
        
        let version = Version::new(1, 0, 0);
        let meta1 = RuleSetMetadata::new("First".to_string(), "First registration".to_string());
        let meta2 = RuleSetMetadata::new("Second".to_string(), "Second registration".to_string());
        
        let result1 = registry.register(create_versioned_rule_set(version.clone(), meta1, 1));
        assert!(result1.is_ok());
        
        let result2 = registry.register(create_versioned_rule_set(version.clone(), meta2, 2));
        assert!(result2.is_err());
    }
    
    #[test]
    fn test_rule_set_immutability() {
        let mut registry = RuleSetRegistry::new();
        
        let version = Version::new(1, 0, 0);
        let metadata = RuleSetMetadata::new("Immutable".to_string(), "Should not change".to_string());
        
        registry.register(create_versioned_rule_set(version.clone(), metadata, 5)).unwrap();
        
        let state = TestState { value: 10 };
        let tx = TestTransaction {
            id: "test".to_string(),
            timestamp: Utc.timestamp_opt(0, 0).unwrap(),
        };
        let context = ExecutionContext::new(Utc.timestamp_opt(0, 0).unwrap(), 0);
        
        // Apply multiple times
        for _ in 0..10 {
            let rule_set = registry.get(&version).unwrap();
            let result = rule_set.rules().apply(&state, &tx, &context).unwrap();
            assert_eq!(result.value, 15); // 10 + 5
        }
    }
}

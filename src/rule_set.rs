//! Rule set management and versioning

use std::collections::HashMap;
use crate::traits::{RuleSet, State, Transaction};
use crate::types::Version;
use crate::error::RuleError;
use serde::{Serialize, Deserialize};

/// Metadata about a rule set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSetMetadata {
    pub name: String,
    pub description: String,
    pub author: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl RuleSetMetadata {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            author: None,
            created_at: chrono::Utc::now(),
        }
    }
}

/// A versioned rule set with metadata
pub struct VersionedRuleSet<S, T>
where
    S: State,
    T: Transaction,
{
    version: Version,
    rules: Box<dyn RuleSet<S, T>>,
    metadata: RuleSetMetadata,
}

impl<S, T> VersionedRuleSet<S, T>
where
    S: State,
    T: Transaction,
{
    /// Create a new versioned rule set
    pub fn new(
        version: Version,
        rules: Box<dyn RuleSet<S, T>>,
        metadata: RuleSetMetadata,
    ) -> Self {
        Self {
            version,
            rules,
            metadata,
        }
    }
    
    /// Get the version of this rule set
    pub fn version(&self) -> &Version {
        &self.version
    }
    
    /// Get the metadata of this rule set
    pub fn metadata(&self) -> &RuleSetMetadata {
        &self.metadata
    }
    
    /// Get a reference to the underlying rule set
    pub fn rules(&self) -> &dyn RuleSet<S, T> {
        self.rules.as_ref()
    }
    
    /// Check if this rule set is compatible with another version
    pub fn is_compatible_with(&self, other_version: &Version) -> bool {
        self.version.is_compatible_with(other_version)
    }
}

/// Registry for managing multiple rule set versions
pub struct RuleSetRegistry<S, T>
where
    S: State,
    T: Transaction,
{
    rule_sets: HashMap<Version, VersionedRuleSet<S, T>>,
}

impl<S, T> RuleSetRegistry<S, T>
where
    S: State,
    T: Transaction,
{
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            rule_sets: HashMap::new(),
        }
    }
    
    /// Register a new rule set version
    pub fn register(&mut self, rule_set: VersionedRuleSet<S, T>) -> Result<(), RuleError> {
        let version = rule_set.version().clone();
        
        // Check if version already exists
        if self.rule_sets.contains_key(&version) {
            return Err(RuleError::RegistrationFailed {
                reason: format!("Version {} already exists", version),
            });
        }
        
        // Check for conflicts with existing versions
        self.check_conflicts(&version)?;
        
        self.rule_sets.insert(version, rule_set);
        Ok(())
    }
    
    /// Get a rule set by version
    pub fn get(&self, version: &Version) -> Option<&VersionedRuleSet<S, T>> {
        self.rule_sets.get(version)
    }
    
    /// Get a mutable reference to a rule set by version
    pub fn get_mut(&mut self, version: &Version) -> Option<&mut VersionedRuleSet<S, T>> {
        self.rule_sets.get_mut(version)
    }
    
    /// Check if a version exists in the registry
    pub fn contains(&self, version: &Version) -> bool {
        self.rule_sets.contains_key(version)
    }
    
    /// Get all registered versions
    pub fn versions(&self) -> Vec<&Version> {
        self.rule_sets.keys().collect()
    }
    
    /// Get all rule sets compatible with a given version
    pub fn get_compatible(&self, version: &Version) -> Vec<&VersionedRuleSet<S, T>> {
        self.rule_sets
            .values()
            .filter(|rs| rs.is_compatible_with(version))
            .collect()
    }
    
    /// Remove a rule set by version
    pub fn remove(&mut self, version: &Version) -> Option<VersionedRuleSet<S, T>> {
        self.rule_sets.remove(version)
    }
    
    /// Check for conflicts with existing rule sets
    fn check_conflicts(&self, new_version: &Version) -> Result<(), RuleError> {
        // Check if there are any incompatible versions with the same major version
        for existing_version in self.rule_sets.keys() {
            if existing_version.major == new_version.major {
                // Same major version - check for patch conflicts
                if existing_version.minor == new_version.minor 
                    && existing_version.patch == new_version.patch {
                    return Err(RuleError::VersionConflict {
                        reason: format!(
                            "Version {} conflicts with existing version {}",
                            new_version, existing_version
                        ),
                    });
                }
            }
        }
        Ok(())
    }
    
    /// Get the latest version in the registry
    pub fn latest_version(&self) -> Option<&Version> {
        self.rule_sets
            .keys()
            .max_by(|a, b| {
                a.major.cmp(&b.major)
                    .then(a.minor.cmp(&b.minor))
                    .then(a.patch.cmp(&b.patch))
            })
    }
    
    /// Get the latest rule set
    pub fn latest(&self) -> Option<&VersionedRuleSet<S, T>> {
        self.latest_version()
            .and_then(|v| self.get(v))
    }
}

impl<S, T> Default for RuleSetRegistry<S, T>
where
    S: State,
    T: Transaction,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ExecutionContext;
    use crate::error::ProcessingError;
    
    // Mock state for testing
    #[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }
    
    impl State for TestState {
        fn validate(&self) -> Result<(), crate::error::ValidationError> {
            Ok(())
        }
    }
    
    // Mock transaction for testing
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestTransaction {
        id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    }
    
    impl Transaction for TestTransaction {
        fn id(&self) -> &str {
            &self.id
        }
        
        fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
            self.timestamp
        }
        
        fn validate(&self) -> Result<(), crate::error::ValidationError> {
            Ok(())
        }
    }
    
    // Mock rule set for testing
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
            _transaction: &TestTransaction,
            _context: &ExecutionContext,
        ) -> Result<TestState, ProcessingError> {
            Ok(TestState { value: state.value + 1 })
        }
    }
    
    #[test]
    fn test_versioned_rule_set_creation() {
        let version = Version::new(1, 0, 0);
        let rules = Box::new(TestRuleSet { version: version.clone() });
        let metadata = RuleSetMetadata::new("Test".to_string(), "Test rule set".to_string());
        
        let versioned = VersionedRuleSet::new(version.clone(), rules, metadata);
        assert_eq!(versioned.version(), &version);
    }
    
    #[test]
    fn test_registry_register_and_get() {
        let mut registry = RuleSetRegistry::new();
        
        let version = Version::new(1, 0, 0);
        let rules = Box::new(TestRuleSet { version: version.clone() });
        let metadata = RuleSetMetadata::new("Test".to_string(), "Test rule set".to_string());
        let versioned = VersionedRuleSet::new(version.clone(), rules, metadata);
        
        assert!(registry.register(versioned).is_ok());
        assert!(registry.contains(&version));
        assert!(registry.get(&version).is_some());
    }
    
    #[test]
    fn test_registry_duplicate_version() {
        let mut registry = RuleSetRegistry::new();
        
        let version = Version::new(1, 0, 0);
        
        let rules1 = Box::new(TestRuleSet { version: version.clone() });
        let metadata1 = RuleSetMetadata::new("Test1".to_string(), "Test rule set 1".to_string());
        let versioned1 = VersionedRuleSet::new(version.clone(), rules1, metadata1);
        
        let rules2 = Box::new(TestRuleSet { version: version.clone() });
        let metadata2 = RuleSetMetadata::new("Test2".to_string(), "Test rule set 2".to_string());
        let versioned2 = VersionedRuleSet::new(version.clone(), rules2, metadata2);
        
        assert!(registry.register(versioned1).is_ok());
        assert!(registry.register(versioned2).is_err());
    }
}

//! Pluggable serialization support for state objects

use crate::error::SerializationError;
use crate::traits::State;

/// Trait for pluggable state serialization
pub trait StateSerializer: Send + Sync {
    /// Serialize a state object to bytes
    fn serialize<S: State>(&self, state: &S) -> Result<Vec<u8>, SerializationError>;
    
    /// Deserialize a state object from bytes
    fn deserialize<S: State>(&self, bytes: &[u8]) -> Result<S, SerializationError>;
    
    /// Get the name of this serialization method
    fn name(&self) -> &str;
    
    /// Get the version of this serialization method
    fn version(&self) -> &str;
}

/// Bincode serialization backend
#[derive(Debug, Clone)]
pub struct BincodeSerializer;

impl BincodeSerializer {
    /// Create a new bincode serializer
    pub fn new() -> Self {
        Self
    }
}

impl Default for BincodeSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl StateSerializer for BincodeSerializer {
    fn serialize<S: State>(&self, state: &S) -> Result<Vec<u8>, SerializationError> {
        bincode::serialize(state).map_err(|e| SerializationError::SerializationFailed {
            reason: format!("Bincode serialization failed: {}", e),
        })
    }
    
    fn deserialize<S: State>(&self, bytes: &[u8]) -> Result<S, SerializationError> {
        bincode::deserialize(bytes).map_err(|e| SerializationError::DeserializationFailed {
            reason: format!("Bincode deserialization failed: {}", e),
        })
    }
    
    fn name(&self) -> &str {
        "bincode"
    }
    
    fn version(&self) -> &str {
        "1.3"
    }
}

/// JSON serialization backend
#[derive(Debug, Clone)]
pub struct JsonSerializer {
    pretty: bool,
}

impl JsonSerializer {
    /// Create a new JSON serializer
    pub fn new() -> Self {
        Self { pretty: false }
    }
    
    /// Create a new JSON serializer with pretty printing
    pub fn new_pretty() -> Self {
        Self { pretty: true }
    }
}

impl Default for JsonSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl StateSerializer for JsonSerializer {
    fn serialize<S: State>(&self, state: &S) -> Result<Vec<u8>, SerializationError> {
        let result = if self.pretty {
            serde_json::to_vec_pretty(state)
        } else {
            serde_json::to_vec(state)
        };
        
        result.map_err(|e| SerializationError::SerializationFailed {
            reason: format!("JSON serialization failed: {}", e),
        })
    }
    
    fn deserialize<S: State>(&self, bytes: &[u8]) -> Result<S, SerializationError> {
        serde_json::from_slice(bytes).map_err(|e| SerializationError::DeserializationFailed {
            reason: format!("JSON deserialization failed: {}", e),
        })
    }
    
    fn name(&self) -> &str {
        "json"
    }
    
    fn version(&self) -> &str {
        "1.0"
    }
}

/// Serialization context that tracks which serializer was used
#[derive(Debug, Clone)]
pub struct SerializationContext {
    serializer_name: String,
    serializer_version: String,
}

impl SerializationContext {
    /// Create a new serialization context from a serializer
    pub fn from_serializer<S: StateSerializer>(serializer: &S) -> Self {
        Self {
            serializer_name: serializer.name().to_string(),
            serializer_version: serializer.version().to_string(),
        }
    }
    
    /// Get the serializer name
    pub fn serializer_name(&self) -> &str {
        &self.serializer_name
    }
    
    /// Get the serializer version
    pub fn serializer_version(&self) -> &str {
        &self.serializer_version
    }
    
    /// Check if this context matches a given serializer
    pub fn matches<S: StateSerializer>(&self, serializer: &S) -> bool {
        self.serializer_name == serializer.name() && self.serializer_version == serializer.version()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ValidationError;
    use serde::{Deserialize, Serialize};
    use std::hash::{Hash, Hasher};
    
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        balance: i64,
        name: String,
    }
    
    impl Hash for TestState {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.balance.hash(state);
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
            Ok(())
        }
    }
    
    #[test]
    fn test_bincode_serializer() {
        let serializer = BincodeSerializer::new();
        let state = TestState {
            balance: 100,
            name: "Alice".to_string(),
        };
        
        let bytes = serializer.serialize(&state).unwrap();
        let deserialized: TestState = serializer.deserialize(&bytes).unwrap();
        
        assert_eq!(state, deserialized);
    }
    
    #[test]
    fn test_json_serializer() {
        let serializer = JsonSerializer::new();
        let state = TestState {
            balance: 100,
            name: "Alice".to_string(),
        };
        
        let bytes = serializer.serialize(&state).unwrap();
        let deserialized: TestState = serializer.deserialize(&bytes).unwrap();
        
        assert_eq!(state, deserialized);
    }
    
    #[test]
    fn test_json_pretty_serializer() {
        let serializer = JsonSerializer::new_pretty();
        let state = TestState {
            balance: 100,
            name: "Alice".to_string(),
        };
        
        let bytes = serializer.serialize(&state).unwrap();
        let json_str = String::from_utf8(bytes).unwrap();
        
        // Pretty printed JSON should contain newlines
        assert!(json_str.contains('\n'));
        
        let deserialized: TestState = serializer.deserialize(json_str.as_bytes()).unwrap();
        assert_eq!(state, deserialized);
    }
    
    #[test]
    fn test_serialization_context() {
        let bincode = BincodeSerializer::new();
        let json = JsonSerializer::new();
        
        let bincode_ctx = SerializationContext::from_serializer(&bincode);
        let json_ctx = SerializationContext::from_serializer(&json);
        
        assert_eq!(bincode_ctx.serializer_name(), "bincode");
        assert_eq!(json_ctx.serializer_name(), "json");
        
        assert!(bincode_ctx.matches(&bincode));
        assert!(!bincode_ctx.matches(&json));
        assert!(json_ctx.matches(&json));
        assert!(!json_ctx.matches(&bincode));
    }
    
    #[test]
    fn test_serializer_names_and_versions() {
        let bincode = BincodeSerializer::new();
        let json = JsonSerializer::new();
        
        assert_eq!(bincode.name(), "bincode");
        assert_eq!(bincode.version(), "1.3");
        
        assert_eq!(json.name(), "json");
        assert_eq!(json.version(), "1.0");
    }
}

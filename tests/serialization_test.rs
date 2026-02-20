use dtre::{
    BincodeSerializer, JsonSerializer, SerializationContext, State, StateSerializer,
    ValidationError,
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

// Test state for property testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    balance: i64,
    name: String,
    active: bool,
    count: u32,
}

impl Hash for TestState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.balance.hash(state);
        self.name.hash(state);
        self.active.hash(state);
        self.count.hash(state);
    }
}

impl State for TestState {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.balance < -1000000 {
            return Err(ValidationError::InvalidState {
                reason: "Balance too low".to_string(),
            });
        }
        Ok(())
    }
}

// Generator for arbitrary test states
fn arbitrary_test_state() -> impl Strategy<Value = TestState> {
    (
        -1000000i64..1000000i64,
        "[a-zA-Z]{1,20}",
        any::<bool>(),
        0u32..1000000u32,
    )
        .prop_map(|(balance, name, active, count)| TestState {
            balance,
            name,
            active,
            count,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: deterministic-transaction-replay-engine, Property 35: Pluggable Serialization Consistency**
    /// **Validates: Requirements 10.2**
    ///
    /// For any state and any serialization method (bincode or JSON), serializing then
    /// deserializing should produce an identical state (round-trip property).
    #[test]
    fn property_bincode_serialization_round_trip(state in arbitrary_test_state()) {
        let serializer = BincodeSerializer::new();
        
        // Serialize the state
        let bytes = serializer.serialize(&state).unwrap();
        
        // Deserialize back
        let deserialized: TestState = serializer.deserialize(&bytes).unwrap();
        
        // Should be identical
        prop_assert_eq!(state, deserialized);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 35: Pluggable Serialization Consistency**
    /// **Validates: Requirements 10.2**
    ///
    /// For any state and JSON serialization, serializing then deserializing should
    /// produce an identical state (round-trip property).
    #[test]
    fn property_json_serialization_round_trip(state in arbitrary_test_state()) {
        let serializer = JsonSerializer::new();
        
        // Serialize the state
        let bytes = serializer.serialize(&state).unwrap();
        
        // Deserialize back
        let deserialized: TestState = serializer.deserialize(&bytes).unwrap();
        
        // Should be identical
        prop_assert_eq!(state, deserialized);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 35: Pluggable Serialization Consistency**
    /// **Validates: Requirements 10.2**
    ///
    /// For any state, serializing with bincode and JSON should both produce valid
    /// serializations that can be deserialized back to the same state.
    #[test]
    fn property_serialization_method_consistency(state in arbitrary_test_state()) {
        let bincode = BincodeSerializer::new();
        let json = JsonSerializer::new();
        
        // Serialize with both methods
        let bincode_bytes = bincode.serialize(&state).unwrap();
        let json_bytes = json.serialize(&state).unwrap();
        
        // Deserialize with respective methods
        let from_bincode: TestState = bincode.deserialize(&bincode_bytes).unwrap();
        let from_json: TestState = json.deserialize(&json_bytes).unwrap();
        
        // Both should produce the same state
        prop_assert_eq!(&state, &from_bincode);
        prop_assert_eq!(&state, &from_json);
        prop_assert_eq!(&from_bincode, &from_json);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 35: Pluggable Serialization Consistency**
    /// **Validates: Requirements 10.2**
    ///
    /// For any state, the serialization context should correctly track which serializer
    /// was used and match appropriately.
    #[test]
    fn property_serialization_context_tracking(_state in arbitrary_test_state()) {
        let bincode = BincodeSerializer::new();
        let json = JsonSerializer::new();
        
        let bincode_ctx = SerializationContext::from_serializer(&bincode);
        let json_ctx = SerializationContext::from_serializer(&json);
        
        // Context should match the correct serializer
        prop_assert!(bincode_ctx.matches(&bincode));
        prop_assert!(!bincode_ctx.matches(&json));
        prop_assert!(json_ctx.matches(&json));
        prop_assert!(!json_ctx.matches(&bincode));
        
        // Names should be correct
        prop_assert_eq!(bincode_ctx.serializer_name(), "bincode");
        prop_assert_eq!(json_ctx.serializer_name(), "json");
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 35: Pluggable Serialization Consistency**
    /// **Validates: Requirements 10.2**
    ///
    /// For any state, multiple serializations with the same serializer should produce
    /// identical byte sequences (deterministic serialization).
    #[test]
    fn property_serialization_determinism(state in arbitrary_test_state()) {
        let bincode = BincodeSerializer::new();
        
        // Serialize multiple times
        let bytes1 = bincode.serialize(&state).unwrap();
        let bytes2 = bincode.serialize(&state).unwrap();
        let bytes3 = bincode.serialize(&state).unwrap();
        
        // All serializations should be identical
        prop_assert_eq!(&bytes1, &bytes2);
        prop_assert_eq!(&bytes2, &bytes3);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 35: Pluggable Serialization Consistency**
    /// **Validates: Requirements 10.2**
    ///
    /// For any state, JSON pretty printing should produce valid JSON that deserializes
    /// to the same state as non-pretty JSON.
    #[test]
    fn property_json_pretty_consistency(state in arbitrary_test_state()) {
        let json = JsonSerializer::new();
        let json_pretty = JsonSerializer::new_pretty();
        
        // Serialize with both
        let normal_bytes = json.serialize(&state).unwrap();
        let pretty_bytes = json_pretty.serialize(&state).unwrap();
        
        // Deserialize both
        let from_normal: TestState = json.deserialize(&normal_bytes).unwrap();
        let from_pretty: TestState = json.deserialize(&pretty_bytes).unwrap();
        
        // Both should produce the same state
        prop_assert_eq!(&state, &from_normal);
        prop_assert_eq!(&state, &from_pretty);
        prop_assert_eq!(&from_normal, &from_pretty);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_serialization_with_edge_case_values() {
        let states = vec![
            TestState {
                balance: 0,
                name: String::new(),
                active: false,
                count: 0,
            },
            TestState {
                balance: i64::MAX,
                name: "a".repeat(100),
                active: true,
                count: u32::MAX,
            },
            TestState {
                balance: i64::MIN + 1000001, // Valid according to validation
                name: "Test".to_string(),
                active: false,
                count: 12345,
            },
        ];

        let bincode = BincodeSerializer::new();
        let json = JsonSerializer::new();

        for state in states {
            // Test bincode
            let bincode_bytes = bincode.serialize(&state).unwrap();
            let from_bincode: TestState = bincode.deserialize(&bincode_bytes).unwrap();
            assert_eq!(state, from_bincode);

            // Test JSON
            let json_bytes = json.serialize(&state).unwrap();
            let from_json: TestState = json.deserialize(&json_bytes).unwrap();
            assert_eq!(state, from_json);
        }
    }

    #[test]
    fn test_serialization_error_handling() {
        let json = JsonSerializer::new();
        
        // Invalid JSON should fail deserialization
        let invalid_json = b"{ invalid json }";
        let result: Result<TestState, _> = json.deserialize(invalid_json);
        assert!(result.is_err());
        
        // Empty bytes should fail
        let result: Result<TestState, _> = json.deserialize(b"");
        assert!(result.is_err());
    }
}

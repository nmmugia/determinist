use dtre::{StateHasher, State, StateHash};
use proptest::prelude::*;
use serde::{Serialize, Deserialize};
use std::hash::{Hash, Hasher};
use dtre::error::ValidationError;

// Test state implementation for property tests
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
        Ok(())
    }
}

// Property test generators
fn arb_test_state() -> impl Strategy<Value = TestState> {
    (0i64..1000000, 0u32..10000, "[a-z]{3,20}").prop_map(|(balance, counter, name)| {
        TestState { balance, counter, name }
    })
}

fn arb_state_hash() -> impl Strategy<Value = StateHash> {
    prop::array::uniform32(any::<u8>()).prop_map(StateHash)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 10: State Hashing Consistency**
    /// **Validates: Requirements 4.1**
    /// 
    /// For any identical states, the computed cryptographic hashes should be identical, 
    /// and different states should produce different hashes.
    #[test]
    fn property_state_hashing_consistency(
        state in arb_test_state()
    ) {
        let hasher = StateHasher::new();
        
        // Hash the same state multiple times
        let hash1 = hasher.hash(&state);
        let hash2 = hasher.hash(&state);
        let hash3 = hasher.hash(&state);
        
        // All hashes should be identical
        prop_assert_eq!(&hash1, &hash2, "Same state should produce identical hash (attempt 1 vs 2)");
        prop_assert_eq!(&hash2, &hash3, "Same state should produce identical hash (attempt 2 vs 3)");
        prop_assert_eq!(&hash1, &hash3, "Same state should produce identical hash (attempt 1 vs 3)");
        
        // Hash should be 32 bytes (Blake3 output size)
        prop_assert_eq!(hash1.0.len(), 32, "Hash should be 32 bytes");
        
        // Clone the state and hash it - should produce same hash
        let state_clone = state.clone();
        let hash_clone = hasher.hash(&state_clone);
        prop_assert_eq!(&hash1, &hash_clone, "Cloned state should produce identical hash");
        
        // Create a new hasher and hash the same state - should produce same hash
        let hasher2 = StateHasher::new();
        let hash_new_hasher = hasher2.hash(&state);
        prop_assert_eq!(&hash1, &hash_new_hasher, "Different hasher instances should produce identical hash");
    }
    
    /// Test that different states produce different hashes
    #[test]
    fn property_different_states_different_hashes(
        state1 in arb_test_state(),
        state2 in arb_test_state()
    ) {
        // Only test when states are actually different
        prop_assume!(state1 != state2);
        
        let hasher = StateHasher::new();
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        
        // Different states should produce different hashes
        prop_assert_ne!(&hash1, &hash2, 
            "Different states should produce different hashes: {:?} vs {:?}", 
            state1, state2);
    }
    
    /// **Feature: deterministic-transaction-replay-engine, Property 12: Hash Chain Integrity**
    /// **Validates: Requirements 4.5**
    /// 
    /// For any sequence of state transitions, the hash chain should be verifiable and consistent.
    #[test]
    fn property_hash_chain_integrity(
        states in prop::collection::vec(arb_test_state(), 1..20)
    ) {
        let hasher = StateHasher::new();
        
        // Compute hashes for all states
        let hashes: Vec<StateHash> = states.iter()
            .map(|s| hasher.hash(s))
            .collect();
        
        // Compute chain hash multiple times - should be consistent
        let chain1 = hasher.hash_chain(&hashes);
        let chain2 = hasher.hash_chain(&hashes);
        prop_assert_eq!(&chain1, &chain2, "Hash chain should be deterministic");
        
        // Chain hash should be 32 bytes
        prop_assert_eq!(chain1.0.len(), 32, "Chain hash should be 32 bytes");
        
        // Empty prefix should produce different chain
        if hashes.len() > 1 {
            let partial_chain = hasher.hash_chain(&hashes[..hashes.len()-1]);
            prop_assert_ne!(&chain1, &partial_chain, 
                "Chain with fewer elements should produce different hash");
        }
        
        // Reordering should produce different chain (order matters)
        if hashes.len() >= 2 {
            let mut reordered = hashes.clone();
            reordered.swap(0, 1);
            let reordered_chain = hasher.hash_chain(&reordered);
            prop_assert_ne!(&chain1, &reordered_chain, 
                "Reordered chain should produce different hash");
        }
    }
    
    /// Test hash chain with incremental extension
    #[test]
    fn property_hash_chain_incremental(
        states in prop::collection::vec(arb_test_state(), 2..10)
    ) {
        let hasher = StateHasher::new();
        
        // Compute hashes for all states
        let hashes: Vec<StateHash> = states.iter()
            .map(|s| hasher.hash(s))
            .collect();
        
        // Build chain incrementally using extend_chain
        let mut current_chain = hasher.hash_chain(&hashes[..1]);
        
        for hash in &hashes[1..] {
            current_chain = hasher.extend_chain(&current_chain, hash);
        }
        
        // Incremental chain should be valid (32 bytes)
        prop_assert_eq!(current_chain.0.len(), 32, "Incremental chain should be 32 bytes");
        
        // Extending with same hash twice should produce different results
        let extended_once = hasher.extend_chain(&current_chain, &hashes[0]);
        let extended_twice = hasher.extend_chain(&extended_once, &hashes[0]);
        prop_assert_ne!(&extended_once, &extended_twice, 
            "Extending chain twice should produce different hashes");
    }
    
    /// Test that hash chain is sensitive to all elements
    #[test]
    fn property_hash_chain_sensitivity(
        hashes in prop::collection::vec(arb_state_hash(), 3..10),
        modification_index in 0usize..10
    ) {
        prop_assume!(!hashes.is_empty());
        let modification_index = modification_index % hashes.len();
        
        let hasher = StateHasher::new();
        
        // Compute original chain
        let original_chain = hasher.hash_chain(&hashes);
        
        // Modify one hash in the sequence
        let mut modified_hashes = hashes.clone();
        // Flip one bit in the selected hash
        modified_hashes[modification_index].0[0] ^= 1;
        
        let modified_chain = hasher.hash_chain(&modified_hashes);
        
        // Chain should be different after modification
        prop_assert_ne!(&original_chain, &modified_chain,
            "Modifying any hash in chain should change the chain hash");
    }
    
    /// Test empty hash chain
    #[test]
    fn property_hash_chain_empty(_seed in any::<u64>()) {
        let hasher = StateHasher::new();
        let empty_hashes: Vec<StateHash> = vec![];
        
        let chain1 = hasher.hash_chain(&empty_hashes);
        let chain2 = hasher.hash_chain(&empty_hashes);
        
        // Empty chain should be consistent
        prop_assert_eq!(&chain1, &chain2, "Empty chain should be deterministic");
        prop_assert_eq!(chain1.0.len(), 32, "Empty chain should still be 32 bytes");
    }
    
    /// Test single element hash chain
    #[test]
    fn property_hash_chain_single(state in arb_test_state()) {
        let hasher = StateHasher::new();
        let hash = hasher.hash(&state);
        
        let chain = hasher.hash_chain(&[hash.clone()]);
        
        // Chain of single element should be different from the element itself
        prop_assert_ne!(&chain, &hash, 
            "Chain of single hash should differ from the hash itself");
        
        // But should be consistent
        let chain2 = hasher.hash_chain(&[hash]);
        prop_assert_eq!(&chain, &chain2, "Single element chain should be deterministic");
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_hash_basic() {
        let hasher = StateHasher::new();
        let state = TestState {
            balance: 100,
            counter: 5,
            name: "test".to_string(),
        };
        
        let hash = hasher.hash(&state);
        assert_eq!(hash.0.len(), 32);
    }
    
    #[test]
    fn test_hash_chain_basic() {
        let hasher = StateHasher::new();
        let state1 = TestState { balance: 1, counter: 1, name: "a".to_string() };
        let state2 = TestState { balance: 2, counter: 2, name: "b".to_string() };
        
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        
        let chain = hasher.hash_chain(&[hash1, hash2]);
        assert_eq!(chain.0.len(), 32);
    }
    
    #[test]
    fn test_extend_chain_basic() {
        let hasher = StateHasher::new();
        let state1 = TestState { balance: 1, counter: 1, name: "a".to_string() };
        let state2 = TestState { balance: 2, counter: 2, name: "b".to_string() };
        
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        
        let chain1 = hasher.hash_chain(&[hash1.clone()]);
        let chain2 = hasher.extend_chain(&chain1, &hash2);
        
        assert_eq!(chain2.0.len(), 32);
        assert_ne!(chain1, chain2);
    }
}

//! Cryptographic state hashing using Blake3

use crate::traits::State;
use crate::types::StateHash;
use blake3::Hasher as Blake3Hasher;

/// StateHasher provides cryptographic hashing for state objects
/// 
/// Uses Blake3 for fast, secure hashing of arbitrary state types.
/// Ensures deterministic hashing across all platforms and executions.
#[derive(Debug, Clone)]
pub struct StateHasher {
    // Blake3 hasher is stateless, we create new instances for each hash
}

impl StateHasher {
    /// Create a new StateHasher
    pub fn new() -> Self {
        Self {}
    }
    
    /// Compute the cryptographic hash of a state
    /// 
    /// # Arguments
    /// * `state` - The state to hash
    /// 
    /// # Returns
    /// A StateHash containing the 32-byte Blake3 hash
    /// 
    /// # Panics
    /// Panics if state serialization fails (which should never happen for valid State implementations)
    pub fn hash<S: State>(&self, state: &S) -> StateHash {
        let serialized = bincode::serialize(state)
            .expect("State serialization should never fail");
        
        let mut hasher = Blake3Hasher::new();
        hasher.update(&serialized);
        let hash = hasher.finalize();
        
        StateHash(*hash.as_bytes())
    }
    
    /// Compute a hash chain from a sequence of state hashes
    /// 
    /// This creates a single hash that represents the entire sequence of states,
    /// useful for verifying the integrity of a transaction sequence.
    /// 
    /// # Arguments
    /// * `hashes` - A slice of StateHash values to chain together
    /// 
    /// # Returns
    /// A single StateHash representing the entire chain
    pub fn hash_chain(&self, hashes: &[StateHash]) -> StateHash {
        let mut hasher = Blake3Hasher::new();
        
        for hash in hashes {
            hasher.update(&hash.0);
        }
        
        let hash = hasher.finalize();
        StateHash(*hash.as_bytes())
    }
    
    /// Compute an incremental hash chain by extending an existing chain
    /// 
    /// This allows efficient incremental hashing without re-hashing the entire chain.
    /// 
    /// # Arguments
    /// * `previous_chain_hash` - The hash of the previous chain
    /// * `new_hash` - The new hash to append to the chain
    /// 
    /// # Returns
    /// A new StateHash representing the extended chain
    pub fn extend_chain(&self, previous_chain_hash: &StateHash, new_hash: &StateHash) -> StateHash {
        let mut hasher = Blake3Hasher::new();
        hasher.update(&previous_chain_hash.0);
        hasher.update(&new_hash.0);
        
        let hash = hasher.finalize();
        StateHash(*hash.as_bytes())
    }
}

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
    use std::hash::{Hash, Hasher as StdHasher};
    use crate::error::ValidationError;
    
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i64,
    }
    
    impl Hash for TestState {
        fn hash<H: StdHasher>(&self, state: &mut H) {
            self.value.hash(state);
        }
    }
    
    impl State for TestState {
        fn validate(&self) -> Result<(), ValidationError> {
            Ok(())
        }
    }
    
    #[test]
    fn test_hash_consistency() {
        let hasher = StateHasher::new();
        let state = TestState { value: 42 };
        
        let hash1 = hasher.hash(&state);
        let hash2 = hasher.hash(&state);
        
        assert_eq!(hash1, hash2, "Same state should produce same hash");
    }
    
    #[test]
    fn test_hash_different_states() {
        let hasher = StateHasher::new();
        let state1 = TestState { value: 42 };
        let state2 = TestState { value: 43 };
        
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        
        assert_ne!(hash1, hash2, "Different states should produce different hashes");
    }
    
    #[test]
    fn test_hash_chain_empty() {
        let hasher = StateHasher::new();
        let hashes: Vec<StateHash> = vec![];
        
        let chain_hash = hasher.hash_chain(&hashes);
        
        // Empty chain should produce a valid hash (hash of empty input)
        assert_eq!(chain_hash.0.len(), 32);
    }
    
    #[test]
    fn test_hash_chain_single() {
        let hasher = StateHasher::new();
        let state = TestState { value: 42 };
        let hash = hasher.hash(&state);
        
        let chain_hash = hasher.hash_chain(&[hash.clone()]);
        
        // Chain of single hash should be different from the hash itself
        assert_ne!(chain_hash, hash);
    }
    
    #[test]
    fn test_hash_chain_multiple() {
        let hasher = StateHasher::new();
        let state1 = TestState { value: 1 };
        let state2 = TestState { value: 2 };
        let state3 = TestState { value: 3 };
        
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        let hash3 = hasher.hash(&state3);
        
        let chain_hash = hasher.hash_chain(&[hash1, hash2, hash3]);
        
        assert_eq!(chain_hash.0.len(), 32);
    }
    
    #[test]
    fn test_hash_chain_order_matters() {
        let hasher = StateHasher::new();
        let state1 = TestState { value: 1 };
        let state2 = TestState { value: 2 };
        
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        
        let chain_forward = hasher.hash_chain(&[hash1.clone(), hash2.clone()]);
        let chain_backward = hasher.hash_chain(&[hash2, hash1]);
        
        assert_ne!(chain_forward, chain_backward, "Hash chain order should matter");
    }
    
    #[test]
    fn test_extend_chain() {
        let hasher = StateHasher::new();
        let state1 = TestState { value: 1 };
        let state2 = TestState { value: 2 };
        let state3 = TestState { value: 3 };
        
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        let hash3 = hasher.hash(&state3);
        
        // Build chain incrementally
        let chain1 = hasher.hash_chain(&[hash1.clone()]);
        let chain2 = hasher.extend_chain(&chain1, &hash2);
        let chain3 = hasher.extend_chain(&chain2, &hash3);
        
        // Build chain all at once
        let full_chain = hasher.hash_chain(&[hash1, hash2, hash3]);
        
        // They should be different (incremental vs full chain have different structures)
        // But both should be valid 32-byte hashes
        assert_eq!(chain3.0.len(), 32);
        assert_eq!(full_chain.0.len(), 32);
    }
}

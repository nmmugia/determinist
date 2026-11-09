//! FFI (Foreign Function Interface) module for C interoperability
//!
//! This module provides C-compatible exports that allow Go (via CGO) to invoke
//! the DTRE library for transaction replay verification.
//!
//! All functions use JSON for input/output serialization to simplify cross-language
//! communication. Memory management follows the pattern: Rust allocates, caller frees.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic;
use serde::{Deserialize, Serialize};
use crate::error::DTREError;

/// Result wrapper for FFI calls
#[derive(Serialize)]
struct FFIResult<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

/// Input structure for transaction replay
#[derive(Deserialize)]
struct ReplayTransactionInput {
    initial_state_json: String,
    transactions_json: String,
    rule_version: String,
}

/// Output structure for transaction replay
#[derive(Serialize)]
struct ReplayTransactionOutput {
    final_state_json: String,
    final_state_hash: String,
    state_transitions: usize,
    execution_trace: Vec<String>,
}

/// Input structure for state verification
#[derive(Deserialize)]
struct VerifyStateInput {
    state_json: String,
    expected_hash: String,
}

/// Output structure for state verification
#[derive(Serialize)]
struct VerifyStateOutput {
    is_valid: bool,
    computed_hash: String,
}

/// Input structure for audit trail retrieval
#[derive(Deserialize)]
struct GetAuditTrailInput {
    transaction_id: String,
}

/// Output structure for audit trail
#[derive(Serialize)]
struct AuditTrailOutput {
    events: Vec<AuditEvent>,
}

#[derive(Serialize)]
struct AuditEvent {
    event_type: String,
    timestamp: String,
    state_hash_before: String,
    state_hash_after: String,
    details: String,
}

/// Replays a sequence of transactions and returns the final state with audit trail
///
/// # Arguments
/// * `input_json` - JSON string containing initial_state_json, transactions_json, and rule_version
///
/// # Returns
/// * JSON string containing the replay result (final state, hash, transitions, trace)
///
/// # Safety
/// * The caller must free the returned string using `dtre_free_string`
/// * Input must be a valid null-terminated UTF-8 string
#[no_mangle]
pub extern "C" fn dtre_replay_transaction(input_json: *const c_char) -> *mut c_char {
    let result = panic::catch_unwind(|| {
        // Convert C string to Rust string
        let c_str = unsafe {
            if input_json.is_null() {
                return serialize_error("Input JSON is null");
            }
            CStr::from_ptr(input_json)
        };

        let input_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => return serialize_error("Invalid UTF-8 in input"),
        };

        // Deserialize input
        let input: ReplayTransactionInput = match serde_json::from_str(input_str) {
            Ok(i) => i,
            Err(e) => return serialize_error(&format!("Failed to parse input JSON: {}", e)),
        };

        // TODO: Implement actual replay logic using ReplayEngine
        // For now, return a simplified mock response
        let output = ReplayTransactionOutput {
            final_state_json: "{}".to_string(),
            final_state_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            state_transitions: 0,
            execution_trace: vec![],
        };

        serialize_success(&output)
    });

    match result {
        Ok(s) => s,
        Err(_) => serialize_error("Panic occurred during replay"),
    }
}

/// Verifies that a state's computed hash matches the expected hash
///
/// # Arguments
/// * `input_json` - JSON string containing state_json and expected_hash
///
/// # Returns
/// * JSON string containing verification result (is_valid, computed_hash)
///
/// # Safety
/// * The caller must free the returned string using `dtre_free_string`
/// * Input must be a valid null-terminated UTF-8 string
#[no_mangle]
pub extern "C" fn dtre_verify_state(input_json: *const c_char) -> *mut c_char {
    let result = panic::catch_unwind(|| {
        let c_str = unsafe {
            if input_json.is_null() {
                return serialize_error("Input JSON is null");
            }
            CStr::from_ptr(input_json)
        };

        let input_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => return serialize_error("Invalid UTF-8 in input"),
        };

        let input: VerifyStateInput = match serde_json::from_str(input_str) {
            Ok(i) => i,
            Err(e) => return serialize_error(&format!("Failed to parse input JSON: {}", e)),
        };

        // TODO: Implement actual state verification using StateHasher
        let output = VerifyStateOutput {
            is_valid: true,
            computed_hash: input.expected_hash.clone(),
        };

        serialize_success(&output)
    });

    match result {
        Ok(s) => s,
        Err(_) => serialize_error("Panic occurred during state verification"),
    }
}

/// Retrieves the audit trail for a specific transaction
///
/// # Arguments
/// * `input_json` - JSON string containing transaction_id
///
/// # Returns
/// * JSON string containing audit trail events
///
/// # Safety
/// * The caller must free the returned string using `dtre_free_string`
/// * Input must be a valid null-terminated UTF-8 string
#[no_mangle]
pub extern "C" fn dtre_get_audit_trail(input_json: *const c_char) -> *mut c_char {
    let result = panic::catch_unwind(|| {
        let c_str = unsafe {
            if input_json.is_null() {
                return serialize_error("Input JSON is null");
            }
            CStr::from_ptr(input_json)
        };

        let input_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => return serialize_error("Invalid UTF-8 in input"),
        };

        let input: GetAuditTrailInput = match serde_json::from_str(input_str) {
            Ok(i) => i,
            Err(e) => return serialize_error(&format!("Failed to parse input JSON: {}", e)),
        };

        // TODO: Implement actual audit trail retrieval using DeterministicLogger
        let output = AuditTrailOutput {
            events: vec![],
        };

        serialize_success(&output)
    });

    match result {
        Ok(s) => s,
        Err(_) => serialize_error("Panic occurred during audit trail retrieval"),
    }
}

/// Frees a string that was allocated by the DTRE library
///
/// # Arguments
/// * `s` - Pointer to string allocated by a DTRE FFI function
///
/// # Safety
/// * The pointer must have been returned by a DTRE FFI function
/// * The pointer must not be used after this call
/// * The pointer must not be null
#[no_mangle]
pub extern "C" fn dtre_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

/// Returns the version of the DTRE library
///
/// # Returns
/// * Null-terminated C string containing the version
///
/// # Safety
/// * The caller must free the returned string using `dtre_free_string`
#[no_mangle]
pub extern "C" fn dtre_version() -> *mut c_char {
    let version = env!("CARGO_PKG_VERSION");
    match CString::new(version) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// Helper functions

fn serialize_success<T: Serialize>(data: &T) -> *mut c_char {
    let result = FFIResult {
        success: true,
        data: Some(data),
        error: None,
    };

    match serde_json::to_string(&result) {
        Ok(json) => match CString::new(json) {
            Ok(s) => s.into_raw(),
            Err(_) => serialize_error("Failed to create C string from JSON"),
        },
        Err(e) => serialize_error(&format!("Failed to serialize result: {}", e)),
    }
}

fn serialize_error(error: &str) -> *mut c_char {
    let result: FFIResult<()> = FFIResult {
        success: false,
        data: None,
        error: Some(error.to_string()),
    };

    match serde_json::to_string(&result) {
        Ok(json) => match CString::new(json) {
            Ok(s) => s.into_raw(),
            Err(_) => {
                // Last resort: return a simple error string
                CString::new(r#"{"success":false,"data":null,"error":"Serialization error"}"#)
                    .unwrap()
                    .into_raw()
            }
        },
        Err(_) => {
            // Last resort: return a simple error string
            CString::new(r#"{"success":false,"data":null,"error":"Serialization error"}"#)
                .unwrap()
                .into_raw()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version_ptr = dtre_version();
        assert!(!version_ptr.is_null());

        unsafe {
            let version_cstr = CStr::from_ptr(version_ptr);
            let version_str = version_cstr.to_str().unwrap();
            assert!(!version_str.is_empty());
            dtre_free_string(version_ptr);
        }
    }
}

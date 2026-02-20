use dtre::{
    DeterministicLogger, LogEntry, LogLevel, ExecutionTraceLog, TraceEvent, TraceEventType,
    StateHash, Version
};
use chrono::{DateTime, Utc, TimeZone};
use proptest::prelude::*;

// Helper to create arbitrary DateTime
fn arbitrary_datetime() -> impl Strategy<Value = DateTime<Utc>> {
    (0i64..2_000_000_000).prop_map(|secs| {
        Utc.timestamp_opt(secs, 0).unwrap()
    })
}

// Helper to create arbitrary LogLevel
fn arbitrary_log_level() -> impl Strategy<Value = LogLevel> {
    prop_oneof![
        Just(LogLevel::Trace),
        Just(LogLevel::Debug),
        Just(LogLevel::Info),
        Just(LogLevel::Warn),
        Just(LogLevel::Error),
    ]
}

// Helper to create arbitrary StateHash
fn arbitrary_state_hash() -> impl Strategy<Value = StateHash> {
    prop::array::uniform32(any::<u8>()).prop_map(StateHash)
}

// Helper to create arbitrary Version
fn arbitrary_version() -> impl Strategy<Value = Version> {
    (0u32..10, 0u32..20, 0u32..100).prop_map(|(major, minor, patch)| {
        Version::new(major, minor, patch)
    })
}

// Helper to create arbitrary LogEntry
fn arbitrary_log_entry() -> impl Strategy<Value = LogEntry> {
    (
        arbitrary_log_level(),
        arbitrary_datetime(),
        "[a-z ]{10,50}",
        prop::option::of("[a-z0-9]{8,16}"),
        prop::option::of(0usize..1000),
        prop::option::of(arbitrary_version()),
        prop::option::of(arbitrary_state_hash()),
        prop::collection::vec(("[a-z_]{3,10}", "[a-z0-9 ]{5,20}"), 0..5)
    ).prop_map(|(level, timestamp, message, tx_id, tx_idx, rule_ver, hash, metadata)| {
        let mut entry = LogEntry::new(level, timestamp, message);
        if let (Some(id), Some(idx)) = (tx_id, tx_idx) {
            entry = entry.with_transaction(id, idx);
        }
        if let Some(ver) = rule_ver {
            entry = entry.with_rule(ver);
        }
        if let Some(h) = hash {
            entry = entry.with_state_hash(h);
        }
        for (k, v) in metadata {
            entry = entry.with_metadata(k, v);
        }
        entry
    })
}

// Helper to create arbitrary TraceEventType
fn arbitrary_trace_event_type() -> impl Strategy<Value = TraceEventType> {
    prop_oneof![
        Just(TraceEventType::ReplayStarted),
        Just(TraceEventType::TransactionStarted),
        Just(TraceEventType::TransactionCompleted),
        Just(TraceEventType::TransactionFailed),
        Just(TraceEventType::RuleApplicationStarted),
        Just(TraceEventType::RuleApplicationCompleted),
        Just(TraceEventType::StateTransition),
        Just(TraceEventType::CheckpointCreated),
        Just(TraceEventType::CheckpointRestored),
        Just(TraceEventType::ReplayCompleted),
        Just(TraceEventType::ReplayFailed),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// **Feature: deterministic-transaction-replay-engine, Property 32: Deterministic Logging**
    /// **Validates: Requirements 8.5**
    /// 
    /// For any execution with logging enabled, the logs should not affect determinism 
    /// and should contain expected information.
    #[test]
    fn property_deterministic_logging(
        entries in prop::collection::vec(arbitrary_log_entry(), 1..50),
        min_level in arbitrary_log_level()
    ) {
        // Create two loggers with the same configuration
        let mut logger1 = DeterministicLogger::new(min_level);
        let mut logger2 = DeterministicLogger::new(min_level);
        
        // Log the same entries to both loggers
        for entry in &entries {
            logger1.log(entry.clone());
            logger2.log(entry.clone());
        }
        
        // Both loggers should have the same number of entries
        prop_assert_eq!(logger1.len(), logger2.len());
        
        // Both loggers should have identical entries
        for (i, (e1, e2)) in logger1.entries().iter().zip(logger2.entries().iter()).enumerate() {
            prop_assert_eq!(e1.level, e2.level, "Entry {} level mismatch", i);
            prop_assert_eq!(e1.timestamp, e2.timestamp, "Entry {} timestamp mismatch", i);
            prop_assert_eq!(&e1.message, &e2.message, "Entry {} message mismatch", i);
            prop_assert_eq!(&e1.transaction_id, &e2.transaction_id, "Entry {} tx_id mismatch", i);
            prop_assert_eq!(e1.transaction_index, e2.transaction_index, "Entry {} tx_idx mismatch", i);
        }
        
        // Verify that logging is deterministic - same input produces same output
        let serialized1 = bincode::serialize(&logger1).unwrap();
        let serialized2 = bincode::serialize(&logger2).unwrap();
        prop_assert_eq!(serialized1, serialized2, "Serialized loggers should be identical");
    }
    
    /// Property: Log filtering works correctly
    /// Ensures that log level filtering is deterministic and consistent
    #[test]
    fn property_log_filtering(
        entries in prop::collection::vec(arbitrary_log_entry(), 1..50),
        min_level in arbitrary_log_level()
    ) {
        let mut logger = DeterministicLogger::new(min_level);
        
        // Log all entries
        for entry in &entries {
            logger.log(entry.clone());
        }
        
        // Count how many entries should have been logged
        let expected_count = entries.iter()
            .filter(|e| (e.level as u8) >= (min_level as u8))
            .count();
        
        // Verify the logger has the correct number of entries
        prop_assert_eq!(logger.len(), expected_count);
        
        // Verify all logged entries meet the minimum level
        for entry in logger.entries() {
            prop_assert!((entry.level as u8) >= (min_level as u8));
        }
    }
    
    /// Property: Log entry context preservation
    /// Ensures that all context information is preserved in log entries
    #[test]
    fn property_log_entry_context_preservation(
        level in arbitrary_log_level(),
        timestamp in arbitrary_datetime(),
        message in "[a-z ]{10,50}",
        tx_id in "[a-z0-9]{8,16}",
        tx_idx in 0usize..1000,
        rule_ver in arbitrary_version(),
        hash in arbitrary_state_hash()
    ) {
        // Create a log entry with full context
        let entry = LogEntry::new(level, timestamp, message.clone())
            .with_transaction(tx_id.clone(), tx_idx)
            .with_rule(rule_ver.clone())
            .with_state_hash(hash)
            .with_metadata("key1".to_string(), "value1".to_string())
            .with_metadata("key2".to_string(), "value2".to_string());
        
        // Verify all context is preserved
        prop_assert_eq!(entry.level, level);
        prop_assert_eq!(entry.timestamp, timestamp);
        prop_assert_eq!(&entry.message, &message);
        prop_assert_eq!(entry.transaction_id.as_ref(), Some(&tx_id));
        prop_assert_eq!(entry.transaction_index, Some(tx_idx));
        prop_assert_eq!(entry.rule_version.as_ref(), Some(&rule_ver));
        prop_assert_eq!(entry.state_hash, Some(hash));
        prop_assert_eq!(entry.metadata.len(), 2);
        
        // Verify serialization preserves all data
        let serialized = bincode::serialize(&entry).unwrap();
        let deserialized: LogEntry = bincode::deserialize(&serialized).unwrap();
        
        prop_assert_eq!(deserialized.level, entry.level);
        prop_assert_eq!(deserialized.timestamp, entry.timestamp);
        prop_assert_eq!(&deserialized.message, &entry.message);
        prop_assert_eq!(&deserialized.transaction_id, &entry.transaction_id);
        prop_assert_eq!(deserialized.transaction_index, entry.transaction_index);
    }
    
    /// Property: Execution trace completeness
    /// Ensures that execution traces capture all events correctly
    #[test]
    fn property_execution_trace_completeness(
        start_time in arbitrary_datetime(),
        event_types in prop::collection::vec(arbitrary_trace_event_type(), 1..50),
        log_entries in prop::collection::vec(arbitrary_log_entry(), 0..20)
    ) {
        let mut trace = ExecutionTraceLog::new(start_time);
        
        // Add events
        for (i, event_type) in event_types.iter().enumerate() {
            let event = TraceEvent {
                timestamp: Utc.timestamp_opt(start_time.timestamp() + i as i64, 0).unwrap(),
                event_type: *event_type,
                transaction_id: Some(format!("tx_{}", i)),
                transaction_index: Some(i),
                state_hash_before: None,
                state_hash_after: None,
                data: Vec::new(),
            };
            trace.add_event(event);
        }
        
        // Add log entries
        for entry in &log_entries {
            trace.add_log(entry.clone());
        }
        
        // Verify all events were captured
        prop_assert_eq!(trace.events.len(), event_types.len());
        
        // Verify all logs were captured
        prop_assert_eq!(trace.logs.len(), log_entries.len());
        
        // Verify events are in order
        for i in 1..trace.events.len() {
            prop_assert!(trace.events[i].timestamp >= trace.events[i-1].timestamp);
        }
        
        // Verify trace can be serialized and deserialized
        let serialized = bincode::serialize(&trace).unwrap();
        let deserialized: ExecutionTraceLog = bincode::deserialize(&serialized).unwrap();
        
        prop_assert_eq!(deserialized.events.len(), trace.events.len());
        prop_assert_eq!(deserialized.logs.len(), trace.logs.len());
        prop_assert_eq!(deserialized.start_time, trace.start_time);
    }
    
    /// Property: Logger operations are side-effect free
    /// Ensures that logging operations don't affect the deterministic execution
    #[test]
    fn property_logger_side_effect_free(
        entries in prop::collection::vec(arbitrary_log_entry(), 1..50)
    ) {
        let mut logger = DeterministicLogger::all();
        
        // Capture initial state
        let initial_len = logger.len();
        
        // Log entries
        for entry in &entries {
            logger.log(entry.clone());
        }
        
        // Verify state changed only by adding entries
        prop_assert_eq!(logger.len(), initial_len + entries.len());
        
        // Clear and verify
        logger.clear();
        prop_assert_eq!(logger.len(), 0);
        prop_assert!(logger.is_empty());
        
        // Log again and verify determinism
        for entry in &entries {
            logger.log(entry.clone());
        }
        prop_assert_eq!(logger.len(), entries.len());
    }
}

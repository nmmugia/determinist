//! Deterministic logging and tracing for the DTRE
//!
//! This module provides logging capabilities that do not affect determinism.
//! All logging operations are side-effect free from the perspective of the
//! deterministic execution.

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use crate::types::{Version, StateHash};

/// Log level for deterministic logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// Trace level - most verbose
    Trace,
    /// Debug level - detailed information
    Debug,
    /// Info level - general information
    Info,
    /// Warning level - potential issues
    Warn,
    /// Error level - errors that occurred
    Error,
}

/// A deterministic log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Log level
    pub level: LogLevel,
    /// Timestamp from deterministic time context
    pub timestamp: DateTime<Utc>,
    /// Transaction ID if applicable
    pub transaction_id: Option<String>,
    /// Transaction index if applicable
    pub transaction_index: Option<usize>,
    /// Rule version if applicable
    pub rule_version: Option<Version>,
    /// State hash if applicable
    pub state_hash: Option<StateHash>,
    /// Log message
    pub message: String,
    /// Additional structured data
    pub metadata: Vec<(String, String)>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(level: LogLevel, timestamp: DateTime<Utc>, message: String) -> Self {
        Self {
            level,
            timestamp,
            transaction_id: None,
            transaction_index: None,
            rule_version: None,
            state_hash: None,
            message,
            metadata: Vec::new(),
        }
    }
    
    /// Add transaction context to the log entry
    pub fn with_transaction(mut self, id: String, index: usize) -> Self {
        self.transaction_id = Some(id);
        self.transaction_index = Some(index);
        self
    }
    
    /// Add rule context to the log entry
    pub fn with_rule(mut self, version: Version) -> Self {
        self.rule_version = Some(version);
        self
    }
    
    /// Add state hash to the log entry
    pub fn with_state_hash(mut self, hash: StateHash) -> Self {
        self.state_hash = Some(hash);
        self
    }
    
    /// Add metadata to the log entry
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.push((key, value));
        self
    }
}

/// Deterministic logger that collects log entries without side effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterministicLogger {
    /// Collected log entries
    entries: Vec<LogEntry>,
    /// Minimum log level to record
    min_level: LogLevel,
}

impl DeterministicLogger {
    /// Create a new deterministic logger
    pub fn new(min_level: LogLevel) -> Self {
        Self {
            entries: Vec::new(),
            min_level,
        }
    }
    
    /// Create a logger that captures all levels
    pub fn all() -> Self {
        Self::new(LogLevel::Trace)
    }
    
    /// Create a logger that captures info and above
    pub fn with_info_level() -> Self {
        Self::new(LogLevel::Info)
    }
    
    /// Log an entry if it meets the minimum level
    pub fn log(&mut self, entry: LogEntry) {
        if self.should_log(entry.level) {
            self.entries.push(entry);
        }
    }
    
    /// Check if a log level should be recorded
    fn should_log(&self, level: LogLevel) -> bool {
        level as u8 >= self.min_level as u8
    }
    
    /// Log a trace message
    pub fn trace(&mut self, timestamp: DateTime<Utc>, message: String) {
        self.log(LogEntry::new(LogLevel::Trace, timestamp, message));
    }
    
    /// Log a debug message
    pub fn debug(&mut self, timestamp: DateTime<Utc>, message: String) {
        self.log(LogEntry::new(LogLevel::Debug, timestamp, message));
    }
    
    /// Log an info message
    pub fn info(&mut self, timestamp: DateTime<Utc>, message: String) {
        self.log(LogEntry::new(LogLevel::Info, timestamp, message));
    }
    
    /// Log a warning message
    pub fn warn(&mut self, timestamp: DateTime<Utc>, message: String) {
        self.log(LogEntry::new(LogLevel::Warn, timestamp, message));
    }
    
    /// Log an error message
    pub fn error(&mut self, timestamp: DateTime<Utc>, message: String) {
        self.log(LogEntry::new(LogLevel::Error, timestamp, message));
    }
    
    /// Get all log entries
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }
    
    /// Get the number of log entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    
    /// Check if the logger is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    
    /// Clear all log entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    
    /// Filter entries by log level
    pub fn filter_by_level(&self, level: LogLevel) -> Vec<&LogEntry> {
        self.entries.iter()
            .filter(|e| e.level == level)
            .collect()
    }
    
    /// Filter entries by transaction ID
    pub fn filter_by_transaction(&self, transaction_id: &str) -> Vec<&LogEntry> {
        self.entries.iter()
            .filter(|e| e.transaction_id.as_deref() == Some(transaction_id))
            .collect()
    }
}

impl Default for DeterministicLogger {
    fn default() -> Self {
        Self::with_info_level()
    }
}

/// Execution trace for debugging and audit purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTraceLog {
    /// Start time of execution
    pub start_time: DateTime<Utc>,
    /// End time of execution
    pub end_time: Option<DateTime<Utc>>,
    /// Log entries collected during execution
    pub logs: Vec<LogEntry>,
    /// Execution events
    pub events: Vec<TraceEvent>,
}

/// An event in the execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: TraceEventType,
    /// Transaction ID if applicable
    pub transaction_id: Option<String>,
    /// Transaction index if applicable
    pub transaction_index: Option<usize>,
    /// State hash before the event
    pub state_hash_before: Option<StateHash>,
    /// State hash after the event
    pub state_hash_after: Option<StateHash>,
    /// Additional event data
    pub data: Vec<(String, String)>,
}

/// Type of trace event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceEventType {
    /// Replay started
    ReplayStarted,
    /// Transaction processing started
    TransactionStarted,
    /// Transaction processing completed
    TransactionCompleted,
    /// Transaction processing failed
    TransactionFailed,
    /// Rule application started
    RuleApplicationStarted,
    /// Rule application completed
    RuleApplicationCompleted,
    /// State transition occurred
    StateTransition,
    /// Checkpoint created
    CheckpointCreated,
    /// Checkpoint restored
    CheckpointRestored,
    /// Replay completed
    ReplayCompleted,
    /// Replay failed
    ReplayFailed,
}

impl ExecutionTraceLog {
    /// Create a new execution trace
    pub fn new(start_time: DateTime<Utc>) -> Self {
        Self {
            start_time,
            end_time: None,
            logs: Vec::new(),
            events: Vec::new(),
        }
    }
    
    /// Add a log entry to the trace
    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push(entry);
    }
    
    /// Add an event to the trace
    pub fn add_event(&mut self, event: TraceEvent) {
        self.events.push(event);
    }
    
    /// Mark the trace as completed
    pub fn complete(&mut self, end_time: DateTime<Utc>) {
        self.end_time = Some(end_time);
    }
    
    /// Get all events of a specific type
    pub fn events_by_type(&self, event_type: TraceEventType) -> Vec<&TraceEvent> {
        self.events.iter()
            .filter(|e| e.event_type == event_type)
            .collect()
    }
    
    /// Get all events for a specific transaction
    pub fn events_by_transaction(&self, transaction_id: &str) -> Vec<&TraceEvent> {
        self.events.iter()
            .filter(|e| e.transaction_id.as_deref() == Some(transaction_id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    #[test]
    fn test_logger_basic() {
        let mut logger = DeterministicLogger::with_info_level();
        let now = Utc::now();
        
        logger.info(now, "Test message".to_string());
        
        assert_eq!(logger.len(), 1);
        assert_eq!(logger.entries()[0].message, "Test message");
        assert_eq!(logger.entries()[0].level, LogLevel::Info);
    }
    
    #[test]
    fn test_logger_filtering() {
        let mut logger = DeterministicLogger::with_info_level();
        let now = Utc::now();
        
        // Trace should be filtered out
        logger.trace(now, "Trace message".to_string());
        // Info should be included
        logger.info(now, "Info message".to_string());
        
        assert_eq!(logger.len(), 1);
        assert_eq!(logger.entries()[0].level, LogLevel::Info);
    }
    
    #[test]
    fn test_execution_trace() {
        let start = Utc::now();
        let mut trace = ExecutionTraceLog::new(start);
        
        trace.add_event(TraceEvent {
            timestamp: start,
            event_type: TraceEventType::ReplayStarted,
            transaction_id: None,
            transaction_index: None,
            state_hash_before: None,
            state_hash_after: None,
            data: Vec::new(),
        });
        
        assert_eq!(trace.events.len(), 1);
        assert_eq!(trace.events[0].event_type, TraceEventType::ReplayStarted);
    }
}

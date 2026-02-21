# Deterministic Transaction Replay Engine (DTRE)

A Rust library for deterministic execution of financial transactions through pure functional programming principles. The DTRE ensures that replaying the same sequence of transactions with identical initial state and business rules produces byte-for-byte identical results across different machines, times, and environments.

## Features

- **Deterministic Execution**: Guaranteed identical results across all replay environments
- **Pure Functional Design**: All business logic functions are pure with no side effects
- **Cryptographic Verification**: Every state change is cryptographically hashed for integrity
- **Rule Versioning**: Support for multiple concurrent rule set versions with migration analysis
- **Parallel Execution**: Thread-safe parallel processing while maintaining determinism
- **Comprehensive Testing**: Property-based testing with 100+ iterations per property
- **Audit Trail**: Complete execution traces for compliance and debugging

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
dtre = "0.1.0"
```

## Quick Start

```rust
use dtre::{
    ReplayEngineBuilder, ExecutionContext, State, Transaction, RuleSet,
    ValidationError, ProcessingError, Version
};
use chrono::Utc;
use serde::{Serialize, Deserialize};

// Define your state
#[derive(Clone, Serialize, Deserialize, Hash)]
struct MyState {
    balance: i64,
}

impl State for MyState {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.balance < 0 {
            return Err(ValidationError::InvalidState {
                reason: "Balance cannot be negative".to_string(),
            });
        }
        Ok(())
    }
}

// Define your transactions
#[derive(Clone, Serialize, Deserialize)]
struct MyTransaction {
    id: String,
    amount: i64,
    timestamp: chrono::DateTime<Utc>,
}

impl Transaction for MyTransaction {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn timestamp(&self) -> chrono::DateTime<Utc> {
        self.timestamp
    }
    
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

// Define your business rules
struct MyRules;

impl RuleSet<MyState, MyTransaction> for MyRules {
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    
    fn apply(
        &self,
        state: &MyState,
        transaction: &MyTransaction,
        _context: &ExecutionContext,
    ) -> Result<MyState, ProcessingError> {
        Ok(MyState {
            balance: state.balance + transaction.amount,
        })
    }
}

// Create and run the replay engine
fn main() {
    let initial_state = MyState { balance: 0 };
    let transactions = vec![/* your transactions */];
    let context = ExecutionContext::new(Utc::now(), 42);
    
    let engine = ReplayEngineBuilder::new()
        .with_initial_state(initial_state)
        .with_rule_set(MyRules)
        .with_context(context)
        .build()
        .unwrap();
    
    let result = engine.replay(&transactions).unwrap();
    
    println!("Final balance: {}", result.final_state.balance);
    println!("Final hash: {:?}", result.final_hash);
}
```

## Core API

### Traits

#### `State`
Represents the financial state at any point in time.

```rust
pub trait State: Clone + Serialize + DeserializeOwned + Hash {
    fn validate(&self) -> Result<(), ValidationError>;
}
```

#### `Transaction`
Represents an immutable financial transaction event.

```rust
pub trait Transaction: Clone + Serialize + DeserializeOwned {
    fn id(&self) -> &str;
    fn timestamp(&self) -> DateTime<Utc>;
    fn validate(&self) -> Result<(), ValidationError>;
}
```

#### `RuleSet`
Defines business logic for processing transactions.

```rust
pub trait RuleSet<S, T> {
    fn version(&self) -> Version;
    fn apply(&self, state: &S, transaction: &T, context: &ExecutionContext) 
        -> Result<S, ProcessingError>;
}
```

### Core Types

#### `ReplayEngine<S, T, R>`
The main engine for replaying transaction sequences.

**Methods:**
- `replay(&self, transactions: &[T]) -> Result<ReplayResult<S>, ProcessingError>`
- `replay_with_checkpoints(&self, transactions: &[T], interval: usize) -> Result<ReplayResult<S>, ProcessingError>`
- `replay_parallel(&self, transactions: &[T]) -> Result<ReplayResult<S>, ProcessingError>`

#### `ReplayEngineBuilder<S, T, R>`
Builder for constructing replay engines with fluent API.

**Methods:**
- `new() -> Self`
- `with_initial_state(self, state: S) -> Self`
- `with_rule_set(self, rules: R) -> Self`
- `with_context(self, context: ExecutionContext) -> Self`
- `with_checkpoint_interval(self, interval: usize) -> Self`
- `build(self) -> Result<ReplayEngine<S, T, R>, ValidationError>`

#### `ExecutionContext`
Provides controlled access to external dependencies.

**Methods:**
- `new(time: DateTime<Utc>, seed: u64) -> Self`
- `now(&self) -> DateTime<Utc>`
- `random(&mut self) -> &mut SeededRandom`
- `get_external_fact<T>(&self, key: &str) -> Option<&T>`
- `add_external_fact<T>(&mut self, key: String, value: T)`

#### `StateHasher`
Computes cryptographic hashes of states using Blake3.

**Methods:**
- `new() -> Self`
- `hash<S: State>(&self, state: &S) -> StateHash`
- `hash_chain(&self, hashes: &[StateHash]) -> StateHash`

#### `StateManager<S>`
Manages state transitions and checkpoints.

**Methods:**
- `new(initial_state: S) -> Self`
- `apply_transaction<T, R>(&mut self, transaction: &T, rules: &R, context: &ExecutionContext) -> Result<StateTransition<S>, ProcessingError>`
- `create_checkpoint(&self) -> Checkpoint<S>`
- `restore_checkpoint(&mut self, checkpoint: &Checkpoint<S>) -> Result<(), StateError>`
- `calculate_diff(&self, other: &S) -> StateDiff<S>`

#### `VersionedRuleSet<S, T>`
Wrapper for rule sets with version metadata.

**Methods:**
- `new(version: Version, rules: Box<dyn RuleSet<S, T>>, metadata: RuleSetMetadata) -> Self`
- `version(&self) -> &Version`
- `apply(&self, state: &S, transaction: &T, context: &ExecutionContext) -> Result<S, ProcessingError>`

#### `RuleSetRegistry<S, T>`
Registry for managing multiple rule set versions.

**Methods:**
- `new() -> Self`
- `register(&mut self, rule_set: VersionedRuleSet<S, T>) -> Result<(), RegistrationError>`
- `get(&self, version: &Version) -> Option<&VersionedRuleSet<S, T>>`
- `list_versions(&self) -> Vec<&Version>`

#### `ResultComparator`
Compares replay results across different rule versions.

**Methods:**
- `compare<S>(&self, result1: &ReplayResult<S>, result2: &ReplayResult<S>) -> ResultComparison<S>`
- `analyze_impact<S>(&self, results: &[ReplayResult<S>]) -> ImpactAnalysis<S>`

### Result Types

#### `ReplayResult<S>`
Contains the complete result of a replay operation.

**Fields:**
- `final_state: S` - The final state after all transactions
- `final_hash: StateHash` - Cryptographic hash of the final state
- `execution_trace: ExecutionTrace` - Complete trace of execution
- `performance_metrics: PerformanceMetrics` - Performance statistics
- `checkpoints: Vec<CheckpointInfo>` - Checkpoint information

#### `ExecutionTrace`
Detailed trace of replay execution.

**Fields:**
- `transactions_processed: usize` - Number of transactions processed
- `state_transitions: Vec<StateTransition<S>>` - All state transitions
- `rule_applications: Vec<RuleApplication>` - Rule application history
- `errors: Vec<ErrorContext>` - Any errors encountered

#### `StateTransition<S>`
Represents a single state transition.

**Fields:**
- `from_state: S` - State before transition
- `to_state: S` - State after transition
- `from_hash: StateHash` - Hash of from_state
- `to_hash: StateHash` - Hash of to_state
- `transaction_id: String` - ID of transaction that caused transition

### Error Types

#### `DTREError`
Top-level error type for all DTRE operations.

**Variants:**
- `Processing(ProcessingError)` - Transaction processing errors
- `Validation(ValidationError)` - State/transaction validation errors
- `State(StateError)` - State management errors
- `Rule(RuleError)` - Rule set errors
- `Serialization(SerializationError)` - Serialization errors

#### `ProcessingError`
Errors during transaction processing.

**Variants:**
- `TransactionFailed { transaction_id: String, reason: String }`
- `RuleApplicationFailed { rule_version: Version, details: String }`
- `NonDeterministicOperation { operation: String, location: String }`

#### `ValidationError`
Validation errors for states and transactions.

**Variants:**
- `InvalidState { reason: String }`
- `InvalidTransaction { reason: String }`
- `InvalidRuleSet { reason: String }`

## Advanced Features

### Parallel Execution

```rust
let result = engine.replay_parallel(&transactions)?;
// Results are guaranteed to match sequential execution
```

### Checkpointing

```rust
let engine = ReplayEngineBuilder::new()
    .with_initial_state(initial_state)
    .with_rule_set(rules)
    .with_context(context)
    .with_checkpoint_interval(1000) // Checkpoint every 1000 transactions
    .build()?;

let result = engine.replay(&transactions)?;
```

### Rule Migration Analysis

```rust
let result_v1 = engine_v1.replay(&transactions)?;
let result_v2 = engine_v2.replay(&transactions)?;

let comparator = ResultComparator::new();
let comparison = comparator.compare(&result_v1, &result_v2);

println!("State differences: {:?}", comparison.state_differences);
println!("Balance differences: {:?}", comparison.balance_differences);
```

### External Facts

```rust
let mut context = ExecutionContext::new(Utc::now(), 42);
context.add_external_fact("exchange_rate".to_string(), 1.25);
context.add_external_fact("fee_schedule".to_string(), fee_schedule);

let engine = ReplayEngineBuilder::new()
    .with_context(context)
    // ... other configuration
    .build()?;
```

### Deterministic Logging

```rust
use dtre::{DeterministicLogger, LogLevel};

let mut logger = DeterministicLogger::new();
logger.log(LogLevel::Info, "Processing transaction", &context);

// Logs don't affect determinism
let trace = logger.get_trace();
```

## Testing

The library includes comprehensive test coverage:

- **Unit Tests**: 49 tests covering core functionality
- **Integration Tests**: 13 tests for end-to-end scenarios
- **Property-Based Tests**: 36 properties tested with 100+ iterations each

Run tests:

```bash
cargo test --all
```

Run benchmarks:

```bash
cargo bench
```

## Performance

Benchmarks on a typical development machine:

- Small sequences (10-100 transactions): ~50-550 µs
- Large sequences (1000-10000 transactions): ~45-500 ms
- State hashing (1000 accounts): ~1-2 ms
- Parallel execution: Near-linear scaling with cores

## Examples

See the `examples/` directory for complete examples:

- `bank_transfer.rs` - Bank account transfers with fee calculation
- Integration tests demonstrate trading systems, audit trails, and rule migration

## Requirements

- Rust 1.70 or later
- Dependencies:
  - `serde` 1.0 - Serialization
  - `blake3` 1.5 - Cryptographic hashing
  - `chrono` 0.4 - Date/time handling
  - `rayon` 1.8 - Parallel execution
  - `bincode` 1.3 - Binary serialization

## License

This project is licensed under the MIT License.

## Contributing

Contributions are welcome! Please ensure:

1. All tests pass (`cargo test --all`)
2. Code is formatted (`cargo fmt`)
3. No clippy warnings (`cargo clippy`)
4. Property tests pass consistently

## Architecture

The DTRE follows a layered architecture:

1. **API Layer**: Builder pattern and fluent API
2. **Execution Layer**: Transaction processing and rule application
3. **Verification Layer**: State hashing and diff calculation
4. **Storage Layer**: Serialization and checkpoint management

All components are designed for determinism, immutability, and pure functional programming.

## Support

For issues, questions, or contributions, please visit the project repository.

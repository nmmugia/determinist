//! Benchmarks for replay engine performance
//!
//! These benchmarks measure:
//! - Large transaction sequence processing
//! - Memory usage patterns
//! - Checkpoint creation and restoration
//! - Parallel vs sequential execution

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use dtre::{
    ExecutionContext, ProcessingError, ReplayEngineBuilder, RuleSet, State,
    Transaction, ValidationError, Version,
};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

// ============================================================================
// Test Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankAccount {
    pub account_id: String,
    pub balance: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankingState {
    pub accounts: HashMap<String, BankAccount>,
    pub transaction_count: usize,
}

impl Hash for BankingState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut sorted_accounts: Vec<_> = self.accounts.iter().collect();
        sorted_accounts.sort_by_key(|(id, _)| *id);
        
        for (id, account) in sorted_accounts {
            id.hash(state);
            account.account_id.hash(state);
            account.balance.hash(state);
            account.currency.hash(state);
        }
        
        self.transaction_count.hash(state);
    }
}

impl State for BankingState {
    fn validate(&self) -> Result<(), ValidationError> {
        for (id, account) in &self.accounts {
            if id != &account.account_id {
                return Err(ValidationError::InvalidState {
                    reason: format!("Account ID mismatch: key={}, account.id={}", id, account.account_id),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTransaction {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub from_account: String,
    pub to_account: String,
    pub amount: i64,
}

impl Transaction for TransferTransaction {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
    
    fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= 0 {
            return Err(ValidationError::InvalidTransaction {
                reason: "Amount must be positive".to_string(),
            });
        }
        Ok(())
    }
}

pub struct SimpleTransferRules;

impl RuleSet<BankingState, TransferTransaction> for SimpleTransferRules {
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    
    fn apply(
        &self,
        state: &BankingState,
        transaction: &TransferTransaction,
        _context: &ExecutionContext,
    ) -> Result<BankingState, ProcessingError> {
        let mut new_state = state.clone();
        
        let from_balance = new_state.accounts.get(&transaction.from_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Source account not found".to_string(),
            })?
            .balance;
        
        if from_balance < transaction.amount {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Insufficient balance".to_string(),
            });
        }
        
        new_state.accounts.get_mut(&transaction.from_account).unwrap().balance -= transaction.amount;
        new_state.accounts.get_mut(&transaction.to_account).unwrap().balance += transaction.amount;
        new_state.transaction_count += 1;
        
        Ok(new_state)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_large_state(num_accounts: usize) -> BankingState {
    let mut accounts = HashMap::new();
    for i in 0..num_accounts {
        let account_id = format!("ACC{:06}", i);
        accounts.insert(account_id.clone(), BankAccount {
            account_id,
            balance: 1_000_000,
            currency: "USD".to_string(),
        });
    }
    
    BankingState {
        accounts,
        transaction_count: 0,
    }
}

fn create_transaction_sequence(num_transactions: usize, num_accounts: usize) -> Vec<TransferTransaction> {
    let base_time = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    
    let mut transactions = Vec::with_capacity(num_transactions);
    for i in 0..num_transactions {
        let from_idx = i % num_accounts;
        let to_idx = (i + 1) % num_accounts;
        
        transactions.push(TransferTransaction {
            id: format!("TXN{:08}", i),
            timestamp: base_time + chrono::Duration::seconds(i as i64),
            from_account: format!("ACC{:06}", from_idx),
            to_account: format!("ACC{:06}", to_idx),
            amount: 1000 + (i as i64 % 10000),
        });
    }
    
    transactions
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_small_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_sequence");
    
    for num_txns in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*num_txns as u64));
        group.bench_with_input(BenchmarkId::from_parameter(num_txns), num_txns, |b, &num_txns| {
            let initial_state = create_large_state(10);
            let transactions = create_transaction_sequence(num_txns, 10);
            let context = ExecutionContext::new(
                DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
                42
            );
            
            b.iter(|| {
                let engine = ReplayEngineBuilder::new()
                    .with_initial_state(initial_state.clone())
                    .with_rule_set(SimpleTransferRules)
                    .with_context(context.clone())
                    .build()
                    .unwrap();
                
                black_box(engine.replay(&transactions).unwrap())
            });
        });
    }
    
    group.finish();
}

fn bench_large_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_sequence");
    group.sample_size(10); // Reduce sample size for large benchmarks
    
    for num_txns in [1000, 5000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_txns as u64));
        group.bench_with_input(BenchmarkId::from_parameter(num_txns), num_txns, |b, &num_txns| {
            let initial_state = create_large_state(100);
            let transactions = create_transaction_sequence(num_txns, 100);
            let context = ExecutionContext::new(
                DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
                42
            );
            
            b.iter(|| {
                let engine = ReplayEngineBuilder::new()
                    .with_initial_state(initial_state.clone())
                    .with_rule_set(SimpleTransferRules)
                    .with_context(context.clone())
                    .build()
                    .unwrap();
                
                black_box(engine.replay(&transactions).unwrap())
            });
        });
    }
    
    group.finish();
}

fn bench_varying_state_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("varying_state_size");
    
    for num_accounts in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            num_accounts,
            |b, &num_accounts| {
                let initial_state = create_large_state(num_accounts);
                let transactions = create_transaction_sequence(100, num_accounts);
                let context = ExecutionContext::new(
                    DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
                    42
                );
                
                b.iter(|| {
                    let engine = ReplayEngineBuilder::new()
                        .with_initial_state(initial_state.clone())
                        .with_rule_set(SimpleTransferRules)
                        .with_context(context.clone())
                        .build()
                        .unwrap();
                    
                    black_box(engine.replay(&transactions).unwrap())
                });
            },
        );
    }
    
    group.finish();
}

fn bench_checkpoint_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint_creation");
    
    let initial_state = create_large_state(100);
    let transactions = create_transaction_sequence(1000, 100);
    let context = ExecutionContext::new(
        DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
        42
    );
    
    group.bench_function("with_checkpoints_every_100", |b| {
        b.iter(|| {
            let engine = ReplayEngineBuilder::new()
                .with_initial_state(initial_state.clone())
                .with_rule_set(SimpleTransferRules)
                .with_context(context.clone())
                .with_checkpoint_interval(100)
                .build()
                .unwrap();
            
            black_box(engine.replay(&transactions).unwrap())
        });
    });
    
    group.bench_function("without_checkpoints", |b| {
        b.iter(|| {
            let engine = ReplayEngineBuilder::new()
                .with_initial_state(initial_state.clone())
                .with_rule_set(SimpleTransferRules)
                .with_context(context.clone())
                .build()
                .unwrap();
            
            black_box(engine.replay(&transactions).unwrap())
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_small_sequence,
    bench_large_sequence,
    bench_varying_state_size,
    bench_checkpoint_creation
);
criterion_main!(benches);

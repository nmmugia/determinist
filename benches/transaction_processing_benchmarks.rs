//! Benchmarks for transaction processing performance
//!
//! These benchmarks measure:
//! - Transaction validation overhead
//! - Rule application performance
//! - State transition costs
//! - Memory allocation patterns

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use dtre::{
    ExecutionContext, ProcessingError, RuleSet, State, StateManager,
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
                    reason: format!("Account ID mismatch"),
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
        if self.from_account == self.to_account {
            return Err(ValidationError::InvalidTransaction {
                reason: "Cannot transfer to same account".to_string(),
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

pub struct ComplexTransferRules;

impl RuleSet<BankingState, TransferTransaction> for ComplexTransferRules {
    fn version(&self) -> Version {
        Version::new(2, 0, 0)
    }
    
    fn apply(
        &self,
        state: &BankingState,
        transaction: &TransferTransaction,
        _context: &ExecutionContext,
    ) -> Result<BankingState, ProcessingError> {
        let mut new_state = state.clone();
        
        // More complex validation
        let from_account = new_state.accounts.get(&transaction.from_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Source account not found".to_string(),
            })?;
        
        let to_account = new_state.accounts.get(&transaction.to_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Destination account not found".to_string(),
            })?;
        
        // Currency check
        if from_account.currency != to_account.currency {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Currency mismatch".to_string(),
            });
        }
        
        // Fee calculation
        let fee = std::cmp::max(50, transaction.amount / 100);
        let total_debit = transaction.amount + fee;
        
        if from_account.balance < total_debit {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Insufficient balance including fees".to_string(),
            });
        }
        
        new_state.accounts.get_mut(&transaction.from_account).unwrap().balance -= total_debit;
        new_state.accounts.get_mut(&transaction.to_account).unwrap().balance += transaction.amount;
        new_state.transaction_count += 1;
        
        Ok(new_state)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_state(num_accounts: usize) -> BankingState {
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

fn create_transaction(id: usize, num_accounts: usize) -> TransferTransaction {
    let base_time = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    
    let from_idx = id % num_accounts;
    let to_idx = (id + 1) % num_accounts;
    
    TransferTransaction {
        id: format!("TXN{:08}", id),
        timestamp: base_time + chrono::Duration::seconds(id as i64),
        from_account: format!("ACC{:06}", from_idx),
        to_account: format!("ACC{:06}", to_idx),
        amount: 1000 + (id as i64 % 10000),
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_transaction_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_validation");
    
    let transaction = create_transaction(0, 10);
    
    group.bench_function("validate", |b| {
        b.iter(|| {
            black_box(transaction.validate().unwrap())
        });
    });
    
    group.finish();
}

fn bench_rule_application(c: &mut Criterion) {
    let mut group = c.benchmark_group("rule_application");
    
    let state = create_state(100);
    let transaction = create_transaction(0, 100);
    let context = ExecutionContext::new(
        DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
        42
    );
    
    group.bench_function("simple_rules", |b| {
        let rules = SimpleTransferRules;
        b.iter(|| {
            black_box(rules.apply(&state, &transaction, &context).unwrap())
        });
    });
    
    group.bench_function("complex_rules", |b| {
        let rules = ComplexTransferRules;
        b.iter(|| {
            black_box(rules.apply(&state, &transaction, &context).unwrap())
        });
    });
    
    group.finish();
}

fn bench_state_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transition");
    
    for num_accounts in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            num_accounts,
            |b, &num_accounts| {
                let initial_state = create_state(num_accounts);
                let transaction = create_transaction(0, num_accounts);
                let context = ExecutionContext::new(
                    DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
                    42
                );
                let rules = SimpleTransferRules;
                
                b.iter(|| {
                    let mut manager = StateManager::new(initial_state.clone());
                    black_box(manager.apply_transaction(&transaction, &rules, &context).unwrap())
                });
            },
        );
    }
    
    group.finish();
}

fn bench_state_cloning(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_cloning");
    
    for num_accounts in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_accounts as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            num_accounts,
            |b, &num_accounts| {
                let state = create_state(num_accounts);
                
                b.iter(|| {
                    black_box(state.clone())
                });
            },
        );
    }
    
    group.finish();
}

fn bench_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");
    group.sample_size(10);
    
    for batch_size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let initial_state = create_state(100);
                let transactions: Vec<_> = (0..batch_size)
                    .map(|i| create_transaction(i, 100))
                    .collect();
                let context = ExecutionContext::new(
                    DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
                    42
                );
                let rules = SimpleTransferRules;
                
                b.iter(|| {
                    let mut manager = StateManager::new(initial_state.clone());
                    for txn in &transactions {
                        manager.apply_transaction(txn, &rules, &context).unwrap();
                    }
                    black_box(manager)
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_transaction_validation,
    bench_rule_application,
    bench_state_transition,
    bench_state_cloning,
    bench_batch_processing
);
criterion_main!(benches);

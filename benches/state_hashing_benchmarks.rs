//! Benchmarks for state hashing performance
//!
//! These benchmarks measure:
//! - Hash computation for various state sizes
//! - Hash chain operations
//! - Serialization overhead

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

use dtre::{State, StateHasher, ValidationError};
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
        Ok(())
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
            balance: 1_000_000 + (i as i64 * 1000),
            currency: "USD".to_string(),
        });
    }
    
    BankingState {
        accounts,
        transaction_count: 0,
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_hash_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_computation");
    
    for num_accounts in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_accounts as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            num_accounts,
            |b, &num_accounts| {
                let state = create_state(num_accounts);
                let hasher = StateHasher::new();
                
                b.iter(|| {
                    black_box(hasher.hash(&state))
                });
            },
        );
    }
    
    group.finish();
}

fn bench_hash_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_chain");
    
    let hasher = StateHasher::new();
    
    for chain_length in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*chain_length as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(chain_length),
            chain_length,
            |b, &chain_length| {
                let state = create_state(100);
                let hashes: Vec<_> = (0..chain_length)
                    .map(|_| hasher.hash(&state))
                    .collect();
                
                b.iter(|| {
                    black_box(hasher.hash_chain(&hashes))
                });
            },
        );
    }
    
    group.finish();
}

fn bench_serialization_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization_overhead");
    
    for num_accounts in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            num_accounts,
            |b, &num_accounts| {
                let state = create_state(num_accounts);
                
                b.iter(|| {
                    black_box(bincode::serialize(&state).unwrap())
                });
            },
        );
    }
    
    group.finish();
}

fn bench_hash_vs_equality(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_vs_equality");
    
    let state1 = create_state(1000);
    let state2 = create_state(1000);
    let hasher = StateHasher::new();
    
    group.bench_function("hash_comparison", |b| {
        let hash1 = hasher.hash(&state1);
        let hash2 = hasher.hash(&state2);
        
        b.iter(|| {
            black_box(hash1 == hash2)
        });
    });
    
    group.bench_function("direct_equality", |b| {
        b.iter(|| {
            black_box(state1 == state2)
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_hash_computation,
    bench_hash_chain,
    bench_serialization_overhead,
    bench_hash_vs_equality
);
criterion_main!(benches);

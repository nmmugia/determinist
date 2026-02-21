//! Bank Transfer Example
//!
//! This example demonstrates a realistic banking system with:
//! - Account state management with balances and transaction history
//! - Transfer transactions with validation rules
//! - Rule evolution scenarios (fee changes, transfer limits)
//! - Audit trail and compliance verification
//! - Deterministic replay across different rule versions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use dtre::{
    ExecutionContext, ProcessingError, ReplayEngineBuilder, RuleSet, State,
    Transaction, ValidationError, Version,
};

// ============================================================================
// State Model
// ============================================================================

/// Bank account state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankAccount {
    pub account_id: String,
    pub balance: i64,
    pub currency: String,
    pub status: AccountStatus,
}

/// Account status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountStatus {
    Active,
    Frozen,
    Closed,
}

/// Complete banking system state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankingState {
    pub accounts: HashMap<String, BankAccount>,
    pub transaction_history: Vec<TransactionRecord>,
    pub total_fees_collected: i64,
}

/// Record of a processed transaction for audit trail
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionRecord {
    pub transaction_id: String,
    pub timestamp: DateTime<Utc>,
    pub from_account: String,
    pub to_account: String,
    pub amount: i64,
    pub fee: i64,
    pub rule_version: Version,
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
            format!("{:?}", account.status).hash(state);
        }
        
        for record in &self.transaction_history {
            record.transaction_id.hash(state);
            record.timestamp.timestamp().hash(state);
            record.from_account.hash(state);
            record.to_account.hash(state);
            record.amount.hash(state);
            record.fee.hash(state);
        }
        
        self.total_fees_collected.hash(state);
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
            
            if account.currency.is_empty() {
                return Err(ValidationError::InvalidState {
                    reason: "Currency cannot be empty".to_string(),
                });
            }
        }
        
        if self.total_fees_collected < 0 {
            return Err(ValidationError::InvalidState {
                reason: "Total fees cannot be negative".to_string(),
            });
        }
        
        Ok(())
    }
}

// ============================================================================
// Transaction Model
// ============================================================================

/// Transfer transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTransaction {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub from_account: String,
    pub to_account: String,
    pub amount: i64,
    pub currency: String,
    pub description: String,
}

impl Transaction for TransferTransaction {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
    
    fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_empty() {
            return Err(ValidationError::InvalidTransaction {
                reason: "Transaction ID cannot be empty".to_string(),
            });
        }
        
        if self.from_account.is_empty() {
            return Err(ValidationError::InvalidTransaction {
                reason: "From account cannot be empty".to_string(),
            });
        }
        
        if self.to_account.is_empty() {
            return Err(ValidationError::InvalidTransaction {
                reason: "To account cannot be empty".to_string(),
            });
        }
        
        if self.from_account == self.to_account {
            return Err(ValidationError::InvalidTransaction {
                reason: "Cannot transfer to the same account".to_string(),
            });
        }
        
        if self.amount <= 0 {
            return Err(ValidationError::InvalidTransaction {
                reason: "Amount must be positive".to_string(),
            });
        }
        
        if self.currency.is_empty() {
            return Err(ValidationError::InvalidTransaction {
                reason: "Currency cannot be empty".to_string(),
            });
        }
        
        Ok(())
    }
}

// ============================================================================
// Rule Sets - Version Evolution
// ============================================================================

/// Version 1.0.0: Basic transfer rules with fixed fee
pub struct TransferRulesV1;

impl RuleSet<BankingState, TransferTransaction> for TransferRulesV1 {
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    
    fn apply(
        &self,
        state: &BankingState,
        transaction: &TransferTransaction,
        _context: &ExecutionContext,
    ) -> Result<BankingState, ProcessingError> {
        transaction.validate().map_err(|e| ProcessingError::TransactionFailed {
            transaction_id: transaction.id.clone(),
            reason: format!("{:?}", e),
        })?;
        
        let mut new_state = state.clone();
        
        let from_account = new_state.accounts.get(&transaction.from_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Source account {} not found", transaction.from_account),
            })?;
        
        let to_account = new_state.accounts.get(&transaction.to_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Destination account {} not found", transaction.to_account),
            })?;
        
        if from_account.status != AccountStatus::Active {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Source account {} is not active", transaction.from_account),
            });
        }
        
        if to_account.status != AccountStatus::Active {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Destination account {} is not active", transaction.to_account),
            });
        }
        
        if from_account.currency != transaction.currency || to_account.currency != transaction.currency {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Currency mismatch".to_string(),
            });
        }
        
        let fee = 100;
        let total_debit = transaction.amount + fee;
        
        if from_account.balance < total_debit {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!(
                    "Insufficient balance: have {}, need {}",
                    from_account.balance, total_debit
                ),
            });
        }
        
        new_state.accounts.get_mut(&transaction.from_account).unwrap().balance -= total_debit;
        new_state.accounts.get_mut(&transaction.to_account).unwrap().balance += transaction.amount;
        new_state.total_fees_collected += fee;
        
        new_state.transaction_history.push(TransactionRecord {
            transaction_id: transaction.id.clone(),
            timestamp: transaction.timestamp,
            from_account: transaction.from_account.clone(),
            to_account: transaction.to_account.clone(),
            amount: transaction.amount,
            fee,
            rule_version: self.version(),
        });
        
        Ok(new_state)
    }
}

/// Version 1.1.0: Percentage-based fee
pub struct TransferRulesV1_1;

impl RuleSet<BankingState, TransferTransaction> for TransferRulesV1_1 {
    fn version(&self) -> Version {
        Version::new(1, 1, 0)
    }
    
    fn apply(
        &self,
        state: &BankingState,
        transaction: &TransferTransaction,
        _context: &ExecutionContext,
    ) -> Result<BankingState, ProcessingError> {
        transaction.validate().map_err(|e| ProcessingError::TransactionFailed {
            transaction_id: transaction.id.clone(),
            reason: format!("{:?}", e),
        })?;
        
        let mut new_state = state.clone();
        
        let from_account = new_state.accounts.get(&transaction.from_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Source account {} not found", transaction.from_account),
            })?;
        
        let to_account = new_state.accounts.get(&transaction.to_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Destination account {} not found", transaction.to_account),
            })?;
        
        if from_account.status != AccountStatus::Active {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Source account {} is not active", transaction.from_account),
            });
        }
        
        if to_account.status != AccountStatus::Active {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Destination account {} is not active", transaction.to_account),
            });
        }
        
        if from_account.currency != transaction.currency || to_account.currency != transaction.currency {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Currency mismatch".to_string(),
            });
        }
        
        let fee = std::cmp::max(50, transaction.amount / 100);
        let total_debit = transaction.amount + fee;
        
        if from_account.balance < total_debit {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!(
                    "Insufficient balance: have {}, need {}",
                    from_account.balance, total_debit
                ),
            });
        }
        
        new_state.accounts.get_mut(&transaction.from_account).unwrap().balance -= total_debit;
        new_state.accounts.get_mut(&transaction.to_account).unwrap().balance += transaction.amount;
        new_state.total_fees_collected += fee;
        
        new_state.transaction_history.push(TransactionRecord {
            transaction_id: transaction.id.clone(),
            timestamp: transaction.timestamp,
            from_account: transaction.from_account.clone(),
            to_account: transaction.to_account.clone(),
            amount: transaction.amount,
            fee,
            rule_version: self.version(),
        });
        
        Ok(new_state)
    }
}

/// Version 2.0.0: Transfer limits and tiered fees
pub struct TransferRulesV2;

impl RuleSet<BankingState, TransferTransaction> for TransferRulesV2 {
    fn version(&self) -> Version {
        Version::new(2, 0, 0)
    }
    
    fn apply(
        &self,
        state: &BankingState,
        transaction: &TransferTransaction,
        _context: &ExecutionContext,
    ) -> Result<BankingState, ProcessingError> {
        transaction.validate().map_err(|e| ProcessingError::TransactionFailed {
            transaction_id: transaction.id.clone(),
            reason: format!("{:?}", e),
        })?;
        
        let mut new_state = state.clone();
        
        let from_account = new_state.accounts.get(&transaction.from_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Source account {} not found", transaction.from_account),
            })?;
        
        let to_account = new_state.accounts.get(&transaction.to_account)
            .ok_or_else(|| ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Destination account {} not found", transaction.to_account),
            })?;
        
        if from_account.status != AccountStatus::Active {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Source account {} is not active", transaction.from_account),
            });
        }
        
        if to_account.status != AccountStatus::Active {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!("Destination account {} is not active", transaction.to_account),
            });
        }
        
        if from_account.currency != transaction.currency || to_account.currency != transaction.currency {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: "Currency mismatch".to_string(),
            });
        }
        
        if transaction.amount > 1_000_000 {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!(
                    "Transfer amount {} exceeds limit of 1,000,000",
                    transaction.amount
                ),
            });
        }
        
        let fee = if transaction.amount < 10_000 {
            50
        } else if transaction.amount < 100_000 {
            transaction.amount / 100
        } else {
            transaction.amount / 200
        };
        
        let total_debit = transaction.amount + fee;
        
        if from_account.balance < total_debit {
            return Err(ProcessingError::TransactionFailed {
                transaction_id: transaction.id.clone(),
                reason: format!(
                    "Insufficient balance: have {}, need {}",
                    from_account.balance, total_debit
                ),
            });
        }
        
        new_state.accounts.get_mut(&transaction.from_account).unwrap().balance -= total_debit;
        new_state.accounts.get_mut(&transaction.to_account).unwrap().balance += transaction.amount;
        new_state.total_fees_collected += fee;
        
        new_state.transaction_history.push(TransactionRecord {
            transaction_id: transaction.id.clone(),
            timestamp: transaction.timestamp,
            from_account: transaction.from_account.clone(),
            to_account: transaction.to_account.clone(),
            amount: transaction.amount,
            fee,
            rule_version: self.version(),
        });
        
        Ok(new_state)
    }
}

// ============================================================================
// Example Usage
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Bank Transfer Example ===\n");
    
    // Create initial state with accounts
    let initial_state = BankingState {
        accounts: {
            let mut accounts = HashMap::new();
            accounts.insert("ACC001".to_string(), BankAccount {
                account_id: "ACC001".to_string(),
                balance: 100_000,
                currency: "USD".to_string(),
                status: AccountStatus::Active,
            });
            accounts.insert("ACC002".to_string(), BankAccount {
                account_id: "ACC002".to_string(),
                balance: 50_000,
                currency: "USD".to_string(),
                status: AccountStatus::Active,
            });
            accounts.insert("ACC003".to_string(), BankAccount {
                account_id: "ACC003".to_string(),
                balance: 200_000,
                currency: "USD".to_string(),
                status: AccountStatus::Active,
            });
            accounts
        },
        transaction_history: Vec::new(),
        total_fees_collected: 0,
    };
    
    println!("Initial State:");
    println!("  ACC001: ${:.2}", initial_state.accounts["ACC001"].balance as f64 / 100.0);
    println!("  ACC002: ${:.2}", initial_state.accounts["ACC002"].balance as f64 / 100.0);
    println!("  ACC003: ${:.2}", initial_state.accounts["ACC003"].balance as f64 / 100.0);
    println!("  Total Fees: ${:.2}\n", initial_state.total_fees_collected as f64 / 100.0);
    
    // Create transaction sequence
    let base_time = Utc::now();
    let transactions = vec![
        TransferTransaction {
            id: "TXN001".to_string(),
            timestamp: base_time,
            from_account: "ACC001".to_string(),
            to_account: "ACC002".to_string(),
            amount: 10_000,
            currency: "USD".to_string(),
            description: "Payment for services".to_string(),
        },
        TransferTransaction {
            id: "TXN002".to_string(),
            timestamp: base_time + chrono::Duration::seconds(60),
            from_account: "ACC002".to_string(),
            to_account: "ACC003".to_string(),
            amount: 25_000,
            currency: "USD".to_string(),
            description: "Rent payment".to_string(),
        },
        TransferTransaction {
            id: "TXN003".to_string(),
            timestamp: base_time + chrono::Duration::seconds(120),
            from_account: "ACC003".to_string(),
            to_account: "ACC001".to_string(),
            amount: 50_000,
            currency: "USD".to_string(),
            description: "Refund".to_string(),
        },
    ];
    
    println!("Transaction Sequence:");
    for txn in &transactions {
        println!("  {} -> {}: ${:.2}", 
            txn.from_account, txn.to_account, txn.amount as f64 / 100.0);
    }
    println!();
    
    // Replay with Version 1.0.0 (fixed fee)
    println!("=== Replay with Version 1.0.0 (Fixed $1.00 fee) ===");
    let rule_set_v1 = TransferRulesV1;
    let engine_v1 = ReplayEngineBuilder::new()
        .with_initial_state(initial_state.clone())
        .with_rule_set(rule_set_v1)
        .with_time_and_seed(base_time, 12345)
        .build()?;
    
    let result_v1 = engine_v1.replay(&transactions)?;
    
    println!("Final State (v1.0.0):");
    println!("  ACC001: ${:.2}", result_v1.final_state.accounts["ACC001"].balance as f64 / 100.0);
    println!("  ACC002: ${:.2}", result_v1.final_state.accounts["ACC002"].balance as f64 / 100.0);
    println!("  ACC003: ${:.2}", result_v1.final_state.accounts["ACC003"].balance as f64 / 100.0);
    println!("  Total Fees: ${:.2}", result_v1.final_state.total_fees_collected as f64 / 100.0);
    println!("  Final Hash: {}", result_v1.final_hash);
    println!("  Transactions Processed: {}\n", result_v1.execution_trace.transactions_processed);
    
    // Replay with Version 1.1.0 (percentage fee)
    println!("=== Replay with Version 1.1.0 (1% fee, min $0.50) ===");
    let rule_set_v1_1 = TransferRulesV1_1;
    let engine_v1_1 = ReplayEngineBuilder::new()
        .with_initial_state(initial_state.clone())
        .with_rule_set(rule_set_v1_1)
        .with_time_and_seed(base_time, 12345)
        .build()?;
    
    let result_v1_1 = engine_v1_1.replay(&transactions)?;
    
    println!("Final State (v1.1.0):");
    println!("  ACC001: ${:.2}", result_v1_1.final_state.accounts["ACC001"].balance as f64 / 100.0);
    println!("  ACC002: ${:.2}", result_v1_1.final_state.accounts["ACC002"].balance as f64 / 100.0);
    println!("  ACC003: ${:.2}", result_v1_1.final_state.accounts["ACC003"].balance as f64 / 100.0);
    println!("  Total Fees: ${:.2}", result_v1_1.final_state.total_fees_collected as f64 / 100.0);
    println!("  Final Hash: {}", result_v1_1.final_hash);
    println!("  Transactions Processed: {}\n", result_v1_1.execution_trace.transactions_processed);
    
    // Replay with Version 2.0.0 (tiered fees and limits)
    println!("=== Replay with Version 2.0.0 (Tiered fees + limits) ===");
    let rule_set_v2 = TransferRulesV2;
    let engine_v2 = ReplayEngineBuilder::new()
        .with_initial_state(initial_state.clone())
        .with_rule_set(rule_set_v2)
        .with_time_and_seed(base_time, 12345)
        .build()?;
    
    let result_v2 = engine_v2.replay(&transactions)?;
    
    println!("Final State (v2.0.0):");
    println!("  ACC001: ${:.2}", result_v2.final_state.accounts["ACC001"].balance as f64 / 100.0);
    println!("  ACC002: ${:.2}", result_v2.final_state.accounts["ACC002"].balance as f64 / 100.0);
    println!("  ACC003: ${:.2}", result_v2.final_state.accounts["ACC003"].balance as f64 / 100.0);
    println!("  Total Fees: ${:.2}", result_v2.final_state.total_fees_collected as f64 / 100.0);
    println!("  Final Hash: {}", result_v2.final_hash);
    println!("  Transactions Processed: {}\n", result_v2.execution_trace.transactions_processed);
    
    // Compare fee differences
    println!("=== Fee Comparison ===");
    println!("  v1.0.0 fees: ${:.2}", result_v1.final_state.total_fees_collected as f64 / 100.0);
    println!("  v1.1.0 fees: ${:.2}", result_v1_1.final_state.total_fees_collected as f64 / 100.0);
    println!("  v2.0.0 fees: ${:.2}", result_v2.final_state.total_fees_collected as f64 / 100.0);
    
    let fee_diff_v1_v1_1 = result_v1_1.final_state.total_fees_collected - result_v1.final_state.total_fees_collected;
    let fee_diff_v1_1_v2 = result_v2.final_state.total_fees_collected - result_v1_1.final_state.total_fees_collected;
    
    println!("  v1.0.0 -> v1.1.0 difference: ${:.2}", fee_diff_v1_v1_1 as f64 / 100.0);
    println!("  v1.1.0 -> v2.0.0 difference: ${:.2}\n", fee_diff_v1_1_v2 as f64 / 100.0);
    
    // Audit trail demonstration
    println!("=== Audit Trail (v1.0.0) ===");
    for record in &result_v1.final_state.transaction_history {
        println!("  {} | {} -> {} | Amount: ${:.2} | Fee: ${:.2} | Version: {}",
            record.transaction_id,
            record.from_account,
            record.to_account,
            record.amount as f64 / 100.0,
            record.fee as f64 / 100.0,
            record.rule_version,
        );
    }
    println!();
    
    // Demonstrate determinism
    println!("=== Determinism Verification ===");
    let result_v1_replay = engine_v1.replay(&transactions)?;
    println!("  First replay hash:  {}", result_v1.final_hash);
    println!("  Second replay hash: {}", result_v1_replay.final_hash);
    println!("  Hashes match: {}", result_v1.final_hash == result_v1_replay.final_hash);
    println!("  States match: {}\n", result_v1.final_state == result_v1_replay.final_state);
    
    // Demonstrate compliance verification
    println!("=== Compliance Verification ===");
    println!("  Total transactions: {}", result_v1.final_state.transaction_history.len());
    println!("  All transactions recorded: {}", 
        result_v1.final_state.transaction_history.len() == transactions.len());
    
    // Verify balance conservation
    let initial_total: i64 = initial_state.accounts.values().map(|a| a.balance).sum();
    let final_total: i64 = result_v1.final_state.accounts.values().map(|a| a.balance).sum();
    let expected_final = initial_total - result_v1.final_state.total_fees_collected;
    
    println!("  Initial total balance: ${:.2}", initial_total as f64 / 100.0);
    println!("  Final total balance: ${:.2}", final_total as f64 / 100.0);
    println!("  Total fees collected: ${:.2}", result_v1.final_state.total_fees_collected as f64 / 100.0);
    println!("  Balance conservation verified: {}", final_total == expected_final);
    
    Ok(())
}

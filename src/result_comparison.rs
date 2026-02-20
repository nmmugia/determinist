//! Result comparison and analysis tools for replay results

use crate::hasher::StateHasher;
use crate::traits::State;
use crate::types::{
    ImpactAnalysis, ReplayResult, StateDifference, StateHash, StateTransitionInfo, Version,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Comprehensive comparison of two replay results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultComparison<S> {
    pub baseline_result: ReplayResult<S>,
    pub comparison_result: ReplayResult<S>,
    pub final_state_matches: bool,
    pub final_hash_matches: bool,
    pub transaction_count_matches: bool,
    pub state_differences: Vec<TransitionDifference>,
    pub performance_comparison: PerformanceComparison,
}

/// Difference between two state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDifference {
    pub transaction_index: usize,
    pub transaction_id: String,
    pub baseline_hash: StateHash,
    pub comparison_hash: StateHash,
    pub hashes_match: bool,
}

/// Comparison of performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceComparison {
    pub baseline_duration_ms: u64,
    pub comparison_duration_ms: u64,
    pub duration_difference_ms: i64,
    pub baseline_tps: f64,
    pub comparison_tps: f64,
    pub tps_difference: f64,
}

/// Detailed state field comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldComparison {
    pub field_name: String,
    pub baseline_value: String,
    pub comparison_value: String,
    pub values_match: bool,
}

/// Balance difference analysis for financial states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceDifference {
    pub account_id: String,
    pub baseline_balance: i64,
    pub comparison_balance: i64,
    pub difference: i64,
}

/// Result comparison analyzer
pub struct ResultComparator {
    hasher: StateHasher,
}

impl ResultComparator {
    /// Create a new result comparator
    pub fn new() -> Self {
        Self {
            hasher: StateHasher::new(),
        }
    }

    /// Compare two replay results comprehensively
    pub fn compare<S: State>(
        &self,
        baseline: ReplayResult<S>,
        comparison: ReplayResult<S>,
    ) -> ResultComparison<S> {
        let final_state_matches = self.hasher.hash(&baseline.final_state)
            == self.hasher.hash(&comparison.final_state);
        let final_hash_matches = baseline.final_hash == comparison.final_hash;
        let transaction_count_matches = baseline.execution_trace.transactions_processed
            == comparison.execution_trace.transactions_processed;

        let state_differences = self.compare_transitions(
            &baseline.execution_trace.state_transitions,
            &comparison.execution_trace.state_transitions,
        );

        let performance_comparison = PerformanceComparison {
            baseline_duration_ms: baseline.performance_metrics.total_duration_ms,
            comparison_duration_ms: comparison.performance_metrics.total_duration_ms,
            duration_difference_ms: comparison.performance_metrics.total_duration_ms as i64
                - baseline.performance_metrics.total_duration_ms as i64,
            baseline_tps: baseline.performance_metrics.transactions_per_second,
            comparison_tps: comparison.performance_metrics.transactions_per_second,
            tps_difference: comparison.performance_metrics.transactions_per_second
                - baseline.performance_metrics.transactions_per_second,
        };

        ResultComparison {
            baseline_result: baseline,
            comparison_result: comparison,
            final_state_matches,
            final_hash_matches,
            transaction_count_matches,
            state_differences,
            performance_comparison,
        }
    }

    /// Compare state transitions between two executions
    fn compare_transitions(
        &self,
        baseline: &[StateTransitionInfo],
        comparison: &[StateTransitionInfo],
    ) -> Vec<TransitionDifference> {
        let mut differences = Vec::new();
        let max_len = baseline.len().max(comparison.len());

        for i in 0..max_len {
            let baseline_transition = baseline.get(i);
            let comparison_transition = comparison.get(i);

            match (baseline_transition, comparison_transition) {
                (Some(b), Some(c)) => {
                    let hashes_match = b.to_hash == c.to_hash;
                    differences.push(TransitionDifference {
                        transaction_index: i,
                        transaction_id: b.transaction_id.clone(),
                        baseline_hash: b.to_hash,
                        comparison_hash: c.to_hash,
                        hashes_match,
                    });
                }
                (Some(b), None) => {
                    differences.push(TransitionDifference {
                        transaction_index: i,
                        transaction_id: b.transaction_id.clone(),
                        baseline_hash: b.to_hash,
                        comparison_hash: StateHash([0; 32]),
                        hashes_match: false,
                    });
                }
                (None, Some(c)) => {
                    differences.push(TransitionDifference {
                        transaction_index: i,
                        transaction_id: c.transaction_id.clone(),
                        baseline_hash: StateHash([0; 32]),
                        comparison_hash: c.to_hash,
                        hashes_match: false,
                    });
                }
                (None, None) => break,
            }
        }

        differences
    }

    /// Create an impact analysis from two replay results with different rule versions
    pub fn create_impact_analysis<S: State>(
        &self,
        baseline_version: Version,
        comparison_version: Version,
        baseline_result: ReplayResult<S>,
        comparison_result: ReplayResult<S>,
    ) -> ImpactAnalysis<S> {
        let identical_final_hash = baseline_result.final_hash == comparison_result.final_hash;
        let identical_final_state = self.hasher.hash(&baseline_result.final_state)
            == self.hasher.hash(&comparison_result.final_state);

        let differences = self.find_state_differences(
            &baseline_result.execution_trace.state_transitions,
            &comparison_result.execution_trace.state_transitions,
        );

        ImpactAnalysis {
            baseline_version,
            comparison_version,
            baseline_result,
            comparison_result,
            differences,
            identical_final_state,
            identical_final_hash,
        }
    }

    /// Find differences between state transitions
    fn find_state_differences(
        &self,
        baseline: &[StateTransitionInfo],
        comparison: &[StateTransitionInfo],
    ) -> Vec<StateDifference> {
        let mut differences = Vec::new();

        for (i, (b, c)) in baseline.iter().zip(comparison.iter()).enumerate() {
            if b.to_hash != c.to_hash {
                differences.push(StateDifference {
                    transaction_id: b.transaction_id.clone(),
                    transaction_index: i,
                    baseline_hash: b.to_hash,
                    comparison_hash: c.to_hash,
                    description: format!(
                        "State diverged at transaction {} ({})",
                        i, b.transaction_id
                    ),
                });
            }
        }

        differences
    }
}

impl Default for ResultComparator {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> ResultComparison<S> {
    /// Check if the results are identical
    pub fn are_identical(&self) -> bool {
        self.final_state_matches
            && self.final_hash_matches
            && self.transaction_count_matches
            && self.state_differences.iter().all(|d| d.hashes_match)
    }

    /// Get the number of divergent transitions
    pub fn divergent_transition_count(&self) -> usize {
        self.state_differences
            .iter()
            .filter(|d| !d.hashes_match)
            .count()
    }

    /// Get the first transaction where states diverged
    pub fn first_divergence(&self) -> Option<&TransitionDifference> {
        self.state_differences.iter().find(|d| !d.hashes_match)
    }

    /// Generate a summary report
    pub fn summary(&self) -> String {
        if self.are_identical() {
            "Results are identical".to_string()
        } else {
            let mut parts = Vec::new();

            if !self.final_state_matches {
                parts.push("final states differ".to_string());
            }
            if !self.final_hash_matches {
                parts.push("final hashes differ".to_string());
            }
            if !self.transaction_count_matches {
                parts.push("transaction counts differ".to_string());
            }

            let divergent_count = self.divergent_transition_count();
            if divergent_count > 0 {
                parts.push(format!("{} transitions diverged", divergent_count));
            }

            format!("Results differ: {}", parts.join(", "))
        }
    }
}

/// Diff analyzer for detailed state comparison
pub struct DiffAnalyzer;

impl DiffAnalyzer {
    /// Analyze balance differences between two states
    /// This is a helper for financial applications
    pub fn analyze_balance_differences(
        baseline_balances: &HashMap<String, i64>,
        comparison_balances: &HashMap<String, i64>,
    ) -> Vec<BalanceDifference> {
        let mut differences = Vec::new();

        // Check all accounts in baseline
        for (account_id, &baseline_balance) in baseline_balances {
            let comparison_balance = comparison_balances.get(account_id).copied().unwrap_or(0);
            let difference = comparison_balance - baseline_balance;

            if difference != 0 {
                differences.push(BalanceDifference {
                    account_id: account_id.clone(),
                    baseline_balance,
                    comparison_balance,
                    difference,
                });
            }
        }

        // Check accounts only in comparison
        for (account_id, &comparison_balance) in comparison_balances {
            if !baseline_balances.contains_key(account_id) {
                differences.push(BalanceDifference {
                    account_id: account_id.clone(),
                    baseline_balance: 0,
                    comparison_balance,
                    difference: comparison_balance,
                });
            }
        }

        differences
    }

    /// Calculate total balance difference
    pub fn total_balance_difference(differences: &[BalanceDifference]) -> i64 {
        differences.iter().map(|d| d.difference).sum()
    }

    /// Find accounts with largest differences
    pub fn largest_differences(
        differences: &[BalanceDifference],
        limit: usize,
    ) -> Vec<BalanceDifference> {
        let mut sorted = differences.to_vec();
        sorted.sort_by_key(|d| -d.difference.abs());
        sorted.into_iter().take(limit).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ValidationError;
    use crate::traits::State;
    use crate::types::{ExecutionTrace, PerformanceMetrics};
    use std::hash::{Hash, Hasher};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        balance: i64,
    }

    impl Hash for TestState {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.balance.hash(state);
        }
    }

    impl State for TestState {
        fn validate(&self) -> Result<(), ValidationError> {
            Ok(())
        }
    }

    fn create_test_result(balance: i64, tx_count: usize) -> ReplayResult<TestState> {
        let hasher = StateHasher::new();
        let state = TestState { balance };
        let hash = hasher.hash(&state);

        ReplayResult {
            final_state: state,
            final_hash: hash,
            execution_trace: ExecutionTrace {
                transactions_processed: tx_count,
                state_transitions: vec![],
                rule_applications: vec![],
                checkpoints: vec![],
            },
            performance_metrics: PerformanceMetrics {
                total_duration_ms: 100,
                transactions_per_second: 10.0,
                average_transaction_time_ms: 10.0,
            },
        }
    }

    #[test]
    fn test_identical_results() {
        let comparator = ResultComparator::new();
        let result1 = create_test_result(100, 5);
        let result2 = create_test_result(100, 5);

        let comparison = comparator.compare(result1, result2);

        assert!(comparison.are_identical());
        assert!(comparison.final_state_matches);
        assert!(comparison.final_hash_matches);
        assert_eq!(comparison.divergent_transition_count(), 0);
    }

    #[test]
    fn test_different_results() {
        let comparator = ResultComparator::new();
        let result1 = create_test_result(100, 5);
        let result2 = create_test_result(150, 5);

        let comparison = comparator.compare(result1, result2);

        assert!(!comparison.are_identical());
        assert!(!comparison.final_state_matches);
        assert!(!comparison.final_hash_matches);
    }

    #[test]
    fn test_balance_difference_analysis() {
        let mut baseline = HashMap::new();
        baseline.insert("account1".to_string(), 100);
        baseline.insert("account2".to_string(), 200);

        let mut comparison = HashMap::new();
        comparison.insert("account1".to_string(), 150);
        comparison.insert("account2".to_string(), 200);
        comparison.insert("account3".to_string(), 50);

        let differences = DiffAnalyzer::analyze_balance_differences(&baseline, &comparison);

        assert_eq!(differences.len(), 2); // account1 and account3 differ
        assert_eq!(DiffAnalyzer::total_balance_difference(&differences), 100);
    }

    #[test]
    fn test_largest_differences() {
        let differences = vec![
            BalanceDifference {
                account_id: "a1".to_string(),
                baseline_balance: 100,
                comparison_balance: 150,
                difference: 50,
            },
            BalanceDifference {
                account_id: "a2".to_string(),
                baseline_balance: 200,
                comparison_balance: 100,
                difference: -100,
            },
            BalanceDifference {
                account_id: "a3".to_string(),
                baseline_balance: 50,
                comparison_balance: 60,
                difference: 10,
            },
        ];

        let largest = DiffAnalyzer::largest_differences(&differences, 2);
        assert_eq!(largest.len(), 2);
        assert_eq!(largest[0].account_id, "a2"); // -100 has largest absolute value
        assert_eq!(largest[1].account_id, "a1"); // 50 is second largest
    }

    #[test]
    fn test_impact_analysis_creation() {
        let comparator = ResultComparator::new();
        let baseline_version = Version::new(1, 0, 0);
        let comparison_version = Version::new(1, 1, 0);
        let result1 = create_test_result(100, 5);
        let result2 = create_test_result(100, 5);

        let analysis = comparator.create_impact_analysis(
            baseline_version.clone(),
            comparison_version.clone(),
            result1,
            result2,
        );

        assert_eq!(analysis.baseline_version, baseline_version);
        assert_eq!(analysis.comparison_version, comparison_version);
        assert!(analysis.is_safe_migration());
    }
}

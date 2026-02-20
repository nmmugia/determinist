use dtre::{
    BalanceDifference, DiffAnalyzer, ReplayResult, ResultComparator, State,
    ValidationError,
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// Test state for property testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    balance: i64,
    count: u32,
}

impl Hash for TestState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.balance.hash(state);
        self.count.hash(state);
    }
}

impl State for TestState {
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

// Helper to create a replay result
fn create_replay_result(balance: i64, count: u32, tx_count: usize) -> ReplayResult<TestState> {
    use dtre::{ExecutionTrace, PerformanceMetrics, StateHasher};

    let state = TestState { balance, count };
    let hasher = StateHasher::new();
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

// Generator for arbitrary test states
fn arbitrary_test_state() -> impl Strategy<Value = TestState> {
    (-1000000i64..1000000i64, 0u32..1000000u32).prop_map(|(balance, count)| TestState {
        balance,
        count,
    })
}

// Generator for balance maps
fn arbitrary_balance_map() -> impl Strategy<Value = HashMap<String, i64>> {
    prop::collection::hash_map("[a-z]{3,10}", -1000000i64..1000000i64, 0..20)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: deterministic-transaction-replay-engine, Property 14: Result Comparison Accuracy**
    /// **Validates: Requirements 5.2**
    ///
    /// For any two identical replay results, the comparison should correctly identify them as identical.
    #[test]
    fn property_identical_results_detected(
        balance in -1000000i64..1000000i64,
        count in 0u32..1000000u32,
        tx_count in 1usize..100
    ) {
        let comparator = ResultComparator::new();
        let result1 = create_replay_result(balance, count, tx_count);
        let result2 = create_replay_result(balance, count, tx_count);

        let comparison = comparator.compare(result1, result2);

        prop_assert!(comparison.are_identical());
        prop_assert!(comparison.final_state_matches);
        prop_assert!(comparison.final_hash_matches);
        prop_assert!(comparison.transaction_count_matches);
        prop_assert_eq!(comparison.divergent_transition_count(), 0);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 14: Result Comparison Accuracy**
    /// **Validates: Requirements 5.2**
    ///
    /// For any two different replay results, the comparison should correctly identify the differences.
    #[test]
    fn property_different_results_detected(
        balance1 in -1000000i64..1000000i64,
        balance2 in -1000000i64..1000000i64,
        count in 0u32..1000000u32,
        tx_count in 1usize..100
    ) {
        // Skip if balances are the same
        prop_assume!(balance1 != balance2);

        let comparator = ResultComparator::new();
        let result1 = create_replay_result(balance1, count, tx_count);
        let result2 = create_replay_result(balance2, count, tx_count);

        let comparison = comparator.compare(result1, result2);

        // Should detect the difference
        prop_assert!(!comparison.are_identical());
        prop_assert!(!comparison.final_state_matches);
        prop_assert!(!comparison.final_hash_matches);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 14: Result Comparison Accuracy**
    /// **Validates: Requirements 5.2**
    ///
    /// For any balance maps, the diff analyzer should accurately identify all differences.
    #[test]
    fn property_balance_diff_accuracy(
        baseline in arbitrary_balance_map(),
        comparison in arbitrary_balance_map()
    ) {
        let differences = DiffAnalyzer::analyze_balance_differences(&baseline, &comparison);

        // Verify each reported difference is accurate
        for diff in &differences {
            let baseline_val = baseline.get(&diff.account_id).copied().unwrap_or(0);
            let comparison_val = comparison.get(&diff.account_id).copied().unwrap_or(0);

            prop_assert_eq!(diff.baseline_balance, baseline_val);
            prop_assert_eq!(diff.comparison_balance, comparison_val);
            prop_assert_eq!(diff.difference, comparison_val - baseline_val);
        }

        // Verify no differences are missed
        for (account_id, &baseline_val) in &baseline {
            let comparison_val = comparison.get(account_id).copied().unwrap_or(0);
            if baseline_val != comparison_val {
                prop_assert!(differences.iter().any(|d| &d.account_id == account_id));
            }
        }

        for (account_id, &comparison_val) in &comparison {
            let baseline_val = baseline.get(account_id).copied().unwrap_or(0);
            if baseline_val != comparison_val {
                prop_assert!(differences.iter().any(|d| &d.account_id == account_id));
            }
        }
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 14: Result Comparison Accuracy**
    /// **Validates: Requirements 5.2**
    ///
    /// For any set of balance differences, the total difference should equal the sum of individual differences.
    #[test]
    fn property_total_balance_difference_accuracy(
        differences in prop::collection::vec(
            (
                "[a-z]{3,10}",
                -1000000i64..1000000i64,
                -1000000i64..1000000i64
            ).prop_map(|(account_id, baseline, comparison)| {
                BalanceDifference {
                    account_id,
                    baseline_balance: baseline,
                    comparison_balance: comparison,
                    difference: comparison - baseline,
                }
            }),
            0..50
        )
    ) {
        let total = DiffAnalyzer::total_balance_difference(&differences);
        let expected: i64 = differences.iter().map(|d| d.difference).sum();

        prop_assert_eq!(total, expected);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 14: Result Comparison Accuracy**
    /// **Validates: Requirements 5.2**
    ///
    /// For any set of balance differences, the largest differences should be correctly identified
    /// and sorted by absolute value.
    #[test]
    fn property_largest_differences_accuracy(
        differences in prop::collection::vec(
            (
                "[a-z]{3,10}",
                -1000000i64..1000000i64,
                -1000000i64..1000000i64
            ).prop_map(|(account_id, baseline, comparison)| {
                BalanceDifference {
                    account_id,
                    baseline_balance: baseline,
                    comparison_balance: comparison,
                    difference: comparison - baseline,
                }
            }),
            1..50
        ),
        limit in 1usize..10
    ) {
        let largest = DiffAnalyzer::largest_differences(&differences, limit);

        // Should not exceed the limit
        prop_assert!(largest.len() <= limit);
        prop_assert!(largest.len() <= differences.len());

        // Should be sorted by absolute value in descending order
        for i in 1..largest.len() {
            prop_assert!(largest[i - 1].difference.abs() >= largest[i].difference.abs());
        }

        // All returned differences should be in the original set
        for diff in &largest {
            prop_assert!(differences.iter().any(|d| d.account_id == diff.account_id));
        }
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 14: Result Comparison Accuracy**
    /// **Validates: Requirements 5.2**
    ///
    /// For any two replay results, comparing them should be commutative in terms of detecting differences
    /// (though the direction of differences may differ).
    #[test]
    fn property_comparison_symmetry(
        balance1 in -1000000i64..1000000i64,
        balance2 in -1000000i64..1000000i64,
        count in 0u32..1000000u32,
        tx_count in 1usize..100
    ) {
        let comparator = ResultComparator::new();
        let result1 = create_replay_result(balance1, count, tx_count);
        let result2 = create_replay_result(balance2, count, tx_count);

        let comparison1 = comparator.compare(result1.clone(), result2.clone());
        let comparison2 = comparator.compare(result2, result1);

        // Both should agree on whether results are identical
        prop_assert_eq!(comparison1.are_identical(), comparison2.are_identical());
        prop_assert_eq!(comparison1.final_state_matches, comparison2.final_state_matches);
        prop_assert_eq!(comparison1.final_hash_matches, comparison2.final_hash_matches);
    }

    /// **Feature: deterministic-transaction-replay-engine, Property 14: Result Comparison Accuracy**
    /// **Validates: Requirements 5.2**
    ///
    /// For any replay result compared with itself, the comparison should always show identical results.
    #[test]
    fn property_self_comparison_identity(
        balance in -1000000i64..1000000i64,
        count in 0u32..1000000u32,
        tx_count in 1usize..100
    ) {
        let comparator = ResultComparator::new();
        let result = create_replay_result(balance, count, tx_count);

        let comparison = comparator.compare(result.clone(), result);

        prop_assert!(comparison.are_identical());
        prop_assert!(comparison.final_state_matches);
        prop_assert!(comparison.final_hash_matches);
        prop_assert_eq!(comparison.divergent_transition_count(), 0);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_comparison_summary_identical() {
        let comparator = ResultComparator::new();
        let result1 = create_replay_result(100, 5, 10);
        let result2 = create_replay_result(100, 5, 10);

        let comparison = comparator.compare(result1, result2);
        let summary = comparison.summary();

        assert_eq!(summary, "Results are identical");
    }

    #[test]
    fn test_comparison_summary_different() {
        let comparator = ResultComparator::new();
        let result1 = create_replay_result(100, 5, 10);
        let result2 = create_replay_result(150, 5, 10);

        let comparison = comparator.compare(result1, result2);
        let summary = comparison.summary();

        assert!(summary.contains("Results differ"));
        assert!(summary.contains("final states differ"));
        assert!(summary.contains("final hashes differ"));
    }

    #[test]
    fn test_empty_balance_maps() {
        let baseline = HashMap::new();
        let comparison = HashMap::new();

        let differences = DiffAnalyzer::analyze_balance_differences(&baseline, &comparison);

        assert_eq!(differences.len(), 0);
        assert_eq!(DiffAnalyzer::total_balance_difference(&differences), 0);
    }

    #[test]
    fn test_largest_differences_with_empty_vec() {
        let differences = vec![];
        let largest = DiffAnalyzer::largest_differences(&differences, 5);

        assert_eq!(largest.len(), 0);
    }
}

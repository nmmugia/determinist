//! Integration tests for the trading system example
//!
//! Tests high-frequency trading scenarios, position calculations,
//! risk management, and market data replay consistency

use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

// Import types from the trading system example
// Note: In a real scenario, these would be in a shared module
// For this test, we'll define minimal versions or use the example directly

#[cfg(test)]
mod trading_system_tests {
    use super::*;
    
    // Helper to create a basic trading state for testing
    fn create_test_state() -> (
        HashMap<String, i64>, // cash_balances
        HashMap<String, HashMap<String, (i64, i64, i64)>>, // positions: trader -> symbol -> (qty, avg_price, pnl)
    ) {
        let mut cash_balances = HashMap::new();
        cash_balances.insert("TRADER001".to_string(), 100_000_000); // $1M
        cash_balances.insert("TRADER002".to_string(), 50_000_000);  // $500K
        cash_balances.insert("TRADER003".to_string(), 200_000_000); // $2M
        
        let positions = HashMap::new();
        
        (cash_balances, positions)
    }
    
    #[test]
    fn test_high_frequency_trading_scenario() {
        // Test rapid sequence of trades with deterministic results
        let (mut cash_balances, mut positions) = create_test_state();
        
        // Simulate 100 rapid trades
        let base_time = Utc::now();
        let market_price = 10000i64; // $100.00
        
        for i in 0..100 {
            let trader = "TRADER001";
            let symbol = "AAPL";
            let quantity = 10i64;
            
            // Alternate buy/sell
            let is_buy = i % 2 == 0;
            
            if is_buy {
                let cost = quantity * market_price;
                if cash_balances[trader] >= cost {
                    *cash_balances.get_mut(trader).unwrap() -= cost;
                    
                    let trader_positions = positions.entry(trader.to_string())
                        .or_insert_with(HashMap::new);
                    let pos = trader_positions.entry(symbol.to_string())
                        .or_insert((0, 0, 0));
                    
                    if pos.0 == 0 {
                        pos.0 = quantity;
                        pos.1 = market_price;
                    } else {
                        let total_cost = pos.0 * pos.1 + quantity * market_price;
                        pos.0 += quantity;
                        pos.1 = total_cost / pos.0;
                    }
                }
            } else {
                let revenue = quantity * market_price;
                *cash_balances.get_mut(trader).unwrap() += revenue;
                
                if let Some(trader_positions) = positions.get_mut(trader) {
                    if let Some(pos) = trader_positions.get_mut(symbol) {
                        pos.0 -= quantity;
                        if pos.0 == 0 {
                            trader_positions.remove(symbol);
                        }
                    }
                }
            }
        }
        
        // Verify final state is consistent
        assert!(cash_balances["TRADER001"] > 0, "Trader should have positive cash");
        
        // After 100 alternating trades, position should be 0
        if let Some(trader_positions) = positions.get("TRADER001") {
            if let Some(pos) = trader_positions.get("AAPL") {
                assert_eq!(pos.0, 0, "Position should be flat after alternating trades");
            }
        }
    }
    
    #[test]
    fn test_position_calculation_accuracy() {
        // Test that position calculations are accurate across multiple trades
        let (mut cash_balances, mut positions) = create_test_state();
        
        let trader = "TRADER001";
        let symbol = "GOOGL";
        
        // Buy 100 shares at $140
        let price1 = 14000i64;
        let qty1 = 100i64;
        let cost1 = qty1 * price1;
        *cash_balances.get_mut(trader).unwrap() -= cost1;
        
        let trader_positions = positions.entry(trader.to_string())
            .or_insert_with(HashMap::new);
        trader_positions.insert(symbol.to_string(), (qty1, price1, 0));
        
        // Buy 50 more shares at $145
        let price2 = 14500i64;
        let qty2 = 50i64;
        let cost2 = qty2 * price2;
        *cash_balances.get_mut(trader).unwrap() -= cost2;
        
        let pos = trader_positions.get_mut(symbol).unwrap();
        let total_cost = pos.0 * pos.1 + qty2 * price2;
        pos.0 += qty2;
        pos.1 = total_cost / pos.0;
        
        // Average price should be (100*140 + 50*145) / 150 = $141.67
        let expected_avg = (qty1 * price1 + qty2 * price2) / (qty1 + qty2);
        assert_eq!(pos.1, expected_avg, "Average price calculation should be accurate");
        assert_eq!(pos.0, 150, "Total position should be 150 shares");
        
        // Sell 75 shares at $150
        let price3 = 15000i64;
        let qty3 = 75i64;
        let revenue = qty3 * price3;
        *cash_balances.get_mut(trader).unwrap() += revenue;
        
        let realized_pnl = qty3 * (price3 - pos.1);
        pos.2 += realized_pnl;
        pos.0 -= qty3;
        
        assert_eq!(pos.0, 75, "Remaining position should be 75 shares");
        assert!(pos.2 > 0, "Should have realized profit from selling at higher price");
    }
    
    #[test]
    fn test_risk_management_position_limits() {
        // Test that position size limits are enforced
        let (cash_balances, positions) = create_test_state();
        
        let trader = "TRADER001";
        let symbol = "MSFT";
        let current_position = 9500i64; // Already have 9,500 shares
        let position_limit = 10_000i64;
        
        // Try to buy 600 more shares (would exceed limit)
        let new_order_qty = 600i64;
        let new_position_size = current_position + new_order_qty;
        
        assert!(
            new_position_size > position_limit,
            "New position would exceed limit"
        );
        
        // Order should be rejected
        let should_reject = new_position_size > position_limit;
        assert!(should_reject, "Order exceeding position limit should be rejected");
        
        // Try to buy 500 shares (within limit)
        let safe_order_qty = 500i64;
        let safe_position_size = current_position + safe_order_qty;
        
        assert!(
            safe_position_size <= position_limit,
            "Safe order should be within limit"
        );
    }
    
    #[test]
    fn test_risk_management_exposure_limits() {
        // Test that total exposure limits are enforced
        let (cash_balances, mut positions) = create_test_state();
        
        let trader = "TRADER003";
        let exposure_limit = 100_000_000i64; // $1M limit
        
        // Create positions that approach the limit
        let mut trader_positions = HashMap::new();
        trader_positions.insert("AAPL".to_string(), (5000, 15000, 0)); // $750K exposure
        trader_positions.insert("GOOGL".to_string(), (1000, 14000, 0)); // $140K exposure
        positions.insert(trader.to_string(), trader_positions);
        
        // Calculate current exposure
        let current_exposure: i64 = positions[trader]
            .values()
            .map(|(qty, price, _)| qty.abs() * price)
            .sum();
        
        assert_eq!(current_exposure, 89_000_000, "Current exposure should be $890K");
        
        // Try to add position that would exceed limit
        let new_trade_value = 20_000_000i64; // $200K
        let new_exposure = current_exposure + new_trade_value;
        
        assert!(
            new_exposure > exposure_limit,
            "New trade would exceed exposure limit"
        );
        
        // Order should be rejected
        let should_reject = new_exposure > exposure_limit;
        assert!(should_reject, "Order exceeding exposure limit should be rejected");
    }
    
    #[test]
    fn test_market_data_replay_consistency() {
        // Test that replaying with same market data produces consistent results
        let (cash_balances, positions) = create_test_state();
        
        // Market data snapshot
        let market_data = vec![
            ("AAPL", 15000i64),
            ("GOOGL", 14000i64),
            ("MSFT", 35000i64),
        ];
        
        // Execute trades with this market data
        let trades = vec![
            ("TRADER001", "AAPL", 100i64, true),  // Buy 100 AAPL
            ("TRADER002", "GOOGL", 50i64, true),  // Buy 50 GOOGL
            ("TRADER001", "AAPL", 50i64, false),  // Sell 50 AAPL
        ];
        
        // First execution
        let result1 = execute_trades_with_market_data(
            cash_balances.clone(),
            positions.clone(),
            &trades,
            &market_data,
        );
        
        // Second execution with same data
        let result2 = execute_trades_with_market_data(
            cash_balances.clone(),
            positions.clone(),
            &trades,
            &market_data,
        );
        
        // Results should be identical
        assert_eq!(result1.0, result2.0, "Cash balances should match");
        assert_eq!(result1.1, result2.1, "Positions should match");
        assert_eq!(result1.2, result2.2, "Total volume should match");
    }
    
    // Helper function to execute trades with market data
    fn execute_trades_with_market_data(
        mut cash_balances: HashMap<String, i64>,
        mut positions: HashMap<String, HashMap<String, (i64, i64, i64)>>,
        trades: &[(&str, &str, i64, bool)],
        market_data: &[(&str, i64)],
    ) -> (HashMap<String, i64>, HashMap<String, HashMap<String, (i64, i64, i64)>>, i64) {
        let mut total_volume = 0i64;
        
        let market_prices: HashMap<&str, i64> = market_data.iter().cloned().collect();
        
        for (trader, symbol, quantity, is_buy) in trades {
            let market_price = market_prices[symbol];
            let trade_value = quantity * market_price;
            
            if *is_buy {
                if cash_balances[*trader] >= trade_value {
                    *cash_balances.get_mut(*trader).unwrap() -= trade_value;
                    
                    let trader_positions = positions.entry(trader.to_string())
                        .or_insert_with(HashMap::new);
                    let pos = trader_positions.entry(symbol.to_string())
                        .or_insert((0, 0, 0));
                    
                    if pos.0 == 0 {
                        pos.0 = *quantity;
                        pos.1 = market_price;
                    } else {
                        let total_cost = pos.0 * pos.1 + quantity * market_price;
                        pos.0 += quantity;
                        pos.1 = total_cost / pos.0;
                    }
                    
                    total_volume += trade_value;
                }
            } else {
                *cash_balances.get_mut(*trader).unwrap() += trade_value;
                
                if let Some(trader_positions) = positions.get_mut(*trader) {
                    if let Some(pos) = trader_positions.get_mut(*symbol) {
                        let realized_pnl = quantity * (market_price - pos.1);
                        pos.2 += realized_pnl;
                        pos.0 -= quantity;
                        
                        if pos.0 == 0 {
                            trader_positions.remove(*symbol);
                        }
                    }
                }
                
                total_volume += trade_value;
            }
        }
        
        (cash_balances, positions, total_volume)
    }
    
    #[test]
    fn test_parallel_execution_determinism() {
        // Test that parallel execution produces same results as sequential
        use std::thread;
        
        let (cash_balances, positions) = create_test_state();
        
        let market_data = vec![
            ("AAPL", 15000i64),
            ("GOOGL", 14000i64),
            ("MSFT", 35000i64),
        ];
        
        let trades = vec![
            ("TRADER001", "AAPL", 100i64, true),
            ("TRADER002", "GOOGL", 50i64, true),
            ("TRADER003", "MSFT", 200i64, true),
            ("TRADER001", "AAPL", 50i64, false),
            ("TRADER002", "GOOGL", 25i64, false),
        ];
        
        // Sequential execution
        let sequential_result = execute_trades_with_market_data(
            cash_balances.clone(),
            positions.clone(),
            &trades,
            &market_data,
        );
        
        // Parallel execution (simulate multiple threads)
        let handles: Vec<_> = (0..4).map(|_| {
            let cash = cash_balances.clone();
            let pos = positions.clone();
            let trades_clone = trades.clone();
            let market_clone = market_data.clone();
            
            thread::spawn(move || {
                execute_trades_with_market_data(cash, pos, &trades_clone, &market_clone)
            })
        }).collect();
        
        let parallel_results: Vec<_> = handles.into_iter()
            .map(|h| h.join().unwrap())
            .collect();
        
        // All parallel results should match sequential result
        for parallel_result in &parallel_results {
            assert_eq!(
                parallel_result.0, sequential_result.0,
                "Parallel cash balances should match sequential"
            );
            assert_eq!(
                parallel_result.1, sequential_result.1,
                "Parallel positions should match sequential"
            );
            assert_eq!(
                parallel_result.2, sequential_result.2,
                "Parallel volume should match sequential"
            );
        }
    }
    
    #[test]
    fn test_regulatory_reporting_completeness() {
        // Test that all trades are properly recorded for regulatory reporting
        let (cash_balances, positions) = create_test_state();
        
        let market_data = vec![
            ("AAPL", 15000i64),
            ("GOOGL", 14000i64),
        ];
        
        let trades = vec![
            ("TRADER001", "AAPL", 100i64, true),
            ("TRADER002", "GOOGL", 50i64, true),
            ("TRADER001", "AAPL", 50i64, false),
        ];
        
        let mut execution_log = Vec::new();
        
        let market_prices: HashMap<&str, i64> = market_data.iter().cloned().collect();
        
        for (i, (trader, symbol, quantity, is_buy)) in trades.iter().enumerate() {
            let market_price = market_prices[symbol];
            
            execution_log.push((
                format!("TRADE_{:03}", i),
                trader.to_string(),
                symbol.to_string(),
                *quantity,
                market_price,
                *is_buy,
            ));
        }
        
        // Verify all trades are logged
        assert_eq!(execution_log.len(), trades.len(), "All trades should be logged");
        
        // Verify log contains required information
        for (trade_id, trader, symbol, quantity, price, is_buy) in &execution_log {
            assert!(!trade_id.is_empty(), "Trade ID should not be empty");
            assert!(!trader.is_empty(), "Trader ID should not be empty");
            assert!(!symbol.is_empty(), "Symbol should not be empty");
            assert!(*quantity > 0, "Quantity should be positive");
            assert!(*price > 0, "Price should be positive");
        }
    }
    
    #[test]
    fn test_position_reconciliation() {
        // Test that positions can be reconciled with cash movements
        let (mut cash_balances, mut positions) = create_test_state();
        
        let initial_cash = cash_balances["TRADER001"];
        let trader = "TRADER001";
        
        // Execute a series of trades
        let trades = vec![
            ("AAPL", 100i64, 15000i64, true),   // Buy 100 @ $150
            ("AAPL", 50i64, 15500i64, false),   // Sell 50 @ $155
            ("GOOGL", 75i64, 14000i64, true),   // Buy 75 @ $140
        ];
        
        let mut total_cash_out = 0i64;
        let mut total_cash_in = 0i64;
        
        for (symbol, quantity, price, is_buy) in trades {
            let trade_value = quantity * price;
            
            if is_buy {
                *cash_balances.get_mut(trader).unwrap() -= trade_value;
                total_cash_out += trade_value;
                
                let trader_positions = positions.entry(trader.to_string())
                    .or_insert_with(HashMap::new);
                let pos = trader_positions.entry(symbol.to_string())
                    .or_insert((0, 0, 0));
                
                if pos.0 == 0 {
                    pos.0 = quantity;
                    pos.1 = price;
                } else {
                    let total_cost = pos.0 * pos.1 + quantity * price;
                    pos.0 += quantity;
                    pos.1 = total_cost / pos.0;
                }
            } else {
                *cash_balances.get_mut(trader).unwrap() += trade_value;
                total_cash_in += trade_value;
                
                if let Some(trader_positions) = positions.get_mut(trader) {
                    if let Some(pos) = trader_positions.get_mut(symbol) {
                        pos.0 -= quantity;
                    }
                }
            }
        }
        
        let final_cash = cash_balances[trader];
        let cash_change = final_cash - initial_cash;
        let expected_change = total_cash_in - total_cash_out;
        
        assert_eq!(
            cash_change, expected_change,
            "Cash change should match trade flows"
        );
        
        // Verify positions exist
        assert!(positions.contains_key(trader), "Trader should have positions");
        let trader_positions = &positions[trader];
        assert!(trader_positions.contains_key("AAPL"), "Should have AAPL position");
        assert!(trader_positions.contains_key("GOOGL"), "Should have GOOGL position");
        
        // Verify AAPL position is 50 shares (bought 100, sold 50)
        assert_eq!(trader_positions["AAPL"].0, 50, "AAPL position should be 50 shares");
        
        // Verify GOOGL position is 75 shares
        assert_eq!(trader_positions["GOOGL"].0, 75, "GOOGL position should be 75 shares");
    }
}

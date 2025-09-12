# Enhanced Position Tracking System - Implementation Plan
**Created:** 2025-09-12  
**Branch:** 9.11.25.1  
**Status:** Ready for Implementation

## Overview
Complete redesign of position tracking system to handle proper cost basis calculations with weighted averages when adding to existing positions. This plan addresses the fundamental flaws in the current cost basis calculation architecture.

## Phase 1: Database Schema Enhancement

### New Positions Table
```sql
CREATE TABLE positions (
    id TEXT PRIMARY KEY,
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    avg_cost_basis REAL NOT NULL,    -- Weighted average cost per share
    total_cost REAL NOT NULL,        -- Total dollars invested in position
    current_price REAL NOT NULL,     -- Latest market price
    realized_pnl REAL NOT NULL DEFAULT 0.0,  -- P&L from closed trades
    opened_at INTEGER NOT NULL,
    closed_at INTEGER,
    status TEXT NOT NULL DEFAULT 'OPEN',  -- 'OPEN' or 'CLOSED'
    session_id TEXT NOT NULL
);
```

### Enhanced Paper Trades Table
```sql
-- Keep existing paper_trades table for transaction history
-- Add position_id reference to link trades to positions
ALTER TABLE paper_trades ADD COLUMN position_id TEXT;
```

## Phase 2: Cost Basis Calculation Logic (WLDS Example)

### Reference Trading Scenario:
- **Step 1**: Buy 100 @ $10.12 → Position: 100 shares @ $10.12, Cost: $1,012.00
- **Step 2**: Sell 100 @ $11.72 → Realized P&L: +$160.00, Position closed
- **Step 3**: Buy 500 @ $12.34 → New position: 500 shares @ $12.34, Cost: $6,170.00
- **Step 4**: Sell 250 @ $13.97 → Realized P&L: +$407.50, Position: 250 @ $12.34, Cost: $3,085.00
- **Step 5**: Buy 50 @ $18.44 → **Weighted avg**: 300 shares @ $13.3567, Total cost: $4,007.00
- **Step 6**: Sell 300 @ $6.32 → Realized P&L: -$2,111.00, Position closed
- **Final Cumulative Realized P&L**: -$1,543.50

### Cost Basis Algorithm:
```rust
fn calculate_weighted_average_cost_basis(
    existing_shares: u64,
    existing_cost_basis: f64,
    new_shares: u64,
    new_price: f64
) -> f64 {
    let old_total_cost = existing_shares as f64 * existing_cost_basis;
    let new_total_cost = new_shares as f64 * new_price;
    let total_shares = existing_shares + new_shares;
    
    let weighted_basis = (old_total_cost + new_total_cost) / total_shares as f64;
    
    // Round to 5 decimal places, then to nearest penny for display
    (weighted_basis * 100000.0).round() / 100000.0
}
```

### Step 5 Detailed Calculation:
- Old Position: 250 shares @ $12.34 = $3,085.00
- New Purchase: 50 shares @ $18.44 = $922.00  
- Combined: 300 shares, total cost = $4,007.00
- **New basis = $4,007 ÷ 300 = $13.3567/share** ✅

## Phase 3: Database Operations (High-Level Pseudo-Code)

### Buy Order Processing:
```rust
async fn process_buy_order(symbol: &str, quantity: u64, price: f64) -> Result<()> {
    let mut tx = db.begin().await?;
    
    // Check for existing OPEN position
    if let Some(existing_position) = get_open_position(&symbol).await? {
        // Add to existing position with weighted average
        let new_basis = calculate_weighted_average_cost_basis(
            existing_position.quantity,
            existing_position.avg_cost_basis,
            quantity,
            price
        );
        
        update_position(&mut tx, PositionUpdate {
            id: existing_position.id,
            quantity: existing_position.quantity + quantity,
            avg_cost_basis: new_basis,
            total_cost: existing_position.total_cost + (quantity as f64 * price),
        }).await?;
    } else {
        // Create new position
        create_position(&mut tx, Position {
            id: generate_id(),
            symbol: symbol.to_string(),
            quantity,
            avg_cost_basis: price,
            total_cost: quantity as f64 * price,
            status: PositionStatus::Open,
            opened_at: Utc::now(),
        }).await?;
    }
    
    // Record transaction in paper_trades
    record_trade(&mut tx, trade_data).await?;
    tx.commit().await?;
}
```

### Sell Order Processing:
```rust
async fn process_sell_order(symbol: &str, quantity: u64, price: f64) -> Result<()> {
    let mut tx = db.begin().await?;
    
    let position = get_open_position(&symbol).await?
        .ok_or_else(|| anyhow!("No open position for {}", symbol))?;
    
    // Calculate realized P&L
    let realized_pnl = quantity as f64 * (price - position.avg_cost_basis);
    let remaining_quantity = position.quantity - quantity;
    
    if remaining_quantity == 0 {
        // Close position completely
        close_position(&mut tx, ClosePositionUpdate {
            id: position.id,
            quantity: 0,
            realized_pnl: position.realized_pnl + realized_pnl,
            status: PositionStatus::Closed,
            closed_at: Some(Utc::now()),
        }).await?;
    } else {
        // Partial close - update quantity, keep same cost basis
        update_position(&mut tx, PositionUpdate {
            id: position.id,
            quantity: remaining_quantity,
            realized_pnl: position.realized_pnl + realized_pnl,
            total_cost: remaining_quantity as f64 * position.avg_cost_basis,
            // avg_cost_basis stays the same
        }).await?;
    }
    
    record_trade(&mut tx, trade_data).await?;
    tx.commit().await?;
}
```

## Phase 4: Web UI Integration

### Positions Display Logic:
```javascript
async function loadPositions() {
    const response = await fetch('/api/positions');
    const data = await response.json();
    
    const openPositions = data.positions.filter(p => p.status === 'OPEN' && p.quantity > 0);
    const closedToday = data.positions.filter(p => p.status === 'CLOSED' && isToday(p.closed_at));
    
    displayPositions(openPositions, closedToday);
}

function calculateOpenPnL(position) {
    if (position.quantity === 0) return 0;
    
    const unrealized = (position.current_price - position.avg_cost_basis) * position.quantity;
    return Math.round(unrealized * 100) / 100; // Penny rounding
}

function displayPositions(openPositions, closedPositions) {
    const tbody = document.getElementById('positionsBody');
    tbody.innerHTML = '';
    
    // Show open positions first
    openPositions.forEach(pos => {
        const openPnL = calculateOpenPnL(pos);
        const realizedPnL = Math.round(pos.realized_pnl * 100) / 100;
        
        tbody.innerHTML += `
            <tr class="open-position">
                <td>${pos.symbol}</td>
                <td>${pos.quantity} @ $${pos.avg_cost_basis.toFixed(5)}</td>
                <td>$${pos.total_cost.toFixed(2)}</td>
                <td class="${openPnL >= 0 ? 'profit' : 'loss'}">$${openPnL.toFixed(2)}</td>
                <td class="${realizedPnL >= 0 ? 'profit' : 'loss'}">$${realizedPnL.toFixed(2)}</td>
            </tr>
        `;
    });
    
    // Show closed positions with different styling
    closedPositions.forEach(pos => {
        tbody.innerHTML += `
            <tr class="closed-position">
                <td>${pos.symbol} (CLOSED)</td>
                <td>0 (was ${pos.quantity})</td>
                <td>$0.00</td>
                <td>$0.00</td>
                <td class="${pos.realized_pnl >= 0 ? 'profit' : 'loss'}">$${pos.realized_pnl.toFixed(2)}</td>
            </tr>
        `;
    });
}
```

### WebSocket Auto-Management:
```rust
// In get_positions() method - auto-subscribe to open position symbols
let open_symbols: Vec<String> = positions.iter()
    .filter(|p| p.status == PositionStatus::Open && p.quantity > 0)
    .map(|p| p.symbol.clone())
    .collect();

if !open_symbols.is_empty() {
    self.data_module.websocket_subscribe(open_symbols).await?;
}
```

## Phase 5: Database Reset Enhancement

### Dynamic Reset with Environment Variable:
```rust
async fn reset_database(State(engine): State<Arc<TradingEngine>>) -> Json<ResetDatabaseResponse> {
    let config = engine.get_config();
    let initial_cash = config.paper_trading_config.initial_cash; // From PAPER_INITIAL_CASH env var
    
    let reset_queries = [
        "DELETE FROM paper_trades",
        "DELETE FROM positions",           // NEW - clear positions table
        "DELETE FROM trading_rules_config", 
        "DELETE FROM paper_portfolio_snapshots",
        &format!(
            "UPDATE paper_sessions SET 
                current_cash = {},
                total_pnl = 0.0,
                realized_pnl = 0.0,
                unrealized_pnl = 0.0,
                open_positions = 0
            WHERE session_id = '{}'",
            initial_cash, session_id
        ),
    ];
    
    // Execute in transaction
    execute_reset_queries(reset_queries).await
}
```

### User Confirmation Dialog:
```javascript
const confirmed = confirm(
    "⚠️ RESET DATABASE WARNING ⚠️\n\n" +
    "This will permanently delete:\n" +
    "• All trade history and transactions\n" +
    "• All position records and cost basis data\n" +  // Updated
    "• Portfolio balances and P&L history\n" +
    "• Trading rules configuration\n\n" +
    `Cash will be reset to $${initialCashFromEnv.toLocaleString()}\n\n` +  // Dynamic
    "This action CANNOT be undone.\n\n" +
    "Are you absolutely sure you want to reset the database?"
);
```

## Implementation Priority:

1. **Phase 1**: Create new positions table schema
2. **Phase 2**: Implement weighted average cost basis calculations 
3. **Phase 3**: Update buy/sell order processing logic
4. **Phase 4**: Enhance web UI for position display and WebSocket management
5. **Phase 5**: Test complete WLDS scenario end-to-end
6. **Phase 6**: Verify reset functionality with new tables

## Success Criteria:
- **WLDS 6-step scenario** should result in exactly **-$1,543.50** final realized P&L
- **Cost basis accuracy** with proper weighted averages when adding to positions
- **Real-time price updates** via WebSocket auto-management
- **UI displays** both open positions and positions closed today
- **Database reset** properly clears all position data and uses environment variable for initial cash

## Key Files to Modify:
- `src/broker/paper.rs` - Core position tracking logic
- `src/web/mod.rs` - UI and API endpoints
- `src/types/mod.rs` - Position data structures
- Database migrations - New positions table
- Web UI JavaScript - Enhanced position display

---
**Note**: This plan preserves all architectural decisions and calculations from the comprehensive analysis. Implementation should follow this exactly to ensure proper cost basis handling.
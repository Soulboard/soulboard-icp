# ICP Integration Guide for Soulboard

## Overview

This document explains the ICP (Internet Computer Protocol) integration implemented in the Soulboard canister for campaign funding and provider earnings withdrawal.

## Key Features Implemented

### 1. Campaign Funding with ICP Transfers

**Function:** `fund_campaign(campaign_id: String, amount: NumTokens)`

- **Purpose:** Allows campaign owners to fund their campaigns with actual ICP tokens
- **Process:**
  1. Verifies campaign ownership
  2. Transfers ICP from caller's account to the canister's account
  3. Updates campaign budget upon successful transfer
- **Returns:** Transfer block index for transaction verification
- **Security:** Only campaign owners can fund their own campaigns

### 2. Provider Earnings Withdrawal

**Function:** `withdraw_provider_earnings(provider_id: String, amount: NumTokens)`

- **Purpose:** Allows providers to withdraw their earned tokens to their wallet
- **Process:**
  1. Verifies provider ownership and sufficient earnings
  2. Transfers ICP from canister to provider's account
  3. Updates provider's total earnings upon successful transfer
- **Returns:** Transfer block index for transaction verification
- **Security:** Only provider owners can withdraw from their own accounts

### 3. Campaign Fund Withdrawal

**Function:** `withdraw_campaign_funds(campaign_id: String, amount: NumTokens)`

- **Purpose:** Allows campaign owners to withdraw unused funds from their campaigns
- **Process:**
  1. Verifies campaign ownership and sufficient balance
  2. Transfers ICP from canister to campaign owner's account
  3. Updates campaign budget, with rollback on transfer failure
- **Returns:** Transfer block index for transaction verification
- **Security:** Only campaign owners can withdraw from their own campaigns

### 4. Provider Payment System

**Function:** `pay_provider(campaign_id: String, provider_id: String, amount: NumTokens)`

- **Purpose:** Allows campaign owners to pay providers for their services
- **Process:**
  1. Verifies campaign ownership and sufficient budget
  2. Deducts amount from campaign budget
  3. Adds amount to provider's total earnings
  4. Records detailed earnings in the earnings registry
- **Security:** Only campaign owners can pay from their own campaigns

## Data Structures

### Enhanced Provider Structure
```rust
struct Provider {
    id: String,
    name: String,
    owner: Principal,
    locations: Vec<Location>,
    total_earnings: NumTokens, // NEW: Track total earnings
}
```

### New ProviderEarnings Structure
```rust
struct ProviderEarnings {
    provider_id: String,
    campaign_id: String,
    total_earned: NumTokens,
    last_withdrawal: Option<u64>, // timestamp
}
```

## Query Functions

### Get Provider Earnings
- `get_provider_earnings(provider_id: String) -> Result<NumTokens, String>`
- Returns total earnings for a provider (owner only)

### Get Provider Earnings Breakdown
- `get_provider_earnings_breakdown(provider_id: String) -> Result<Vec<ProviderEarnings>, String>`
- Returns detailed earnings breakdown by campaign (owner only)

### Get Campaign Balance
- `get_campaign_balance(campaign_id: String) -> Result<NumTokens, String>`
- Returns current campaign budget (owner only)

## ICP Transfer Implementation

### Core Transfer Function
```rust
async fn icp_transfer(
    from_subaccount: Option<Subaccount>,
    to: Account,
    memo: Option<Vec<u8>>,
    amount: NumTokens,
) -> Result<BlockIndex, String>
```

- **Ledger:** Uses ICP mainnet ledger canister (`ryjl3-tyaaa-aaaaa-aaaba-cai`)
- **Fee:** Fixed at 10,000 e8s (0.0001 ICP)
- **Inter-canister calls:** Uses `ic_cdk::call` for async communication
- **Error handling:** Comprehensive error handling with detailed messages

## Security Features

1. **Ownership Verification:** All functions verify caller ownership before proceeding
2. **Balance Checks:** Ensures sufficient funds before any transfer operations
3. **Rollback Mechanism:** Failed transfers trigger automatic state rollbacks
4. **Transaction Memos:** All transfers include descriptive memos for tracking
5. **Principal-based Authentication:** Uses IC's built-in principal system

## Transaction Flow Examples

### Campaign Funding Flow
1. User calls `fund_campaign("campaign_1", 1000000)` // 0.01 ICP
2. System verifies user owns campaign_1
3. ICP transfers from user's wallet to canister
4. Campaign budget increases by 1000000 e8s
5. Returns transaction block index

### Provider Withdrawal Flow
1. Provider calls `withdraw_provider_earnings("provider_1", 500000)` // 0.005 ICP
2. System verifies provider ownership and sufficient earnings
3. ICP transfers from canister to provider's wallet
4. Provider's total_earnings decreases by 500000 e8s
5. Returns transaction block index

## Error Handling

All functions provide detailed error messages for:
- Unauthorized access attempts
- Insufficient funds/earnings
- Non-existent campaigns/providers
- ICP transfer failures
- Network communication errors

## Integration Notes

1. **Async Functions:** All transfer functions are async due to inter-canister calls
2. **NumTokens Type:** Uses ICRC-1 standard token type (not Copy, requires cloning)
3. **Account Creation:** Automatically creates accounts from Principal IDs
4. **Memory Management:** Uses stable storage for persistent data across upgrades

## Usage Examples

```javascript
// Fund a campaign with 0.1 ICP
await actor.fund_campaign("campaign_123", 10000000n);

// Provider withdraws 0.05 ICP
await actor.withdraw_provider_earnings("provider_456", 5000000n);

// Campaign owner pays provider 0.02 ICP
await actor.pay_provider("campaign_123", "provider_456", 2000000n);

// Check provider earnings
const earnings = await actor.get_provider_earnings("provider_456");

// Check campaign balance
const balance = await actor.get_campaign_balance("campaign_123");
```

## Important Notes

- All amounts are in e8s format (1 ICP = 100,000,000 e8s)
- Transfer fees are automatically deducted from the transfer amount
- All functions require proper authentication via IC's principal system
- State changes are atomic - either everything succeeds or everything rolls back

use std::{cell::RefCell, borrow::Cow};
use ic_cdk::{caller, call};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable, storable::Bound};
use candid::{CandidType, Deserialize, Encode, Decode, Principal};
use icrc_ledger_types::icrc1::account::{Account, Subaccount};
use icrc_ledger_types::icrc1::transfer::{BlockIndex, Memo, NumTokens, TransferArg, TransferError};

type Memory = VirtualMemory<DefaultMemoryImpl>;
const MAX_VALUE_SIZE: u32 = 100; // Increased size for additional data


#[derive(CandidType, Deserialize, Clone)]
struct Provider {
    id: String,
    name: String,
    owner: Principal, // Track who owns this provider
    locations: Vec<Location>,
    total_earnings: NumTokens, // Track total earnings
}

#[derive(CandidType, Deserialize, Clone)]
struct Location {
    id: String,
    name: String,
    image: String,
    base_fees: NumTokens,
    views: u64,
    status: LocationStatus,
}

#[derive(CandidType, Deserialize, Clone)]
struct Campaign {
    id: String,
    name: String,
    description: String,
    image: Option<String>,
    locations: Option<Vec<Location>>,
    budget: NumTokens,
    owner: Principal, // Track who created this campaign
    status: CampaignStatus,
}

// New struct to track individual campaign-provider earnings
#[derive(CandidType, Deserialize, Clone)]
struct ProviderEarnings {
    provider_id: String,
    campaign_id: String,
    total_earned: NumTokens,
    last_withdrawal: Option<u64>, // timestamp
}

impl Storable for ProviderEarnings {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn into_bytes(self) -> Vec<u8> {
        Encode!(&self).unwrap()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_VALUE_SIZE,
        is_fixed_size: false,
    };
}

#[derive(CandidType, Deserialize, Clone)]
enum LocationStatus {
    Active,
    Inactive,
    Booked,
}

#[derive(CandidType, Deserialize, Clone)]
enum CampaignStatus {
    Active,
    Paused,
}

// Implement Storable for Campaign
impl Storable for Campaign {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn into_bytes(self) -> Vec<u8> {
        Encode!(&self).unwrap()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_VALUE_SIZE,
        is_fixed_size: false,
    };
}

// Implement Storable for Provider
impl Storable for Provider {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn into_bytes(self) -> Vec<u8> {
        Encode!(&self).unwrap()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_VALUE_SIZE,
        is_fixed_size: false,
    };
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    // Maps campaign IDs to campaigns - but access will be filtered by owner
    static CAMPAIGN_REGISTRY: RefCell<StableBTreeMap<String, Campaign, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
        )
    );

    // Maps provider IDs to providers - these will be publicly visible for marketplace
    static PROVIDER_REGISTRY: RefCell<StableBTreeMap<String, Provider, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
        )
    );

    // Maps earnings key (provider_id:campaign_id) to earnings
    static EARNINGS_REGISTRY: RefCell<StableBTreeMap<String, ProviderEarnings, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2))),
        )
    );

    // Counter for generating unique IDs
    static CAMPAIGN_COUNTER: RefCell<u64> = RefCell::new(0);
    static PROVIDER_COUNTER: RefCell<u64> = RefCell::new(0);
}

// Generate unique campaign ID
fn generate_campaign_id() -> String {
    CAMPAIGN_COUNTER.with(|counter| {
        let mut c = counter.borrow_mut();
        *c += 1;
        format!("campaign_{}", *c)
    })
}

// Generate unique provider ID
fn generate_provider_id() -> String {
    PROVIDER_COUNTER.with(|counter| {
        let mut c = counter.borrow_mut();
        *c += 1;
        format!("provider_{}", *c)
    })
}

// Registers a new provider for the calling wallet
#[ic_cdk::update]
fn register_provider(name: String, locations: Vec<Location>) -> Result<String, String> {
    let caller_principal = caller();
    let provider_id = generate_provider_id();
    
    let provider = Provider {
        id: provider_id.clone(),
        name,
        owner: caller_principal,
        locations,
        total_earnings: NumTokens::from(0u64),
    };

    PROVIDER_REGISTRY.with(|registry| {
        registry.borrow_mut().insert(provider_id.clone(), provider);
    });

    Ok(provider_id)
}

// Creates a new campaign (private to the caller)
#[ic_cdk::update]
fn create_campaign(
    name: String,
    description: String,
    image: Option<String>,
    locations: Option<Vec<Location>>,
    budget: NumTokens,
) -> Result<String, String> {
    let caller_principal = caller();
    let campaign_id = generate_campaign_id();
    
    let campaign = Campaign {
        id: campaign_id.clone(),
        name,
        description,
        image,
        locations,
        budget,
        owner: caller_principal,
        status: CampaignStatus::Active,
    };

    CAMPAIGN_REGISTRY.with(|registry| {
        registry.borrow_mut().insert(campaign_id.clone(), campaign);
    });

    Ok(campaign_id)
}

/// Transfers some ICP to the specified account.
async fn icp_transfer(
    from_subaccount: Option<Subaccount>,
    to: Account,
    memo: Option<Vec<u8>>,
    amount: NumTokens,
) -> Result<BlockIndex, String> {
    // The ID of the ledger canister on the IC mainnet.
    const ICP_LEDGER_CANISTER_ID: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
    let icp_ledger = Principal::from_text(ICP_LEDGER_CANISTER_ID).unwrap();
    let args = TransferArg {
        // A "memo" is an arbitrary blob that has no meaning to the ledger, but can be used by
        // the sender or receiver to attach additional information to the transaction.
        memo: memo.map(|m| Memo::from(m)),
        to,
        amount,
        // The ledger supports subaccounts. You can pick the subaccount of the caller canister's
        // account to use for transferring the ICP. If you don't specify a subaccount, the default
        // subaccount of the caller's account is used.
        from_subaccount,
        // The ICP ledger canister charges a fee for transfers, which is deducted from the
        // sender's account. The fee is fixed to 10_000 e8s (0.0001 ICP). You can specify it here,
        // to ensure that it hasn't changed, or leave it as None to use the current fee.
        fee: Some(NumTokens::from(10_000u32)),
        // The created_at_time is used for deduplication. Not set in this example since it uses
        // unbounded-wait calls. You should, however, set it if you opt to use bounded-wait
        // calls, or if you use ingress messages, or if you are worried about bugs in the ICP
        // ledger.
        created_at_time: None,
    };

    // Make the inter-canister call to the ICP ledger
    match call(icp_ledger, "icrc1_transfer", (args,)).await {
        Ok((result,)) => {
            let transfer_result: Result<BlockIndex, TransferError> = result;
            match transfer_result {
                Ok(block_index) => Ok(block_index),
                Err(e) => Err(format!("Ledger returned an error: {:?}", e)),
            }
        }
        Err((code, msg)) => Err(format!("Error calling ledger canister: {:?}: {}", code, msg)),
    }
}

// Helper function to create an account from a principal
fn principal_to_account(principal: Principal) -> Account {
    Account {
        owner: principal,
        subaccount: None,
    }
}

// Only the campaign owner can fund their campaign with actual ICP transfer
#[ic_cdk::update]
async fn fund_campaign(campaign_id: String, amount: NumTokens) -> Result<String, String> {
    let caller_principal = caller();
    let amount_clone = amount.clone();
    
    // First, verify the campaign exists and the caller is the owner
    CAMPAIGN_REGISTRY.with(|registry| {
        match registry.borrow().get(&campaign_id) {
            Some(campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only fund your own campaigns".to_string());
                }
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })?;

    // Get this canister's principal as the recipient
    let canister_principal = ic_cdk::api::id();
    let canister_account = principal_to_account(canister_principal);
    
    // Transfer ICP from the caller to this canister
    let transfer_memo = format!("Fund campaign: {}", campaign_id).into_bytes();
    let transfer_amount = amount; // Create a copy for the transfer
    match icp_transfer(
        None, // from_subaccount - uses caller's default
        canister_account, // to - this canister
        Some(transfer_memo),
        transfer_amount,
    ).await {
        Ok(block_index) => {
            // If transfer successful, update the campaign budget
            CAMPAIGN_REGISTRY.with(|registry| {
                let mut registry_borrow = registry.borrow_mut();
                if let Some(mut campaign) = registry_borrow.get(&campaign_id) {
                    campaign.budget += amount_clone;
                    registry_borrow.insert(campaign_id.clone(), campaign);
                }
            });
            
            Ok(format!("Campaign funded successfully. Transfer block index: {}", block_index))
        }
        Err(e) => Err(format!("Failed to transfer ICP: {}", e)),
    }
}

// Provider can withdraw their earnings with actual ICP transfer
#[ic_cdk::update]
async fn withdraw_provider_earnings(provider_id: String, amount: NumTokens) -> Result<String, String> {
    let caller_principal = caller();
    let amount_clone = amount.clone(); // Clone for later use
    
    // Verify the provider exists and the caller is the owner
    PROVIDER_REGISTRY.with(|registry| {
        match registry.borrow().get(&provider_id) {
            Some(provider) => {
                if provider.owner != caller_principal {
                    return Err("Unauthorized: You can only withdraw from your own provider account".to_string());
                }
                if provider.total_earnings < amount_clone {
                    return Err("Insufficient earnings to withdraw".to_string());
                }
                Ok(())
            }
            None => Err("Provider not found".to_string()),
        }
    })?;

    // Create account for the provider owner
    let provider_account = principal_to_account(caller_principal);
    
    // Transfer ICP from this canister to the provider
    let transfer_memo = format!("Provider withdrawal: {}", provider_id).into_bytes();
    match icp_transfer(
        None, // from_subaccount - uses canister's default
        provider_account, // to - provider's account
        Some(transfer_memo),
        amount,
    ).await {
        Ok(block_index) => {
            // If transfer successful, update the provider's earnings
            PROVIDER_REGISTRY.with(|registry| {
                let mut registry_borrow = registry.borrow_mut();
                if let Some(mut provider) = registry_borrow.get(&provider_id) {
                    provider.total_earnings -= amount_clone;
                    registry_borrow.insert(provider_id.clone(), provider);
                }
            });
            
            Ok(format!("Withdrawal successful. Transfer block index: {}", block_index))
        }
        Err(e) => Err(format!("Failed to transfer ICP: {}", e)),
    }
}

// Function to add earnings to a provider (called when campaign pays provider)
#[ic_cdk::update]
async fn pay_provider(campaign_id: String, provider_id: String, amount: NumTokens) -> Result<String, String> {
    let caller_principal = caller();
    let amount_clone1 = amount.clone();
    let amount_clone2 = amount.clone();
    let amount_clone3 = amount.clone();
    
    // Verify the campaign exists and the caller is the owner
    CAMPAIGN_REGISTRY.with(|registry| {
        match registry.borrow().get(&campaign_id) {
            Some(campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only pay from your own campaigns".to_string());
                }
                if campaign.budget < amount_clone1 {
                    return Err("Insufficient campaign budget".to_string());
                }
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })?;

    // Verify the provider exists
    PROVIDER_REGISTRY.with(|registry| {
        match registry.borrow().get(&provider_id) {
            Some(_) => Ok(()),
            None => Err("Provider not found".to_string()),
        }
    })?;

    // Update campaign budget
    CAMPAIGN_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        if let Some(mut campaign) = registry_borrow.get(&campaign_id) {
            campaign.budget -= amount_clone2;
            registry_borrow.insert(campaign_id.clone(), campaign);
        }
    });

    // Update provider earnings
    PROVIDER_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        if let Some(mut provider) = registry_borrow.get(&provider_id) {
            provider.total_earnings += amount_clone3;
            registry_borrow.insert(provider_id.clone(), provider);
        }
    });

    // Update or create earnings record
    let earnings_key = format!("{}:{}", provider_id, campaign_id);
    EARNINGS_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        match registry_borrow.get(&earnings_key) {
            Some(mut earnings) => {
                earnings.total_earned += amount.clone();
                registry_borrow.insert(earnings_key, earnings);
            }
            None => {
                let new_earnings = ProviderEarnings {
                    provider_id: provider_id.clone(),
                    campaign_id: campaign_id.clone(),
                    total_earned: amount.clone(),
                    last_withdrawal: None,
                };
                registry_borrow.insert(earnings_key, new_earnings);
            }
        }
    });

    Ok(format!("Payment of {} tokens made to provider {}", amount, provider_id))
}

// Only the campaign owner can withdraw funds from their campaign budget (emergency/unused funds)
#[ic_cdk::update]
async fn withdraw_campaign_funds(campaign_id: String, amount: NumTokens) -> Result<String, String> {
    let caller_principal = caller();
    let amount_clone = amount.clone();
    
    // Verify the campaign exists and the caller is the owner, then update budget
    CAMPAIGN_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        
        match registry_borrow.get(&campaign_id) {
            Some(mut campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only withdraw from your own campaigns".to_string());
                }
                
                if campaign.budget < amount_clone {
                    return Err("Insufficient funds".to_string());
                }
                
                campaign.budget -= amount_clone.clone();
                registry_borrow.insert(campaign_id.clone(), campaign);
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })?;

    // Create account for the campaign owner
    let owner_account = principal_to_account(caller_principal);
    
    // Transfer ICP from this canister to the campaign owner
    let transfer_memo = format!("Campaign withdrawal: {}", campaign_id).into_bytes();
    match icp_transfer(
        None, // from_subaccount - uses canister's default
        owner_account, // to - campaign owner's account
        Some(transfer_memo),
        amount,
    ).await {
        Ok(block_index) => {
            Ok(format!("Campaign funds withdrawal successful. Transfer block index: {}", block_index))
        }
        Err(e) => {
            // Rollback the budget change if transfer failed
            CAMPAIGN_REGISTRY.with(|registry| {
                let mut registry_borrow = registry.borrow_mut();
                if let Some(mut campaign) = registry_borrow.get(&campaign_id) {
                    campaign.budget += amount_clone;
                    registry_borrow.insert(campaign_id, campaign);
                }
            });
            Err(format!("Failed to transfer ICP: {}", e))
        }
    }
}

// Only the campaign owner can close their campaign
#[ic_cdk::update]
fn close_campaign(campaign_id: String) -> Result<(), String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        
        match registry_borrow.get(&campaign_id) {
            Some(campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only close your own campaigns".to_string());
                }
                
                registry_borrow.remove(&campaign_id);
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })
}

// Get provider earnings (only provider owner can see)
#[ic_cdk::query]
fn get_provider_earnings(provider_id: String) -> Result<NumTokens, String> {
    let caller_principal = caller();
    
    PROVIDER_REGISTRY.with(|registry| {
        match registry.borrow().get(&provider_id) {
            Some(provider) => {
                if provider.owner != caller_principal {
                    return Err("Unauthorized: You can only view your own provider earnings".to_string());
                }
                Ok(provider.total_earnings)
            }
            None => Err("Provider not found".to_string()),
        }
    })
}

// Get detailed earnings breakdown for a provider
#[ic_cdk::query]
fn get_provider_earnings_breakdown(provider_id: String) -> Result<Vec<ProviderEarnings>, String> {
    let caller_principal = caller();
    
    // Verify provider ownership
    PROVIDER_REGISTRY.with(|registry| {
        match registry.borrow().get(&provider_id) {
            Some(provider) => {
                if provider.owner != caller_principal {
                    return Err("Unauthorized: You can only view your own provider earnings".to_string());
                }
                Ok(())
            }
            None => return Err("Provider not found".to_string()),
        }
    })?;

    // Get all earnings for this provider
    EARNINGS_REGISTRY.with(|registry| {
        Ok(registry
            .borrow()
            .iter()
            .filter_map(|entry| {
                let earnings = entry.value();
                if earnings.provider_id == provider_id {
                    Some(earnings)
                } else {
                    None
                }
            })
            .collect())
    })
}

// Get campaign balance (only campaign owner can see)
#[ic_cdk::query]
fn get_campaign_balance(campaign_id: String) -> Result<NumTokens, String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        match registry.borrow().get(&campaign_id) {
            Some(campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only view your own campaign balance".to_string());
                }
                Ok(campaign.budget)
            }
            None => Err("Campaign not found".to_string()),
        }
    })
}

#[ic_cdk::update]
fn add_provider(campaign_id: String, _provider_id: String) -> Result<(), String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        let registry_borrow = registry.borrow();
        
        match registry_borrow.get(&campaign_id) {
            Some(campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only modify your own campaigns".to_string());
                }
                
                // Here you would add logic to associate the provider with the campaign
                // This might involve updating the campaign's locations or maintaining
                // a separate mapping of campaign-provider relationships
                
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })
}

#[ic_cdk::update]
fn remove_provider(campaign_id: String, _provider_id: String) -> Result<(), String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        let registry_borrow = registry.borrow();
        
        match registry_borrow.get(&campaign_id) {
            Some(campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only modify your own campaigns".to_string());
                }
                
                // Logic to remove provider association
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })
}

// Returns only campaigns created by the caller (PRIVATE)
#[ic_cdk::query]
fn get_my_campaigns() -> Vec<Campaign> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        registry
            .borrow()
            .iter()
            .filter_map(|entry| {
                let campaign = entry.value();
                if campaign.owner == caller_principal {
                    Some(campaign)
                } else {
                    None
                }
            })
            .collect()
    })
}

#[ic_cdk::query]
fn get_my_providers() -> Vec<Provider> {
    let caller_principal = caller();
    
    PROVIDER_REGISTRY.with(|registry| {
        registry
            .borrow()
            .iter()
            .filter_map(|entry| {
                let provider = entry.value();
                if provider.owner == caller_principal {
                    Some(provider)
                } else {
                    None
                }
            })
            .collect()
    })
}

#[ic_cdk::query]
fn get_all_providers() -> Vec<Provider> {
    PROVIDER_REGISTRY.with(|registry| {
        registry
            .borrow()
            .iter()
            .map(|entry| entry.value())
            .collect()
    })
}

#[ic_cdk::query]
fn get_all_locations() -> Vec<Location> {
    PROVIDER_REGISTRY.with(|registry| {
        registry
            .borrow()
            .iter()
            .flat_map(|entry| entry.value().locations.clone())
            .collect()
    })
}

// Get providers for a specific campaign (only if caller owns the campaign)
#[ic_cdk::query]
fn get_providers_for_campaign(campaign_id: String) -> Result<Vec<Provider>, String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        match registry.borrow().get(&campaign_id) {
            Some(campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only view your own campaigns".to_string());
                }
                
                // Here you would return the providers associated with this campaign
                // This requires additional logic to track campaign-provider relationships
                Ok(Vec::new()) // Placeholder
            }
            None => Err("Campaign not found".to_string()),
        }
    })
}



ic_cdk::export_candid!();


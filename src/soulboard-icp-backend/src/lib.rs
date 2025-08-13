use std::{cell::RefCell, borrow::Cow};
use ic_cdk::{init, caller};
use rand::rngs::StdRng;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable, storable::Bound};
use candid::{CandidType, Deserialize, Encode, Decode, Principal};
use icrc_ledger_types::icrc1::transfer::{BlockIndex, Memo, NumTokens, TransferArg, TransferError};

type Memory = VirtualMemory<DefaultMemoryImpl>;
const MAX_VALUE_SIZE: u32 = 500; // Increased size for additional data

#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

#[derive(CandidType, Deserialize, Clone)]
struct Provider {
    id: String,
    name: String,
    owner: Principal, // Track who owns this provider
    locations: Vec<Location>,
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

// Only the campaign owner can fund their campaign
#[ic_cdk::update]
fn fund_campaign(campaign_id: String, amount: u64) -> Result<(), String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        
        match registry_borrow.get(&campaign_id) {
            Some(mut campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only fund your own campaigns".to_string());
                }
                
                campaign.budget += amount;
                registry_borrow.insert(campaign_id, campaign);
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })
}

// Only the campaign owner can withdraw funds
#[ic_cdk::update]
fn withdraw_funds(campaign_id: String, amount: u64) -> Result<(), String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        
        match registry_borrow.get(&campaign_id) {
            Some(mut campaign) => {
                if campaign.owner != caller_principal {
                    return Err("Unauthorized: You can only withdraw from your own campaigns".to_string());
                }
                
                if campaign.budget < amount {
                    return Err("Insufficient funds".to_string());
                }
                
                campaign.budget -= amount;
                registry_borrow.insert(campaign_id, campaign);
                Ok(())
            }
            None => Err("Campaign not found".to_string()),
        }
    })
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

#[ic_cdk::update]
fn add_provider(campaign_id: String, provider_id: String) -> Result<(), String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        
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
fn remove_provider(campaign_id: String, provider_id: String) -> Result<(), String> {
    let caller_principal = caller();
    
    CAMPAIGN_REGISTRY.with(|registry| {
        let mut registry_borrow = registry.borrow_mut();
        
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
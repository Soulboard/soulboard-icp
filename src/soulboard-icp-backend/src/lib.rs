use std::{cell::RefCell, os::unix::thread};
use ic_cdk::init;
use rand::rngs::StdRng;
use candid::{ CandidType, Deserialize};
use candid::Principal;
use icrc_ledger_types::icrc1::transfer::{BlockIndex, Memo, NumTokens, TransferArg, TransferError};

thread_local! {

}

#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

#[derive(CandidType, Deserialize)]
struct Provider {
    id: String,
    name: String,
    locations : Vec<Location>,

}

#[derive(CandidType, Deserialize)]
struct Location {
    id : String , // same as the device id used for getting the metrics using the HTTPS Outcalls 
    name : String ,
    image : String, 
    base_fees : NumTokens, // base fees for the location
    views : u64, // number of views for the location
    status : LocationStatus
}

#[derive(CandidType, Deserialize)]
struct Campaign { 
    id : String , 
    name : String ,
    description : String,
    image : String,
    locations : Vec<Location>,
    budget : NumTokens,
}

#[derive(CandidType, Deserialize)]
enum LocationStatus {
    Active, //Currently looking for bookings 
    Inactive , // Not active or under maintainance 
    Booked , // Booked by a campaign 
}

#[derive(CandidType, Deserialize)]
enum CampaignStatus {
    Active , 
    Paused // when paused the location providers can be set to active 
}




// lemme breakdown what's happening 
// 1. Ad service Providers can register which will create the provider struct and keep them in the cannister 
//2. Advertisers can create new campaigns and fund them with ICP
//3 . Advertisers can add new providers in the campaign
//4. Advertisers can remove providers from the campaign
//Payments will be dispersed according to the clock using the icp::timers
// use the ic stable structures for managing data and state across the cannister 


//Registers a new provider for the wallet id in the cannister 
//
#[ic_cdk::update]
fn register_provider() -> Result<(), String> {
    // let ledger_id = Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap();

    Ok(())
}

#[ic_cdk::update]
fn create_campaign(name: String, description: String) -> Result<(), String> {
    Ok(())
}

#[ic_cdk::update]
fn fund_campaign(campaign_id: String, amount: u64) -> Result<(), String> {
    Ok(())
}

#[ic_cdk::update]
fn withdraw_funds(campaign_id: String, amount: u64) -> Result<(), String> {
    Ok(())
}

#[ic_cdk::update]
fn close_campaign(campaign_id: String) -> Result<(), String> {
    Ok(())
}

#[ic_cdk::update]
fn add_provider(campaign_id: String, provider_id: String) -> Result<(), String> {
    Ok(())
}   

#[ic_cdk::update]
fn remove_provider(campaign_id: String, provider_id: String) -> Result<(), String> {
    Ok(())
}

#[ic_cdk::query]
fn get_campaigns() -> Vec<Campaign> {
    Vec::new()
}

#[ic_cdk::query]
fn get_providers_for_campaign(campaign_id: String) -> Vec<Provider> {
    Vec::new()
}

#[ic_cdk::query]
fn get_providers() -> Vec<Provider> {
    Vec::new()
}   


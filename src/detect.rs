use ethers::types::{Address, Log, U256};
use ethers::utils::keccak256;

use crate::constants::{
    flap_impl_tax_v1, flap_portal, fourmeme_helper_v3,
    fourmeme_v1, fourmeme_v2, selector_flap_new_token_v2, selector_flap_new_token_v3,
    selector_flap_new_token_v4, selector_flap_new_token_v5, selector_fourmeme_create_token_bytes,
    token_create_topic, token_created_topic, FLAP_CREATE2_PREFIX, FLAP_CREATE2_SUFFIX,
};

fn is_flap_creator_selector(sel: &[u8; 4]) -> bool {
    
}

fn flap_portal_lower() -> String {
}

pub fn is_fourmeme_target(to: &str) -> bool {
    
}

pub fn is_flap_launch(to: &str, data: &[u8]) -> bool {
    
}

pub fn is_fourmeme_launch(to: &str, data: &[u8]) -> bool {
    
}

fn to_addr_from_tail(data: &[u8]) -> Option<Address> {
    
}

pub fn flap_token_from_calldata(data: &[u8]) -> Option<Address> {
    
}

fn flap_create2_with_impl(salt: &[u8], impl_addr: Address) -> Option<Address> {
    
}

fn flap_create2_address(salt: &[u8]) -> Option<Address> {
    
}

fn is_token_created_emitter(addr: &Address) -> bool {
    
}

fn is_fourmeme_token_created_emitter(addr: &Address) -> bool {
    
}

pub fn token_from_receipt_logs(logs: &[Log]) -> Option<Address> {
    
}

pub fn flap_token_from_receipt(logs: &[Log]) -> Option<Address> {
    
}

pub fn fourmeme_token_from_receipt(logs: &[Log]) -> Option<Address> {
    
}

fn token_from_token_create_data(data: &[u8]) -> Option<Address> {
    
}

pub fn fourmeme_token_from_token_create_logs(logs: &[Log]) -> Option<Address> {
    
}

pub fn fourmeme_token_from_any_token_create_log(logs: &[Log]) -> Option<Address> {
    

pub fn fourmeme_only_token_from_receipt(logs: &[Log]) -> Option<Address> {
    
}

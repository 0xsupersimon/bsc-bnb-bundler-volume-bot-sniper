
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, TransactionRequest, U256};

use crate::constants::{
    flap_portal, fourmeme_v2, selector_flap_swap_exact_input,
    selector_fourmeme_buy_token_amap, zero_address,
};

pub fn build_flap_buy(
    token: Address,
    bnb_wei: U256,
    _min_out: u64,
) -> Option<TransactionRequest> {
    
}

pub fn build_fourmeme_buy(token: Address, bnb_wei: U256, min_amount: U256) -> Option<TransactionRequest> {
    
}

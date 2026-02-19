
use ethers::types::Address;

pub const BSC_CHAIN_ID: u64 = 56;

pub const BUY_GAS_LIMIT: u64 = 600_000;

// Flap
pub fn flap_portal() -> Address {
    "0xe2cE6ab80874Fa9Fa2aAE65D277Dd6B8e65C9De0".parse().unwrap()
}
pub fn flap_impl_standard() -> Address {
    "0x8b4329947e34b6d56d71a3385cac122bade7d78d".parse().unwrap()
}
pub fn flap_impl_tax_v1() -> Address {
    "0x29e6383F0ce68507b5A72a53c2B118a118332aA8".parse().unwrap()
}
pub fn flap_impl_tax_v2() -> Address {
    "0xae562c6A05b798499507c6276C6Ed796027807BA".parse().unwrap()
}
pub const FLAP_CREATE2_PREFIX: &str = "3d602d80600a3d3981f3363d3d373d3d3d363d73";
pub const FLAP_CREATE2_SUFFIX: &str = "5af43d82803e903d91602b57fd5bf3";

// FourMeme
pub fn fourmeme_v1() -> Address {
    "0xec4549cadce5da21df6e6422d448034b5233bfbc".parse().unwrap()
}
pub fn fourmeme_v2() -> Address {
    "0x5c952063c7fc8610ffdb798152d69f0b9550762b".parse().unwrap()
}
pub fn fourmeme_helper_v3() -> Address {
    "0xf251f83e40a78868fcfa3fa4599dad6494e46034".parse().unwrap()
}

pub fn zero_address() -> Address {
    Address::zero()
}

fn selector(sig: &str) -> [u8; 4] {
    let h = ethers::utils::keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

pub fn selector_flap_swap_exact_input() -> [u8; 4] {
    selector("swapExactInput((address,address,uint256,uint256,bytes))")
}
pub fn selector_flap_new_token_v2() -> [u8; 4] {
    selector("newTokenV2((string,string,string,uint8,bytes32,uint16,uint8,address,uint256,address,bytes))")
}
pub fn selector_flap_new_token_v3() -> [u8; 4] {
    selector("newTokenV3((string,string,string,uint8,bytes32,uint16,uint8,address,uint256,address,bytes,bytes32,bytes))")
}
pub fn selector_flap_new_token_v4() -> [u8; 4] {
    selector("newTokenV4((string,string,string,uint8,bytes32,uint16,uint8,address,uint256,address,bytes,bytes32,bytes,uint8,uint8))")
}
pub fn selector_flap_new_token_v5() -> [u8; 4] {
    selector("newTokenV5((string,string,string,uint8,bytes32,uint16,uint8,address,uint256,address,bytes,bytes32,bytes,uint8,uint8,uint64,uint64,uint16,uint16,uint16,uint16,uint256))")
}
pub fn selector_fourmeme_create_token_bytes() -> [u8; 4] {
    selector("createToken(bytes,bytes)")
}
pub fn selector_fourmeme_try_buy() -> [u8; 4] {
    selector("tryBuy(address,uint256,uint256)")
}
pub fn selector_fourmeme_buy_token_amap() -> [u8; 4] {
    selector("buyTokenAMAP(address,uint256,uint256)")
}

pub fn token_created_topic() -> [u8; 32] {
    ethers::utils::keccak256("TokenCreated(uint256,address,uint256,address,string,string,string)").into()
}

pub fn token_create_topic() -> [u8; 32] {
    ethers::utils::keccak256("TokenCreate(address,address,uint256,string,string,uint256,uint256,uint256)").into()
}

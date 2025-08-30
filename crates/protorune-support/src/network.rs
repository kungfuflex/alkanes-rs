use anyhow::Result;
use bech32::Hrp;
use bitcoin::Script;
// use metashrew_support::address::{AddressEncoding, Payload};
use bitcoin::address::Address;
use bitcoin::Network;

pub struct AddressEncoding<'a> {
	pub p2pkh_prefix: u8,
	pub p2sh_prefix: u8,
	pub hrp: bech32::Hrp,
	pub payload: &'a Address,
}

impl<'a> AddressEncoding<'a> {
	pub fn to_string(&self) -> String {
		self.payload.to_string()
	}
}
static mut _NETWORK: Option<NetworkParams> = None;

#[derive(Clone, Debug, Default)]
pub struct NetworkParams {
    pub bech32_prefix: String,
    pub p2pkh_prefix: u8,
    pub p2sh_prefix: u8,
}

#[allow(static_mut_refs)]
pub fn set_network(params: NetworkParams) {
    unsafe {
        _NETWORK = Some(params);
    }
}

#[allow(static_mut_refs)]
pub fn get_network() -> &'static NetworkParams {
    unsafe { _NETWORK.as_ref().unwrap() }
}

#[allow(static_mut_refs)]
pub fn get_network_option() -> Option<&'static NetworkParams> {
    unsafe { _NETWORK.as_ref().clone() }
}

pub fn to_address_str(script: &Script) -> Result<String> {
    let config = get_network();
    Ok(AddressEncoding {
        p2pkh_prefix: config.p2pkh_prefix,
        p2sh_prefix: config.p2sh_prefix,
        hrp: Hrp::parse_unchecked(&config.bech32_prefix),
        payload: &Address::from_script(script, Network::Bitcoin)?,
    }
    .to_string())
}

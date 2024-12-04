use bitcoin::{Script};
use metashrew_support::address::{AddressEncoding, Payload};
use bech32::{Hrp};
static mut _NETWORK: Option<NetworkParams> = None;

pub struct NetworkParams {
  pub bech32_prefix: String,
  pub p2pkh_prefix: u8,
  pub p2sh_prefix: u8,
}

pub fn set_network(params: NetworkParams) {
  unsafe {
    _NETWORK = Some(params);
  }
}

pub fn get_network() -> &'static NetworkParams {
  unsafe { _NETWORK.as_ref().unwrap() }
}

pub fn to_address_str(script: &Script) -> Option<String> {
    let config = get_network();
    Some(format!("{}", AddressEncoding {
      p2pkh_prefix: config.p2pkh_prefix,
      p2sh_prefix: config.p2sh_prefix,
      hrp: Hrp::parse_unchecked(&config.bech32_prefix),
      payload: &Payload::from_script(script).ok()?
    }))
}
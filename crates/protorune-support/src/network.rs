/*
 * Chadson's Journal:
 *
 * The previous implementation used a custom `NetworkParams` struct which was causing
 * compilation errors because it didn't implement `AsRef<bitcoin::network::Params>`.
 * This was a problem for `bitcoin::Address::from_script`.
 *
 * To fix this, I'm replacing the custom struct and its associated functions with
 * the standard `bitcoin::Network` enum. This enum is designed to work with the
 * `bitcoin` library's functions and will resolve the trait bound issue.
 *
 * The `get_network` function now returns a `bitcoin::Network` value directly.
 * If no network is set, it defaults to `Network::Bitcoin`. This is a safe
 * default for production environments.
 *
 * This change simplifies the code and ensures compatibility with the `bitcoin` crate.
 */
use bitcoin::Network;

static mut _NETWORK: Option<Network> = None;

#[allow(static_mut_refs)]
pub fn set_network(network: Network) {
    unsafe {
        _NETWORK = Some(network);
    }
}

#[allow(static_mut_refs)]
pub fn get_network() -> Network {
    unsafe { _NETWORK.unwrap_or(Network::Bitcoin) }
}

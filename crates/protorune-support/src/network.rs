//! Process-wide address-encoding configuration.
//!
//! The configuration is held as an `Arc<NetworkParams>` behind a `RwLock`.
//! Readers get an owned `Arc` snapshot rather than a borrow of the global, so
//! replacing the configuration with [`set_network`] can never invalidate a value
//! a caller is still holding: the old parameters stay alive until the last
//! snapshot of them is dropped. No lock guard is ever handed out, so callers
//! cannot deadlock a later `set_network` by holding on to a read.
//!
//! Replacement is deliberately allowed: the indexer re-applies its
//! (feature-selected) configuration at every exported entry point, and tests
//! switch networks. Every configuration is validated before it is installed.

use anyhow::{anyhow, bail, Result};
use bech32::Hrp;
use bitcoin::Script;
use metashrew_support::address::{AddressEncoding, Payload};
use std::sync::{Arc, PoisonError, RwLock};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkParams {
    pub bech32_prefix: String,
    pub p2pkh_prefix: u8,
    pub p2sh_prefix: u8,
}

impl NetworkParams {
    /// Build a validated configuration.
    pub fn new(bech32_prefix: impl Into<String>, p2pkh_prefix: u8, p2sh_prefix: u8) -> Result<Self> {
        let params = Self {
            bech32_prefix: bech32_prefix.into(),
            p2pkh_prefix,
            p2sh_prefix,
        };
        params.validate()?;
        Ok(params)
    }

    /// Check that the prefixes can produce unambiguous addresses: the bech32
    /// human-readable part must be a valid, lowercase HRP, and the base58
    /// version bytes for P2PKH and P2SH must differ.
    pub fn validate(&self) -> Result<()> {
        let hrp = Hrp::parse(&self.bech32_prefix)
            .map_err(|e| anyhow!("invalid bech32 prefix {:?}: {}", self.bech32_prefix, e))?;
        if hrp.to_lowercase() != self.bech32_prefix {
            bail!(
                "bech32 prefix {:?} must be lowercase",
                self.bech32_prefix
            );
        }
        if self.p2pkh_prefix == self.p2sh_prefix {
            bail!(
                "p2pkh and p2sh prefixes must differ (both are {:#04x})",
                self.p2pkh_prefix
            );
        }
        Ok(())
    }
}

/// Synchronised holder for the active configuration. Exists as a type (rather
/// than bare functions over a static) so the behaviour can be tested on private
/// instances without racing other tests on the process-wide one.
struct NetworkSlot {
    current: RwLock<Option<Arc<NetworkParams>>>,
}

impl NetworkSlot {
    const fn new() -> Self {
        Self {
            current: RwLock::new(None),
        }
    }

    /// Validate and install `params`, returning the configuration it replaced.
    /// On error nothing is changed.
    fn set(&self, params: Arc<NetworkParams>) -> Result<Option<Arc<NetworkParams>>> {
        params.validate()?;
        // The critical sections only move an `Option<Arc<_>>`, which cannot be
        // left half-written by a panic, so a poisoned lock is still consistent.
        let previous = {
            let mut slot = self.current.write().unwrap_or_else(PoisonError::into_inner);
            slot.replace(params)
        };
        // `previous` is dropped by the caller, outside the lock.
        Ok(previous)
    }

    fn get(&self) -> Option<Arc<NetworkParams>> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[cfg(test)]
    fn clear(&self) -> Option<Arc<NetworkParams>> {
        self.current
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

static NETWORK: NetworkSlot = NetworkSlot::new();

/// Validate and install `params` as the active configuration, replacing any
/// previous one. Snapshots obtained earlier keep the values they were taken with.
pub fn try_set_network(params: impl Into<Arc<NetworkParams>>) -> Result<()> {
    NETWORK.set(params.into()).map(drop)
}

/// Like [`try_set_network`], but panics if `params` is invalid. Intended for
/// the compile-time configurations used by the indexer and tests, where an
/// invalid value is a programming error.
pub fn set_network(params: impl Into<Arc<NetworkParams>>) {
    if let Err(e) = try_set_network(params) {
        panic!("set_network: {}", e);
    }
}

/// A snapshot of the active configuration.
///
/// # Panics
/// If no configuration has been installed. Use [`get_network_option`] to
/// handle that case.
pub fn get_network() -> Arc<NetworkParams> {
    get_network_option()
        .expect("network not configured: call set_network() before get_network()")
}

/// A snapshot of the active configuration, or `None` before initialisation.
pub fn get_network_option() -> Option<Arc<NetworkParams>> {
    NETWORK.get()
}

/// Encode `script` as an address for the active network.
///
/// # Panics
/// If no network has been configured, as before; the indexer configures the
/// network at every entry point.
pub fn to_address_str(script: &Script) -> Result<String> {
    let config = get_network();
    Ok(AddressEncoding {
        p2pkh_prefix: config.p2pkh_prefix,
        p2sh_prefix: config.p2sh_prefix,
        // Already validated when the configuration was installed.
        hrp: Hrp::parse_unchecked(&config.bech32_prefix),
        payload: &Payload::from_script(script)?,
    }
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn mainnet() -> NetworkParams {
        NetworkParams::new("bc", 0x00, 0x05).unwrap()
    }

    fn regtest() -> NetworkParams {
        NetworkParams::new("bcrt", 0x64, 0xc4).unwrap()
    }

    #[test]
    fn access_before_initialisation_is_none() {
        let slot = NetworkSlot::new();
        assert!(slot.get().is_none());
    }

    #[test]
    fn snapshot_survives_replacement() {
        let slot = NetworkSlot::new();
        slot.set(Arc::new(mainnet())).unwrap();
        let snapshot = slot.get().unwrap();
        let prefix: &str = &snapshot.bech32_prefix;

        // Replace it twice, and drop the value `set` hands back, so nothing but
        // `snapshot` still refers to the original parameters.
        drop(slot.set(Arc::new(regtest())).unwrap());
        drop(slot.set(Arc::new(regtest())).unwrap());

        assert_eq!(prefix, "bc");
        assert_eq!(*snapshot, mainnet());
        assert_eq!(Arc::strong_count(&snapshot), 1);
        assert_eq!(*slot.get().unwrap(), regtest());
    }

    #[test]
    fn repeated_initialisation_replaces_and_returns_previous() {
        let slot = NetworkSlot::new();
        assert!(slot.set(Arc::new(regtest())).unwrap().is_none());
        // Same value again is fine (the indexer does this on every call).
        assert_eq!(*slot.set(Arc::new(regtest())).unwrap().unwrap(), regtest());
        // A different value replaces it.
        assert_eq!(*slot.set(Arc::new(mainnet())).unwrap().unwrap(), regtest());
        assert_eq!(*slot.get().unwrap(), mainnet());
    }

    #[test]
    fn invalid_configuration_is_rejected_and_leaves_current_in_place() {
        let slot = NetworkSlot::new();
        slot.set(Arc::new(mainnet())).unwrap();
        for bad in [
            NetworkParams::default(),
            NetworkParams { bech32_prefix: "BC".into(), p2pkh_prefix: 0, p2sh_prefix: 5 },
            NetworkParams { bech32_prefix: "b c".into(), p2pkh_prefix: 0, p2sh_prefix: 5 },
            NetworkParams { bech32_prefix: "bc".into(), p2pkh_prefix: 5, p2sh_prefix: 5 },
        ] {
            assert!(slot.set(Arc::new(bad.clone())).is_err(), "{:?} accepted", bad);
            assert_eq!(*slot.get().unwrap(), mainnet());
        }
        assert!(NetworkParams::new("", 0, 5).is_err());
    }

    #[test]
    fn clear_returns_to_uninitialised() {
        let slot = NetworkSlot::new();
        slot.set(Arc::new(mainnet())).unwrap();
        let held = slot.get().unwrap();
        assert!(slot.clear().is_some());
        assert!(slot.get().is_none());
        assert_eq!(*held, mainnet());
    }

    #[test]
    fn concurrent_readers_and_writers_see_whole_configurations() {
        let slot = NetworkSlot::new();
        slot.set(Arc::new(mainnet())).unwrap();
        let stop = AtomicBool::new(false);
        let (a, b) = (mainnet(), regtest());

        std::thread::scope(|s| {
            for i in 0..2 {
                let (slot, stop) = (&slot, &stop);
                let (a, b) = (a.clone(), b.clone());
                s.spawn(move || {
                    let mut n = 0u32;
                    while !stop.load(Ordering::Relaxed) {
                        let next = if (n + i) % 2 == 0 { &a } else { &b };
                        slot.set(Arc::new(next.clone())).unwrap();
                        n += 1;
                    }
                });
            }
            let readers: Vec<_> = (0..4)
                .map(|_| {
                    let slot = &slot;
                    let (a, b) = (a.clone(), b.clone());
                    s.spawn(move || {
                        for _ in 0..20_000 {
                            let snap = slot.get().expect("initialised before threads start");
                            // Each snapshot is exactly one of the installed values,
                            // and stays readable while writers keep replacing it.
                            assert!(*snap == a || *snap == b, "torn config {:?}", snap);
                            assert!(snap.bech32_prefix == "bc" || snap.bech32_prefix == "bcrt");
                        }
                    })
                })
                .collect();
            for r in readers {
                r.join().unwrap();
            }
            stop.store(true, Ordering::Relaxed);
        });
    }

    // The only test in this crate that touches the process-wide slot.
    #[test]
    fn global_api_round_trip() {
        set_network(regtest());
        let before = get_network();
        set_network(mainnet());
        assert_eq!(*before, regtest());
        assert_eq!(*get_network(), mainnet());
        assert_eq!(get_network_option().as_deref(), Some(&mainnet()));

        assert!(try_set_network(NetworkParams::default()).is_err());
        assert_eq!(*get_network(), mainnet());

        let p2wpkh = hex::decode("001410cacbc34f4681fddbc68b5be4465d8bdc45c2a7").unwrap();
        set_network(regtest());
        assert_eq!(
            to_address_str(Script::from_bytes(&p2wpkh)).unwrap(),
            "bcrt1qzr9vhs60g6qlmk7x3dd7g3ja30wyts48sxuemv"
        );
    }

    #[test]
    #[should_panic(expected = "set_network")]
    fn set_network_panics_on_invalid_params() {
        set_network(NetworkParams::default());
    }
}

//! Adapter — exposes the new-protocol `WalletConnectSigner` (from
//! `alkanes_cli_common::wc_signer`) as the
//! `alkanes_cli_common::traits::RemoteSigner` trait.
//!
//! Replaces the old `subfrost_wc::signer::WalletConnectSigner` adapter
//! that lived here. Same call shape, same Mutex-guarded interior — only
//! the underlying transport changed (wc-relay → frtun-pair `/v1/pair`).
//!
//! Wire protocol exactly matches the SUBFROST mobile vc=419 listener
//! and the `@alkanes/ts-sdk` walletconnect-cli sender.

use std::sync::Arc;

use alkanes_cli_common::traits::RemoteSigner;
use alkanes_cli_common::wc_signer::{
    storage::NativeFileStorage,
    transport::NativeTransport,
    WalletConnectSigner,
};
use alkanes_cli_common::AlkanesError;
use bitcoin::psbt::Psbt;
use tokio::sync::Mutex;

/// Concrete native instantiation of the generic signer driver.
pub type NativeWcSigner = WalletConnectSigner<NativeTransport, NativeFileStorage>;

/// `RemoteSigner` impl backed by the new-protocol signer. Holds the
/// signer in a `Mutex` because the trait is `&self` but the WC UX
/// assumes one outstanding sign request at a time per session.
pub struct WcRemoteSigner {
    inner: Arc<Mutex<NativeWcSigner>>,
}

impl WcRemoteSigner {
    pub fn new(signer: NativeWcSigner) -> Self {
        Self {
            inner: Arc::new(Mutex::new(signer)),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl RemoteSigner for WcRemoteSigner {
    async fn sign_psbt(
        &self,
        psbt: &Psbt,
        addresses: &[String],
    ) -> Result<Psbt, AlkanesError> {
        // PSBTs go over the wire as hex — matches the TS sender's
        // `signPsbt(psbtHex, addresses, ...)` shape exactly.
        let psbt_bytes = psbt.serialize();
        let psbt_hex = hex::encode(&psbt_bytes);

        let signed_hex = {
            let mut guard = self.inner.lock().await;
            guard
                .sign_psbt(psbt_hex, addresses.to_vec())
                .await
                .map_err(|e| AlkanesError::Wallet(format!("walletconnect sign_psbt: {e}")))?
        };

        let signed_bytes = hex::decode(&signed_hex)
            .map_err(|e| AlkanesError::Wallet(format!("wallet returned non-hex PSBT: {e}")))?;
        Psbt::deserialize(&signed_bytes)
            .map_err(|e| AlkanesError::Wallet(format!("wallet returned malformed PSBT: {e}")))
    }

    async fn get_addresses(&self) -> Result<Vec<String>, AlkanesError> {
        let mut guard = self.inner.lock().await;
        if !guard.accounts().is_empty() {
            return Ok(guard.accounts());
        }
        guard
            .get_accounts()
            .await
            .map_err(|e| AlkanesError::Wallet(format!("walletconnect get_accounts: {e}")))
    }

    fn backend_name(&self) -> &'static str {
        "walletconnect"
    }
}

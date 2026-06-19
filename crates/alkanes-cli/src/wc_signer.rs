//! Adapter that exposes a paired `subfrost_wc::WalletConnectSigner` as
//! the `alkanes_cli_common::traits::RemoteSigner` trait.
//!
//! Lives in `alkanes-cli` (not in `subfrost-wc`) so the vendored
//! WalletConnect crate stays generic and doesn't need to know about
//! `alkanes-cli-common`'s trait shapes.
//!
//! Lifetime model: the adapter owns the signer (it's a one-shot per CLI
//! invocation; we re-attach to the relay at startup, sign, exit). The
//! relay's background reader task stays alive for the lifetime of the
//! `WalletConnectSigner` it was spawned with.

use std::sync::Arc;

use alkanes_cli_common::traits::RemoteSigner;
use alkanes_cli_common::AlkanesError;
use bitcoin::psbt::Psbt;
use subfrost_wc::signer::WalletConnectSigner;
use tokio::sync::Mutex;

/// `RemoteSigner` impl backed by a `WalletConnectSigner`. Holds the
/// signer in a `Mutex` because the trait is `&self` but we want serial
/// access to the relay (only one outstanding sign request at a time —
/// the mobile UX assumes that).
pub struct WcRemoteSigner {
    inner: Arc<Mutex<WalletConnectSigner>>,
}

impl WcRemoteSigner {
    pub fn new(signer: WalletConnectSigner) -> Self {
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
        // PSBTs go over the wire as hex (matching the JS SDK's wire
        // shape — `signPsbt(psbt_hex, addresses, ...)`).
        let psbt_bytes = psbt.serialize();
        let psbt_hex = hex::encode(&psbt_bytes);

        let signed_hex = {
            let guard = self.inner.lock().await;
            guard
                .sign_psbt(&psbt_hex, addresses.to_vec())
                .await
                .map_err(|e| AlkanesError::Wallet(format!("walletconnect sign_psbt: {e}")))?
        };

        let signed_bytes = hex::decode(&signed_hex)
            .map_err(|e| AlkanesError::Wallet(format!("wallet returned non-hex PSBT: {e}")))?;
        Psbt::deserialize(&signed_bytes)
            .map_err(|e| AlkanesError::Wallet(format!("wallet returned malformed PSBT: {e}")))
    }

    async fn get_addresses(&self) -> Result<Vec<String>, AlkanesError> {
        let guard = self.inner.lock().await;
        // Prefer the cached account list from the persisted session;
        // fall back to a fresh getAccounts RPC if empty.
        if !guard.accounts().is_empty() {
            return Ok(guard.accounts().to_vec());
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

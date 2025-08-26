use alkanes_support::prelude::*;
use types_support::{Payment, deserialize_payments};
use alkanes_support::proto::alkanes::{SimulateResponse, MessageContextParcel};
use protorune_support::proto::protorune::{ProtorunesWalletRequest, WalletResponse, Outpoint};
use prost::Message;
use std::collections::HashMap;
use bitcoin::{OutPoint, Txid, Address, network::Network, secp256k1::{Secp256k1, XOnlyPublicKey}, TapTweak};

const FR_BTC_ALKANE_ID: AlkaneId = AlkaneId { block: 32, tx: 0 };

fn into_outpoint(v: Outpoint) -> OutPoint {
  OutPoint {
      txid: Txid::from_byte_array(v.txid.as_slice().try_into().expect("v.txid should be a 32 byte array")),
      vout: v.vout
  }
}

use metashrew_core::index_pointer::IndexPointer;

#[metashrew_view]
pub fn unwrap() -> Result<Vec<u8>> {
    let signer_pk_bytes = IndexPointer::from_keyword("/signer").get();
    if signer_pk_bytes.is_empty() {
        return Err(anyhow!("Signer public key not found"));
    }

    let network = if cfg!(feature = "test-hooks") {
        Network::Regtest
    } else {
        Network::Bitcoin
    };

    let signer_address = {
        let x_only_pk = XOnlyPublicKey::from_slice(&signer_pk_bytes)?;
        let secp = Secp256k1::new();
        let (tweaked_pubkey, _) = x_only_pk.tap_tweak(&secp, None);
        Address::p2tr_tweaked(tweaked_pubkey, network)
    };

    let current_height = unsafe { height() };
    let mut payments: Vec<Payment> = Vec::new();
    // Iterate over the last 10 blocks to find unresolved payments
    for i in 0..10 {
        let height_to_check = current_height - i;
        let payment_data = IndexPointer::from_keyword("/payments/byheight/")
            .select_value(height_to_check)
            .get_list();
        for p_data in payment_data {
            if let Ok(deserialized_payments) = deserialize_payments(&p_data) {
                payments.extend(deserialized_payments);
            }
        }
    }

    // Get Spendable UTXOs for the signer address
    let wallet_request = ProtorunesWalletRequest {
        protocol_tag: Some((0u128).into()),
        wallet: signer_address.to_string().into_bytes(),
    };
    let wallet_request_bytes = wallet_request.encode_to_vec();
    
    let wallet_response_bytes = unsafe { view("spendablesbyaddress".to_owned(), wallet_request_bytes)? };
    let wallet: WalletResponse = WalletResponse::decode(wallet_response_bytes.as_slice())?;
    let outpoints: Vec<OutPoint> = wallet.outpoints.into_iter().filter_map(|o| o.outpoint).map(into_outpoint).collect();
    
    // Filter payments
    payments.retain(|payment| outpoints.contains(&payment.spendable));

    let mut serialized_payments = Vec::new();
    for payment in payments {
        serialized_payments.extend(payment.serialize()?);
    }
    Ok(serialized_payments)
}
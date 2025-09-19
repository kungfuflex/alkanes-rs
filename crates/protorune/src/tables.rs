use metashrew_support::index_pointer::{IndexPointer, KeyValuePointer};
use metashrew_support::environment::RuntimeEnvironment;
use std::marker::PhantomData;

#[allow(non_snake_case)]
#[derive(Clone)]
pub struct RuneTable<E: RuntimeEnvironment> {
    pub HEIGHT_TO_BLOCKHASH: IndexPointer<E>,
    pub BLOCKHASH_TO_HEIGHT: IndexPointer<E>,
    pub OUTPOINT_TO_RUNES: IndexPointer<E>,
    pub OUTPOINT_TO_HEIGHT: IndexPointer<E>,
    pub HEIGHT_TO_TRANSACTION_IDS: IndexPointer<E>,
    pub SYMBOL: IndexPointer<E>,
    pub CAP: IndexPointer<E>,
    pub SPACERS: IndexPointer<E>,
    pub OFFSETEND: IndexPointer<E>,
    pub OFFSETSTART: IndexPointer<E>,
    pub HEIGHTSTART: IndexPointer<E>,
    pub HEIGHTEND: IndexPointer<E>,
    pub AMOUNT: IndexPointer<E>,
    pub MINTS_REMAINING: IndexPointer<E>,
    pub PREMINE: IndexPointer<E>,
    pub DIVISIBILITY: IndexPointer<E>,
    pub RUNE_ID_TO_HEIGHT: IndexPointer<E>,
    pub ETCHINGS: IndexPointer<E>,
    pub RUNE_ID_TO_ETCHING: IndexPointer<E>,
    pub ETCHING_TO_RUNE_ID: IndexPointer<E>,
    pub RUNTIME_BALANCE: IndexPointer<E>,
    pub HEIGHT_TO_RUNE_ID: IndexPointer<E>,
    pub RUNE_ID_TO_INITIALIZED: IndexPointer<E>,
    pub INTERNAL_MINT: IndexPointer<E>,
    pub TXID_TO_TXINDEX: IndexPointer<E>,
	pub OUTPOINT_SPENDABLE_BY: IndexPointer<E>,
    pub OUTPOINTS_FOR_ADDRESS: IndexPointer<E>,
    pub OUTPOINT_SPENDABLE_BY_ADDRESS: IndexPointer<E>,
    pub OUTPOINT_TO_OUTPUT: IndexPointer<E>,
    _phantom: PhantomData<E>,
}

impl<E: RuntimeEnvironment + Default> Default for RuneTable<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: RuntimeEnvironment> RuneTable<E> {
    pub fn new() -> Self {
        RuneTable {
            HEIGHT_TO_BLOCKHASH: IndexPointer::default().keyword("/blockhash/byheight/"),
            BLOCKHASH_TO_HEIGHT: IndexPointer::default().keyword("/height/byblockhash/"),
            OUTPOINT_TO_RUNES: IndexPointer::default().keyword("/runes/byoutpoint/"),
            OUTPOINT_TO_HEIGHT: IndexPointer::default().keyword("/height/byoutpoint/"),
            HEIGHT_TO_TRANSACTION_IDS: IndexPointer::default().keyword("/txids/byheight"),
            SYMBOL: IndexPointer::default().keyword("/runes/symbol/"),
            CAP: IndexPointer::default().keyword("/runes/cap/"),
            SPACERS: IndexPointer::default().keyword("/runes/spaces/"),
            OFFSETEND: IndexPointer::default().keyword("/runes/offset/end/"),
            OFFSETSTART: IndexPointer::default().keyword("/runes/offset/start/"),
            HEIGHTSTART: IndexPointer::default().keyword("/runes/height/start/"),
            HEIGHTEND: IndexPointer::default().keyword("/runes/height/end/"),
            AMOUNT: IndexPointer::default().keyword("/runes/amount/"),
            MINTS_REMAINING: IndexPointer::default().keyword("/runes/mints-remaining/"),
            PREMINE: IndexPointer::default().keyword("/runes/premine/"),
            DIVISIBILITY: IndexPointer::default().keyword("/runes/divisibility/"),
            RUNE_ID_TO_HEIGHT: IndexPointer::default().keyword("/height/byruneid/"),
            ETCHINGS: IndexPointer::default().keyword("/runes/names"),
            RUNE_ID_TO_ETCHING: IndexPointer::default().keyword("/etching/byruneid/"),
            ETCHING_TO_RUNE_ID: IndexPointer::default().keyword("/runeid/byetching/"),
            RUNTIME_BALANCE: IndexPointer::default().keyword("/runes/null"),
            HEIGHT_TO_RUNE_ID: IndexPointer::default().keyword("/runes/null"),
            RUNE_ID_TO_INITIALIZED: IndexPointer::default().keyword("/runes/null"),
            INTERNAL_MINT: IndexPointer::default().keyword("/runes/null"),
            TXID_TO_TXINDEX: IndexPointer::default().keyword("/txindex/byid"),
            OUTPOINT_SPENDABLE_BY: IndexPointer::default().keyword("/spendable/byoutpoint/"),
            OUTPOINTS_FOR_ADDRESS: IndexPointer::default().keyword("/outpoints/byaddress/"),
            OUTPOINT_SPENDABLE_BY_ADDRESS: IndexPointer::default().keyword("/spendable/byaddress/"),
            OUTPOINT_TO_OUTPUT: IndexPointer::default().keyword("/output/byoutpoint/"),
            _phantom: PhantomData,
        }
    }
    pub fn for_protocol(tag: u128) -> Self {
        RuneTable {
            HEIGHT_TO_BLOCKHASH: IndexPointer::default().keyword("/runes/null"),
            BLOCKHASH_TO_HEIGHT: IndexPointer::default().keyword("/runes/null"),
            HEIGHT_TO_RUNE_ID: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/byheight/").as_str(),
            ),
            RUNE_ID_TO_INITIALIZED: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/initialized/").as_str(),
            ),
            OUTPOINT_TO_RUNES: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/byoutpoint/").as_str(),
            ),
            OUTPOINT_TO_HEIGHT: IndexPointer::default().keyword("/runes/null"),
            HEIGHT_TO_TRANSACTION_IDS: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/txids/byheight").as_str(),
            ),
            SYMBOL: IndexPointer::default().keyword(format!("/runes/proto/{tag}/symbol/").as_str()),
            CAP: IndexPointer::default().keyword(format!("/runes/proto/{tag}/cap/").as_str()),
            SPACERS: IndexPointer::default().keyword(format!("/runes/proto/{tag}/spaces/").as_str()),
            OFFSETEND: IndexPointer::default().keyword("/runes/null"),
            OFFSETSTART: IndexPointer::default().keyword("/runes/null"),
            HEIGHTSTART: IndexPointer::default().keyword(format!("/runes/null").as_str()),
            HEIGHTEND: IndexPointer::default().keyword(format!("/runes/null").as_str()),
            AMOUNT: IndexPointer::default().keyword(format!("/runes/null").as_str()),
            MINTS_REMAINING: IndexPointer::default().keyword(format!("/runes/null").as_str()),
            PREMINE: IndexPointer::default().keyword(format!("/runes/null").as_str()),
            DIVISIBILITY: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/divisibility/").as_str(),
            ),
            RUNE_ID_TO_HEIGHT: IndexPointer::default().keyword(format!("/rune/null").as_str()),
            ETCHINGS: IndexPointer::default().keyword(format!("/runes/proto/{tag}/names").as_str()),
            RUNE_ID_TO_ETCHING: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/etching/byruneid/").as_str(),
            ),
            ETCHING_TO_RUNE_ID: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/runeid/byetching/").as_str(),
            ),
            RUNTIME_BALANCE: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/runtime/balance").as_str(),
            ),
            INTERNAL_MINT: IndexPointer::default().keyword(
                format!("/runes/proto/{tag}/mint/isinternal").as_str(),
            ),
            TXID_TO_TXINDEX: IndexPointer::default().keyword("/txindex/byid"),
            OUTPOINT_SPENDABLE_BY: IndexPointer::default().keyword("/spendable/byoutpoint/"),
            OUTPOINTS_FOR_ADDRESS: IndexPointer::default().keyword("/outpoints/byaddress/"),
            OUTPOINT_SPENDABLE_BY_ADDRESS: IndexPointer::default().keyword("/spendable/byaddress/"),
            OUTPOINT_TO_OUTPUT: IndexPointer::default().keyword("/output/byoutpoint/"),
            _phantom: PhantomData,
        }
    }
}
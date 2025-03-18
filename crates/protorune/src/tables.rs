use metashrew::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;

#[allow(non_snake_case)]
#[derive(Default, Clone)]
pub struct RuneTable {
    pub HEIGHT_TO_BLOCKHASH: IndexPointer,
    pub BLOCKHASH_TO_HEIGHT: IndexPointer,
    pub OUTPOINT_TO_RUNES: IndexPointer,
    pub OUTPOINT_TO_HEIGHT: IndexPointer,
    pub HEIGHT_TO_TRANSACTION_IDS: IndexPointer,
    pub SYMBOL: IndexPointer,
    pub CAP: IndexPointer,
    pub SPACERS: IndexPointer,
    pub OFFSETEND: IndexPointer,
    pub OFFSETSTART: IndexPointer,
    pub HEIGHTSTART: IndexPointer,
    pub HEIGHTEND: IndexPointer,
    pub AMOUNT: IndexPointer,
    pub MINTS_REMAINING: IndexPointer,
    pub PREMINE: IndexPointer,
    pub DIVISIBILITY: IndexPointer,
    pub RUNE_ID_TO_HEIGHT: IndexPointer,
    pub ETCHINGS: IndexPointer,
    pub RUNE_ID_TO_ETCHING: IndexPointer,
    pub ETCHING_TO_RUNE_ID: IndexPointer,
    pub RUNTIME_BALANCE: IndexPointer,
    pub HEIGHT_TO_RUNE_ID: IndexPointer,
    pub RUNE_ID_TO_INITIALIZED: IndexPointer,
    pub INTERNAL_MINT: IndexPointer,
    pub TXID_TO_TXINDEX: IndexPointer,
    /*
    pub HEIGHT_TO_BLOCKHASH: IndexPointer::from_keyword("/blockhash/byheight/"),
    pub BLOCKHASH_TO_HEIGHT: IndexPointer::from_keyword("/height/byblockhash/"),
    pub OUTPOINT_TO_RUNES : IndexPointer::from_keyword("/runes/byoutpoint/"),
    pub OUTPOINT_TO_HEIGHT : IndexPointer::from_keyword("/height/byoutpoint/"),
    pub HEIGHT_TO_TRANSACTION_IDS : IndexPointer::from_keyword("/txids/byheight"),
    pub SYMBOL : IndexPointer::from_keyword("/runes/symbol/"),
    pub CAP : IndexPointer::from_keyword("/runes/cap/"),
    pub SPACERS : IndexPointer::from_keyword("/runes/spaces/"),
        pub OFFSETEND : IndexPointer::from_keyword("/runes/offset/end/"),
    pub OFFSETSTART : IndexPointer::from_keyword("/runes/offset/start/"),
    pub HEIGHTSTART : IndexPointer::from_keyword("/runes/height/start/"),
    pub HEIGHTEND : IndexPointer::from_keyword("/runes/height/end/"),
        pub AMOUNT : IndexPointer::from_keyword("/runes/amount/"),
    pub MINTS_REMAINING : IndexPointer::from_keyword("/runes/mints-remaining/"),
    pub PREMINE : IndexPointer::from_keyword("/runes/premine/"),
    pub DIVISIBILITY : IndexPointer::from_keyword("/runes/divisibility/"),
    pub RUNE_ID_TO_HEIGHT : IndexPointer::from_keyword("/height/byruneid/"),
    pub ETCHINGS : IndexPointer::from_keyword("/runes/names"),
    pub RUNE_ID_TO_ETCHING : IndexPointer::from_keyword("/etching/byruneid/"),
    pub ETCHING_TO_RUNE_ID : IndexPointer::from_keyword("/runeid/byetching/"),
    */
}

impl RuneTable {
    pub fn new() -> Self {
        RuneTable {
            HEIGHT_TO_BLOCKHASH: IndexPointer::from_keyword("/blockhash/byheight/"),
            BLOCKHASH_TO_HEIGHT: IndexPointer::from_keyword("/height/byblockhash/"),
            OUTPOINT_TO_RUNES: IndexPointer::from_keyword("/runes/byoutpoint/"),
            OUTPOINT_TO_HEIGHT: IndexPointer::from_keyword("/height/byoutpoint/"),
            HEIGHT_TO_TRANSACTION_IDS: IndexPointer::from_keyword("/txids/byheight"),
            SYMBOL: IndexPointer::from_keyword("/runes/symbol/"),
            CAP: IndexPointer::from_keyword("/runes/cap/"),
            SPACERS: IndexPointer::from_keyword("/runes/spaces/"),
            OFFSETEND: IndexPointer::from_keyword("/runes/offset/end/"),
            OFFSETSTART: IndexPointer::from_keyword("/runes/offset/start/"),
            HEIGHTSTART: IndexPointer::from_keyword("/runes/height/start/"),
            HEIGHTEND: IndexPointer::from_keyword("/runes/height/end/"),
            AMOUNT: IndexPointer::from_keyword("/runes/amount/"),
            MINTS_REMAINING: IndexPointer::from_keyword("/runes/mints-remaining/"),
            PREMINE: IndexPointer::from_keyword("/runes/premine/"),
            DIVISIBILITY: IndexPointer::from_keyword("/runes/divisibility/"),
            RUNE_ID_TO_HEIGHT: IndexPointer::from_keyword("/height/byruneid/"),
            ETCHINGS: IndexPointer::from_keyword("/runes/names"),
            RUNE_ID_TO_ETCHING: IndexPointer::from_keyword("/etching/byruneid/"),
            ETCHING_TO_RUNE_ID: IndexPointer::from_keyword("/runeid/byetching/"),
            RUNTIME_BALANCE: IndexPointer::from_keyword("/runes/null"),
            HEIGHT_TO_RUNE_ID: IndexPointer::from_keyword("/runes/null"),
            RUNE_ID_TO_INITIALIZED: IndexPointer::from_keyword("/runes/null"),
            INTERNAL_MINT: IndexPointer::from_keyword("/runes/null"),
            TXID_TO_TXINDEX: IndexPointer::from_keyword("/txindex/byid"),
        }
    }
    pub fn for_protocol(tag: u128) -> Self {
        RuneTable {
            HEIGHT_TO_BLOCKHASH: IndexPointer::from_keyword("/runes/null"),
            BLOCKHASH_TO_HEIGHT: IndexPointer::from_keyword("/runes/null"),
            HEIGHT_TO_RUNE_ID: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/byheight/").as_str(),
            ),
            RUNE_ID_TO_INITIALIZED: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/initialized/").as_str(),
            ),
            OUTPOINT_TO_RUNES: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/byoutpoint/").as_str(),
            ),
            OUTPOINT_TO_HEIGHT: IndexPointer::from_keyword("/runes/null"),
            HEIGHT_TO_TRANSACTION_IDS: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/txids/byheight").as_str(),
            ),
            SYMBOL: IndexPointer::from_keyword(format!("/runes/proto/{tag}/symbol/").as_str()),
            CAP: IndexPointer::from_keyword(format!("/runes/proto/{tag}/cap/").as_str()),
            SPACERS: IndexPointer::from_keyword(format!("/runes/proto/{tag}/spaces/").as_str()),
            OFFSETEND: IndexPointer::from_keyword("/runes/null"),
            OFFSETSTART: IndexPointer::from_keyword("/runes/null"),
            HEIGHTSTART: IndexPointer::from_keyword(format!("/runes/null").as_str()),
            HEIGHTEND: IndexPointer::from_keyword(format!("/runes/null").as_str()),
            AMOUNT: IndexPointer::from_keyword(format!("/runes/null").as_str()),
            MINTS_REMAINING: IndexPointer::from_keyword(format!("/runes/null").as_str()),
            PREMINE: IndexPointer::from_keyword(format!("/runes/null").as_str()),
            DIVISIBILITY: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/divisibility/").as_str(),
            ),
            RUNE_ID_TO_HEIGHT: IndexPointer::from_keyword(format!("/rune/null").as_str()),
            ETCHINGS: IndexPointer::from_keyword(format!("/runes/proto/{tag}/names").as_str()),
            RUNE_ID_TO_ETCHING: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/etching/byruneid/").as_str(),
            ),
            ETCHING_TO_RUNE_ID: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/runeid/byetching/").as_str(),
            ),
            RUNTIME_BALANCE: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/runtime/balance").as_str(),
            ),
            INTERNAL_MINT: IndexPointer::from_keyword(
                format!("/runes/proto/{tag}/mint/isinternal").as_str(),
            ),
            TXID_TO_TXINDEX: IndexPointer::from_keyword("/txindex/byid"),
        }
    }
}

pub static RUNES: Lazy<RuneTable> = Lazy::new(|| RuneTable::new());

pub static HEIGHT_TO_RUNES: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/runes/byheight/"));

pub static OUTPOINTS_FOR_ADDRESS: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/outpoint/byaddress/"));

pub static OUTPOINT_SPENDABLE_BY: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/outpoint/spendableby/"));
pub static OUTPOINT_SPENDABLE_BY_ADDRESS: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/outpoint/spendablebyaddress/"));
pub static OUTPOINT_TO_OUTPUT: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/output/byoutpoint/"));

// Table to store cached WalletResponse for each address (full set of spendable outputs)
#[cfg(feature = "cache")]
pub static CACHED_WALLET_RESPONSE: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/cached/wallet/byaddress/"));

// Table to store cached filtered WalletResponse for each address (only outpoints with runes)
#[cfg(feature = "cache")]
pub static CACHED_FILTERED_WALLET_RESPONSE: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/cached/filtered/wallet/byaddress/"));

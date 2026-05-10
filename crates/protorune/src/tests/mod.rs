pub mod index_op_return_position;
pub mod index_pointer_ll;
pub mod index_protoburns;
pub mod index_protomessage;
pub mod index_protorunes_by_address;
pub mod index_runes;
pub mod index_runes_edicts;
pub mod index_runes_mint;
pub mod multi_block;
#[cfg(test)]
// pub mod multi_protocol;
// `ord_runes_parity` validates protorune's rune-balance indexing against
// canonical ord behaviour. It is only meaningful for builds that actually
// surface rune balances — i.e. those that opt in to the `runes` feature. The
// canonical alkanes indexer wasm builds without `runes`, so these tests are
// gated to avoid running them in the alkanes build context.
#[cfg(feature = "runes")]
pub mod ord_runes_parity;
pub mod test_cenotaphs;
pub mod test_many_outputs_bug;
pub mod view_functions;

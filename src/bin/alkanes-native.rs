//! Native standalone binary for the ALKANES indexer.
//!
//! This binary uses the metashrew-lib native runtime to run the ALKANES indexer
//! without requiring a WASM VM, resulting in better performance.

use alkanes::AlkanesIndexer;
use metashrew_core::native_binary;

// Define the native binary using the AlkanesIndexer from the main library
native_binary! {
    indexer: AlkanesIndexer,
    name: "alkanes-indexer",
    version: "0.1.0",
    about: "ALKANES metaprotocol indexer",
}

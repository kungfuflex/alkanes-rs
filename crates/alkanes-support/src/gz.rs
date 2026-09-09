use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::prelude::*;

/// Hard cap on decompressed wasm bytes. Largest legitimately-deployed alkane
/// wasm seen in mainnet is ~1-2 MB compressed / 4-8 MB decompressed; 16 MB
/// gives 2x headroom and is comfortably below wasmtime's own module-validation
/// ceiling.
///
/// The cap defeats a gzip-bomb DoS where the attacker submits a tiny
/// compressed witness payload (e.g. 1 KB of all-zero bytes deflates to <100 B)
/// that, prior to this cap, would `read_to_end` into the indexer's heap until
/// OOM-kill — wedging the entire metashrew pool because every restart attempts
/// to re-process the offending block. Observed in the wild at mainnet
/// h=953281+953282 (66 CREATE cellpacks across two blocks, pool stuck at tip
/// 953280, ~3 days of downtime).
pub const MAX_DECOMPRESSED_WASM_BYTES: usize = 16 * 1024 * 1024;

pub fn decompress(binary: Vec<u8>) -> Result<Vec<u8>> {
    let mut result = Vec::<u8>::new();
    // `take` bounds the underlying reader so `read_to_end` cannot drain
    // beyond the cap. We pass `cap + 1` so a *legitimate* `cap`-byte
    // wasm still fits, while anything strictly larger trips the
    // post-read check below.
    let mut reader = GzDecoder::new(&binary[..])
        .take((MAX_DECOMPRESSED_WASM_BYTES as u64).saturating_add(1));
    reader.read_to_end(&mut result)?;
    if result.len() > MAX_DECOMPRESSED_WASM_BYTES {
        return Err(anyhow!(
            "decompressed wasm exceeded {} B cap — rejected as gzip bomb",
            MAX_DECOMPRESSED_WASM_BYTES
        ));
    }
    Ok(result)
}

pub fn compress(binary: Vec<u8>) -> Result<Vec<u8>> {
    let mut writer = GzEncoder::new(Vec::<u8>::with_capacity(binary.len()), Compression::best());
    writer.write_all(&binary)?;
    Ok(writer.finish()?)
}

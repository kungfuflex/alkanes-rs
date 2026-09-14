#!/bin/bash
# Test script to verify ExtendedCallResponse serialization

set -e

WASM_FILE="crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-test-callresponse/build/release.wasm"

echo "Testing ExtendedCallResponse serialization..."
echo "WASM file: $WASM_FILE"
echo ""

# Create a simple Rust test program
cat > /tmp/test_tx_script.rs << 'EOF'
use alkanes_cli_common::provider::JsonRpcProvider;
use alkanes_cli_common::traits::AlkanesProvider;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read WASM file
    let wasm_bytes = fs::read("crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-test-callresponse/build/release.wasm")?;
    
    println!("Loaded WASM: {} bytes", wasm_bytes.len());
    
    // Create provider
    let provider = JsonRpcProvider::new(
        None,
        Some("https://mainnet.sandshrew.io/v2/lasereyes".to_string()),
        None,
        None,
        None,
    )?;
    
    // Execute tx-script
    println!("Executing tx-script...");
    let response_data = provider.tx_script(&wasm_bytes, vec![], None).await?;
    
    println!("✅ Got response: {} bytes", response_data.len());
    println!("Response (hex): {}", hex::encode(&response_data));
    
    // Parse ExtendedCallResponse
    // Format: [alkanes_count(16)][AlkaneTransfers...][storage_count(4)][StorageEntries...][data...]
    
    if response_data.len() >= 20 {
        // Read alkanes count (u128 = 16 bytes)
        let alkanes_count_bytes: [u8; 16] = response_data[0..16].try_into()?;
        let alkanes_count = u128::from_le_bytes(alkanes_count_bytes);
        println!("Alkanes count: {}", alkanes_count);
        
        // Skip alkanes section (16 + alkanes_count * 48)
        let alkanes_section_size = 16 + (alkanes_count as usize * 48);
        
        if response_data.len() >= alkanes_section_size + 4 {
            // Read storage count (u32 = 4 bytes)
            let storage_count_bytes: [u8; 4] = response_data[alkanes_section_size..alkanes_section_size+4].try_into()?;
            let storage_count = u32::from_le_bytes(storage_count_bytes);
            println!("Storage count: {}", storage_count);
            
            // For now, assume storage is empty and data starts right after
            let data_offset = alkanes_section_size + 4;
            
            if response_data.len() > data_offset {
                let data = &response_data[data_offset..];
                println!("Data section: {} bytes", data.len());
                println!("Data (hex): {}", hex::encode(data));
                
                // Check if it's our test data: 0x01020304
                if data.len() >= 4 && data[0] == 0x01 && data[1] == 0x02 && data[2] == 0x03 && data[3] == 0x04 {
                    println!("✅ SUCCESS! Got expected test data: 0x01020304");
                } else {
                    println!("❌ FAIL! Expected 0x01020304, got: {}", hex::encode(&data[..4.min(data.len())]));
                }
            }
        }
    }
    
    Ok(())
}
EOF

echo "Compiling test program..."
cd /data/alkanes-rs
rustc --edition 2021 \
    -L target/release/deps \
    --extern alkanes_cli_common=target/release/libalkanes_cli_common.rlib \
    --extern tokio=target/release/deps/libtokio-*.rlib \
    --extern hex=target/release/deps/libhex-*.rlib \
    /tmp/test_tx_script.rs \
    -o /tmp/test_tx_script 2>&1 | head -20

if [ -f /tmp/test_tx_script ]; then
    echo "Running test..."
    /tmp/test_tx_script
else
    echo "Failed to compile test program"
    exit 1
fi

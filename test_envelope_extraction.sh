#!/bin/bash

TXID="d0b8447f0e1efe17fd9a85485287728bb35b80e5e55e3d0eabe25246293bbe5c"

echo "Testing envelope extraction from transaction: $TXID"
echo ""

# Get the raw transaction
RAW_TX=$(./target/release/alkanes-cli -p regtest bitcoind getrawtransaction $TXID 2>&1 | tail -1 | python3 -c "import sys, json; print(json.load(sys.stdin).get('hex', ''))")

if [ -z "$RAW_TX" ]; then
    echo "ERROR: Could not get raw transaction"
    exit 1
fi

echo "Raw transaction length: ${#RAW_TX} chars"
echo ""

# Write a Python script to test envelope extraction
python3 << 'PYTHON_SCRIPT'
import sys
sys.path.insert(0, '/data/alkanes-rs')

from bitcoin import Transaction
from alkanes_support.envelope import AlkanesEnvelope

# Get the raw transaction hex
raw_hex = """RAW_TX_PLACEHOLDER"""

# Parse transaction
tx_bytes = bytes.fromhex(raw_hex)
tx = Transaction.deserialize(tx_bytes)

print(f"Transaction parsed successfully")
print(f"Number of inputs: {len(tx.vin)}")
print(f"Number of witness items in first input: {len(tx.vin[0].witness) if tx.vin else 0}")

# Try to extract envelope from witness
if tx.vin and len(tx.vin[0].witness) >= 2:
    witness_data = tx.vin[0].witness[1]
    print(f"\nWitness item 1 length: {len(witness_data)} bytes")
    print(f"First 40 bytes (hex): {witness_data[:40].hex()}")
    
    # Try to parse as envelope
    try:
        envelope = AlkanesEnvelope.from_witness(witness_data)
        print(f"\n✅ Envelope parsed successfully!")
        print(f"Protocol tag: {envelope.protocol_tag}")
        print(f"Compression: {envelope.compression}")
        print(f"Payload length: {len(envelope.payload)} bytes")
        
        # Check if payload starts with WASM magic bytes
        if len(envelope.payload) >= 4:
            magic = envelope.payload[:4]
            if magic == b'\x00asm':
                print(f"✅ Payload contains WASM module (magic bytes: {magic.hex()})")
            else:
                print(f"⚠️  Payload doesn't start with WASM magic bytes: {magic.hex()}")
    except Exception as e:
        print(f"\n❌ Failed to parse envelope: {e}")
        import traceback
        traceback.print_exc()
else:
    print("❌ Transaction doesn't have enough witness items")

PYTHON_SCRIPT

# Replace placeholder with actual raw tx
sed -i "s/RAW_TX_PLACEHOLDER/$RAW_TX/" /tmp/test_envelope.py 2>/dev/null || true

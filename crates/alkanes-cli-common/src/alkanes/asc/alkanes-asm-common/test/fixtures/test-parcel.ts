// Test fixture for AlkaneTransferParcel serialization
import { u128 } from "as-bignum/assembly";
import { AlkaneId, AlkaneTransfer, AlkaneTransferParcel } from "../../assembly/parcel";

/**
 * Test empty parcel
 * Expected: 16 bytes with count=0
 */
export function testEmpty(): ArrayBuffer {
  const parcel = new AlkaneTransferParcel();
  return parcel.serialize();
}

/**
 * Test single transfer
 * id: block=5, tx=10, value=100
 */
export function testSingle(): ArrayBuffer {
  const parcel = new AlkaneTransferParcel();
  const id = new AlkaneId(u128.from(5), u128.from(10));
  const transfer = new AlkaneTransfer(id, u128.from(100));
  parcel.pay(transfer);
  return parcel.serialize();
}

/**
 * Test multiple transfers
 * transfer1: block=1, tx=2, value=10
 * transfer2: block=3, tx=4, value=20
 */
export function testMultiple(): ArrayBuffer {
  const parcel = new AlkaneTransferParcel();
  
  parcel.pay(new AlkaneTransfer(
    new AlkaneId(u128.from(1), u128.from(2)),
    u128.from(10)
  ));
  
  parcel.pay(new AlkaneTransfer(
    new AlkaneId(u128.from(3), u128.from(4)),
    u128.from(20)
  ));
  
  return parcel.serialize();
}

/**
 * Test round-trip: serialize then parse
 */
export function testRoundTrip(): ArrayBuffer {
  const parcel = new AlkaneTransferParcel();
  
  parcel.pay(new AlkaneTransfer(
    new AlkaneId(u128.from(100), u128.from(200)),
    u128.from(1000)
  ));
  
  parcel.pay(new AlkaneTransfer(
    new AlkaneId(u128.from(300), u128.from(400)),
    u128.from(2000)
  ));
  
  const serialized = parcel.serialize();
  const parsed = AlkaneTransferParcel.parse(serialized);
  
  // Re-serialize to verify
  return parsed.serialize();
}

/**
 * Test AlkaneId serialization
 * block=12345, tx=67890
 */
export function testAlkaneId(): ArrayBuffer {
  const id = new AlkaneId(u128.from(12345), u128.from(67890));
  return id.serialize();
}

/**
 * Test AlkaneTransfer serialization
 * id: block=10, tx=20, value=500
 */
export function testAlkaneTransfer(): ArrayBuffer {
  const id = new AlkaneId(u128.from(10), u128.from(20));
  const transfer = new AlkaneTransfer(id, u128.from(500));
  return transfer.serialize();
}

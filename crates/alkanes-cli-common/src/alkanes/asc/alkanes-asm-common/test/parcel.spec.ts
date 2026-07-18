import { describe, it, run } from "../node_modules/@as-pect/core/assembly/index";
import { u128 } from "as-bignum/assembly";
import { AlkaneId, AlkaneTransfer, AlkaneTransferParcel } from "../assembly/parcel";
import { loadU128 } from "../assembly/alkanes/utils";

describe("AlkaneId", () => {
  it("serializes correctly", () => {
    const id = new AlkaneId(u128.from(100), u128.from(200));
    const serialized = id.serialize();
    
    assert(serialized.byteLength == 32, "AlkaneId should be 32 bytes");
    
    const ptr = changetype<usize>(serialized);
    const block = loadU128(ptr);
    const tx = loadU128(ptr + 16);
    
    assert(u128.eq(block, u128.from(100)), "Block should be 100");
    assert(u128.eq(tx, u128.from(200)), "Tx should be 200");
  });

  it("round-trips serialize/parse correctly", () => {
    const original = new AlkaneId(u128.from(12345), u128.from(67890));
    const serialized = original.serialize();
    const parsed = AlkaneId.parse(serialized);
    
    assert(u128.eq(parsed.block, original.block), "Block should match");
    assert(u128.eq(parsed.tx, original.tx), "Tx should match");
  });
});

describe("AlkaneTransfer", () => {
  it("serializes correctly", () => {
    const id = new AlkaneId(u128.from(10), u128.from(20));
    const transfer = new AlkaneTransfer(id, u128.from(500));
    
    const serialized = transfer.serialize();
    
    assert(serialized.byteLength == 48, "AlkaneTransfer should be 48 bytes");
    
    const ptr = changetype<usize>(serialized);
    const block = loadU128(ptr);
    const tx = loadU128(ptr + 16);
    const value = loadU128(ptr + 32);
    
    assert(u128.eq(block, u128.from(10)), "Block should be 10");
    assert(u128.eq(tx, u128.from(20)), "Tx should be 20");
    assert(u128.eq(value, u128.from(500)), "Value should be 500");
  });

  it("round-trips serialize/parse correctly", () => {
    const id = new AlkaneId(u128.from(111), u128.from(222));
    const original = new AlkaneTransfer(id, u128.from(333));
    
    const serialized = original.serialize();
    const parsed = AlkaneTransfer.parse(serialized);
    
    assert(u128.eq(parsed.id.block, original.id.block), "Block should match");
    assert(u128.eq(parsed.id.tx, original.id.tx), "Tx should match");
    assert(u128.eq(parsed.value, original.value), "Value should match");
  });
});

describe("AlkaneTransferParcel", () => {
  it("serializes empty parcel correctly", () => {
    const parcel = new AlkaneTransferParcel();
    const serialized = parcel.serialize();
    
    // Should be 16 bytes for count = 0
    assert(serialized.byteLength == 16, "Empty parcel should be 16 bytes");
    
    const ptr = changetype<usize>(serialized);
    const count = loadU128(ptr);
    assert(u128.eq(count, u128.Zero), "Count should be 0");
  });

  it("serializes single transfer correctly", () => {
    const parcel = new AlkaneTransferParcel();
    
    const id = new AlkaneId(u128.from(5), u128.from(10));
    const transfer = new AlkaneTransfer(id, u128.from(100));
    
    parcel.pay(transfer);
    
    const serialized = parcel.serialize();
    
    // Expected: 16 (count) + 48 (transfer) = 64
    assert(serialized.byteLength == 64, "Serialized size should be 64 bytes");
    
    let ptr = changetype<usize>(serialized);
    
    // Check count
    const count = loadU128(ptr);
    assert(u128.eq(count, u128.One), "Count should be 1");
    ptr += 16;
    
    // Check transfer
    const block = loadU128(ptr);
    assert(u128.eq(block, u128.from(5)), "Block should be 5");
    ptr += 16;
    
    const tx = loadU128(ptr);
    assert(u128.eq(tx, u128.from(10)), "Tx should be 10");
    ptr += 16;
    
    const value = loadU128(ptr);
    assert(u128.eq(value, u128.from(100)), "Value should be 100");
  });

  it("serializes multiple transfers correctly", () => {
    const parcel = new AlkaneTransferParcel();
    
    parcel.pay(new AlkaneTransfer(
      new AlkaneId(u128.from(1), u128.from(2)),
      u128.from(10)
    ));
    
    parcel.pay(new AlkaneTransfer(
      new AlkaneId(u128.from(3), u128.from(4)),
      u128.from(20)
    ));
    
    const serialized = parcel.serialize();
    
    // Expected: 16 (count) + 48 + 48 = 112
    assert(serialized.byteLength == 112, "Serialized size should be 112 bytes");
    
    const ptr = changetype<usize>(serialized);
    const count = loadU128(ptr);
    assert(u128.eq(count, u128.from(2)), "Count should be 2");
  });

  it("round-trips serialize/parse correctly", () => {
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
    
    assert(parsed.transfers.length == 2, "Parsed parcel should have 2 transfers");
    
    // Check first transfer
    assert(u128.eq(parsed.transfers[0].id.block, u128.from(100)), "First block should be 100");
    assert(u128.eq(parsed.transfers[0].id.tx, u128.from(200)), "First tx should be 200");
    assert(u128.eq(parsed.transfers[0].value, u128.from(1000)), "First value should be 1000");
    
    // Check second transfer
    assert(u128.eq(parsed.transfers[1].id.block, u128.from(300)), "Second block should be 300");
    assert(u128.eq(parsed.transfers[1].id.tx, u128.from(400)), "Second tx should be 400");
    assert(u128.eq(parsed.transfers[1].value, u128.from(2000)), "Second value should be 2000");
  });

  it("calculates size correctly", () => {
    const parcel = new AlkaneTransferParcel();
    
    assert(parcel.calculateSize() == 16, "Empty parcel size should be 16");
    
    parcel.pay(new AlkaneTransfer(
      new AlkaneId(u128.Zero, u128.Zero),
      u128.Zero
    ));
    
    assert(parcel.calculateSize() == 64, "Single transfer parcel size should be 64");
    
    parcel.pay(new AlkaneTransfer(
      new AlkaneId(u128.Zero, u128.Zero),
      u128.Zero
    ));
    
    assert(parcel.calculateSize() == 112, "Two transfer parcel size should be 112");
  });
});

run();

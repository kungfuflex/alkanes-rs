import { console } from "./logging";
import { Box } from "./box";
import { sha256 } from "./sha256";

export function decodeHex(hex: string): ArrayBuffer {
  const result = new ArrayBuffer(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    store<u8>(
      changetype<usize>(result) + i / 2,
      <u8>parseInt(hex.substring(i, i + 2), 16),
    );
  }
  return result;
}

export function test_sha256(): void {
  const hash = sha256(
    Box.from(
      decodeHex(
        "010000000174a016bad8927ee739aff7289f5574b8f57fcc32272829ac3aef3edb9959ad580000000048473044022018d1fc212affb41dd322fdcdc8d0474d7eab7ab414a712361f81f93df7fd9bed02200d7d431ac3c7b61a46b30f911797fde4ede3014434cb00149ab71eed26335f0201ffffffff0200111024010000001976a91420420e56079150b50fb0617dce4c374bd61eccea88ac0ae1f505000000001976a91410f958cbe9cf6d26c2e2ace39766c2cdb87179f788ac00000000",
      ),
    ).toArrayBuffer(),
  );
  console.log(Box.from(hash).toHexString());
}

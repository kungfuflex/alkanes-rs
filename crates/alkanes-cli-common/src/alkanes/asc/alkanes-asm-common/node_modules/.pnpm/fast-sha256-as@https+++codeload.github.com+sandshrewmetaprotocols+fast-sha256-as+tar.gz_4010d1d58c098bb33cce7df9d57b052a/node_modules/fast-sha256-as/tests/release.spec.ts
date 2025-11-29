import { IndexerProgram } from "metashrew-test";
import path from "path";
import fs from "fs-extra";
import { EventEmitter } from "events";
import { hash } from "fast-sha256";
import { expect } from "chai";

const WASM_BINARY = fs.readFileSync(
  path.join(__dirname, "..", "build", "debug.wasm"),
);

const tx = new Uint8Array(
  Array.from(
    Buffer.from(
      "010000000174a016bad8927ee739aff7289f5574b8f57fcc32272829ac3aef3edb9959ad580000000048473044022018d1fc212affb41dd322fdcdc8d0474d7eab7ab414a712361f81f93df7fd9bed02200d7d431ac3c7b61a46b30f911797fde4ede3014434cb00149ab71eed26335f0201ffffffff0200111024010000001976a91420420e56079150b50fb0617dce4c374bd61eccea88ac0ae1f505000000001976a91410f958cbe9cf6d26c2e2ace39766c2cdb87179f788ac00000000",
      "hex",
    ),
  ),
);

describe("metashrew index", () => {
  it("should sha256", async () => {
    const indexer = new IndexerProgram(
      new Uint8Array(Array.from(WASM_BINARY)).buffer,
    );
    indexer.setBlock("0x");
    indexer.setBlockHeight(0);
    const logPromise: Promise<any> = new Promise((resolve) =>
      indexer.on("log", (v) => resolve(v.substr(v, v.length - 1))),
    );
    const result: any = await indexer.run("test_sha256");
    const _hash = "0x" + Buffer.from(hash(tx)).toString("hex");
    const resultHash: string = await logPromise;
    expect(resultHash).to.be.equal(_hash);
  });
});

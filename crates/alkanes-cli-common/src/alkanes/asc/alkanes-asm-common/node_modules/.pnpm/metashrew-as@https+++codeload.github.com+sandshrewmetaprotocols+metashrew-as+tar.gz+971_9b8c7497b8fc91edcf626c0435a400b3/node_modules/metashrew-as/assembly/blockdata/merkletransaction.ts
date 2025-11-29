import { Transaction } from "./transaction";
import {
  parsePrimitive,
  parseVarInt,
  parseBytes
} from "../utils/utils";
import { Box } from "../utils/box";

export class MerkleTransaction extends Transaction {
  public redundantParentBlockHash: Box;
  public merkleBranch: Array<Box>;
  public merkleBranchSideMask: u32;
  constructor(data: Box) {
    super(data);
    this.redundantParentBlockHash = parseBytes(data, 0x20);
    this.merkleBranch = new Array<Box>(<i32>parseVarInt(data));
    for (let i = 0; i < this.merkleBranch.length; i++) {
      this.merkleBranch[i] = parseBytes(data, 0x20);
    }
    this.merkleBranchSideMask = parsePrimitive<u32>(data);
  }
}

import {
  parsePrimitive,
  parseVarInt,
  parseBytes
} from "../utils/utils";
import { Box } from "../utils/box";
import { MerkleTransaction } from "./merkletransaction";
import { Header } from "./block";
export class AuxPow extends MerkleTransaction {
  public chainMerkleBranch: Array<Box>;
  public chainMerkleBranchSideMask: u32;
  public parentBlockHeader: Header;
  constructor(data: Box) {
    super(data);
    this.parentBlockHeader = changetype<Header>(0);
    this.chainMerkleBranch = new Array<Box>(<i32>parseVarInt(data));
    for (let i = 0; i < this.chainMerkleBranch.length; i++) {
      this.chainMerkleBranch[i] = parseBytes(data, 0x20);
    }
    this.chainMerkleBranchSideMask = parsePrimitive<u32>(data);
    this.parentBlockHeader = new Header(data.sliceFrom(0).setLength(80));
    data.shrinkFront(80);
  }
}

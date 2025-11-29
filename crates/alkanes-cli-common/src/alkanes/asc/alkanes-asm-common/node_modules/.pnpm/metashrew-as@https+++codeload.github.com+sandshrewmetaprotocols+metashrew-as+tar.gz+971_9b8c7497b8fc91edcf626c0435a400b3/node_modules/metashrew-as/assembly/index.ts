import { Box } from "./utils/box";
import { memcpy } from "./utils/memcpy";
import {
  isOrdTag,
  concat,
  reverse,
  parseLenThenBytes,
  parsePrimitive,
  parsePushOp,
  parseBytes,
  isPushOp,
  decodeTag,
  parseVarInt,
  primitiveToBuffer,
} from "./utils/utils";
import { Index, get, set, _flush, input } from "./indexer/index";
import { console } from "./utils/logging";
import { abort } from "./utils/abort";
import { Transaction, Block, Inscription, OutPoint } from "./blockdata";
import { encodeHexFromBuffer } from "./utils/hex";
import {
  BST,
  setBitU256,
  binarySearchU64,
  binarySearchU32,
  binarySearchU16,
  binarySearchU8,
  binarySearchU256,
  maskGreaterThan,
  maskLowerThan,
} from "./indexer/bst";
import { BSTU128 } from "./indexer/widebst";
import { u128ToHex } from "./utils/utils";
import { sha256 } from "fast-sha256-as/assembly/sha256";
import { IndexPointer } from "./indexer/tables";
import { Input } from "./blockdata/transaction";
import { u128 } from "as-bignum/assembly";
import { fromWords, b32decode, bech32, bech32m, toWords } from "./utils/b32";

export function logArrayBuffer(arr: ArrayBuffer): void {
  const tempArray = Uint8Array.wrap(arr);
  console.log("Got arraybuffer of size " + tempArray.length.toString());
  tempArray.forEach((val, i) => {
    console.log(
      `byte[${i}] = 0x${val.toString(16).padStart(2, "0")} (${String.fromCharCode(val)})`,
    );
  });
}

export function test_b32decode(): void {
  const val = sha256(new ArrayBuffer(2));
  console.log(Box.from(val).toHexString());
  const words = toWords(val);
  const b = bech32(String.UTF8.encode("bc"), words);
  console.log(String.UTF8.decode(b));
  const decoded = b32decode(String.UTF8.decode(b)).words;
  console.log(Box.from(fromWords(decoded)).toHexString());
}

export function test_parseBlock(): void {
  const data = input();
  const box = Box.from(data);
  const height = parsePrimitive<u32>(box);
  const block = new Block(box);
}

function logK<K>(v: K): void {
  const data = new ArrayBuffer(sizeof<K>());
  store<K>(changetype<usize>(data), bswap<u64>(v));
  console.log(Box.from(data).toHexString());
}

export function test_parseWitness(): void {
  const data = input();
  const box = Box.from(data);
  const height = parsePrimitive<u32>(box);
  const tx = new Transaction(box);
  tx.ins[1].inscription();
}

export function test_BSTU128(): void {
  const bst = BSTU128.at(IndexPointer.for("/test"));
  u128.from(0x100);
  bst.set(u128.from(0x100), String.UTF8.encode("test"));
  bst.set(u128.from(0x140), String.UTF8.encode("test"));
  bst.set(u128.from(0x104), String.UTF8.encode("test"));
  console.log(u128ToHex(bst.seekGreater(u128.from(0x103))));
  bst.set(u128.from(0x104), new ArrayBuffer(0));
  console.log(u128ToHex(bst.seekGreater(u128.from(0x103))));
}

export function test_seekLower(): void {
  const ptr = IndexPointer.wrap(String.UTF8.encode("/")).keyword("outpoint");
  const bst = BST.at<u64>(
    IndexPointer.wrap(String.UTF8.encode("/")).keyword("outpoint"),
  );
  bst.set(3, String.UTF8.encode("test"));
  bst.set(<u64>(0x03 << 16), String.UTF8.encode("test3"));
  bst.set(bswap<u64>(3), String.UTF8.encode("test2"));
  //  console.log("bst.seekLower(0xffffffffffffffff))");
  //  logK<u64>(bst.seekLower(0xffffffffffffffff));
  //  console.log("bst.seekLower(0x0300000000000000)");
  //logK<u64>(bst.seekLower(0x0300000000000000));
  _flush();
}

export function test_asmmath(): void {
  const mask = new ArrayBuffer(32);
  setBitU256(mask, 0xc8);
  console.log(Box.from(mask).toHexString());
}

export function test_unmarkPath(): void {
  const bst = BST.at<u64>(IndexPointer.for("/test"));
  bst.set(0x465e3f16b2e25dc8, String.UTF8.encode("test"));
  bst.set(0x41e6cbaf98f67b50, String.UTF8.encode("test"));
  bst.set(0x5d4c39ec5b5126c0, String.UTF8.encode("test"));
  bst.set(0x5d4c39ec5b5126c1, String.UTF8.encode("test"));
  bst.set(0x5d4c39ec5b5126c2, String.UTF8.encode("test"));
  bst.set(0x5d4c39ec5b5126c3, String.UTF8.encode("test"));
  bst.set(0x5d4c39ff00000000, String.UTF8.encode("test"));
  bst.set(0x5d4c39ff00ff0000, String.UTF8.encode("test"));
  bst.set(0x8c8534514a8fb502, String.UTF8.encode("test"));
  bst.set(0x9df8622e7b073043, String.UTF8.encode("test"));
  bst.set(0x465e3f16b2e25dc8, new ArrayBuffer(0));
  bst.set(0x5d4c39ec5b5126c1, new ArrayBuffer(0));
  console.log("0x" + bst.seekGreater(0x41e6cbaf98f67b50).toString(16));
  console.log("0x" + bst.seekGreater(0x5d4c39ec5b5126c0).toString(16));
  console.log("0x" + bst.seekGreater(0x5d4c39ec5b5126c1).toString(16));
  console.log("0x" + bst.seekGreater(0x5d4c39ec5b5126c2).toString(16));
  console.log("0x" + bst.seekGreater(0x5d4c39ec5b5126c3).toString(16));
  console.log("0x" + bst.seekGreater(0x5d4c39ff00000000).toString(16));
  bst.set(0x5d4c39ec5b5126c3, new ArrayBuffer(0));
  console.log("0x" + bst.seekGreater(0x5d4c39ec5b5126c2).toString(16));
  bst.set(0x5d4c39ec5b5126c0, new ArrayBuffer(0));
  bst.set(0x5d4c39ec5b5126c2, new ArrayBuffer(0));
  bst.set(0x5d4c39ec5b5126c1, new ArrayBuffer(0));
  console.log("0x" + bst.seekGreater(0x465e3f16b2e25dc8).toString(16));
}

export function test_seekLower2(): void {
  const ptr = IndexPointer.wrap(String.UTF8.encode("/")).keyword("outpoint");
  const bst = BST.at<u64>(
    IndexPointer.wrap(String.UTF8.encode("/")).keyword("outpoint"),
  );
  bst.set(3, String.UTF8.encode("test"));
  bst.set(<u64>(0x03 << 16), String.UTF8.encode("test3"));
  bst.set(bswap<u64>(3), String.UTF8.encode("test2"));
  //  logK<u64>(bst.seekLower(0x03 << 16));
  //bst.nullify(<u64>(0x03 << 16));
  _flush();
}

export function test_isOrdTag(): void {
  const s = String.UTF8.encode("ord");
  isOrdTag(Box.from(s));
}

export function test_seekGreater(): void {
  const ptr = IndexPointer.wrap(String.UTF8.encode("/")).keyword("outpoint");
  const bst = BST.at<u64>(
    IndexPointer.wrap(String.UTF8.encode("/")).keyword("outpoint"),
  );
  bst.set(3, String.UTF8.encode("test"));
  bst.set(4, String.UTF8.encode("test2"));
  bst.set(<u64>(0x03 << 16), String.UTF8.encode("test3"));
  bst.set(<u64>((0x03 << 16) | 0x04), String.UTF8.encode("test3"));
  /*
  bst.set(bswap<u64>(3), String.UTF8.encode("test2"));
  bst.set(bswap<u64>(4), String.UTF8.encode("test4"));
 */
  console.log(bst.seekGreater(0).toString(10));
  _flush();
}

export function test_maskLowerThan(): void {
  const data = new ArrayBuffer(32);
  store<u64>(changetype<usize>(data), U64.MAX_VALUE);
  store<u64>(changetype<usize>(data) + 8, U64.MAX_VALUE);
  store<u64>(changetype<usize>(data) + 16, U64.MAX_VALUE);
  store<u64>(changetype<usize>(data) + 24, U64.MAX_VALUE);
  maskLowerThan(data, <u8>255);
  maskLowerThan(data, <u8>190);
  maskLowerThan(data, <u8>100);
  maskLowerThan(data, <u8>32);
}

export function test_maskGreaterThan(): void {
  const data = new ArrayBuffer(32);
  setBitU256(data, 0);
  setBitU256(data, 3);
  //console.log("maskGreaterThan((bytes), 3)");
  //console.log(Box.from(data).toHexString());
  maskGreaterThan(data, 3);
  //console.log(Box.from(data).toHexString());
}

export function test_maskLowerThan2(): void {
  const data = new ArrayBuffer(32);
  setBitU256(data, 3);
  setBitU256(data, 0);
  maskLowerThan(data, <u8>4);
  //  console.log("maskLowerThan(3)");
  //  console.log(Box.from(data).toHexString());
}

export function test_binarySearch(): void {
  const data = new ArrayBuffer(32);
  store<u8>(changetype<usize>(data) + 9, 0x01);
}

export function test_binarySearch2(): void {
  const data = new ArrayBuffer(32);
  setBitU256(data, 0);
  setBitU256(data, 3);
  //console.log("binarySearch(data, true)");
  //console.log(Box.from(data).toHexString());
  //console.log(binarySearchU256(data, false).toString(10));;
}
export function test_binarySearch3(): void {
  const data = new ArrayBuffer(32);
  setBitU256(data, 0);
  setBitU256(data, 2);
}

export function test_inscription(): void {
  const data = input();
  const box = Box.from(data);
  const height = parsePrimitive<u32>(box);
  const block = new Block(box);
  // const tx = new Transaction(box);
  for (let i = 0; i < block.transactions.length; i++) {
    let t = block.transactions[i];
    for (let j = 0; j < t.ins.length; j++) {
      let v = t.ins[j];
      let inscription = v.inscription();
      if (inscription != null) {
        console.log(
          "inscription from tx " + i.toString() + " input " + j.toString(),
        );
        logArrayBuffer(inscription.toArrayBuffer());
        let inscription_body = inscription.body();
        if (inscription_body !== null) {
          console.log("Inscription body is: ");
          logArrayBuffer(inscription_body);
        }
      }
    }
  }
}

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

export function test_indexBrc20(): void {
  const tx = new Transaction(
    Box.from(
      decodeHex(
        "02000000000101aabe7e0d5a3a56bb049a417b4e6f90dd2d6a57890fbc6568f476baaae061300a8f03000000fffffffd01260100000000000016001413c8d4f4be75d11b463a35b141a1067155c407a80340b29a41195ee892d8b3ddf0b7912fa0c93cfdc3dc7b22dfdd65ecdc88c2fdcc52684bc09f7f220e7e92be4bfd44c50d05853bee38c1b5800d0c52b7a16f980e9e71203b3e59eda857e9fc750376b1deb6147aa45410410a561e062fb36172b600634eac0063036f726401010a746578742f706c61696e00397b2270223a226272632d3230222c226f70223a226d696e74222c227469636b223a2273617473222c22616d74223a223939393939393939227d6821c03b3e59eda857e9fc750376b1deb6147aa45410410a561e062fb36172b600634e00000000",
      ),
    ),
  );
  tx.ins[0].inscription() as Inscription;
}

export function test_txid(): void {
  const tx = new Transaction(
    Box.from(
      decodeHex(
        "01000000035662e476c47fd38587ac6a56e8627a135176324ca3ab1583536bfd161f12fc4a000000008c493046022100db9f05c8b729d0a01214ea3c7e1d0baf7ba6defdd772aa099886a56381ae8cae02210080932786ff66ddc0b2c045042108faaaeff4d5fd0aa9a3fd05027dc274e5ec8d014104ad3ef37e4daffc516199fe117f3a4eea88069c5c0154a52aef3167fc4d29b6d2a66102d74d5ad8725418735f6c8a8d5e94cdd1c4a2b2e308005563d943066edcffffffff7764c803f55c46338ea49ebd626847cb5f2964eafa5853e7afb27927bf0f3d660000000049483045022100c8b144ed14047b27f28ce764663d34d1873910d3f244f25716b0c5210f8515420220127ab34bac68789eae03aac2c3296bbb70d814045715f7c9e45a6eec8e8de48a01ffffffffd45fe77cea5c828cb908cf3d4e10f79303df1d043a33daf6eb6a419483fc2c7f020000008b483045022100f68da2548976302158ce332c85bd2164e84039616158423fb6bf822af50a3562022017ed9644cdb0b677407b24d34b9238e5deaecbdcc942bb6164b4c0e2a2075d20014104ec8a68e0a90d5d3e4f45e1269e884293e05acfd197a8d83c1be21783a929ca938e9e82f09bc0f3eb39f4cf4d669a29deea3e36d7d7fee966a8b527b07db8399cffffffff72bcf57300000000001976a914b2ce35f25ec71045f41c020ed0e2edc857f6a29c88ac3b926800000000001976a91412bfc89a8a6569e5b39363e87d219b33c7ed7ebe88ac68241004000000001976a914c289eb93258bc6f325a1e9c01bb92c53222748a188ac90754b06000000001976a914f5500511acea190874aa52facda087bd096f806b88acd20a2d00000000001976a914c079f336d963264e713ab03148532622350984d288ac68675802000000001976a91465dce380aa1897ec1fa39e79d214cb9dd6b1abe488ac02e07c07000000001976a914dc1a45ad8513064547c4afb356bb2b197469bd7888ac26db1600000000001976a914a5abab06675a3ea007a8049cd9bbc80035042f3388ac6c651900000000001976a914e1c9a945feb43026b7db17cb1defcc8a55182ec388ac755d2803000000001976a91478742a0cc094fcf60d7c7ee455cd5d1e0ca5922888ac12338209000000001976a914b523a85f5c7d3418c976d8b2d9297ec12c70849e88ac93f54500000000001976a91410edcf3f22405c8774257ce711bcb11bb00cef2c88acc640e000000000001976a914fc0b11112fa958d34fb68404e2b2d983a26225e388ac351cbe01000000001976a91438d52bcd77b2ed82d318957881b317360746a4c388ac710d2403000000001976a9140b8e664b92439cd13ffed804f1e7dc212f0ef49d88aca6855b00000000001976a914b34179a9d6081bd3fbffa495e98b1d94a28e72d988ac2e127702000000001976a9145e2c6e9c08b0c0f0415da91032e1a4782a3363f988acd29fd405000000001976a9140f207d9aa05ff6c49be367a95d1e6fe6082086be88ace9a6c801000000001976a9142d3c37d5c1df6927424ca9811fc7dd37f77f7cbc88ac3b3a7701000000001976a9147c180c0d76c0cc69afaaf9b38da9ba3c1145c09888ac37316001000000001976a9147b156f100915db080e1b7c936434b45a9f28475488ac99e67707000000001976a914bd1277f2be3e520e39d1eea35885323aff997b8388ac51005c00000000001976a914ad73e093bf9757cd0993751d1f4773e4c1a7703688accdd10902000000001976a9143c0a17db27f6fd5ebe2f2f377749c18712eb6faf88ac15c25b02000000001976a914bdfc65c3fc20db64f1464375647e6c5f8389829288ace5c00201000000001976a9149553ef8efffc3ccff32a577a52f51c94731dd34088acaef56d06000000001976a914c7d30cfb1252d25a3a1adc5f7066982109007b3b88acdf359c00000000001976a9148ccc40300c6511053ab7ecb1fafe64fdad8777e588accac58300000000001976a914f9a7b67eb0ff97997987476e4313e213213b5ad088ac4eabe61e000000001976a914792878d7e8858a2feff8707f227043a8722c8faf88ac7fc08f00000000001976a9146998fc649319d2d8a839871996e348287d10547a88ac09f80e03000000001976a9146541790d8d80828ff7f7f771ffe453496c0a204888ac45c11c06000000001976a914804cbd4446352b9fdcfea7618239e6fb334058a388ac497f1c07000000001976a91445a7380aa252c694165945130e117695f3924abc88ac152c9a01000000001976a9140bdecd39efa14f76963257a23ee86f1cca53065b88acd5ab5a07000000001976a9144df0b982e406ca9a1fd97967e33cebc9dd3100bf88ac4e891000000000001976a914b99e231416872ee09a417edc3ad80eb1563f558a88ac26202b00000000001976a9148c6d8b19a0e4bb3b97f3f330f940d7c156a2d39d88ac79bc0b02000000001976a914141108e04d82a5ce68f03ed74795c1ba2455003788ac80fbf100000000001976a914d11dbf8fb3370c6c2cd5c7cdafd03d42b59aed8688ac33e15b01000000001976a914fbb60c927bcc468c4a44019d4e602f234efa378188ac30a03600000000001976a9147651319b8cffe63e9b68c5c6bf521a3106c7997d88ac48f05100000000001976a914100a518dd20f2519a6153a6eb08c965da717a4ce88ac1037f501000000001976a914f3dbd4c3cc821dbfbdc2147815664297fa2d908588ac65f19301000000001976a914ee18b19a43fb8c85a02c8f29d6a41f46c3ccbcb488ac5a3bcf06000000001976a9147839c9d78aafed32ae5b160c30678e8d5f2f56ef88ac41951800000000001976a914651eb68ec7b10020fe31c0013e206b963c0a19c088ac28bcaf01000000001976a91464cd3a4aafa781f211e18c62b80b79abf5f088b688acb8f7b202000000001976a9145dde9ad09cecf6b60b31d2462cba7754626ad5e588ac168dbc02000000001976a914dabe81f230d4cb667efece6a4fbba317d0d9590188ac815c1402000000001976a91493ca38010a060dbddcdc28da6eba1357d445990a88ac2d55db05000000001976a9141e3e7a6bf53438ac3cb73ff22496a3c861fcb68a88ac19c01c00000000001976a9149555e98656ecb364a6630a25a71627b66d65e5ec88ac750be500000000001976a914435b1aec9db9b9eded06b4740eeb2e4bc9e12bae88ac2d2f5508000000001976a91457d2ce12a8b8b02564822289745232be52176fa188ac39cde402000000001976a914bb0f91b51c1a0e3d72a0ca3486ab4b876573314788ac7a741a0b000000001976a914d5087af674ee3235f744cea378a6526895689fba88ac54e05e00000000001976a914a9f922de2ae7b5903e135b3923b26e04a00f288a88aca930bf00000000001976a91429a679accc356ed530c03b078bd3dd7d6bc9ac9a88acb71d7303000000001976a914a496f83c2a17dc32a8534ce0639e346f4dcc5bf688ac51873e02000000001976a9142b775105466a0066741f9c81e656b4cb7b873cb288acaf569007000000001976a91456fed2959426edb456da8563beb1d622b46bc78288acf69c9802000000001976a914a4e45001b3e786a32b6ea4e3c61961b38bb7b8aa88acc94c6602000000001976a914c84fb4f5fbb506724d878227b4dc199443fe78dc88ac1b651900000000001976a9148f5bcec5367021e4fb7934986ea06407171424ec88ac9ad8b203000000001976a914f290f23a4c2ab7275af14dd295448c39d8bcc2de88ac38b1f708000000001976a9146d88d58767afcbfbf707f99b2cadfe7d66068cd888ac25114c01000000001976a9141c304004db9fd68ab156a31488fdcacb9080c44c88acb8e91400000000001976a91480b7c6751a7d6ca9fa8f04949f3f6fa02191d8a088ac24a14a01000000001976a914b1eae2ea539c62ed3b15e44380239b7ecd270ceb88ac11a7f601000000001976a914447c45c014193952444e31178c364e53e2fcff9e88ac30cf1000000000001976a9144cc438d0adfe767e1ab84593f6370596f537f56b88acc0457800000000001976a914ea504894b7e43ab60123fbab331b485b8cd2404588accc96a701000000001976a914e35cb7d15f18a76360d24753ce624d6ed1a059ed88acba769301000000001976a9144c4d4682cbe92c33823c77d66f82cde92739a19788aca60c3e02000000001976a9147f75da22eb4f895cdd30dada160845654603b17388acab620503000000001976a9146a8ac4036d01e5edd56937483d436b0e3c16763f88ac61375102000000001976a914dc6cce4873b413bfaadd293931d097f73a7d80f688ac0ed00f00000000001976a914f39ec0c1792e654c77882f50605611f94e4678c488ac09a6cb00000000001976a914a5affe21f94a759aaf8d0e097cbd52c26d95645988ac88d54206000000001976a9146ae43458a5af7212247a15d019bae97f34b051c088ac92351900000000001976a9148abc13214fa0c423a6f7069e5f96f4a55f5f898788acd47a2e00000000001976a914b79e41450be074efea5cb6113a4035bdf19ae6c988ac843c1702000000001976a914ad80d24e0e710bb6ea09403d2940fade9bdb166388acb0056700000000001976a914ddcce78de39a5685d88e27dfb30010c8385be5a188acccf7c902000000001976a9140b13c49a1bcfea30726ffdcc71ee3bd720a6f11f88ac88e3070a000000001976a914e406f0bf5395f9b4abb42018862ecfe31428425088acbf063000000000001976a914fa726e29204480f34528f213138ab0690c32d65088acc4c1ff01000000001976a914b02440806e922cb7b546ad7dbcc31f412e0d3f9e88ac0d2b6f00000000001976a91442b2090f0681fc7332f62852bdc5efd0bb4b361388acc1211801000000001976a9140fa167899046b0204705482397fb319e0a94b2c288acb3e56900000000001976a914582f61f414bf01a217cb2f313256851b862ea79988ac3adcc301000000001976a914e295ece7f7de51b8f0b72662e149efcd2d36771488ac94654700000000001976a9148ce63c46738c22ea5415e6867aa3fe3ae2f3ea2688acc7f34504000000001976a9142fbe2ca6f7681d2ca62e0468944997649247ba3088ac82a09200000000001976a9145227c8ac0623c0658f20775ca75f36ce357f8cfa88acf7ba5600000000001976a9143c4ae23f112e6acbd57a33bf909e733219517b5488ac8cf0a706000000001976a91471626f211034181a381093e5733a8321c72ac21e88ac2c0cb401000000001976a914bcbf719b7e1ffdb24ccecb8c83175832120474f488acd8ca3200000000001976a9146aded3ce39459968572e6edc61478304b631f8a888acb8a56f00000000001976a9145532e4c590a9a35c2d2a3572b75ab151534d913488ac23c70a02000000001976a914fa8de686bc69b9eb3ce18ef290edfca62bbd3f7d88ac67edbf00000000001976a9149776ada9da4416c9567402536f3f76ce22a51cdc88acc071fb01000000001976a914daac1f1ed70ce5412642b29e856222fb779836df88accd4a2700000000001976a91431db0c57b787f1e7c1c565ae29d60d63434f1aaa88ac9a254d00000000001976a914c2ecacc940537887ab5902e0418dc50de6286baa88ac78c52700000000001976a91447685aad697ed1cb0cbb834d88552cfc3f9c156d88ac0f313201000000001976a914920dbd457e69fc9ed56a262ad38b62a2f602db2c88ac832d3803000000001976a9141dc4707fad8a17fb260798c97ee2a1310975a78a88ac39cde402000000001976a91475d6609edc563fb501a47a24f5eb8e2bad90905088acd2fb4d01000000001976a914fc720e58c9c144b529a79e73078b2d8ef804922588acfc6b7d01000000001976a914db1538cee7b93f6ce2029a8f2d0c21543f14d84588ac376ba806000000001976a9147e95347a018387dba02ec94bd620d79c03da47b688aca5155a00000000001976a91449a12b28c19d0351fe4ff11bcd393e8892baf29288ac00000000",
      ),
    ),
  );
  /*
  console.log(Box.from(tx.txid()).toHexString());
  console.log(tx.outs.length.toString());
  console.log(tx.outs[17].value.toString());
 */
}

/*
export function test_fixedbst(): void {
  const bst = FixedBST.at(IndexPointer.for("/bst"), 36);
  const txid = decodeHex(
    "8c6c6b86069435308f468a3db4063d8b266b6dfc845ea4c5202920b13b464c44",
  );
  const txid2 = decodeHex(
    "00113299ae52846641a22e5324152414bff89ca9b76f3af5ab98249ecb31b701",
  );
  const op = OutPoint.from(txid, 0).toArrayBuffer();
  const op2 = OutPoint.from(txid2, 0).toArrayBuffer();
  bst.set(op, new ArrayBuffer(1));
  bst.set(op2, new ArrayBuffer(1));
  let start = new ArrayBuffer(36);
  const res = bst.seekGreater(start);
  //console.log(Box.from(res).toHexString());

  const uint = parsePrimitive<u32>(Box.from(res).sliceFrom(32));
  const start2 = Box.concat([
    Box.from(res).shrinkBack(4),
    Box.from(primitiveToBuffer(uint + 1)),
  ]);
  //console.log(Box.from(start2).toHexString());
  const res2 = bst.seekGreater(start2);
  //console.log(Box.from(res2).toHexString());
  const uint2 = parsePrimitive<u32>(Box.from(res2).sliceFrom(32));
  const start3 = Box.concat([
    Box.from(res2).shrinkBack(4),
    Box.from(primitiveToBuffer(uint + 1)),
  ]);
  const res3 = bst.seekGreater(start3);
  //console.log(changetype<usize>(res3).toString());
  _flush();
}
*/

export function test_complement(): void {
  let v: i32 = -1;
  console.log((<u8>v).toString(10));
}

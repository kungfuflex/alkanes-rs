export class Version {
  static AUXPOW: u32 = 0x100;
  static POS_START: u32 = 0x200;
  static CHAIN_START: u32 = 0x10000;
  static CHAIN_ID: u32 = 20;
  public value: u32;
  constructor(value: u32) {
    this.value = value;
  }
  unwrap(): u32 {
    return this.value;
  }
  getBaseVersion(): u32 {
    return this.value % Version.AUXPOW;
  }
  getChainId(): u32 {
    return this.value / Version.CHAIN_START;
  }
  isAuxPow(): boolean {
    return (this.value & Version.AUXPOW) !== 0;
  }
  isProofOfStake(): boolean {
    return (this.value & Version.POS_START) !== 0;
  }
  isLegacy(): boolean {
    return this.value < Version.AUXPOW;
  }
}

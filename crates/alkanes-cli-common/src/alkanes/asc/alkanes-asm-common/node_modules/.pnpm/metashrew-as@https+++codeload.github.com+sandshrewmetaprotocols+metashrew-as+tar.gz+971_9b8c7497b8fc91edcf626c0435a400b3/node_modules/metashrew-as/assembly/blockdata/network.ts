
let selectedNetwork: network.Network = changetype<network.Network>(0);

export namespace network {
  export class Network {
    public p2pkhPrefix: u8;
    public p2shPrefix: u8;
    public bech32Prefix: ArrayBuffer;
    constructor(p2pkhByte: u8, p2shByte: u8, bech32Prefix: string) {
      this.p2pkhPrefix = p2pkhByte;
      this.p2shPrefix = p2shByte;
      this.bech32Prefix = String.UTF8.encode(bech32Prefix);
    }
    clone(): Network {
      return Network.fromTriple(this.p2pkhPrefix, this.p2shPrefix, String.UTF8.decode(this.bech32Prefix));
    }
    static fromTriple(p2pkhByte: u8, p2shByte: u8, bech32Prefix: string): Network {
      return new Network(p2pkhByte, p2shByte, bech32Prefix);
    }
    static get MAINNET(): Network {
      return Network.fromTriple(0x00, 0x05, "bc");
    }
    static get REGTEST(): Network {
      return Network.fromTriple(0x00, 0x05, "bcrt");
    }
  } 
  export function setNetwork(network: Network): void {
    selectedNetwork = network;
  }
  export function getNetwork(): Network { 
    if (changetype<usize>(selectedNetwork) === 0) return Network.MAINNET;
    return selectedNetwork.clone();
  }
}

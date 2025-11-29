import {
  CYCLE_EPOCHS,
  DIFFCHANGE_INTERVAL,
  primitiveToBuffer,
  concat,
  Box,
} from "../utils";
import { Height } from "./height";
import { Epoch } from "./epoch";
import { OutPoint } from "./transaction";

export class Sat {
  private _idx: u64 = 0;
  static SUPPLY: u64 = 2099999997690000;

  constructor(idx: u64) {
    this._idx = idx;
  }

  public n(): u64 {
    return this._idx;
  }

  public add(x: u64): Sat {
    this._idx += x;
    return this;
  }

  public height(): Height {
    return this.epoch()
      .startingHeight()
      .add(<u32>(this.epochPosition() / this.epoch().subsidy()));
  }
  public cycle(): u32 {
    return <u32>(this.epoch().n() / CYCLE_EPOCHS);
  }
  public third(): u64 {
    return <u64>this.epochPosition() % <u64>this.epoch().subsidy();
  }
  public period(): u32 {
    return <u32>(this.height().n() / DIFFCHANGE_INTERVAL);
  }
  public epochPosition(): u64 {
    return this._idx - this.epoch().startingSat().n();
  }
  public epoch(): Epoch {
    return Epoch.fromSat(this);
  }
  public isCommon(): bool {
    let epoch = this.epoch();
    return (this.n() - epoch.startingSat().n()) % epoch.subsidy() != 0;
  }

  static LAST(): Sat {
    return new Sat(Sat.SUPPLY - 1);
  }
}

export class SatPoint {
  public offset: u64;
  public outpoint: ArrayBuffer;
  constructor(outpoint: ArrayBuffer, offset: u64) {
    this.outpoint = outpoint;
    this.offset = offset;
  }
  toArrayBuffer(): ArrayBuffer {
    const outpointBytes = this.outpoint;
    const offsetBytes = new ArrayBuffer(
      outpointBytes.byteLength + <i32>sizeof<u64>(),
    );
    store<u64>(changetype<usize>(offsetBytes), this.offset);
    return Box.concat([Box.from(outpointBytes), Box.from(offsetBytes)]);
  }
  static from(outpoint: ArrayBuffer, offset: u64): SatPoint {
    return new SatPoint(outpoint, offset);
  }
}

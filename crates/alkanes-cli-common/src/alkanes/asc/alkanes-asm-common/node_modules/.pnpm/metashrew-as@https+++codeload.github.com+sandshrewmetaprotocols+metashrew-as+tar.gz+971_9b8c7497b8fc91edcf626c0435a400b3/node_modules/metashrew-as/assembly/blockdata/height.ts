import { Epoch } from "./epoch";
import { Sat } from "./sat";

export class Height {
  private _idx: u32;
  constructor(idx: u32) {
    this._idx = idx;
  }
  public n(): u32 {
    return this._idx;
  }
  public subsidy(): u64 {
    return Epoch.fromHeight(this).subsidy();
  }
  public startingSat(): Sat {
    let startingSat = Epoch.fromHeight(this).startingSat();
    let startingHeight = Epoch.fromHeight(this).startingHeight();
    return startingSat.add(
      <u64>(this.n() - startingHeight.n()) * Epoch.fromHeight(this).subsidy(),
    );
  }
  public add(x: u32): Height {
    return new Height(this.n() + x);
  }
}

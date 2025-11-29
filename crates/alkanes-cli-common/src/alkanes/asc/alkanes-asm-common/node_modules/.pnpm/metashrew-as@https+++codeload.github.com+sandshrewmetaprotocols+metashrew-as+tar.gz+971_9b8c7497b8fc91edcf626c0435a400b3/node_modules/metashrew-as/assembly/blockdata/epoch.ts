import { Sat } from "./sat";
import { Height } from "./height";
import { SUBSIDY_HALVING_INTERVAL, COIN_VALUE } from "../utils/constant";

export class Epoch {
  public idx: u32;

  static STARTING_SATS: Array<Sat> = [
    new Sat(0),
    new Sat(1050000000000000),
    new Sat(1575000000000000),
    new Sat(1837500000000000),
    new Sat(1968750000000000),
    new Sat(2034375000000000),
    new Sat(2067187500000000),
    new Sat(2083593750000000),
    new Sat(2091796875000000),
    new Sat(2095898437500000),
    new Sat(2097949218750000),
    new Sat(2098974609270000),
    new Sat(2099487304530000),
    new Sat(2099743652160000),
    new Sat(2099871825870000),
    new Sat(2099935912620000),
    new Sat(2099967955890000),
    new Sat(2099983977420000),
    new Sat(2099991988080000),
    new Sat(2099995993410000),
    new Sat(2099997995970000),
    new Sat(2099998997250000),
    new Sat(2099999497890000),
    new Sat(2099999748210000),
    new Sat(2099999873370000),
    new Sat(2099999935950000),
    new Sat(2099999967240000),
    new Sat(2099999982780000),
    new Sat(2099999990550000),
    new Sat(2099999994330000),
    new Sat(2099999996220000),
    new Sat(2099999997060000),
    new Sat(2099999997480000),
    new Sat(Sat.SUPPLY),
  ];
  static FIRST_POST_SUBSIDY: Epoch = new Epoch(33);
  static fromHeight(i: Height): Epoch {
    return new Epoch(i.n() / SUBSIDY_HALVING_INTERVAL);
  }

  static fromSat(sat: Sat): Epoch {
    let idx = 0;
    while (sat.n() > Epoch.STARTING_SATS[idx].n() && idx < 33) {
      idx++;
    }
    return new Epoch(idx);
  }

  constructor(idx: u32) {
    this.idx = idx;
  }

  n(): u32 {
    return this.idx;
  }

  startingSat(): Sat {
    if (this.idx > 33) return Epoch.STARTING_SATS[33];
    return Epoch.STARTING_SATS[this.idx];
  }

  // return the subsidy for the given epoch
  subsidy(): u64 {
    if (this.idx > 33) {
      return 0;
    } else {
      return (50 * COIN_VALUE) >> this.idx;
    }
  }

  startingHeight(): Height {
    return new Height(this.idx * SUBSIDY_HALVING_INTERVAL);
  }
}

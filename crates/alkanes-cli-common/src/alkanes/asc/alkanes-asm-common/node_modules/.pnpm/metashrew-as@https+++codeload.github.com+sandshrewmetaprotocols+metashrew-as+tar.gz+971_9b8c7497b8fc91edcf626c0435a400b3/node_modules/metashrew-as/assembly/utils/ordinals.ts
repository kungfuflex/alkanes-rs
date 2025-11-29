export const SUPPLY: u64 = 2099999997690000;
export const LAST: u64 = SUPPLY - 1;

// subsidy of block @ given height
export function subsidy(height: u64): u64 {
  return (50 * 100000000) >> (height / 210000);
}

// population
export function population(ordinal: u64): u64 {
  let population = 0;
  for (let i = 0; i < ordinal; i++) {
    population += ordinal & 1;
    ordinal >>= 1;
  }
  return population;
}

// first ordinal of subsidy of block @ given height
export function firstOrdinal(height: u64): number {
  let start: u64 = 0;
  for (let i = 0; i < height; i++) {
    start += subsidy(height);
  }
  return start;
}

// WIP
// export class Range {
//   ordinals: Array<u64>;
//   constructor()
// }

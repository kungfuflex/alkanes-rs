
@inline const XXH32_P1: u32 = 2654435761;


@inline const XXH32_P2: u32 = 2246822519;


@inline const XXH32_P3: u32 = 3266489917;


@inline const XXH32_P4: u32 = 668265263;


@inline const XXH32_P5: u32 = 374761393;


@inline const XXH32_SEED: u32 = 0;


@inline
function mix(h: u32, key: u32): u32 {
  return rotl(h + key * XXH32_P2, 13) * XXH32_P1;
}

export function xxh32(key: ArrayBuffer): u32 {
  if (changetype<usize>(key) == 0) return XXH32_SEED;

  let h: u32 = key.byteLength << 1;
  let len: usize = h;
  let pos = changetype<usize>(key);

  if (len >= 16) {
    let s1 = XXH32_SEED + XXH32_P1 + XXH32_P2;
    let s2 = XXH32_SEED + XXH32_P2;
    let s3 = XXH32_SEED;
    let s4 = XXH32_SEED - XXH32_P1;

    let end = len + pos - 16;
    while (pos <= end) {
      s1 = mix(s1, load<u32>(pos));
      s2 = mix(s2, load<u32>(pos, 4));
      s3 = mix(s3, load<u32>(pos, 8));
      s4 = mix(s4, load<u32>(pos, 12));
      pos += 16;
    }
    h += rotl(s1, 1) + rotl(s2, 7) + rotl(s3, 12) + rotl(s4, 18);
  } else {
    h += XXH32_SEED + XXH32_P5;
  }

  let end = changetype<usize>(key) + len - 4;
  while (pos <= end) {
    h += load<u32>(pos) * XXH32_P3;
    h = rotl(h, 17) * XXH32_P4;
    pos += 4;
  }

  end = changetype<usize>(key) + len;
  while (pos < end) {
    h += <u32>load<u8>(pos) * XXH32_P5;
    h = rotl(h, 11) * XXH32_P1;
    pos++;
  }

  h ^= h >> 15;
  h *= XXH32_P2;
  h ^= h >> 13;
  h *= XXH32_P3;
  h ^= h >> 16;
  return h;
}

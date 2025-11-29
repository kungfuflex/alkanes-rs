import { reverse } from "./utils";
import { sha256 } from "fast-sha256-as/assembly/sha256.ts";

export { sha256 };

export function sha256d(data: ArrayBuffer): ArrayBuffer {
  return reverse(sha256(sha256(data)));
}

import { ExtendedCallResponse } from "./crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly";

export function __execute(): i32 {
  const response = new ExtendedCallResponse();
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}

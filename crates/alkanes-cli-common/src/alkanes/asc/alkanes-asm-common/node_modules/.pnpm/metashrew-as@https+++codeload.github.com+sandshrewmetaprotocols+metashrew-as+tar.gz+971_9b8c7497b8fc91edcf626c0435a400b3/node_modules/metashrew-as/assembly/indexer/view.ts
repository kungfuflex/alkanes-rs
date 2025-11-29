import { Index, input } from "./index";
import { HEIGHT_TO_BLOCK_HEADER } from "./tables";
import { input } from "./input";

export function blockbyheight(): ArrayBuffer {
  return get(Index.keyFor(HEIGHT_TO_BLOCK_HEADER, input()));
}

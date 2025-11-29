
@external("env", "__log") declare function __log(s: ArrayBuffer): void;
function _log(s: string): void {
  console.log(s);
}
class Console {
  log(v: string): void {
    __log(String.UTF8.encode(v, true));
  }
  logUTF8(data: ArrayBuffer): void {
    __log(data);
  }
}

export const console: Console = new Console();

// Pointer utilities (simplified from metashrew-as)
import { memcpy } from "./memcpy";
import { Box } from "./box";

export function toPointer(v: usize): Pointer {
  return new Pointer(v);
}

export function nullptr<T>(): T {
  return changetype<T>(0);
}

@final
export class Pointer {
  [key: number]: number;

  @inline constructor(ptr: usize) {
    return changetype<Pointer>(ptr);
  }

  @inline deref<T>(): T {
    return load<T>(this.asUsize());
  }

  @inline store<T>(v: T): Pointer {
    store<T>(this.asUsize(), v);
    return this;
  }

  @inline asRef<T>(): T {
    return changetype<T>(this);
  }

  @inline asUsize(): usize {
    return this.asRef<usize>();
  }

  @inline copyInto<T>(src: T): Pointer {
    memcpy(this.asUsize(), toPointer(src).asUsize(), sizeof<T>());
    return this;
  }

  @inline offset<T>(i: usize): Pointer {
    return toPointer(this.asUsize() + i * sizeof<T>());
  }

  @inline index<T>(i: usize): T {
    return this.offset<T>(i).asRef();
  }

  distanceTo(b: Pointer): usize {
    const au = this.asUsize();
    const bu = b.asUsize();
    if (bu > au) return bu - au;
    else return au - bu;
  }

  @inline
  toBox(len: usize): Box {
    return new Box(this.asUsize(), len);
  }
}

// Rust-style Option monad for TypeScript
// Adapted from thames-technology/monads (MIT License)

type NonUndefined = {} | null;

interface Match<A, B> {
  some: (val: A) => B;
  none: (() => B) | B;
}

export interface Option<T extends NonUndefined> {
  type: symbol;
  isSome(): boolean;
  isNone(): boolean;
  match<U extends NonUndefined>(fn: Match<T, U>): U;
  map<U extends NonUndefined>(fn: (val: T) => U): Option<U>;
  andThen<U extends NonUndefined>(fn: (val: T) => Option<U>): Option<U>;
  or(optb: Option<T>): Option<T>;
  orElse(optb: () => Option<T>): Option<T>;
  and<U extends NonUndefined>(optb: Option<U>): Option<U>;
  unwrapOr(def: T): T;
  unwrap(): T | never;
}

export const OptionType = {
  Some: Symbol(':some'),
  None: Symbol(':none'),
};

class SomeImpl<T extends NonUndefined> implements Option<T> {
  constructor(private readonly val: T) {}
  get type() { return OptionType.Some; }
  isSome() { return true; }
  isNone() { return false; }
  match<B>(fn: Match<T, B>): B { return fn.some(this.val); }
  map<U extends NonUndefined>(fn: (val: T) => U): Option<U> { return Some(fn(this.val)); }
  andThen<U extends NonUndefined>(fn: (val: T) => Option<U>): Option<U> { return fn(this.val); }
  or(_optb: Option<T>): Option<T> { return this; }
  orElse(_optb: () => Option<T>): Option<T> { return this; }
  and<U extends NonUndefined>(optb: Option<U>): Option<U> { return optb; }
  unwrapOr(_def: T): T { return this.val; }
  unwrap(): T { return this.val; }
}

class NoneImpl<T extends NonUndefined> implements Option<T> {
  get type() { return OptionType.None; }
  isSome() { return false; }
  isNone() { return true; }
  match<U>({ none }: Match<T, U>): U {
    return typeof none === 'function' ? (none as () => U)() : none;
  }
  map<U extends NonUndefined>(_fn: (val: T) => U): Option<U> { return new NoneImpl<U>(); }
  andThen<U extends NonUndefined>(_fn: (val: T) => Option<U>): Option<U> { return new NoneImpl<U>(); }
  or<U extends NonUndefined>(optb: Option<U>): Option<U> { return optb; }
  orElse(optb: () => Option<T>): Option<T> { return optb(); }
  and<U extends NonUndefined>(_optb: Option<U>): Option<U> { return new NoneImpl<U>(); }
  unwrapOr(def: T): T { return def; }
  unwrap(): never { throw new ReferenceError('Trying to unwrap None.'); }
}

export function Some<T extends NonUndefined>(val: T): Option<T> {
  return new SomeImpl(val);
}

export const None: Option<any> = new NoneImpl();

export function isSome<T extends NonUndefined>(val: Option<T>): boolean {
  return val.isSome();
}

export function isNone<T extends NonUndefined>(val: Option<T>): boolean {
  return val.isNone();
}

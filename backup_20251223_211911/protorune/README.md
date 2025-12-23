# protorune-rs

Rust abstractions for protorunes subprotocols, built for a metashrew runtime.

# Some helpful commands

Building

```
cargo build
```

Integration Testing (end to end)

- These test the compiled wasm and generally have test fixtures that create runes, do protoburns, and test other functionality

```
cargo test
```

Unit testing

- These are written inside the library rust code
- Do not compile to wasm, instead unit test the native rust. Therefore, you need to find the correct target for your local machine to properly run these tests. Below are some common targets for some architectures:
  - Macbook intel x86: `x86_64-apple-darwin`
  - Macbook Apple silicon: `aarch64-apple-darwin`

```
cargo test -p ordinals --target TARGET # to test the ordinals crate, which has some protostones unit tests

cargo test --target TARGET
```

## License

MIT

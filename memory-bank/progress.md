# Progress: Refactor ALKANES-RS Imports

## Task Status

- [ ] **`alkanes-std-*` Crates:** Refactor imports to use `alkanes_runtime`.
- [ ] **`src/tests/**`:** Refactor imports to use `metashrew_core::{println, stdio::{stdout}}`.
- [ ] **Build and Test:** Compile and test the entire project to ensure all changes are correct.

## Detailed Steps

### `alkanes-std-*` Refactoring
- [ ] Identify all `alkanes-std-*` crates.
- [ ] For each crate, replace old import paths with `alkanes_runtime` paths.
- [ ] Verify that each crate builds individually.

### `src/tests/**` Refactoring
- [ ] Identify all test files in `src/tests/`.
- [ ] Replace `println!` and `stdout` usages with `metashrew_core` equivalents.
- [ ] Verify that tests compile and run.
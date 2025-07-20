# Active Context: Refactor ALKANES-RS Imports

## Current Task

The primary objective is to refactor the ALKANES-RS codebase to standardize import paths and ensure the project builds successfully. This involves two main goals:

1.  **Update `alkanes-std-*` Crates:** All standard library crates (`crates/alkanes-std-*`) must be updated to use imports from the `alkanes_runtime` crate. This centralizes the runtime dependencies for all standard smart contracts.

2.  **Update `src/tests/**` Imports:** All integration tests located within the `src/tests/` directory must be updated to use `metashrew_core::{println, stdio::{stdout}}` for console output. This aligns the testing framework with the `metashrew` ecosystem's core libraries.

## Plan of Action

1.  **Create `progress.md`:** Establish a progress tracking document to monitor the status of the refactoring tasks.
2.  **Identify Target Files:** Systematically identify all files within `crates/alkanes-std-*` and `src/tests/` that require import changes.
3.  **Refactor `alkanes-std-*` Imports:** Modify the identified standard library crates to use `alkanes_runtime`.
4.  **Refactor `src/tests/**` Imports:** Modify the identified test files to use `metashrew_core` for output.
5.  **Build and Test:** Compile the entire project and run all tests to verify that the changes are correct and the build is successful.
//! `BuildInfo` — the canonical, self-contained, parameterized artifact that fully
//! specifies how to reproduce ONE alkane's on-chain wasm from source. It captures
//! every environment axis we found matters for a byte-exact reproduction:
//! toolchain (rustc + the darwin host-triple trick), the C toolchain (clang vendor
//! + how to obtain it, incl. the assembled-Homebrew-clang recipe), the exact HOME
//! and path-remap, the registry strategy (committed lockfile vs a crates.io-index
//! time-machine), git-dep source-id spelling (bare vs rev), and the full resolved
//! lockfile — plus the diff result.
//!
//! A `BuildInfo` is meant to round-trip: `reverse` populates the reversed fields
//! from the target wasm; `build` consumes it to reconstruct the wasm; `verify`
//! fills `result`. The whole thing serializes to one JSON the explorer / verifier
//! or any third party can consume to reproduce the alkane in a controlled sandbox.

use serde::{Deserialize, Serialize};

/// The spec version. Semver contract: consumers reject an unknown MAJOR; a higher
/// MINOR is backward-compatible (only additive OPTIONAL fields), so a 1.0.0 consumer
/// safely reads a 1.1.0 artifact by ignoring the fields it doesn't know. Bump the
/// MAJOR only on a breaking change to an existing field's shape/meaning.
///
/// # Lineage
/// - **1.0.0** — initial spec. The axes required for byte-exact single-alkane
///   reproduction: toolchain (rustc + the darwin host-triple copy trick), C
///   toolchain (clang vendor/version + apt / llvm-release / homebrew-bottle
///   assembly), HOME + path-remap, registry (committed-lock vs a crates.io-index
///   time-machine), a single git-dep source-id spelling (`bare`|`rev`), the full
///   resolved lockfile, and the diff verdict.
/// - **1.1.0** — reproduction reach extended; ALL additions are optional/additive:
///   * `GitDep.redirect_to` — unify a crate pulled via DIFFERENT mirror URLs (e.g.
///     `sandshrewmetaprotocols` ↔ `kungfuflex` `metashrew`) onto ONE source, so
///     cargo does not build two incompatible copies ("multiple versions" E0599).
///   * `GitDep.pin_transitive` — force the rev across ALL lockfile occurrences,
///     including a dep pulled TRANSITIVELY through another git dep (lockfile
///     `cargo update -p <crate> --precise <rev>`), not just the in-tree Cargo.toml.
///   * `CToolchain.wasm_cflags` — extra CFLAGS for the wasm C compile (e.g.
///     `-nostdlibinc`), letting a stock apt clang cross-compile secp256k1→wasm
///     (previously only a self-contained Homebrew clang could — the host glibc
///     `/usr/include` leak on `wasm32-unknown-unknown`).
///   * `Toolchain.rustc_version` MAY now be a CHANNEL (`nightly-YYYY-MM-DD` /
///     `beta`), not only a semver `x.y.z` — for a contract built off a nightly.
///   * `Registry.archive_commit` is OPTIONAL: when absent it is auto-derived from
///     the deploy date (`timestamp`) by selecting the crates.io-index-archive
///     `snapshot-YYYY-MM-DD` branch that CONTAINS that date and `rev-list --before`.
///   * MINIMAL RECIPE: a BuildInfo carrying only `identity` + `source` (repo +
///     commit + package) is valid — the verifier AUTO-REVERSES toolchain /
///     environment / registry / git_deps from the on-chain bytecode fingerprint;
///     any field the author DID supply overrides its auto-derived value.
/// - **1.2.0** — rustc provenance modeled; additive/optional:
///   * `Toolchain.rustc_source` (`RustcSource`) — HOW to obtain rustc, parallel to
///     `CToolchain.source` for clang. Absent ⇒ rustup installs the channel/semver
///     (the common case). `method = "git-build"` ⇒ COMPILE rustc from a
///     rust-lang/rust commit (with `channel` + `download_ci_llvm`), the only way to
///     reproduce a contract built off an UNPUBLISHED toolchain — e.g. a stable
///     branch-point like `9fc6b431` ("Prepare 1.85.0") whose `/rustc/<hash>/` sysroot
///     no rustup channel ships. The builder-verifier can thus reproduce ANY rustc.
pub const SCHEMA_VERSION: &str = "1.2.0";

/// Top-level artifact.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BuildInfo {
    /// Always `SCHEMA_VERSION` at emit time; consumers reject unknown majors.
    pub schema_version: String,
    pub identity: Identity,
    pub source: Source,
    pub toolchain: Toolchain,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c_toolchain: Option<CToolchain>,
    pub environment: Environment,
    pub registry: Registry,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub git_deps: Vec<GitDep>,
    /// The resolved dependency graph (parsed from the lockfile, for browsing).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_deps: Vec<ResolvedDep>,
    /// base64 of the exact Cargo.lock used, when known (the ground truth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_lock_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<MatchResult>,
    /// Free-form provenance notes from the reverser / builder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Where the target wasm came from — an on-chain alkane id OR a local file. Every
/// downstream step (reverse → build → verify) is identical regardless of source.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Identity {
    /// `"onchain"` (fetched by alkane id via jsonrpc) | `"local-file"` (offline).
    pub target_source: String,
    /// Alkane id `block:tx` when known (always for onchain; optional for a file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alkane_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    /// sha256 of the target wasm (the thing to reproduce).
    pub target_sha256: String,
    pub target_size: usize,
    /// For a clone: `getbytecode` resolved to a template's bytecode — verifying the
    /// template transitively verifies this alkane. Records the template id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloned_from: Option<String>,
}

/// The source: a git ref OR inline stringified sources (self-contained), plus the
/// exact build command.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source {
    /// `"git"` | `"inline"`.
    pub kind: String,
    // ── git ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Sub-directory of the repo the crate lives in (workspace member path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdir: Option<String>,
    /// Cargo package name being built (`-p <package>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    pub is_workspace_member: bool,
    // ── inline (self-contained) ──
    /// path -> file contents. Text stored raw; binary base64 with a `base64:` prefix.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inline_files: Vec<InlineFile>,
    // ── build ──
    /// Cargo features enabled (e.g. `["mainnet"]` for a per-network genesis build).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    /// The literal cargo invocation, for transparency + exact replay.
    pub build_command: String,
    /// wasm artifact filename produced (e.g. `alkanes_std_genesis_alkane_upgraded_eoa.wasm`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InlineFile {
    pub path: String,
    /// Raw UTF-8 contents, or `base64:<...>` for binary.
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Toolchain {
    /// e.g. `1.86.0`. (spec 1.1.0) MAY also be a CHANNEL string — `nightly-YYYY-MM-DD`
    /// or `beta` — for a contract built off a non-release toolchain; the builder
    /// `rustup`-installs the channel and does NOT emit it into Cargo.toml's
    /// `rust-version` field (which only accepts semver).
    pub rustc_version: String,
    /// The rustc commit hash embedded in `/rustc/<hash>` paths, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc_commit: Option<String>,
    /// The toolchain directory name whose std panic paths must match, e.g.
    /// `1.86.0-aarch64-apple-darwin`. THE TRICK: physically COPY (not symlink —
    /// rustc canonicalizes symlinks) the linux toolchain to `$RUSTUP_HOME/toolchains/<this>`
    /// so the embedded `.../toolchains/<host_triple>/lib/rustlib/src/...` paths match.
    pub host_triple: String,
    /// OS base image for the sandbox, e.g. `ubuntu-22.04`.
    pub os_image: String,
    /// Wasm target (almost always `wasm32-unknown-unknown`).
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linker: Option<String>,
    /// (spec 1.2.0) HOW to obtain rustc. Absent ⇒ `rustup` installs `rustc_version`
    /// (a release semver or a nightly/beta channel) directly — the common case. Present
    /// with `method = "git-build"` ⇒ compile rustc from a rust-lang/rust commit that no
    /// rustup channel ships (e.g. a stable BRANCH-POINT like `9fc6b431` "Prepare 1.85.0",
    /// whose `/rustc/<hash>/` sysroot paths NO installable toolchain reproduces). Parallel
    /// to `CToolchain.source` for clang.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc_source: Option<RustcSource>,
}

/// (spec 1.2.0) How rustc itself is obtained. The builder installs a channel by default;
/// `git-build` makes it compile rustc from source so ANY toolchain — released or not —
/// can be reproduced, which is the only path for a contract built off an unpublished
/// rustc commit (the `/rustc/<hash>/` panic-path hash is the sole fingerprint).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RustcSource {
    /// `"rustup"` (default; install `rustc_version` as a channel/semver) |
    /// `"git-build"` (compile rust-lang/rust from `git_commit`).
    pub method: String,
    /// git-build: the rust-lang/rust commit to build. This is the hash that ends up in
    /// `/rustc/<hash>/` sysroot paths, so it must equal `Toolchain.rustc_commit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    /// git-build: source repo, when not the canonical `rust-lang/rust`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_repo: Option<String>,
    /// git-build: release channel to configure (`stable` | `beta` | `nightly` | `dev`).
    /// Matters for CODEGEN, not just the version string: it sets the defaults for
    /// `debug-assertions` / `overflow-checks`, so the wrong channel diverges the output
    /// even at the right commit. A stable branch-point wants `stable`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// git-build: use the commit's CI-built LLVM (`download-ci-llvm`) so codegen matches
    /// the OFFICIAL release build. `false` ⇒ build the bundled `src/llvm-project` submodule
    /// (much slower, and may diverge from the official LLVM). Defaults to true when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_ci_llvm: Option<bool>,
}

/// The C toolchain, when the crate links C (e.g. secp256k1-sys). Its vendor+version
/// is recorded verbatim in the wasm `producers` section, so it must match exactly.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CToolchain {
    /// `Ubuntu` | `Debian` | `Homebrew` | `` (LLVM.org).
    pub vendor: String,
    pub version: String,
    /// How to obtain a matching clang on Linux.
    pub source: ClangSource,
    /// The known secp256k1 C-object footprint host-dependence: a macOS-built
    /// (Homebrew) clang and a Linux-built (Homebrew) clang of the SAME LLVM version
    /// emit a marginally different secp256k1 static footprint, shifting the memory
    /// base by a few bytes. When true, a Linux reconstruction reaches `verified`
    /// (logic + producers exact) but not full `reproducible` without the origin host.
    #[serde(default)]
    pub c_object_host_dependent: bool,
    /// (spec 1.1.0) Extra CFLAGS for the wasm C compile, e.g. `-nostdlibinc`. A stock
    /// apt clang targeting `wasm32-unknown-unknown` leaks the host glibc `/usr/include`
    /// (→ `bits/libc-header-start.h` not found), so secp256k1-sys fails; `-nostdlibinc`
    /// drops the system libc includes (keeping clang builtins + the crate's bundled
    /// `wasm-sysroot`) so an apt clang-13/14/15 compiles secp256k1→wasm. Empty ⇒ a
    /// self-contained clang (Homebrew) that needs no sysroot flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_cflags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClangSource {
    /// `apt` | `llvm-release` | `homebrew-bottle` | `wasi-sdk` (1.2.0: the clang in a
    /// WASI SDK release, e.g. wasi-sdk-20's bin/clang → producers "clang 16.0.0"; `cc`
    /// retargets wasm32-wasi → wasm32-unknown-unknown) | `unpinned`.
    pub method: String,
    /// apt package (e.g. `clang-14`) when method=apt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apt_package: Option<String>,
    /// GHCR bottle blob sha256s to assemble Homebrew clang on Linux (llvm+z3+libedit),
    /// then patchelf the interpreter/rpath (see the `formula` module + hbclang.sh).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub homebrew_bottles: Vec<HomebrewBottle>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HomebrewBottle {
    /// GHCR repo under `homebrew/core`, e.g. `llvm`, `z3`, `libedit`.
    pub formula: String,
    /// x86_64_linux bottle blob sha256 (content-addressed → persists even untagged).
    pub blob_sha256: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Environment {
    /// The builder's HOME, reversed from embedded panic paths (e.g. `/Users/kevinyao`,
    /// `/home/vitor`). The registry/git-checkout/toolchain paths hang off this.
    pub home: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_home: Option<String>,
    /// Extra env exports (`KEY=VALUE`), e.g. `FREE_MINT_BUILD_IN_PROGRESS=1`,
    /// `CC_wasm32_unknown_unknown=...`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_env: Vec<String>,
    /// `--remap-path-prefix` rules applied at build time (empty ⇒ paths are raw,
    /// so HOME must be reconstructed). Recording them lets a reverser know the
    /// embedded paths are NOT the real build paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remap_path_prefix: Vec<String>,
}

/// How the crates.io dependency versions are pinned.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Registry {
    /// `committed-lock` — the repo commits Cargo.lock; use real crates.io + `--locked`
    /// (no time-machine). `time-machine` — no committed lock; freeze the
    /// crates.io-index tree at the build date and serve it AS index.crates.io so the
    /// registry src-dir hash is canonical for that cargo version.
    pub mode: String,
    // ── time-machine ──
    /// `rust-lang/crates.io-index-archive` commit frozen to the build date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Always `index.crates.io` — the frozen tree is served under this host so
    /// cargo computes the canonical `index.crates.io-<hash>` src-dir.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_as: Option<String>,
    /// The canonical registry src-dir hash cargo produces (cargo-version dependent),
    /// e.g. `index.crates.io-6f17d22bba15001f` (cargo 1.82) or `...-1949cf8c6b5b557f`
    /// (cargo 1.86) — reversed from the embedded registry paths as a cross-check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitDep {
    pub url: String,
    /// The resolved commit (full or short) the lock pins.
    pub rev: String,
    /// `bare` — resolve WITH the rev (current HEAD may lack pinned-era crates), then
    /// rewrite the lockfile source-id to bare (`?rev=…#` → `#`) and build `--locked`,
    /// so `-C metadata` (hence monomorphization ordering) matches a build that used a
    /// bare git URL. `rev` — leave the `?rev=` source-id.
    pub source_form: String,
    /// (spec 1.1.0) Unify a MIRRORED repo: rewrite every `git = "url"` referencing
    /// THIS `url` to `redirect_to` (same crate, different host — e.g. the pool pulls
    /// `sandshrewmetaprotocols/metashrew` while alkanes-rs pulls
    /// `kungfuflex/metashrew`). Cargo keys on the URL, so without this it builds two
    /// incompatible copies of the crate. Redirect both onto one source (+ this dep's
    /// `rev`) to collapse them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_to: Option<String>,
    /// (spec 1.1.0) Force `rev` onto ALL lockfile entries from this git source,
    /// including ones pulled TRANSITIVELY via another git dep (which the in-tree
    /// Cargo.toml rewrite can't reach). Applied post-resolve as
    /// `cargo update -p <crate> --precise <rev>`. Set when the same repo is a direct
    /// AND a transitive dep at drifting revs.
    #[serde(default)]
    pub pin_transitive: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// The outcome of diffing the rebuilt wasm against the target.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MatchResult {
    /// `reproducible` (byte-exact sha256) | `verified` (code+structure exact, only
    /// build-metadata/host-artifact bytes differ) | `code_match` | `partial` | `mismatch`.
    pub verdict: String,
    pub built_sha256: String,
    pub byte_exact: bool,
    pub normalized_match_pct: f64,
    pub sections: Vec<SectionDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SectionDiff {
    pub kind: String,
    pub target_size: usize,
    pub candidate_size: usize,
    pub matched: bool,
}

/// One `producers`-section entry.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProducersEntry {
    pub field: String,
    pub name: String,
    pub version: String,
}

/// The raw reversed readout from a target wasm — the intermediate the `reverse`
/// subcommand emits and the `build-info` pipeline folds into a `BuildInfo`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Reversed {
    pub sha256: String,
    pub size: usize,
    pub is_wasm: bool,
    #[serde(default)]
    pub custom_sections: Vec<String>,
    pub has_name_section: bool,
    pub has_producers_section: bool,
    #[serde(default)]
    pub producers: Vec<ProducersEntry>,
    pub rustc: Option<String>,
    pub rustc_commit: Option<String>,
    /// Vendor from `producers` (e.g. "Ubuntu clang", "Homebrew clang").
    pub clang_vendor: Option<String>,
    pub clang_version: Option<String>,
    /// Toolchain triple from `.../toolchains/<triple>/...`, e.g. `1.86.0-aarch64-apple-darwin`.
    pub host_triple: Option<String>,
    /// Reconstructed HOME (`/home/<u>` or `/Users/<u>`).
    pub home: Option<String>,
    /// The `index.crates.io-<hash>` registry src-dir hash (cargo-version dependent).
    pub registry_hash: Option<String>,
    #[serde(default)]
    pub dep_versions: Vec<String>,
    /// git deps as `(name, rev)` from `.cargo/git/checkouts/<name>-<hash>/<rev>/`.
    #[serde(default)]
    pub git_checkouts: Vec<(String, String)>,
    /// Local build directory (workspace root), e.g. `/Users/kevinyao/Documents/Code/alkanes-rs`.
    pub build_dir: Option<String>,
    /// Paths look `--remap-path-prefix`'d (no real home) — reproduction can't rely on paths.
    pub remapped: bool,
    /// Links a C dependency (secp256k1-sys) — clang matters + host-dependence caveat.
    pub links_c: bool,
    #[serde(default)]
    pub embedded_paths: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every golden fixture must deserialize into `BuildInfo` and round-trip back to
    /// JSON without losing required fields, keeping the schema and the docs in sync.
    fn roundtrip(json: &str, expect_id: &str) {
        let bi: BuildInfo = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("fixture {expect_id} failed to deserialize: {e}"));
        // The compat contract is MAJOR-based: a consumer accepts any artifact of the
        // same major (a 1.0.0 fixture round-trips fine under a 1.1.x reader — the new
        // 1.1 fields are optional). Assert the major, not the exact version.
        assert_eq!(
            bi.schema_version.split('.').next(),
            SCHEMA_VERSION.split('.').next(),
            "fixture {expect_id}: schema major must match SCHEMA_VERSION",
        );
        assert_eq!(bi.identity.alkane_id.as_deref(), Some(expect_id));
        let re = serde_json::to_string(&bi).unwrap();
        let _back: BuildInfo = serde_json::from_str(&re).unwrap();
    }

    #[test]
    fn fixture_free_mint_4_797() {
        let j = include_str!("fixtures/free-mint-4-797.json");
        roundtrip(j, "4:797");
        let bi: BuildInfo = serde_json::from_str(j).unwrap();
        assert_eq!(bi.identity.target_source, "onchain");
        assert_eq!(bi.registry.mode, "time-machine");
        assert_eq!(bi.git_deps[0].source_form, "bare");
        assert!(bi.result.as_ref().unwrap().byte_exact);
        assert_eq!(bi.result.as_ref().unwrap().verdict, "reproducible");
        // Linux build → C-object is NOT host-dependent.
        assert!(!bi.c_toolchain.as_ref().unwrap().c_object_host_dependent);
    }

    #[test]
    fn fixture_diesel_2_0() {
        let j = include_str!("fixtures/diesel-2-0.json");
        roundtrip(j, "2:0");
        let bi: BuildInfo = serde_json::from_str(j).unwrap();
        // The darwin host-triple trick + assembled Homebrew clang.
        assert!(bi.toolchain.host_triple.contains("apple-darwin"));
        let c = bi.c_toolchain.as_ref().unwrap();
        assert_eq!(c.vendor, "Homebrew");
        assert_eq!(c.source.method, "homebrew-bottle");
        assert_eq!(c.source.homebrew_bottles.len(), 3);
        assert!(c.c_object_host_dependent);
        assert_eq!(bi.result.as_ref().unwrap().verdict, "verified");
    }

    #[test]
    fn fixture_pair_equality_4_9200() {
        let j = include_str!("fixtures/pair-equality-4-9200.json");
        roundtrip(j, "4:9200");
        let bi: BuildInfo = serde_json::from_str(j).unwrap();
        // Not-yet-deployed → local-file target, committed-lock registry, rev git-dep.
        assert_eq!(bi.identity.target_source, "local-file");
        assert_eq!(bi.registry.mode, "committed-lock");
        assert_eq!(bi.git_deps[0].source_form, "rev");
        assert_eq!(bi.result.as_ref().unwrap().verdict, "partial");
    }
}

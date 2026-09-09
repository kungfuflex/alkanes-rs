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

/// Bump on any breaking change to the shape below.
pub const SCHEMA_VERSION: &str = "1.0.0";

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
    /// e.g. `1.86.0`.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClangSource {
    /// `apt` | `llvm-release` | `homebrew-bottle`.
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
        assert_eq!(bi.schema_version, SCHEMA_VERSION);
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

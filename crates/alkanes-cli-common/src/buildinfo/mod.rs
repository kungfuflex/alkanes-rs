//! BuildInfo workbench: reverse a target wasm's build env, reconstruct it in a
//! controlled sandbox, diff, and emit the canonical `BuildInfo` JSON. Designed so a
//! Claude skill (or any operator) can drive the sub-steps to reproduce an alkane and
//! get it verified on-chain. Everything except fetching an on-chain target is offline.

pub mod cli;
pub mod formula;
pub mod schema;
pub mod upload;
pub mod wasm;

pub use schema::*;
pub use wasm::{compare, reverse, sha256_hex};

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A target wasm has two sources; downstream steps are identical.
pub enum Target {
    /// Fetched by alkane id via jsonrpc getbytecode.
    Onchain { id: String, network: Option<String> },
    /// A local `.wasm` file (offline; also the path for not-yet-deployed alkanes).
    LocalFile(PathBuf),
}

/// Decide whether `--at` is an alkane id (`\d+:\d+`) or a local wasm path.
pub fn parse_at(at: &str) -> Target {
    let is_id = at
        .split_once(':')
        .map(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit()))
        .unwrap_or(false);
    if is_id && !Path::new(at).exists() {
        Target::Onchain { id: at.to_string(), network: None }
    } else {
        Target::LocalFile(PathBuf::from(at))
    }
}

/// Fold a reversed readout into a partially-populated BuildInfo skeleton. Fields that
/// can only come from inspecting the source repo (registry mode, git-dep URLs, the
/// exact build command) are best-guessed and flagged in `notes`.
pub fn reversed_to_buildinfo(rev: &Reversed, target_source: &str, alkane_id: Option<String>, network: Option<String>) -> BuildInfo {
    let is_darwin = rev.host_triple.as_deref().map(|t| t.contains("apple-darwin")).unwrap_or(false)
        || rev.clang_vendor.as_deref().map(|v| v.contains("Homebrew")).unwrap_or(false);
    let host_triple = rev.host_triple.clone().unwrap_or_else(|| {
        let ver = rev.rustc.as_deref().unwrap_or("1.82.0").split_whitespace().next().unwrap_or("1.82.0");
        if is_darwin { format!("{ver}-aarch64-apple-darwin") } else { format!("{ver}-x86_64-unknown-linux-gnu") }
    });
    let rustc_version = rev.rustc.as_deref().unwrap_or("").split_whitespace().next().unwrap_or("").to_string();

    // C toolchain, when the wasm links C.
    let c_toolchain = if rev.links_c {
        let vendor = rev.clang_vendor.clone().unwrap_or_default().replace(" clang", "").replace("clang", "").trim().to_string();
        let version = rev.clang_version.clone().unwrap_or_default();
        let source = if vendor == "Homebrew" {
            ClangSource { method: "homebrew-bottle".into(), apt_package: None, homebrew_bottles: vec![] }
        } else if vendor == "Ubuntu" || vendor == "Debian" {
            let major = version.split('.').next().unwrap_or("14");
            ClangSource { method: "apt".into(), apt_package: Some(format!("clang-{major}")), homebrew_bottles: vec![] }
        } else {
            ClangSource { method: "llvm-release".into(), apt_package: None, homebrew_bottles: vec![] }
        };
        Some(CToolchain {
            vendor,
            version,
            source,
            c_object_host_dependent: is_darwin,
            note: if is_darwin {
                Some("secp256k1 C-object footprint is host-dependent between macOS and Linux Homebrew clang; Linux reconstruction reaches `verified`, full `reproducible` needs the origin host clang".into())
            } else { None },
        })
    } else { None };

    let git_deps = rev
        .git_checkouts
        .iter()
        .map(|(name, rev_)| GitDep {
            url: guess_git_url(name),
            rev: rev_.clone(),
            source_form: "bare".into(),
        })
        .collect();

    let mut notes = vec![
        "skeleton reversed from the target wasm; registry mode + git-dep URLs + build command need source-repo inspection".to_string(),
    ];
    if rev.remapped {
        notes.push("paths look --remap-path-prefix'd; reproduction can't rely on embedded HOME".into());
    }

    BuildInfo {
        schema_version: SCHEMA_VERSION.to_string(),
        identity: Identity {
            target_source: target_source.to_string(),
            alkane_id,
            network,
            target_sha256: rev.sha256.clone(),
            target_size: rev.size,
            cloned_from: None,
        },
        source: Source {
            kind: "git".into(),
            repo: None,
            commit: None,
            subdir: rev.build_dir.clone(),
            package: None,
            is_workspace_member: rev.build_dir.as_deref().map(|d| d.contains("alkanes-rs") || d.contains("/crates/")).unwrap_or(false),
            inline_files: vec![],
            features: vec![],
            build_command: "cargo build --release --target wasm32-unknown-unknown -p <package>".into(),
            artifact: None,
        },
        toolchain: Toolchain {
            rustc_version,
            rustc_commit: rev.rustc_commit.clone(),
            host_triple,
            os_image: "ubuntu-22.04".into(),
            target: "wasm32-unknown-unknown".into(),
            linker: None,
        },
        c_toolchain,
        environment: Environment {
            home: rev.home.clone().unwrap_or_else(|| "/root".into()),
            cargo_home: None,
            build_env: vec![],
            remap_path_prefix: vec![],
        },
        registry: Registry {
            mode: "time-machine".into(),
            archive_commit: None,
            timestamp: None,
            served_as: Some("index.crates.io".into()),
            registry_hash: rev.registry_hash.clone(),
        },
        git_deps,
        resolved_deps: vec![],
        cargo_lock_b64: None,
        result: None,
        notes,
    }
}

fn guess_git_url(name: &str) -> String {
    match name {
        "alkanes-rs" => "https://github.com/kungfuflex/alkanes-rs".into(),
        "metashrew" => "https://github.com/sandshrewmetaprotocols/metashrew".into(),
        other => format!("https://github.com/kungfuflex/{other}"),
    }
}

/// Run the reproduction formula for a BuildInfo in a docker sandbox, returning the
/// built wasm bytes. Requires docker on the host (the CLI IS the controlled-env runner).
pub fn run_build(bi: &BuildInfo, workdir: &Path) -> Result<Vec<u8>> {
    std::fs::create_dir_all(workdir).ok();
    let script = formula::generate_build_script(bi);
    let script_path = workdir.join("repro.sh");
    std::fs::write(&script_path, &script).context("write repro.sh")?;
    let out_dir = workdir.join("out");
    std::fs::create_dir_all(&out_dir).ok();
    let image = docker_image(&bi.toolchain.os_image);
    let status = Command::new("docker")
        .args([
            "run", "--rm", "--add-host", "index.crates.io:127.0.0.1",
            "-v", &format!("{}:/repro.sh", script_path.display()),
            "-v", &format!("{}:/out", out_dir.display()),
            &image, "bash", "-c", "bash /repro.sh",
        ])
        .status()
        .context("docker run (is docker installed + running?)")?;
    if !status.success() {
        return Err(anyhow!("build container exited non-zero"));
    }
    let built = out_dir.join("built.wasm");
    std::fs::read(&built).with_context(|| format!("no wasm produced at {}", built.display()))
}

fn docker_image(os_image: &str) -> String {
    match os_image {
        "ubuntu-22.04" => "ubuntu:22.04".into(),
        "debian-bookworm" => "debian:bookworm".into(),
        other => other.replace('-', ":"),
    }
}

/// Full pipeline: reverse a target, (optionally) enrich from a source dir, build, diff,
/// emit the populated BuildInfo. `source_hints` overrides come from the CLI flags.
pub fn run_build_info(target_wasm: &[u8], bi_overrides: BuildInfo, workdir: &Path) -> Result<BuildInfo> {
    let mut bi = bi_overrides;
    let built = run_build(&bi, workdir)?;
    let result = compare(target_wasm, &built);
    bi.result = Some(result);
    Ok(bi)
}

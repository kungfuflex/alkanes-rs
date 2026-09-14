//! CLI surface for the BuildInfo workbench. Sub-steps are exposed individually so a
//! Claude skill (or any operator) can drive them:
//!   reverse  — target wasm → reversed fixtures (offline)
//!   build    — BuildInfo JSON → wasm (docker sandbox)
//!   verify   — candidate wasm vs target → verdict (offline)
//!   build-info — the full pipeline: reverse + reconstruct + diff → BuildInfo JSON
//!
//! `--at` accepts EITHER an alkane id (`\d+:\d+`, fetched via --jsonrpc-url) OR a local
//! `.wasm` path (fully offline — also the path for not-yet-deployed alkanes).

use super::{compare, formula, parse_at, reverse, reversed_to_buildinfo, run_build, schema::BuildInfo, Target};
use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum BuildInfoCommands {
    /// Reverse the build environment from a target wasm (alkane id or .wasm path).
    Reverse {
        /// Alkane id `block:tx` OR a local `.wasm` file path.
        #[arg(long)]
        at: String,
        /// Write the reversed readout JSON here (else stdout).
        #[arg(long)]
        emit: Option<String>,
    },
    /// Build a wasm from a BuildInfo JSON in a controlled docker sandbox.
    Build {
        /// Path to a BuildInfo JSON.
        build_info: String,
        /// Write the built wasm here (else ./built.wasm).
        #[arg(long)]
        out: Option<String>,
    },
    /// Diff a candidate wasm against the target → verdict.
    Verify {
        /// Alkane id `block:tx` OR target `.wasm` path.
        #[arg(long)]
        at: String,
        /// Candidate wasm to compare.
        candidate: String,
    },
    /// Full pipeline: reverse the target, reconstruct from `<source-dir>`/repo, diff,
    /// and emit the populated BuildInfo JSON.
    BuildInfo {
        /// Local source directory to build from (a git checkout of the alkane's repo).
        source_dir: Option<String>,
        /// Alkane id `block:tx` OR target `.wasm` path.
        #[arg(long)]
        at: String,
        /// Git repo URL (if not building from a local dir).
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        commit: Option<String>,
        /// Cargo package to build (`-p`).
        #[arg(long)]
        package: Option<String>,
        /// Cargo features (comma-separated, e.g. `mainnet`).
        #[arg(long)]
        features: Option<String>,
        /// Write the BuildInfo JSON here (else stdout).
        #[arg(long)]
        emit: Option<String>,
    },
}

/// Load the target wasm from either an on-chain id (via jsonrpc) or a local file.
pub async fn load_target(at: &str, jsonrpc_url: Option<&str>, headers: &[(String, String)]) -> Result<(Vec<u8>, String, Option<String>)> {
    match parse_at(at) {
        Target::LocalFile(p) => {
            let bytes = std::fs::read(&p).with_context(|| format!("read target wasm {}", p.display()))?;
            Ok((bytes, "local-file".into(), None))
        }
        Target::Onchain { id, .. } => {
            let url = jsonrpc_url.ok_or_else(|| anyhow!("--at is an alkane id but no --jsonrpc-url was given (use a .wasm path for offline)"))?;
            let bytes = fetch_onchain_bytecode(url, headers, &id).await?;
            Ok((bytes, "onchain".into(), Some(id)))
        }
    }
}

/// Standalone jsonrpc `metashrew_view getbytecode` — self-contained so `build-info`
/// works without the full provider stack.
pub async fn fetch_onchain_bytecode(jsonrpc_url: &str, headers: &[(String, String)], id: &str) -> Result<Vec<u8>> {
    use alkanes_support::proto::alkanes::{AlkaneId, BytecodeRequest, Uint128};
    use prost::Message;
    let (block, tx) = id.split_once(':').ok_or_else(|| anyhow!("bad alkane id {id}"))?;
    let (block, tx): (u128, u128) = (block.parse()?, tx.parse()?);
    let mut req = BytecodeRequest::default();
    let mut aid = AlkaneId::default();
    let mut b = Uint128::default();
    b.lo = (block & 0xFFFF_FFFF_FFFF_FFFF) as u64;
    b.hi = (block >> 64) as u64;
    let mut t = Uint128::default();
    t.lo = (tx & 0xFFFF_FFFF_FFFF_FFFF) as u64;
    t.hi = (tx >> 64) as u64;
    aid.block = Some(b).into();
    aid.tx = Some(t).into();
    req.id = Some(aid).into();
    let hex_input = format!("0x{}", hex::encode(req.encode_to_vec()));

    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "metashrew_view",
        "params": ["getbytecode", hex_input, "latest"]
    });
    let client = reqwest::Client::new();
    let mut rb = client.post(jsonrpc_url).json(&body);
    for (k, v) in headers {
        rb = rb.header(k.as_str(), v.as_str());
    }
    let resp: serde_json::Value = rb.send().await.context("jsonrpc request")?.json().await.context("jsonrpc json")?;
    let hexstr = resp
        .get("result")
        .and_then(|r| r.as_str())
        .ok_or_else(|| anyhow!("no bytecode result (alkane not deployed?): {resp}"))?;
    let hexstr = hexstr.trim_start_matches("0x");
    hex::decode(hexstr).context("decode bytecode hex")
}

/// Entry point called from `alkanes-cli`. `jsonrpc_url`/`headers` come from global args.
pub async fn handle(cmd: BuildInfoCommands, jsonrpc_url: Option<String>, headers: Vec<(String, String)>, network: Option<String>) -> Result<()> {
    match cmd {
        BuildInfoCommands::Reverse { at, emit } => {
            let (wasm, src, _id) = load_target(&at, jsonrpc_url.as_deref(), &headers).await?;
            let rev = reverse(&wasm);
            let json = serde_json::to_string_pretty(&rev)?;
            emit_or_print(&json, emit.as_deref())?;
            eprintln!("[reverse] {src}: {} bytes, rustc={:?}, clang={:?} {:?}, home={:?}, host_triple={:?}, links_c={}",
                wasm.len(), rev.rustc, rev.clang_vendor, rev.clang_version, rev.home, rev.host_triple, rev.links_c);
        }
        BuildInfoCommands::Build { build_info, out } => {
            let bi: BuildInfo = serde_json::from_str(&std::fs::read_to_string(&build_info)?)?;
            let workdir = std::env::temp_dir().join(format!("alkbuild-{}", std::process::id()));
            let wasm = run_build(&bi, &workdir)?;
            let out = out.unwrap_or_else(|| "built.wasm".into());
            std::fs::write(&out, &wasm)?;
            println!("built {} ({} bytes, sha256={})", out, wasm.len(), super::sha256_hex(&wasm));
        }
        BuildInfoCommands::Verify { at, candidate } => {
            let (target, _src, _id) = load_target(&at, jsonrpc_url.as_deref(), &headers).await?;
            let cand = std::fs::read(&candidate)?;
            let result = compare(&target, &cand);
            println!("{}", serde_json::to_string_pretty(&result)?);
            eprintln!("[verify] verdict={} byte_exact={} normalized={:.3}%", result.verdict, result.byte_exact, result.normalized_match_pct);
        }
        BuildInfoCommands::BuildInfo { source_dir, at, repo, commit, package, features, emit } => {
            let (wasm, src, id) = load_target(&at, jsonrpc_url.as_deref(), &headers).await?;
            let rev = reverse(&wasm);
            let mut bi = reversed_to_buildinfo(&rev, &src, id, network);
            // enrich from the CLI-provided source hints
            if let Some(r) = repo { bi.source.repo = Some(r); }
            if let Some(c) = commit { bi.source.commit = Some(c); }
            if let Some(p) = package { bi.source.package = Some(p); }
            if let Some(f) = features { bi.source.features = f.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(); }
            if let Some(dir) = &source_dir {
                enrich_from_source_dir(&mut bi, dir)?;
            }
            // attempt the build+diff when we have enough to build
            let workdir = std::env::temp_dir().join(format!("alkbuildinfo-{}", std::process::id()));
            match run_build(&bi, &workdir) {
                Ok(built) => {
                    bi.result = Some(compare(&wasm, &built));
                }
                Err(e) => bi.notes.push(format!("build not run: {e}")),
            }
            let _ = formula::generate_build_script(&bi); // ensure it renders
            let json = serde_json::to_string_pretty(&bi)?;
            emit_or_print(&json, emit.as_deref())?;
        }
    }
    Ok(())
}

/// Fold source-repo facts (committed Cargo.lock ⇒ registry mode, build.rs per-network
/// features, the package name) into the BuildInfo.
fn enrich_from_source_dir(bi: &mut BuildInfo, dir: &str) -> Result<()> {
    let dir = PathBuf::from(dir);
    let lock = dir.join("Cargo.lock");
    if lock.exists() {
        let bytes = std::fs::read(&lock)?;
        use base64::Engine;
        bi.cargo_lock_b64 = Some(base64::engine::general_purpose::STANDARD.encode(&bytes));
        bi.registry.mode = "committed-lock".into();
        bi.notes.push("source commits Cargo.lock → committed-lock registry mode (no time-machine)".into());
        bi.resolved_deps = parse_lock(&String::from_utf8_lossy(&bytes));
    } else {
        bi.notes.push("no committed Cargo.lock in source → time-machine registry mode; set registry.archive_commit to the build date".into());
    }
    Ok(())
}

fn parse_lock(lock: &str) -> Vec<super::schema::ResolvedDep> {
    let mut out = Vec::new();
    for blk in lock.split("[[package]]").skip(1) {
        let field = |k: &str| {
            blk.lines()
                .find_map(|l| l.trim().strip_prefix(&format!("{k} = \"")).map(|v| v.trim_end_matches('"').to_string()))
        };
        if let Some(name) = field("name") {
            out.push(super::schema::ResolvedDep {
                name,
                version: field("version").unwrap_or_default(),
                source: field("source"),
                checksum: field("checksum"),
            });
        }
    }
    out
}

fn emit_or_print(json: &str, emit: Option<&str>) -> Result<()> {
    match emit {
        Some(p) => {
            std::fs::write(p, json)?;
            eprintln!("wrote {p}");
        }
        None => println!("{json}"),
    }
    Ok(())
}

//! Shared, transport-free types for the reproducible-build `upload` path.
//!
//! These structs mirror the explorer `/api/v1/<key>/{attest,verify}` request
//! contract and know how to derive themselves from a [`BuildInfo`]. They carry
//! NO HTTP dependency — the actual POST (via the vendored tlsfetch h2 client)
//! lives in `alkanes-cli-sys` under a native feature gate, so `alkanes-web-sys`
//! (which also depends on `alkanes-cli-common`) never pulls the client stack.

use super::schema::BuildInfo;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Body for `POST /api/v1/<key>/attest` — record a human/CLI verdict for an
/// alkane. `block`/`tx` are STRINGS (the explorer ingest rejects integers).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AttestRequest {
    pub block: String,
    pub tx: String,
    pub wasm_sha256: String,
    pub repo_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alkanes_rev: Option<String>,
    pub verdict: String,
    pub match_pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The FULL BuildInfo, forward-compatible: the explorer ingest is being
    /// extended to persist it. Sent now (ignored until that lands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<Value>,
}

/// Body for `POST /api/v1/<key>/verify` — hand the verifier the full fixture set
/// so it can rebuild + diff in-sandbox.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerifyRequest {
    /// `block:tx`.
    pub alkane: String,
    pub repo_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alkanes_rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clang_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_dir: Option<String>,
    /// crates.io-index-archive commit frozen to the build date (time-machine mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freeze_commit: Option<String>,
    /// The alkanes-rs (or template) git dep URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_url: Option<String>,
    /// `bare` | `rev` — how the git-dep source-id must be spelled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_source_form: Option<String>,
    /// Extra build-env exports, NEWLINE-separated `KEY=VALUE` lines. The verifier's
    /// `VerifyRequest.build_env` is `Option<String>` (it is handed to the builder as
    /// one `BUILD_ENV` env var), so a JSON array here is rejected with a 422.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_env: Option<String>,
    /// Normalize a `https:/` (single-slash) typo in the repo's Cargo.toml. The
    /// verifier's field is `Option<bool>`; a string here is rejected with a 422.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typo_fix: Option<bool>,
    /// The full BuildInfo, forward-compatible (see `AttestRequest::manifest`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<Value>,
}

/// Split `block:tx` → (`"block"`, `"tx"`) strings; defaults `"0"`/`"0"`.
fn split_id(id: Option<&str>) -> (String, String) {
    id.and_then(|s| s.split_once(':'))
        .map(|(b, t)| (b.to_string(), t.to_string()))
        .unwrap_or_else(|| ("0".to_string(), "0".to_string()))
}

/// Best-effort alkanes-rs revision: an explicit git-dep on alkanes, else the
/// build's own commit when the repo IS alkanes-rs.
fn alkanes_rev(bi: &BuildInfo) -> Option<String> {
    if let Some(gd) = bi
        .git_deps
        .iter()
        .find(|g| g.url.contains("alkanes-rs") || g.url.contains("alkanes-runtime") || g.url.contains("/alkanes"))
    {
        return Some(gd.rev.clone());
    }
    if bi.source.repo.as_deref().map(|r| r.contains("alkanes-rs")).unwrap_or(false) {
        return bi.source.commit.clone();
    }
    None
}

fn joined_note(bi: &BuildInfo) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(r) = &bi.result {
        if let Some(n) = &r.note {
            parts.push(n.clone());
        }
    }
    parts.extend(bi.notes.iter().cloned());
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

impl AttestRequest {
    /// Derive an attest body from a `BuildInfo`. `verdict`/`match_pct` come from
    /// `result` when present; callers may override afterwards.
    pub fn from_buildinfo(bi: &BuildInfo) -> Self {
        let (block, tx) = split_id(bi.identity.alkane_id.as_deref());
        let (verdict, match_pct) = bi
            .result
            .as_ref()
            .map(|r| (r.verdict.clone(), r.normalized_match_pct))
            .unwrap_or_else(|| ("verified".to_string(), 100.0));
        AttestRequest {
            block,
            tx,
            wasm_sha256: bi.identity.target_sha256.clone(),
            repo_url: bi.source.repo.clone().unwrap_or_default(),
            commit: bi.source.commit.clone(),
            subdir: bi.source.subdir.clone(),
            rustc: Some(bi.toolchain.rustc_version.clone()).filter(|s| !s.is_empty()),
            alkanes_rev: alkanes_rev(bi),
            verdict,
            match_pct,
            note: joined_note(bi),
            manifest: serde_json::to_value(bi).ok(),
        }
    }
}

impl VerifyRequest {
    /// Derive a verify body (full fixture set) from a `BuildInfo`.
    pub fn from_buildinfo(bi: &BuildInfo) -> Self {
        let git = bi
            .git_deps
            .iter()
            .find(|g| g.url.contains("alkanes"))
            .or_else(|| bi.git_deps.first());
        VerifyRequest {
            alkane: bi.identity.alkane_id.clone().unwrap_or_default(),
            repo_url: bi.source.repo.clone().unwrap_or_default(),
            commit: bi.source.commit.clone(),
            package: bi.source.package.clone(),
            subdir: bi.source.subdir.clone(),
            rustc: Some(bi.toolchain.rustc_version.clone()).filter(|s| !s.is_empty()),
            alkanes_rev: alkanes_rev(bi),
            clang_version: bi.c_toolchain.as_ref().map(|c| c.version.clone()).filter(|s| !s.is_empty()),
            home_dir: Some(bi.environment.home.clone()).filter(|s| !s.is_empty()),
            freeze_commit: bi.registry.archive_commit.clone(),
            git_url: git.map(|g| g.url.clone()),
            git_source_form: git.map(|g| g.source_form.clone()),
            build_env: Some(bi.environment.build_env.join("\n")).filter(|s| !s.is_empty()),
            typo_fix: bi
                .notes
                .iter()
                .any(|n| n.to_lowercase().contains("typo"))
                .then_some(true),
            manifest: serde_json::to_value(bi).ok(),
        }
    }
}

#[cfg(test)]
mod verify_request_wire_tests {
    use super::*;

    /// The wire shape must match the alkane-verifier's `VerifyRequest`
    /// (`build_env: Option<String>`, `typo_fix: Option<bool>`).
    #[test]
    fn build_env_and_typo_fix_serialize_as_verifier_expects() {
        let req = VerifyRequest {
            alkane: "4:1".into(),
            repo_url: "inline://x".into(),
            commit: None, package: None, subdir: None, rustc: None, alkanes_rev: None,
            clang_version: None, home_dir: None, freeze_commit: None, git_url: None,
            git_source_form: None,
            build_env: Some(["A=1", "B=2"].join("\n")),
            typo_fix: Some(true),
            manifest: None,
        };
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["build_env"], serde_json::json!("A=1\nB=2"));
        assert_eq!(v["typo_fix"], serde_json::json!(true));
    }
}

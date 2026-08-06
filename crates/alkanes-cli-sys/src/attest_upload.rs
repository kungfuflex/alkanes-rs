//! The durable, no-curl `upload` path for the reproducible-build workbench.
//!
//! POSTs a [`BuildInfo`] JSON to the explorer as an attestation (default) or a
//! full rebuild-verify request, over the vendored `tlsfetch-h2-client` (native
//! HTTP/2 on tokio-rustls). NO curl, NO reqwest. Native-only — gated behind the
//! `attest-upload` feature so `alkanes-web-sys` (which shares `alkanes-cli-common`
//! but not this crate) never pulls the client stack.
//!
//! The shared request/response types live in `alkanes-cli-common` (transport-free);
//! this module only adds the HTTP.

use alkanes_cli_common::buildinfo::schema::BuildInfo;
use alkanes_cli_common::buildinfo::upload::{AttestRequest, VerifyRequest};
use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use std::time::Duration;
use tlsfetch_h2_client::H2Client;

/// Explorer basepath used when `--explorer-url` is omitted.
pub const DEFAULT_EXPLORER_URL: &str = "https://explorer.subfrost.io";

/// Load `build_info_path`, derive the attest (default) or verify body, and POST
/// it to `<explorer_url>/api/v1/<api_key>/{attest,verify}`.
pub async fn run_upload(
    build_info_path: &str,
    api_key: &str,
    explorer_url: Option<&str>,
    verify: bool,
) -> Result<()> {
    let raw = std::fs::read_to_string(build_info_path)
        .with_context(|| format!("read BuildInfo JSON {build_info_path}"))?;
    let bi: BuildInfo =
        serde_json::from_str(&raw).with_context(|| "parse BuildInfo JSON".to_string())?;

    let base = explorer_url.unwrap_or(DEFAULT_EXPLORER_URL).trim_end_matches('/');
    let parsed = url::Url::parse(base).with_context(|| format!("parse explorer-url {base}"))?;
    if parsed.scheme() != "https" {
        return Err(anyhow!(
            "explorer-url must be https (the h2 client is TLS-only): {base}"
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("explorer-url has no host: {base}"))?
        .to_string();
    let port = parsed.port().unwrap_or(443);
    let base_path = parsed.path().trim_end_matches('/'); // "" or "/subpath"
    let action = if verify { "verify" } else { "attest" };
    let path = format!("{base_path}/api/v1/{api_key}/{action}");

    let (body_json, summary) = if verify {
        let req = VerifyRequest::from_buildinfo(&bi);
        let summary = format!("verify alkane={}", req.alkane);
        (serde_json::to_vec(&req)?, summary)
    } else {
        let req = AttestRequest::from_buildinfo(&bi);
        let summary = format!(
            "attest {}:{} verdict={} match={:.1}%",
            req.block, req.tx, req.verdict, req.match_pct
        );
        if req.repo_url.is_empty() {
            return Err(anyhow!(
                "BuildInfo has no source.repo — the explorer requires repo_url"
            ));
        }
        (serde_json::to_vec(&req)?, summary)
    };

    let config =
        tlsfetch_h2_client::server_auth_config().map_err(|e| anyhow!("build tls config: {e}"))?;
    let client = H2Client::new(config, Duration::from_secs(45));
    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("accept".to_string(), "application/json".to_string()),
        (
            "user-agent".to_string(),
            format!("alkanes-cli/{}-buildinfo-upload", env!("CARGO_PKG_VERSION")),
        ),
    ];

    // Redact the key in the logged URL.
    eprintln!(
        "[upload] POST https://{host}:{port}{base_path}/api/v1/<key>/{action}  ({summary}, {} bytes)",
        body_json.len()
    );

    let resp = client
        .post(&host, port, &path, &headers, Bytes::from(body_json))
        .await
        .map_err(|e| anyhow!("h2 POST to {host} failed: {e}"))?;

    let body_str = String::from_utf8_lossy(&resp.body);
    println!("{}", body_str.trim());

    match resp.status {
        200 | 201 => {
            // For verify, surface a run_id if the server returned one.
            if verify {
                if let Some(run_id) = serde_json::from_str::<serde_json::Value>(&body_str)
                    .ok()
                    .and_then(|v| v.get("run_id").and_then(|r| r.as_str()).map(str::to_string))
                {
                    println!("verify run_id={run_id} (poll the explorer for the rebuild+diff result)");
                }
            }
            println!("OK: upload accepted (HTTP {}) — {}", resp.status, summary);
            Ok(())
        }
        401 | 403 => Err(anyhow!(
            "FAIL: HTTP {} — the API key was rejected. Attest/verify is admin-gated: \
             check the key is a valid `sfadm_…` admin key with attest rights.\nserver: {}",
            resp.status,
            body_str.trim()
        )),
        s => Err(anyhow!(
            "FAIL: HTTP {} — upload rejected by the explorer.\nserver: {}",
            s,
            body_str.trim()
        )),
    }
}

//! `tlsfetch-h2-client` — minimal native HTTP/2 client over tokio-rustls.
//!
//! Vendored from `kungfuflex/tlsfetch` (`crates/tlsfetch-h2-client`) for the
//! alkanes-cli reproducible-build `upload` path. Purpose-built for a one-shot
//! POST → response round-trip against a public-internet, h2-speaking endpoint
//! (e.g. `explorer.subfrost.io/api/v1/<key>/attest`). NO curl, NO reqwest.
//!
//! Differences from the upstream crate:
//!   * The PKCS#12 / mTLS `config_from_pkcs12` helper is dropped — the attest
//!     endpoint is bearer-key-gated, not client-cert-gated.
//!   * A [`server_auth_config`] convenience is added: it installs the pure-Rust
//!     `rustls-rustcrypto` CryptoProvider explicitly (via `builder_with_provider`)
//!     so the client never relies on a process-global rustls default provider.
//!
//! This client does plain server-auth TLS (webpki roots) — it does NOT emit a
//! persona/JA3 ClientHello. For the attest/verify endpoints that is sufficient
//! (they gate on the admin bearer key, not on TLS fingerprint). If a future
//! endpoint fingerprint-gates the POST, switch to `tlsfetch-common`'s persona
//! `HttpClient` (which requires the `rustls-tlsfetch` fork + a workspace
//! `[patch.crates-io] rustls = ...`).

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use h2::client::SendRequest;
use http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Uri};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

// ----- public types ---------------------------------------------------------

/// Errors raised by the client. String-payload variants wrap
/// underlying-crate errors without committing to their types at the
/// public surface.
#[derive(Debug, Error)]
pub enum H2Error {
    #[error("connect: {0}")]
    Connect(String),
    #[error("tls: {0}")]
    Tls(String),
    #[error("invalid DNS name: {0}")]
    InvalidDnsName(String),
    #[error("alpn mismatch (peer chose {got:?}, wanted h2)")]
    AlpnMismatch { got: Option<Vec<u8>> },
    #[error("h2 handshake: {0}")]
    Handshake(String),
    #[error("send: {0}")]
    Send(String),
    #[error("recv: {0}")]
    Recv(String),
    #[error("build request: {0}")]
    BuildRequest(String),
    #[error("timeout")]
    Timeout,
    #[error("rustls config: {0}")]
    Config(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// A response from [`H2Client::post`]. Headers are flattened to a
/// `(String, String)` vec so callers can scan for keys without
/// depending on `http::HeaderMap`.
#[derive(Debug, Clone)]
pub struct H2Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
}

impl H2Response {
    /// Look up a header by (case-insensitive) name. Returns the first match.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// The client. Cheap to clone (everything is `Arc`).
#[derive(Clone)]
pub struct H2Client {
    config: Arc<ClientConfig>,
    timeout: Duration,
}

impl H2Client {
    /// Wrap a rustls `ClientConfig` so it can drive POSTs. ALPN `h2`
    /// is forced on if the caller left `alpn_protocols` empty.
    ///
    /// `timeout` is the per-call deadline covering the whole
    /// connect → handshake → send → drain pipeline.
    pub fn new(mut config: ClientConfig, timeout: Duration) -> Self {
        if config.alpn_protocols.is_empty() {
            config.alpn_protocols = vec![b"h2".to_vec()];
        }
        Self {
            config: Arc::new(config),
            timeout,
        }
    }

    /// POST `body` to `https://{host}:{port}{path}` and drain the response.
    /// Headers are inserted as-is. The whole call is bounded by `timeout`.
    pub async fn post(
        &self,
        host: &str,
        port: u16,
        path: &str,
        headers: &[(String, String)],
        body: Bytes,
    ) -> Result<H2Response, H2Error> {
        let fut = self.do_post(host, port, path, headers, body);
        match tokio::time::timeout(self.timeout, fut).await {
            Ok(res) => res,
            Err(_) => Err(H2Error::Timeout),
        }
    }

    async fn do_post(
        &self,
        host: &str,
        port: u16,
        path: &str,
        headers: &[(String, String)],
        body: Bytes,
    ) -> Result<H2Response, H2Error> {
        // 1. TCP connect.
        let tcp = TcpStream::connect((host, port))
            .await
            .map_err(|e| H2Error::Connect(e.to_string()))?;
        tcp.set_nodelay(true).map_err(H2Error::Io)?;

        // 2. TLS handshake with ALPN h2.
        let connector = TlsConnector::from(self.config.clone());
        let server_name: ServerName<'static> = ServerName::try_from(host.to_string())
            .map_err(|e| H2Error::InvalidDnsName(e.to_string()))?;
        let tls = connector
            .connect(server_name, tcp)
            .await
            .map_err(|e| H2Error::Tls(e.to_string()))?;

        // Confirm ALPN landed on h2 (else the h2 codec would speak
        // gibberish at an h1 peer).
        let alpn = tls.get_ref().1.alpn_protocol().map(|s| s.to_vec());
        if alpn.as_deref() != Some(b"h2") {
            return Err(H2Error::AlpnMismatch { got: alpn });
        }

        // 3. h2 client handshake.
        let (h2, h2_conn) = h2::client::handshake(tls)
            .await
            .map_err(|e| H2Error::Handshake(e.to_string()))?;

        let conn_task = tokio::spawn(async move {
            let _ = h2_conn.await;
        });

        // 4. Build + send the request, drain the response.
        let resp = self.send_and_drain(h2, host, path, headers, body).await;

        // 5. Let the connection driver exit cleanly (bounded).
        let _ = tokio::time::timeout(Duration::from_secs(1), conn_task).await;

        resp
    }

    async fn send_and_drain(
        &self,
        h2: SendRequest<Bytes>,
        host: &str,
        path: &str,
        headers: &[(String, String)],
        body: Bytes,
    ) -> Result<H2Response, H2Error> {
        let uri: Uri = format!("https://{}{}", host, path)
            .parse()
            .map_err(|e: http::uri::InvalidUri| H2Error::BuildRequest(e.to_string()))?;
        let mut builder = Request::builder().method(Method::POST).uri(uri);
        for (k, v) in headers {
            let name = HeaderName::try_from(k.as_str())
                .map_err(|e| H2Error::BuildRequest(format!("header name {k}: {e}")))?;
            let val = HeaderValue::try_from(v.as_str())
                .map_err(|e| H2Error::BuildRequest(format!("header value for {k}: {e}")))?;
            builder = builder.header(name, val);
        }
        let req = builder
            .body(())
            .map_err(|e| H2Error::BuildRequest(e.to_string()))?;

        let mut h2 = h2
            .ready()
            .await
            .map_err(|e| H2Error::Send(format!("ready: {e}")))?;
        let body_empty = body.is_empty();
        let (response_fut, mut send_stream) = h2
            .send_request(req, body_empty)
            .map_err(|e| H2Error::Send(format!("send_request: {e}")))?;

        if !body_empty {
            send_stream
                .send_data(body, true)
                .map_err(|e| H2Error::Send(format!("send_data: {e}")))?;
        }

        let resp = response_fut
            .await
            .map_err(|e| H2Error::Recv(format!("response: {e}")))?;

        let status = resp.status().as_u16();
        let headers_out = flatten_headers(resp.headers());

        let mut body_stream = resp.into_body();
        let mut buf: Vec<u8> = Vec::with_capacity(1024);
        while let Some(chunk) = body_stream.data().await {
            let chunk = chunk.map_err(|e| H2Error::Recv(format!("body_chunk: {e}")))?;
            let _ = body_stream.flow_control().release_capacity(chunk.len());
            buf.extend_from_slice(&chunk);
        }
        let _trailers = body_stream.trailers().await.ok();

        drop(body_stream);
        drop(send_stream);
        drop(h2);

        Ok(H2Response {
            status,
            headers: headers_out,
            body: Bytes::from(buf),
        })
    }
}

fn flatten_headers(map: &HeaderMap) -> Vec<(String, String)> {
    let mut out = Vec::with_capacity(map.len());
    for (k, v) in map.iter() {
        if let Ok(s) = v.to_str() {
            out.push((k.as_str().to_string(), s.to_string()));
        }
    }
    out
}

/// A `RootCertStore` populated with the webpki-roots trust anchors —
/// the standard public-internet trust set.
pub fn webpki_root_store() -> RootCertStore {
    let mut store = RootCertStore::empty();
    store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    store
}

/// Build a server-auth-only `ClientConfig` (no client cert) that trusts the
/// webpki public roots, installing the pure-Rust `rustls-rustcrypto`
/// CryptoProvider explicitly so no process-global rustls default is required.
pub fn server_auth_config() -> Result<ClientConfig, H2Error> {
    let provider = Arc::new(rustls_rustcrypto::provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| H2Error::Config(format!("protocol versions: {e}")))?
        .with_root_certificates(webpki_root_store())
        .with_no_client_auth();
    Ok(config)
}

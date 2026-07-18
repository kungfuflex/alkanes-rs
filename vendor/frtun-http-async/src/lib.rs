//! Minimal async HTTP/1.1 client over tokio + tokio-rustls.
//!
//! Engine-agnostic: uses only the public rustls 0.23 API so it co-exists
//! with any other rustls consumer in the workspace. Specifically built
//! to keep the frtun ecosystem (push-proxy FCM, pair-bridge push-wake,
//! WC signer's `/v1/pair-wake` POST) off reqwest AND off the
//! `tlsfetch_common::HttpClient` path (which transitively pulls in
//! tlsfetch's `[patch.crates-io] rustls = …` fork to a rustls-tlsfetch
//! crate with an additive `ClientHelloMutator` symbol the unpatched
//! rustls doesn't expose). The async H1 client here needs only the
//! public rustls 0.23 API, so it's safe to consume from any workspace
//! that doesn't mirror the patch.
//!
//! Wire shape: plain HTTP/1.1, `Connection: close`, body framed by
//! either `Content-Length` or chunked transfer-encoding. Only the
//! identity / close framing is exercised by FCM and `/v1/pair-wake` but
//! chunked decode is included for robustness against intermediaries.
//!
//! This crate is the consolidated home of what used to be three
//! near-byte-for-byte inline copies (`frtun-push-proxy::http_async`,
//! `frtun-pair-bridge::http_async`, `alkanes-cli-common::wc_signer::http_async`).
//! Once tlsfetch drops its `[patch.crates-io] rustls = …` fork and
//! `tlsfetch_common::HttpClient::send_async` becomes directly consumable,
//! this crate can be retired in favor of that canonical.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

#[derive(Debug, thiserror::Error)]
pub enum HttpAsyncError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("io: {0}")]
    Io(String),
    #[error("tls: {0}")]
    Tls(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug)]
pub struct HttpAsyncResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

struct ParsedUrl {
    scheme: String,
    host: String,
    port: u16,
    path: String,
}

fn parse_url(url: &str) -> Result<ParsedUrl, HttpAsyncError> {
    let p = url::Url::parse(url).map_err(|e| HttpAsyncError::InvalidUrl(e.to_string()))?;
    let scheme = p.scheme().to_string();
    if scheme != "http" && scheme != "https" {
        return Err(HttpAsyncError::InvalidUrl(format!(
            "unsupported scheme: {scheme}"
        )));
    }
    let host = p
        .host_str()
        .ok_or_else(|| HttpAsyncError::InvalidUrl("missing host".into()))?
        .to_string();
    let port = p
        .port_or_known_default()
        .ok_or_else(|| HttpAsyncError::InvalidUrl("missing port".into()))?;
    let path = if let Some(q) = p.query() {
        format!("{}?{}", p.path(), q)
    } else {
        p.path().to_string()
    };
    Ok(ParsedUrl { scheme, host, port, path })
}

fn encode_request(
    method: &str,
    host: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(256 + body.len());
    out.extend_from_slice(method.as_bytes());
    out.push(b' ');
    out.extend_from_slice(path.as_bytes());
    out.extend_from_slice(b" HTTP/1.1\r\n");
    let has_host = headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host"));
    if !has_host {
        out.extend_from_slice(b"Host: ");
        out.extend_from_slice(host.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    let has_conn = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("connection"));
    if !has_conn {
        out.extend_from_slice(b"Connection: close\r\n");
    }
    let has_cl = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-length"));
    for (k, v) in headers {
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(v.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    if !body.is_empty() && !has_cl {
        out.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

fn build_tls_config() -> ClientConfig {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

async fn read_to_eof<R: tokio::io::AsyncRead + Unpin>(
    r: &mut R,
) -> Result<Vec<u8>, HttpAsyncError> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 8 * 1024];
    loop {
        let n = r
            .read(&mut tmp)
            .await
            .map_err(|e| HttpAsyncError::Io(format!("read: {e}")))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > 16 * 1024 * 1024 {
            return Err(HttpAsyncError::Io("response > 16 MiB".into()));
        }
    }
    Ok(buf)
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn decode_chunked(mut buf: &[u8]) -> Result<Vec<u8>, HttpAsyncError> {
    let mut out = Vec::with_capacity(buf.len());
    loop {
        let nl = buf
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| HttpAsyncError::InvalidResponse("chunked: missing size CRLF".into()))?;
        let size_str = std::str::from_utf8(&buf[..nl])
            .map_err(|_| HttpAsyncError::InvalidResponse("chunked: non-utf8 size".into()))?
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        let size = usize::from_str_radix(size_str, 16).map_err(|_| {
            HttpAsyncError::InvalidResponse(format!("chunked: bad size {size_str:?}"))
        })?;
        buf = &buf[nl + 2..];
        if size == 0 {
            return Ok(out);
        }
        if buf.len() < size + 2 {
            return Err(HttpAsyncError::InvalidResponse(
                "chunked: short payload".into(),
            ));
        }
        out.extend_from_slice(&buf[..size]);
        buf = &buf[size + 2..];
    }
}

fn parse_response(raw: &[u8]) -> Result<HttpAsyncResponse, HttpAsyncError> {
    let header_end = find_header_end(raw)
        .ok_or_else(|| HttpAsyncError::InvalidResponse("no header terminator".into()))?;
    let mut headers_buf = [httparse::EMPTY_HEADER; 64];
    let mut resp = httparse::Response::new(&mut headers_buf);
    let parsed = resp
        .parse(&raw[..header_end])
        .map_err(|e| HttpAsyncError::InvalidResponse(e.to_string()))?;
    if !parsed.is_complete() {
        return Err(HttpAsyncError::InvalidResponse(
            "incomplete header parse".into(),
        ));
    }
    let status = resp.code.unwrap_or(0);
    let mut headers = HashMap::new();
    let mut chunked = false;
    let mut content_length: Option<usize> = None;
    for h in resp.headers.iter() {
        let name = h.name.to_string();
        let value = String::from_utf8_lossy(h.value).to_string();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().ok();
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            && value.to_ascii_lowercase().contains("chunked")
        {
            chunked = true;
        }
        let lc = name.to_ascii_lowercase();
        headers
            .entry(lc)
            .and_modify(|existing: &mut String| {
                existing.push('\n');
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    let body_bytes = &raw[header_end..];
    let body = if chunked {
        decode_chunked(body_bytes)?
    } else if let Some(want) = content_length {
        let take = want.min(body_bytes.len());
        body_bytes[..take].to_vec()
    } else {
        body_bytes.to_vec()
    };
    Ok(HttpAsyncResponse { status, headers, body })
}

/// Send a single HTTP/1.1 request asynchronously. Opens a fresh
/// connection, sends, reads to EOF (Connection: close), closes.
///
/// Honors a 30s connect timeout. Both `http://` and `https://` are
/// supported; the latter does a tokio-rustls TLS handshake against
/// webpki-roots (no client cert).
pub async fn send_async(
    url: &str,
    method: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
) -> Result<HttpAsyncResponse, HttpAsyncError> {
    let u = parse_url(url)?;
    let wire = encode_request(method, &u.host, &u.path, headers, body.unwrap_or(&[]));

    let connect_timeout = Duration::from_secs(30);
    let tcp = tokio::time::timeout(
        connect_timeout,
        TcpStream::connect((u.host.as_str(), u.port)),
    )
    .await
    .map_err(|_| HttpAsyncError::Io(format!("connect {}:{} timed out", u.host, u.port)))?
    .map_err(|e| HttpAsyncError::Io(format!("connect {}:{}: {e}", u.host, u.port)))?;
    tcp.set_nodelay(true)
        .map_err(|e| HttpAsyncError::Io(e.to_string()))?;

    match u.scheme.as_str() {
        "http" => {
            let mut stream = tcp;
            stream
                .write_all(&wire)
                .await
                .map_err(|e| HttpAsyncError::Io(format!("write: {e}")))?;
            stream
                .flush()
                .await
                .map_err(|e| HttpAsyncError::Io(format!("flush: {e}")))?;
            let raw = read_to_eof(&mut stream).await?;
            parse_response(&raw)
        }
        "https" => {
            let config = build_tls_config();
            let connector = TlsConnector::from(Arc::new(config));
            let server_name: ServerName<'static> = ServerName::try_from(u.host.clone())
                .map_err(|e| HttpAsyncError::Tls(format!("invalid dns name: {e}")))?;
            let mut tls = connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| HttpAsyncError::Tls(format!("handshake: {e}")))?;
            tls.write_all(&wire)
                .await
                .map_err(|e| HttpAsyncError::Io(format!("write: {e}")))?;
            tls.flush()
                .await
                .map_err(|e| HttpAsyncError::Io(format!("flush: {e}")))?;
            let raw = read_to_eof(&mut tls).await?;
            parse_response(&raw)
        }
        other => Err(HttpAsyncError::InvalidUrl(format!(
            "unsupported scheme: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn send_async_plaintext_round_trip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let mut total = 0;
            loop {
                let n = sock.read(&mut buf[total..]).await.unwrap();
                if n == 0 {
                    break;
                }
                total += n;
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let body = br#"{"delivered":true,"reason":"ok"}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.write_all(body).await.unwrap();
            sock.flush().await.unwrap();
        });

        let url = format!("http://127.0.0.1:{port}/v1/pair-wake");
        let resp = send_async(
            &url,
            "POST",
            &[("Content-Type", "application/json")],
            Some(br#"{"peer":"frtun1abc.peer"}"#),
        )
        .await
        .expect("send_async");
        server.await.unwrap();

        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, br#"{"delivered":true,"reason":"ok"}"#);
    }

    #[test]
    fn decode_chunked_smoke() {
        let raw = b"5\r\nHello\r\n6\r\n World\r\n0\r\n\r\n";
        let out = decode_chunked(raw).unwrap();
        assert_eq!(out, b"Hello World");
    }
}

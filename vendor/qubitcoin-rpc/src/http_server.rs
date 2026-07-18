// Copyright (c) 2024 The Qubitcoin developers
// Distributed under the MIT software license.

//! HTTP server for JSON-RPC.
//!
//! Listens on a configurable address and dispatches incoming HTTP POST
//! requests to the `RpcRegistry` for JSON-RPC processing.  Authentication
//! via HTTP Basic Auth is supported when `rpc_user` / `rpc_password` are set.

use crate::server::{parse_rpc_request, process_request, AuthTier, RpcRegistry, RpcResponse};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// RPC server configuration.
#[derive(Debug, Clone)]
pub struct RpcServerConfig {
    /// Address to bind the HTTP listener to.
    pub bind_addr: SocketAddr,
    /// Optional username for HTTP Basic Auth.
    pub rpc_user: Option<String>,
    /// Optional password for HTTP Basic Auth.
    pub rpc_password: Option<String>,
}

impl Default for RpcServerConfig {
    fn default() -> Self {
        RpcServerConfig {
            bind_addr: "127.0.0.1:8332".parse().unwrap(),
            rpc_user: None,
            rpc_password: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// The RPC HTTP server.
pub struct RpcServer {
    config: RpcServerConfig,
    registry: Arc<RpcRegistry>,
}

impl RpcServer {
    /// Create a new server from a configuration and a method registry.
    pub fn new(config: RpcServerConfig, registry: RpcRegistry) -> Self {
        RpcServer {
            config,
            registry: Arc::new(registry),
        }
    }

    /// Start serving requests.  This future runs forever (or until the
    /// underlying listener encounters a fatal I/O error).
    pub async fn serve(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        tracing::info!(bind_addr = %self.config.bind_addr, "RPC server listening");

        loop {
            let (mut stream, _addr) = listener.accept().await?;
            let registry = self.registry.clone();
            let auth_user = self.config.rpc_user.clone();
            let auth_pass = self.config.rpc_password.clone();

            tokio::spawn(async move {
                // Read HTTP request — support large payloads (up to 8 MiB for WASM envelopes).
                let max_size = 8 * 1024 * 1024;
                let mut buf = Vec::with_capacity(64 * 1024);
                let mut tmp = vec![0u8; 64 * 1024];

                // Phase 1: Read headers (until \r\n\r\n).
                loop {
                    let n = match stream.read(&mut tmp).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    buf.extend_from_slice(&tmp[..n]);
                    if buf.len() >= max_size { break; }
                    // Check for end-of-headers
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                }

                // Phase 2: If we have Content-Length, read remaining body bytes.
                let header_str = String::from_utf8_lossy(&buf);
                let content_length: usize = header_str
                    .lines()
                    .find(|l| l.to_lowercase().starts_with("content-length:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);

                if content_length > 0 {
                    // Find where body starts (after \r\n\r\n).
                    let body_start = buf.windows(4)
                        .position(|w| w == b"\r\n\r\n")
                        .map(|p| p + 4)
                        .unwrap_or(buf.len());
                    let body_so_far = buf.len() - body_start;
                    let remaining = content_length.saturating_sub(body_so_far);

                    if remaining > 0 && remaining < max_size {
                        buf.reserve(remaining);
                        let mut left = remaining;
                        while left > 0 {
                            let to_read = left.min(tmp.len());
                            let n = match stream.read(&mut tmp[..to_read]).await {
                                Ok(0) => break,
                                Ok(n) => n,
                                Err(_) => break,
                            };
                            buf.extend_from_slice(&tmp[..n]);
                            left -= n;
                        }
                    }
                }

                let request_str = String::from_utf8_lossy(&buf);

                // Parse HTTP request (minimal HTTP/1.1 parser).
                let (headers, body) = match parse_http_request(&request_str) {
                    Some(result) => result,
                    None => {
                        let response = http_response(400, "Bad Request");
                        let _ = stream.write_all(response.as_bytes()).await;
                        return;
                    }
                };

                // Check method is POST.
                if !headers.starts_with("POST") {
                    let response = http_response(405, "Method Not Allowed");
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                // Per-method auth: parse the JSON-RPC request to get the method
                // name, then check if it requires authentication.
                let auth_configured = auth_user.is_some() && auth_pass.is_some();
                let result = match parse_rpc_request(&body) {
                    Err(err_json) => err_json,
                    Ok(request) => {
                        let tier = registry
                            .auth_tier_for(&request.method)
                            .unwrap_or(AuthTier::Public);

                        // Admin methods require auth.
                        if tier == AuthTier::Admin && auth_configured {
                            if !check_auth(
                                &headers,
                                auth_user.as_deref().unwrap(),
                                auth_pass.as_deref().unwrap(),
                            ) {
                                let response = http_response(401, "Unauthorized");
                                let _ = stream.write_all(response.as_bytes()).await;
                                return;
                            }
                        }

                        // Optional per-user method whitelist.
                        if auth_configured {
                            if let Some(user) = extract_auth_user(&headers) {
                                if !registry.user_allowed(&user, &request.method) {
                                    let resp = RpcResponse::error(
                                        request.id.clone(),
                                        403,
                                        format!("Method not allowed for user: {}", request.method),
                                    );
                                    serde_json::to_string(&resp).unwrap_or_default()
                                } else {
                                    let resp = registry.dispatch(&request);
                                    serde_json::to_string(&resp).unwrap_or_default()
                                }
                            } else {
                                let resp = registry.dispatch(&request);
                                serde_json::to_string(&resp).unwrap_or_default()
                            }
                        } else {
                            let resp = registry.dispatch(&request);
                            serde_json::to_string(&resp).unwrap_or_default()
                        }
                    }
                };

                // Send HTTP response.
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n{}",
                    result.len(),
                    result
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

/// Split a raw HTTP request into its header block and body.
///
/// Returns `None` when the standard `\r\n\r\n` separator is not found.
fn parse_http_request(raw: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = raw.splitn(2, "\r\n\r\n").collect();
    if parts.len() < 2 {
        return None;
    }
    Some((parts[0].to_string(), parts[1].to_string()))
}

/// Validate HTTP Basic Auth credentials against the `Authorization` header.
fn check_auth(headers: &str, user: &str, pass: &str) -> bool {
    for line in headers.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("authorization: basic ") {
            let value = &line[21..]; // "authorization: basic " is 21 chars
            let expected = base64_encode(&format!("{}:{}", user, pass));
            return value.trim() == expected;
        }
    }
    false
}

/// Extract the username from an HTTP Basic Auth header.
///
/// Decodes the Base64 `user:password` value and returns the user portion.
fn extract_auth_user(headers: &str) -> Option<String> {
    for line in headers.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("authorization: basic ") {
            let b64 = line[21..].trim();
            let decoded = base64_decode(b64)?;
            return decoded.split(':').next().map(|s| s.to_string());
        }
    }
    None
}

/// A minimal Base64 decoder.
fn base64_decode(input: &str) -> Option<String> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=')
        .map(|b| {
            CHARS
                .iter()
                .position(|&c| c == b)
                .unwrap_or(0) as u8
        })
        .collect();
    for chunk in bytes.chunks(4) {
        if chunk.len() >= 2 {
            result.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            result.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() >= 4 {
            result.push((chunk[2] << 6) | chunk[3]);
        }
    }
    String::from_utf8(result).ok()
}

/// A minimal Base64 encoder (no external dependency required).
fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3f) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Build a simple HTTP response with a plain-text body.
fn http_response(code: u16, body: &str) -> String {
    let status = match code {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{}",
        code,
        status,
        body.len(),
        body
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{RpcRegistry, RpcRequest, RpcResponse};
    use serde_json::json;

    // -- RpcServerConfig defaults -------------------------------------------

    #[test]
    fn test_rpc_server_config_default() {
        let cfg = RpcServerConfig::default();
        assert_eq!(
            cfg.bind_addr,
            "127.0.0.1:8332".parse::<SocketAddr>().unwrap()
        );
        assert!(cfg.rpc_user.is_none());
        assert!(cfg.rpc_password.is_none());
    }

    #[test]
    fn test_rpc_server_config_custom() {
        let cfg = RpcServerConfig {
            bind_addr: "0.0.0.0:18332".parse().unwrap(),
            rpc_user: Some("alice".into()),
            rpc_password: Some("secret".into()),
        };
        assert_eq!(cfg.bind_addr.port(), 18332);
        assert_eq!(cfg.rpc_user.as_deref(), Some("alice"));
        assert_eq!(cfg.rpc_password.as_deref(), Some("secret"));
    }

    // -- base64 encoding ---------------------------------------------------

    #[test]
    fn test_base64_encode_empty() {
        assert_eq!(base64_encode(""), "");
    }

    #[test]
    fn test_base64_encode_simple() {
        // "user:pass" => "dXNlcjpwYXNz"
        assert_eq!(base64_encode("user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn test_base64_encode_padding_one() {
        // "ab" has length 2 => one padding char
        assert_eq!(base64_encode("ab"), "YWI=");
    }

    #[test]
    fn test_base64_encode_padding_two() {
        // "a" has length 1 => two padding chars
        assert_eq!(base64_encode("a"), "YQ==");
    }

    #[test]
    fn test_base64_encode_no_padding() {
        // "abc" has length 3 => no padding
        assert_eq!(base64_encode("abc"), "YWJj");
    }

    // -- HTTP request parsing -----------------------------------------------

    #[test]
    fn test_parse_http_request_valid() {
        let raw = "POST / HTTP/1.1\r\nHost: localhost\r\n\r\n{\"method\":\"test\"}";
        let (headers, body) = parse_http_request(raw).unwrap();
        assert!(headers.starts_with("POST"));
        assert!(headers.contains("Host: localhost"));
        assert_eq!(body, "{\"method\":\"test\"}");
    }

    #[test]
    fn test_parse_http_request_empty_body() {
        let raw = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let (headers, body) = parse_http_request(raw).unwrap();
        assert!(headers.starts_with("GET"));
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_http_request_no_separator() {
        let raw = "POST / HTTP/1.1\r\nHost: localhost";
        assert!(parse_http_request(raw).is_none());
    }

    #[test]
    fn test_parse_http_request_body_with_newlines() {
        let raw = "POST / HTTP/1.1\r\n\r\n{\"a\":1}\r\n\r\nextra";
        let (headers, body) = parse_http_request(raw).unwrap();
        assert_eq!(headers, "POST / HTTP/1.1");
        // Body includes everything after the first separator
        assert!(body.contains("{\"a\":1}"));
        assert!(body.contains("extra"));
    }

    // -- Auth checking ------------------------------------------------------

    #[test]
    fn test_check_auth_valid() {
        let encoded = base64_encode("user:pass");
        let headers = format!(
            "POST / HTTP/1.1\r\nAuthorization: Basic {}\r\nHost: localhost",
            encoded
        );
        assert!(check_auth(&headers, "user", "pass"));
    }

    #[test]
    fn test_check_auth_invalid_password() {
        let encoded = base64_encode("user:wrong");
        let headers = format!(
            "POST / HTTP/1.1\r\nAuthorization: Basic {}\r\nHost: localhost",
            encoded
        );
        assert!(!check_auth(&headers, "user", "pass"));
    }

    #[test]
    fn test_check_auth_missing_header() {
        let headers = "POST / HTTP/1.1\r\nHost: localhost";
        assert!(!check_auth(headers, "user", "pass"));
    }

    #[test]
    fn test_check_auth_wrong_user() {
        let encoded = base64_encode("other:pass");
        let headers = format!("POST / HTTP/1.1\r\nAuthorization: Basic {}", encoded);
        assert!(!check_auth(&headers, "user", "pass"));
    }

    // -- HTTP response formatting -------------------------------------------

    #[test]
    fn test_http_response_200() {
        let resp = http_response(200, "OK body");
        assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp.contains("Content-Length: 7"));
        assert!(resp.ends_with("OK body"));
    }

    #[test]
    fn test_http_response_400() {
        let resp = http_response(400, "Bad Request");
        assert!(resp.contains("400 Bad Request"));
    }

    #[test]
    fn test_http_response_401() {
        let resp = http_response(401, "Unauthorized");
        assert!(resp.contains("401 Unauthorized"));
    }

    #[test]
    fn test_http_response_405() {
        let resp = http_response(405, "Method Not Allowed");
        assert!(resp.contains("405 Method Not Allowed"));
    }

    #[test]
    fn test_http_response_500() {
        let resp = http_response(500, "Internal Server Error");
        assert!(resp.contains("500 Internal Server Error"));
    }

    #[test]
    fn test_http_response_unknown_code() {
        let resp = http_response(418, "I'm a teapot");
        assert!(resp.contains("418 Unknown"));
    }

    // -- RpcServer construction ---------------------------------------------

    #[test]
    fn test_rpc_server_new() {
        let config = RpcServerConfig::default();
        let registry = RpcRegistry::new();
        let server = RpcServer::new(config, registry);
        assert_eq!(server.config.bind_addr.port(), 8332);
    }

    // -- Integration test: start server, send request, get response ---------

    #[tokio::test]
    async fn test_integration_rpc_server_roundtrip() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        // Build a registry with one method.
        let mut registry = RpcRegistry::new();
        registry.register("ping", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("pong"))
        });

        // Bind to an ephemeral port.
        let config = RpcServerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            rpc_user: None,
            rpc_password: None,
        };

        // We need to get the actual port, so bind manually.
        let listener = TcpListener::bind(&config.bind_addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let registry = Arc::new(registry);

        // Spawn the accept loop.
        let reg = registry.clone();
        let server_handle = tokio::spawn(async move {
            // Accept exactly one connection for the test.
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let n = stream.read(&mut buf).await.unwrap();
            buf.truncate(n);
            let request_str = String::from_utf8_lossy(&buf);
            let (_, body) = parse_http_request(&request_str).unwrap();
            let result = process_request(&reg, &body);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                result.len(),
                result
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        // Send a request.
        let body = r#"{"jsonrpc":"2.0","method":"ping","params":[],"id":1}"#;
        let request = format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response_buf = vec![0u8; 64 * 1024];
        let n = stream.read(&mut response_buf).await.unwrap();
        response_buf.truncate(n);
        let response_str = String::from_utf8_lossy(&response_buf);

        // Verify we got a valid JSON-RPC response.
        let (resp_headers, resp_body) = parse_http_request(&response_str).unwrap();
        assert!(resp_headers.contains("200 OK"));
        let rpc_resp: RpcResponse = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(rpc_resp.result, Some(json!("pong")));
        assert!(rpc_resp.error.is_none());

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_integration_rpc_server_auth_required() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let registry = RpcRegistry::new();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let _registry = Arc::new(registry);

        let auth_user = "testuser".to_string();
        let auth_pass = "testpass".to_string();

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let n = stream.read(&mut buf).await.unwrap();
            buf.truncate(n);
            let request_str = String::from_utf8_lossy(&buf);
            let (headers, _body) = parse_http_request(&request_str).unwrap();

            // Check auth - should fail since we send no auth header
            if !check_auth(&headers, &auth_user, &auth_pass) {
                let response = http_response(401, "Unauthorized");
                stream.write_all(response.as_bytes()).await.unwrap();
                return;
            }
        });

        // Send request without auth.
        let body = r#"{"jsonrpc":"2.0","method":"ping","params":[],"id":1}"#;
        let request = format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response_buf = vec![0u8; 64 * 1024];
        let n = stream.read(&mut response_buf).await.unwrap();
        response_buf.truncate(n);
        let response_str = String::from_utf8_lossy(&response_buf);

        assert!(response_str.contains("401 Unauthorized"));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_integration_rpc_server_auth_success() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let mut registry = RpcRegistry::new();
        registry.register("getblockcount", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!(42))
        });

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let registry = Arc::new(registry);

        let reg = registry.clone();
        let auth_user = "rpcuser".to_string();
        let auth_pass = "rpcpassword".to_string();

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let n = stream.read(&mut buf).await.unwrap();
            buf.truncate(n);
            let request_str = String::from_utf8_lossy(&buf);
            let (headers, body) = parse_http_request(&request_str).unwrap();

            if !check_auth(&headers, &auth_user, &auth_pass) {
                let response = http_response(401, "Unauthorized");
                stream.write_all(response.as_bytes()).await.unwrap();
                return;
            }

            let result = process_request(&reg, &body);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                result.len(),
                result
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        // Send request with correct auth.
        let auth_encoded = base64_encode("rpcuser:rpcpassword");
        let body = r#"{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}"#;
        let request = format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Basic {}\r\nContent-Length: {}\r\n\r\n{}",
            auth_encoded,
            body.len(),
            body
        );

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response_buf = vec![0u8; 64 * 1024];
        let n = stream.read(&mut response_buf).await.unwrap();
        response_buf.truncate(n);
        let response_str = String::from_utf8_lossy(&response_buf);

        let (resp_headers, resp_body) = parse_http_request(&response_str).unwrap();
        assert!(resp_headers.contains("200 OK"));
        let rpc_resp: RpcResponse = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(rpc_resp.result, Some(json!(42)));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_integration_rpc_server_method_not_allowed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let registry = RpcRegistry::new();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let _registry = Arc::new(registry);

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let n = stream.read(&mut buf).await.unwrap();
            buf.truncate(n);
            let request_str = String::from_utf8_lossy(&buf);
            let (headers, _body) = parse_http_request(&request_str).unwrap();

            if !headers.starts_with("POST") {
                let response = http_response(405, "Method Not Allowed");
                stream.write_all(response.as_bytes()).await.unwrap();
                return;
            }
        });

        // Send a GET request.
        let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response_buf = vec![0u8; 64 * 1024];
        let n = stream.read(&mut response_buf).await.unwrap();
        response_buf.truncate(n);
        let response_str = String::from_utf8_lossy(&response_buf);

        assert!(response_str.contains("405 Method Not Allowed"));

        server_handle.await.unwrap();
    }
}

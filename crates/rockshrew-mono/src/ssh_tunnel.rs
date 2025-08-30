use anyhow::{anyhow, Result};
use log::{debug, error, info};
use once_cell::sync::Lazy;
use russh::{client, ChannelMsg};
use russh_keys::{self, ssh_key};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use url::Url as UrlParser;

// Cache for auth cookie content
struct AuthCache {
    content: Mutex<Option<String>>,
    last_updated: Mutex<Option<Instant>>,
    needs_refresh: AtomicBool,
}

impl AuthCache {
    fn new() -> Self {
        Self {
            content: Mutex::new(None),
            last_updated: Mutex::new(None),
            needs_refresh: AtomicBool::new(false),
        }
    }

    async fn get(&self, url: &str) -> Result<String> {
        let mut content = self.content.lock().await;
        let mut last_updated = self.last_updated.lock().await;

        // Check if we need to refresh the cache
        let needs_refresh = self.needs_refresh.load(Ordering::SeqCst)
            || content.is_none()
            || last_updated.map_or(true, |t| t.elapsed() > Duration::from_secs(3600));

        if needs_refresh {
            debug!("Reading auth cookie from {}", url);
            let new_content = read_file_over_ssh(url).await?;
            *content = Some(new_content.clone());
            *last_updated = Some(Instant::now());
            self.needs_refresh.store(false, Ordering::SeqCst);
            Ok(new_content)
        } else {
            debug!("Using cached auth cookie");
            Ok(content.as_ref().unwrap().clone())
        }
    }

    fn mark_for_refresh(&self) {
        self.needs_refresh.store(true, Ordering::SeqCst);
    }
}

// Global auth cache
static AUTH_CACHE: Lazy<AuthCache> = Lazy::new(|| AuthCache::new());

/// Configuration for SSH tunneling
#[derive(Debug, Clone)]
pub struct SshTunnelConfig {
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_password: Option<String>,
    pub target_host: String,
    pub target_port: u16,
    pub local_port: u16,
    pub key_path: Option<PathBuf>,
}

/// Represents an active SSH tunnel
#[derive(Clone)]
pub struct SshTunnel {
    pub local_port: u16,
    #[allow(dead_code)]
    session: Arc<Mutex<client::Handle<Client>>>,
    _listener_task: Arc<()>, // Keep the listener task alive
}

// Manual Debug implementation since client::Handle<Client> doesn't implement Debug
impl std::fmt::Debug for SshTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshTunnel")
            .field("local_port", &self.local_port)
            .field("session", &"<client::Handle>")
            .field("_listener_task", &"<Arc<()>>")
            .finish()
    }
}

impl SshTunnel {
    /// Creates an SSH tunnel and returns the local port
    pub async fn create(mut config: SshTunnelConfig) -> Result<Self> {
        info!(
            "Creating SSH tunnel to {}:{} via {}@{}:{}",
            config.target_host,
            config.target_port,
            if config.ssh_user.is_empty() {
                "<from config>"
            } else {
                &config.ssh_user
            },
            config.ssh_host,
            config.ssh_port
        );

        // Find the SSH key to use
        let key_path = match &config.key_path {
            Some(path) => path.clone(),
            None => {
                // Default to ~/.ssh/id_rsa if not specified
                let home_dir = dirs::home_dir()
                    .ok_or_else(|| anyhow!("Could not determine home directory"))?;
                home_dir.join(".ssh").join("id_rsa")
            }
        };

        // Create the SSH client
        let ssh_session = Self::connect(
            &key_path,
            None, // No certificate
            config.ssh_user.clone(),
            config.ssh_password.clone(),
            (config.ssh_host.clone(), config.ssh_port),
        )
        .await?;

        // Try to create a local listener with multiple attempts
        let max_attempts = 3;
        #[allow(unused_assignments)]
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            // If this is a retry, get a new port
            if attempt > 1 {
                match find_available_port().await {
                    Ok(new_port) => {
                        debug!("Retrying with new port {} (attempt {})", new_port, attempt);
                        config.local_port = new_port;
                    }
                    Err(e) => {
                        error!("Failed to find available port: {}", e);
                        return Err(e);
                    }
                }
            }

            // Try to create a local listener
            match TcpListener::bind(format!("127.0.0.1:{}", config.local_port)).await {
                Ok(listener) => {
                    debug!(
                        "Listening on local port {} (attempt {})",
                        config.local_port, attempt
                    );

                    // Clone values for the listener task
                    let target_host = config.target_host.clone();
                    let target_port = config.target_port;
                    let session_clone = ssh_session.clone();

                    // Spawn a task to handle incoming connections
                    let _listener_task = tokio::spawn(async move {
                        loop {
                            match listener.accept().await {
                                Ok((socket, addr)) => {
                                    debug!("Accepted connection from {}", addr);
                                    let session = session_clone.clone();
                                    let target_host = target_host.clone();

                                    // Spawn a task to handle this connection
                                    tokio::spawn(async move {
                                        if let Err(e) = handle_connection(
                                            socket,
                                            addr,
                                            session,
                                            &target_host,
                                            target_port,
                                        )
                                        .await
                                        {
                                            error!("Error handling connection: {}", e);
                                        }
                                    });
                                }
                                Err(e) => {
                                    error!("Error accepting connection: {}", e);
                                    break;
                                }
                            }
                        }
                    });

                    // Create a dummy Arc to keep the listener task alive
                    let _listener_task = Arc::new(());

                    return Ok(Self {
                        local_port: config.local_port,
                        session: ssh_session,
                        _listener_task,
                    });
                }
                Err(e) => {
                    error!("SSH tunnel attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    // If this is the last attempt, return the error
                    if attempt == max_attempts {
                        return Err(anyhow!(
                            "Failed to create SSH tunnel after {} attempts: {}",
                            max_attempts,
                            last_error.unwrap()
                        ));
                    }
                }
            }
        }

        // This should never be reached due to the return in the loop above
        Err(anyhow!("Failed to create SSH tunnel"))
    }

    /// Connect to an SSH server
    async fn connect<P: AsRef<Path>, A: tokio::net::ToSocketAddrs>(
        key_path: P,
        _openssh_cert_path: Option<P>,
        user: impl Into<String>,
        password: Option<String>,
        addrs: A,
    ) -> Result<Arc<Mutex<client::Handle<Client>>>> {
        let config = client::Config::default();
        let config = Arc::new(config);
        let sh = Client {};
        let mut session = client::connect(config, addrs, sh).await?;
        let user = user.into();

        let auth_res = if let Some(pass) = password {
            session.authenticate_password(user, pass).await?
        } else {
            let key_pair = load_secret_key(key_path, None)?;
            let hash_alg = match session.best_supported_rsa_hash().await? {
                Some(alg) => alg,
                None => return Err(anyhow!("No supported RSA hash algorithm found")),
            };
            session
                .authenticate_publickey(
                    user,
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg),
                )
                .await?
        };

        if !auth_res.success() {
            return Err(anyhow!("Authentication failed"));
        }

        Ok(Arc::new(Mutex::new(session)))
    }

    /// Close the SSH tunnel
    #[allow(dead_code)]
    pub async fn close(&self) -> Result<()> {
        let session = self.session.lock().await;
        session
            .disconnect(russh::Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}

/// Handle a single connection through the SSH tunnel
async fn handle_connection(
    mut stream: TcpStream,
    originator_addr: SocketAddr,
    session: Arc<Mutex<client::Handle<Client>>>,
    forward_host: &str,
    forward_port: u16,
) -> Result<()> {
    let session = session.lock().await;

    // Open a direct-tcpip channel to the target
    let mut channel = session
        .channel_open_direct_tcpip(
            forward_host.to_string(),
            forward_port.into(),
            originator_addr.ip().to_string(),
            originator_addr.port().into(),
        )
        .await?;

    // There's an event available on the session channel
    let mut stream_closed = false;
    let mut buf = vec![0; 65536];

    loop {
        // Handle one of the possible events:
        tokio::select! {
            // There's socket input available from the client
            r = stream.read(&mut buf), if !stream_closed => {
                match r {
                    Ok(0) => {
                        stream_closed = true;
                        channel.eof().await?;
                    },
                    // Send it to the server
                    Ok(n) => channel.data(&buf[..n]).await?,
                    Err(e) => return Err(e.into()),
                };
            },
            // There's an event available on the session channel
            Some(msg) = channel.wait() => {
                match msg {
                    // Write data to the client
                    ChannelMsg::Data { ref data } => {
                        stream.write_all(data).await?;
                    }
                    ChannelMsg::Eof => {
                        if !stream_closed {
                            channel.eof().await?;
                        }
                        break;
                    }
                    ChannelMsg::WindowAdjusted { new_size:_ }=> {
                        // Ignore this message type
                    }
                    _ => {
                        debug!("Unhandled channel message: {:?}", msg);
                    }
                }
            },
        }
    }

    Ok(())
}

/// SSH client handler
struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // In a production environment, you would want to verify the server key
        // against known hosts or prompt the user
        Ok(true)
    }
}

/// Load a secret key from a file
fn load_secret_key<P: AsRef<Path>>(
    path: P,
    passphrase: Option<&str>,
) -> Result<russh_keys::PrivateKey> {
    let key_path = path.as_ref();

    if !key_path.exists() {
        return Err(anyhow!("SSH key file not found: {:?}", key_path));
    }

    let key_data = std::fs::read_to_string(key_path)?;

    match russh_keys::decode_secret_key(&key_data, passphrase) {
        Ok(key) => Ok(key),
        Err(e) => Err(anyhow!("Failed to decode SSH key: {}", e)),
    }
}

/// Find an available local port for the SSH tunnel
pub async fn find_available_port() -> Result<u16> {
    // Try multiple times to find an available port
    for attempt in 0..10 {
        // Try to bind to port 0, which lets the OS assign an available port
        match TcpListener::bind("127.0.0.1:0").await {
            Ok(socket) => {
                match socket.local_addr() {
                    Ok(addr) => {
                        let port = addr.port();

                        // Close the first socket
                        drop(socket);

                        // Verify the port is actually available by trying to bind to it specifically
                        match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                            Ok(verify_socket) => {
                                // Port is available, close the verification socket and return the port
                                drop(verify_socket);
                                debug!("Found available port {} (attempt {})", port, attempt + 1);
                                return Ok(port);
                            }
                            Err(e) => {
                                debug!("Port {} was assigned but not available: {}", port, e);
                                // Try again with a different port
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to get local address: {}", e);
                        continue;
                    }
                }
            }
            Err(e) => {
                debug!("Failed to bind to port: {}", e);
                // Wait a bit before trying again
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        }
    }

    Err(anyhow!(
        "Failed to find an available port after multiple attempts"
    ))
}

/// Read SSH config file to get host information
pub async fn read_ssh_config(hostname: &str) -> Result<(String, u16, String, Option<PathBuf>)> {
    // Try to read the SSH config file
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let ssh_config_path = home_dir.join(".ssh").join("config");

    if !ssh_config_path.exists() {
        debug!("SSH config file not found at {:?}", ssh_config_path);
        return Ok((hostname.to_string(), 22, String::new(), None));
    }

    // Read the SSH config file
    let config_content = tokio::fs::read_to_string(&ssh_config_path)
        .await
        .map_err(|e| anyhow!("Failed to read SSH config file: {}", e))?;

    // Parse the SSH config file
    let mut in_host_section = false;
    let mut host_found = false;
    let mut actual_hostname = hostname.to_string();
    let mut port = 22;
    let mut user = String::new();
    let mut identity_file = None;

    for line in config_content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check if we're entering a Host section
        if line.to_lowercase().starts_with("host ") {
            let host_pattern = line[5..].trim();

            // Check if this Host section matches our hostname
            // Simple pattern matching for now - could be enhanced for wildcards
            if host_pattern == hostname {
                in_host_section = true;
                host_found = true;
                debug!("Found matching Host section for {}", hostname);
            } else {
                in_host_section = false;
            }
            continue;
        }

        // If we're in the matching Host section, parse the relevant options
        if in_host_section {
            if line.to_lowercase().starts_with("hostname ") {
                actual_hostname = line[9..].trim().to_string();
                debug!("Found HostName: {}", actual_hostname);
            } else if line.to_lowercase().starts_with("port ") {
                if let Ok(p) = line[5..].trim().parse::<u16>() {
                    port = p;
                    debug!("Found Port: {}", port);
                }
            } else if line.to_lowercase().starts_with("user ") {
                user = line[5..].trim().to_string();
                debug!("Found User: {}", user);
            } else if line.to_lowercase().starts_with("identityfile ") {
                let path_str = line[12..].trim();
                // Handle ~ in path
                let path = if path_str.starts_with('~') {
                    let path_without_tilde = &path_str[1..];
                    let path_without_leading_slash = path_without_tilde.trim_start_matches('/');
                    home_dir.join(path_without_leading_slash)
                } else {
                    PathBuf::from(path_str)
                };
                identity_file = Some(path.clone());
                debug!("Found IdentityFile: {:?}", path);
            }
        }
    }

    if host_found {
        debug!(
            "Using SSH config for {}: hostname={}, port={}, user={}, identity_file={:?}",
            hostname,
            actual_hostname,
            port,
            if user.is_empty() { "<none>" } else { &user },
            identity_file
        );
        Ok((actual_hostname, port, user, identity_file))
    } else {
        debug!(
            "No matching Host section found for {}, using defaults",
            hostname
        );
        Ok((hostname.to_string(), 22, String::new(), None))
    }
}

/// Parses a daemon RPC URL and determines if SSH tunneling is needed
pub async fn parse_daemon_rpc_url(
    url_str: &str,
) -> Result<(String, bool, Option<SshTunnelConfig>)> {
    // Check if the URL starts with ssh2+ prefix
    if url_str.starts_with("ssh2+http://") || url_str.starts_with("ssh2+https://") {
        let protocol = if url_str.starts_with("ssh2+https://") {
            "https"
        } else {
            "http"
        };

        // Remove the ssh2+ prefix
        let ssh_url = url_str.replace("ssh2+", "");
        let parsed_url = UrlParser::parse(&ssh_url)?;

        // Extract SSH connection details
        let ssh_host = parsed_url
            .host_str()
            .ok_or_else(|| anyhow!("Missing SSH host"))?;
        let ssh_port = parsed_url.port().unwrap_or(22);
        let ssh_user = parsed_url.username().to_string();
        let ssh_password = parsed_url.password().map(|s| s.to_string());

        // Extract target details (after the path)
        let path = parsed_url.path();
        if path.is_empty() || path == "/" {
            return Err(anyhow!("Missing target host in path"));
        }

        // Remove leading slash and parse target
        let target = path.trim_start_matches('/');
        let target_parts: Vec<&str> = target.split(':').collect();

        let target_host = target_parts[0];
        let target_port = if target_parts.len() > 1 {
            target_parts[1].parse::<u16>()?
        } else {
            if protocol == "https" {
                443
            } else {
                80
            }
        };

        // Create the final target URL that will be used after tunneling
        let target_url = format!("{}://localhost:{}", protocol, target_port);

        // Check if we need to read SSH config
        let (actual_ssh_host, actual_ssh_port, config_ssh_user, identity_file) =
            if ssh_user.is_empty() && ssh_port == 22 {
                // This looks like a hostname that might be in SSH config
                read_ssh_config(ssh_host).await?
            } else {
                // Use the provided values
                (ssh_host.to_string(), ssh_port, ssh_user.to_string(), None)
            };

        // Use config user if provided and no explicit user in URL
        let final_ssh_user = if ssh_user.is_empty() && !config_ssh_user.is_empty() {
            config_ssh_user
        } else {
            ssh_user.to_string()
        };

        // Find an available local port
        let local_port = find_available_port().await?;

        // Create SSH tunnel config
        let tunnel_config = SshTunnelConfig {
            ssh_host: actual_ssh_host,
            ssh_port: actual_ssh_port,
            ssh_user: final_ssh_user,
            ssh_password,
            target_host: target_host.to_string(),
            target_port,
            local_port,
            key_path: identity_file, // Use the identity file from SSH config if available
        };

        debug!("Parsed SSH tunnel config: {:?}", tunnel_config);
        return Ok((target_url, protocol == "https", Some(tunnel_config)));
    } else {
        // Regular URL without SSH tunneling
        let parsed_url = UrlParser::parse(url_str)?;
        let is_https = parsed_url.scheme() == "https";

        // Check if this is a localhost or IP address connection
        let host = parsed_url
            .host_str()
            .ok_or_else(|| anyhow!("Missing host"))?;
        let is_localhost = host == "localhost"
            || host == "127.0.0.1"
            || host.starts_with("192.168.")
            || host.starts_with("10.")
            || host.starts_with("172.");

        // For HTTPS connections to localhost or IP addresses, we'll need to bypass SSL validation
        let bypass_ssl = is_https && is_localhost;

        debug!("Regular URL: {}, bypass_ssl: {}", url_str, bypass_ssl);
        return Ok((url_str.to_string(), bypass_ssl, None));
    }
}

/// Creates a reqwest Client with appropriate SSL configuration
#[allow(dead_code)]
pub fn create_http_client(bypass_ssl: bool) -> Result<reqwest::Client> {
    let client_builder = reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(60)) // 60 seconds timeout
        .connect_timeout(std::time::Duration::from_secs(20)) // 20 seconds connect timeout
        .pool_idle_timeout(std::time::Duration::from_secs(60)) // Keep connections alive longer
        .pool_max_idle_per_host(10); // Increased from 5 to 10

    if bypass_ssl {
        debug!("Creating HTTP client with SSL validation disabled");
        client_builder
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))
    } else {
        debug!("Creating HTTP client with standard SSL validation");
        client_builder
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))
    }
}

/// A response with an optional SSH tunnel that keeps the tunnel alive until the response is consumed
#[derive(Debug)]
pub struct TunneledResponse {
    pub response: reqwest::Response,
    // The tunnel is kept alive as long as this struct is alive
    #[allow(dead_code)]
    pub _tunnel: Option<SshTunnel>,
}

impl TunneledResponse {
    pub fn new(response: reqwest::Response, tunnel: Option<SshTunnel>) -> Self {
        Self {
            response,
            _tunnel: tunnel,
        }
    }

    /// Get the response bytes while keeping the tunnel alive
    #[allow(dead_code)]
    pub async fn bytes(self) -> Result<bytes::Bytes> {
        // This consumes self, which keeps the tunnel alive until the bytes are read
        match self.response.bytes().await {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(anyhow!("Failed to get response bytes: {}", e)),
        }
    }

    /// Parse the response body as JSON while keeping the tunnel alive
    pub async fn json<T>(self) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        // This consumes self, which keeps the tunnel alive until the response is parsed
        match self.response.json::<T>().await {
            Ok(json) => Ok(json),
            Err(e) => Err(anyhow!("Failed to parse response as JSON: {}", e)),
        }
    }

    /// Get the response headers
    #[allow(dead_code)]
    pub fn headers(&self) -> &reqwest::header::HeaderMap {
        self.response.headers()
    }

    /// Get the response status
    #[allow(dead_code)]
    pub fn status(&self) -> reqwest::StatusCode {
        self.response.status()
    }
}

/// Makes an HTTP request through an SSH tunnel if needed
pub async fn make_request_with_tunnel(
    url: &str,
    body: String,
    auth: Option<String>,
    tunnel_config: Option<SshTunnelConfig>,
    bypass_ssl: bool,
    existing_tunnel: Option<SshTunnel>,
) -> Result<TunneledResponse> {
    use reqwest::ClientBuilder;

    // Create HTTP client with appropriate SSL configuration
    let client_builder = ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(20))
        .pool_idle_timeout(std::time::Duration::from_secs(60))
        .pool_max_idle_per_host(10);

    let client = if bypass_ssl {
        debug!("Creating HTTP client with SSL validation disabled");
        client_builder.danger_accept_invalid_certs(true).build()?
    } else {
        client_builder.build()?
    };

    // Use existing tunnel if provided, otherwise create a new one if needed
    let tunnel = if let Some(tunnel) = existing_tunnel {
        debug!("Using existing SSH tunnel on port {}", tunnel.local_port);
        Some(tunnel)
    } else if let Some(config) = tunnel_config {
        // Try to create the tunnel up to 3 times
        let mut last_error = None;
        let mut tunnel = None;

        for attempt in 1..=3 {
            debug!("SSH tunnel attempt {} of 3", attempt);
            match SshTunnel::create(config.clone()).await {
                Ok(t) => {
                    debug!("SSH tunnel created successfully on port {}", t.local_port);
                    tunnel = Some(t);
                    break;
                }
                Err(e) => {
                    error!("SSH tunnel attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < 3 {
                        // Wait before retrying
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }

        // If all attempts failed, return the last error
        if tunnel.is_none() {
            return Err(last_error
                .unwrap_or_else(|| anyhow!("Failed to create SSH tunnel after 3 attempts")));
        }

        tunnel
    } else {
        None
    };

    // Modify the URL if we're using a tunnel
    let final_url = match &tunnel {
        Some(t) => {
            // Replace the URL with localhost and the tunnel port
            let parsed_url = UrlParser::parse(url)?;
            let scheme = parsed_url.scheme();
            format!(
                "{}://localhost:{}{}",
                scheme,
                t.local_port,
                parsed_url.path()
            )
        }
        None => url.to_string(),
    };

    debug!("Making request to {}", final_url);

    // Make the request
    let request = client
        .post(&final_url)
        .header("Content-Type", "application/json")
        .body(body);

    // Add authentication if provided
    let request = if let Some(ref auth_str) = auth {
        if auth_str.starts_with("sshfs://") {
            // Read the auth cookie from the remote file
            match AUTH_CACHE.get(&auth_str).await {
                Ok(cookie_content) => {
                    debug!(
                        "Using auth cookie from SSH file (length: {})",
                        cookie_content.len()
                    );
                    // Bitcoin cookie auth format is username:password
                    if cookie_content.contains(':') {
                        let parts: Vec<&str> = cookie_content.splitn(2, ':').collect();
                        request.basic_auth(parts[0], Some(parts[1]))
                    } else {
                        // If it's not in the expected format, use it as a bearer token
                        request.bearer_auth(cookie_content)
                    }
                }
                Err(e) => {
                    return Err(anyhow!("Failed to read auth cookie from SSH: {}", e));
                }
            }
        } else if auth_str.contains(':') {
            // Basic auth
            let parts: Vec<&str> = auth_str.splitn(2, ':').collect();
            request.basic_auth(parts[0], Some(parts[1]))
        } else {
            // Bearer token
            request.bearer_auth(auth_str)
        }
    } else {
        request
    };

    // Send the request
    match request.send().await {
        Ok(response) => {
            // Check if we got a 401 or 403 error and have an sshfs auth
            if (response.status() == reqwest::StatusCode::UNAUTHORIZED
                || response.status() == reqwest::StatusCode::FORBIDDEN)
                && auth.as_ref().map_or(false, |a| a.starts_with("sshfs://"))
            {
                debug!("Got {} response, refreshing auth cookie", response.status());
                // Mark the auth cache for refresh on the next request
                AUTH_CACHE.mark_for_refresh();
            }

            Ok(TunneledResponse::new(response, tunnel))
        }
        Err(e) => Err(anyhow!("Request failed: {}", e)),
    }
}

/// Reads a file over SSH using russh
pub async fn read_file_over_ssh(url_str: &str) -> Result<String> {
    // Parse the URL
    if !url_str.starts_with("sshfs://") {
        return Err(anyhow!("Not an sshfs URL"));
    }

    // Remove the sshfs:// prefix
    let ssh_url = url_str.replace("sshfs://", "");

    // Split into host and path parts
    let parts: Vec<&str> = ssh_url.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid sshfs URL format, expected sshfs://host:path"
        ));
    }

    let host = parts[0];
    let path = parts[1];

    // Check if host contains @ for username
    let (ssh_host, ssh_user, ssh_port, key_path) = if host.contains('@') {
        let parts: Vec<&str> = host.splitn(2, '@').collect();
        (parts[1].to_string(), parts[0].to_string(), 22, None)
    } else {
        // Might be a hostname from SSH config
        let (actual_host, port, user, identity_file) = read_ssh_config(host).await?;
        (actual_host, user, port, identity_file)
    };

    // Determine the key path to use
    let actual_key_path = match key_path {
        Some(path) => path,
        None => dirs::home_dir().unwrap().join(".ssh").join("id_rsa"),
    };

    debug!("Using SSH key: {:?}", actual_key_path);

    // Create SSH client config
    let _config = SshTunnelConfig {
        ssh_host: ssh_host.clone(),
        ssh_port,
        ssh_user: ssh_user.clone(),
        ssh_password: None,
        target_host: "".to_string(), // Not used for file reading
        target_port: 0,              // Not used for file reading
        local_port: 0,               // Not used for file reading
        key_path: Some(actual_key_path.clone()),
    };

    // Connect to SSH server
    let ssh_session = SshTunnel::connect(
        &actual_key_path,
        None, // No certificate
        ssh_user,
        None, // No password for file reading
        (ssh_host, ssh_port),
    )
    .await?;

    // Execute the command to read the file
    let session = ssh_session.lock().await;
    let mut channel = session.channel_open_session().await?;

    // Execute cat command
    channel.exec(true, format!("cat {}", path)).await?;

    // Read the output
    let mut output = String::new();

    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { ref data }) => {
                let s = String::from_utf8_lossy(data);
                output.push_str(&s);
            }
            Some(ChannelMsg::Eof) => {
                break;
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => {
                if exit_status != 0 {
                    return Err(anyhow!("Command exited with status {}", exit_status));
                }
            }
            Some(_) => {}
            None => break,
        }
    }

    debug!("Successfully read file over SSH (length: {})", output.len());
    Ok(output.trim().to_string())
}

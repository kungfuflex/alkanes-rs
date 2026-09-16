#![cfg(feature = "ssh_e2e_tests")]
//! End-to-end test for SSH tunneling functionality.
//!
//! This test verifies that the `BitcoinRpcAdapter` can correctly parse `ssh://` URLs,
//! establish an SSH tunnel, and communicate with a mock Bitcoin daemon through it.

use rockshrew_mono::adapters::BitcoinRpcAdapter;
use rockshrew_mono::ssh_tunnel;
use metashrew_sync::BitcoinNodeAdapter;
use std::process::Command;
use tokio;

struct TestSshServer {
    container_id: String,
    ssh_port: u16,
}

impl TestSshServer {
    async fn new() -> Self {
        let image_name = "rockshrew-ssh-test";
        // Build the Docker image
        let build_status = Command::new("docker")
            .arg("build")
            .arg("-t")
            .arg(image_name)
            .arg("-f")
            .arg("test_assets/Dockerfile.ssh_test")
            .arg("test_assets")
            .status()
            .expect("Failed to execute docker build");
        assert!(build_status.success(), "Docker build failed");

        // Find a free port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ssh_port = listener.local_addr().unwrap().port();
        drop(listener);

        // Run the Docker container
        let container_id = Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("-p")
            .arg(format!("{}:22", ssh_port))
            .arg(image_name)
            .output()
            .expect("Failed to execute docker run")
            .stdout;
        let container_id = String::from_utf8(container_id).unwrap().trim().to_string();
        // Poll for SSH server readiness
        let mut ssh_ready = false;
        for _ in 0..20 {
            if let Ok(_) = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", ssh_port)).await {
                ssh_ready = true;
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        assert!(ssh_ready, "SSH server did not become ready in time");

        Self { container_id, ssh_port }
    }
}

impl Drop for TestSshServer {
    fn drop(&mut self) {
        // Stop and remove the Docker container
        let _ = Command::new("docker").arg("stop").arg(&self.container_id).status();
        let _ = Command::new("docker").arg("rm").arg(&self.container_id).status();
    }
}

#[tokio::test]
async fn test_ssh_tunnel_e2e() {
    let server = TestSshServer::new().await;
    let mock_rpc_port = 8332; // Port inside the container

    // Start the mock daemon inside the container
    let mut mock_daemon_handle = Command::new("docker")
        .arg("exec")
        .arg(&server.container_id)
        .arg("python3")
        .arg("/usr/local/bin/mock_server.py")
        .arg(mock_rpc_port.to_string())
        .spawn()
        .expect("Failed to start mock daemon");

    let rpc_url = format!("ssh2+http://testuser:testpassword@127.0.0.1:{}/127.0.0.1:{}", server.ssh_port, mock_rpc_port);
    let (parsed_url, bypass_ssl, tunnel_config) = ssh_tunnel::parse_daemon_rpc_url(&rpc_url)
        .await
        .expect("Failed to parse daemon RPC URL");
    
    let adapter = BitcoinRpcAdapter::new(parsed_url, None, bypass_ssl, tunnel_config);

    // Poll for the service to be ready
    let mut tip_height = 0;
    for _ in 0..20 {
        if let Ok(height) = adapter.get_tip_height().await {
            tip_height = height;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    assert_eq!(tip_height, 100, "Failed to get correct tip height after polling");

    // Clean up
    mock_daemon_handle.kill().unwrap();
}
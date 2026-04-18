//! MCP server management commands

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str;

use crate::commands::DeezelCommands;

/// Get the absolute path to the current alkanes-cli binary
pub fn get_cli_binary_path() -> Result<PathBuf> {
    let exe_path = env::current_exe()
        .context("Failed to get current executable path")?;
    
    // Canonicalize to get absolute path
    let absolute_path = exe_path
        .canonicalize()
        .context("Failed to canonicalize executable path")?;
    
    Ok(absolute_path)
}

/// Get the workspace root directory (parent of crates/)
pub fn get_workspace_root() -> Result<PathBuf> {
    let current_dir = env::current_dir()?;
    let mut path = current_dir.as_path();
    
    // Walk up the directory tree to find Cargo.toml with [workspace]
    while let Some(parent) = path.parent() {
        let cargo_toml = parent.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                return Ok(parent.to_path_buf());
            }
        }
        path = parent;
    }
    
    // Fallback: assume we're in the workspace root
    Ok(current_dir)
}

/// Get the MCP server directory path
pub fn get_mcp_server_dir() -> Result<PathBuf> {
    let workspace_root = get_workspace_root()?;
    Ok(workspace_root.join("alkanes-mcp-server"))
}

/// Check if Node.js is installed
pub fn check_nodejs() -> Result<bool> {
    let output = Command::new("node")
        .arg("--version")
        .output();
    
    match output {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Check if npm is installed
pub fn check_npm() -> Result<bool> {
    let output = Command::new("npm")
        .arg("--version")
        .output();
    
    match output {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Install and build the MCP server
pub fn install_mcp_server(force: bool) -> Result<()> {
    println!("Installing MCP server...");
    
    // Check prerequisites
    if !check_nodejs()? {
        anyhow::bail!("Node.js is not installed. Please install Node.js first.");
    }
    
    if !check_npm()? {
        anyhow::bail!("npm is not installed. Please install npm first.");
    }
    
    let mcp_dir = get_mcp_server_dir()?;
    
    if !mcp_dir.exists() {
        anyhow::bail!("MCP server directory not found at: {}", mcp_dir.display());
    }
    
    let dist_file = mcp_dir.join("dist").join("index.js");
    if dist_file.exists() && !force {
        println!("MCP server already built. Use --force to reinstall.");
        return Ok(());
    }
    
    // Run npm install
    println!("Running npm install...");
    let install_status = Command::new("npm")
        .arg("install")
        .current_dir(&mcp_dir)
        .status()
        .context("Failed to run npm install")?;
    
    if !install_status.success() {
        anyhow::bail!("npm install failed");
    }
    
    // Run npm run build
    println!("Building MCP server...");
    let build_status = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir(&mcp_dir)
        .status()
        .context("Failed to run npm run build")?;
    
    if !build_status.success() {
        anyhow::bail!("npm run build failed");
    }
    
    // Verify build succeeded
    if !dist_file.exists() {
        anyhow::bail!("Build failed: dist/index.js not found");
    }
    
    println!("✓ MCP server installed and built successfully!");
    Ok(())
}

/// Expand ~ in file paths
fn expand_home(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Generate MCP configuration from current CLI settings
pub fn generate_mcp_config(
    args: &DeezelCommands,
    output: Option<String>,
    environment: Option<String>,
) -> Result<()> {
    println!("Generating MCP configuration...");
    
    // Get paths
    let cli_binary_path = get_cli_binary_path()?;
    let mcp_dir = get_mcp_server_dir()?;
    let mcp_server_path = mcp_dir.join("dist").join("index.js");
    
    if !mcp_server_path.exists() {
        anyhow::bail!(
            "MCP server not built. Run 'alkanes-cli mcp install' first."
        );
    }
    
    // Determine environment name
    let env_name = environment.unwrap_or_else(|| args.provider.clone());
    
    // Build environment configuration
    let mut env_config = json!({
        "cli_path": cli_binary_path.to_string_lossy(),
        "provider": args.provider,
    });
    
    // Add optional fields
    if let Some(wallet_file) = &args.wallet_file {
        env_config["wallet_file"] = json!(wallet_file);
    }
    
    if let Some(passphrase) = &args.passphrase {
        // Use environment variable reference for security
        env_config["passphrase"] = json!("${WALLET_PASSPHRASE}");
    }
    
    if let Some(jsonrpc_url) = &args.jsonrpc_url {
        env_config["jsonrpc_url"] = json!(jsonrpc_url);
    }
    
    if let Some(data_api) = &args.data_api {
        env_config["data_api"] = json!(data_api);
    }
    
    if let Some(bitcoin_rpc_url) = &args.bitcoin_rpc_url {
        env_config["bitcoin_rpc_url"] = json!(bitcoin_rpc_url);
    }
    
    if let Some(esplora_api_url) = &args.esplora_api_url {
        env_config["esplora_api_url"] = json!(esplora_api_url);
    }
    
    if let Some(ord_server_url) = &args.ord_server_url {
        env_config["ord_server_url"] = json!(ord_server_url);
    }
    
    if let Some(metashrew_rpc_url) = &args.metashrew_rpc_url {
        env_config["metashrew_rpc_url"] = json!(metashrew_rpc_url);
    }
    
    if let Some(brc20_prog_rpc_url) = &args.brc20_prog_rpc_url {
        env_config["brc20_prog_rpc_url"] = json!(brc20_prog_rpc_url);
    }
    
    if let Some(opi_url) = &args.opi_url {
        env_config["opi_url"] = json!(opi_url);
    }
    
    if let Some(espo_rpc_url) = &args.espo_rpc_url {
        env_config["espo_rpc_url"] = json!(espo_rpc_url);
    }
    
    if let Some(titan_api_url) = &args.titan_api_url {
        env_config["titan_api_url"] = json!(titan_api_url);
    }
    
    if !args.jsonrpc_headers.is_empty() {
        env_config["jsonrpc_headers"] = json!(args.jsonrpc_headers);
    }
    
    if !args.opi_headers.is_empty() {
        env_config["opi_headers"] = json!(args.opi_headers);
    }
    
    // Build environments JSON (as string for env var)
    let env_name_for_key = env_name.clone();
    let environments = json!({
        env_name_for_key: env_config
    });

    // Build the MCP client configuration entry with individual env keys
    let mcp_client_entry = json!({
        "command": "node",
        "args": [mcp_server_path.to_string_lossy()],
        "env": {
            "environments": environments.to_string(),
            "default_environment": env_name,
            "timeout_seconds": "600",
            "WALLET_PASSPHRASE": args.passphrase.as_ref().map(|_| "".to_string()).unwrap_or_else(|| "".to_string())
        }
    });
    
    // Determine output file path
    let output_path = if let Some(output) = output {
        expand_home(&output)
    } else {
        // Default to ~/.cursor/mcp.json
        let home = env::var("HOME")
            .context("HOME environment variable not set")?;
        PathBuf::from(home).join(".cursor").join("mcp.json")
    };
    
    // Create .cursor directory if it doesn't exist
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create .cursor directory")?;
    }
    
    // Read existing config or create new one
    let mut config: Value = if output_path.exists() {
        let content = fs::read_to_string(&output_path)
            .context("Failed to read existing MCP config")?;
        serde_json::from_str(&content)
            .context("Failed to parse existing MCP config")?
    } else {
        json!({ "mcpServers": {} })
    };
    
    // Check for and fix legacy MCP_SERVER_CONFIG format
    if let Some(mcp_servers) = config.get_mut("mcpServers") {
        if let Some(servers) = mcp_servers.as_object_mut() {
            if let Some(existing_alkanes) = servers.get("alkanes-cli") {
                if let Some(existing_env) = existing_alkanes.get("env") {
                    if existing_env.get("MCP_SERVER_CONFIG").is_some() {
                        println!("⚠ Detected legacy MCP_SERVER_CONFIG format, migrating to individual env keys...");
                        // The new entry will replace the old one below
                    }
                }
            }
            servers.insert("alkanes-cli".to_string(), mcp_client_entry);
        }
    }
    
    // Write updated config
    let config_json = serde_json::to_string_pretty(&config)
        .context("Failed to serialize MCP config")?;
    
    fs::write(&output_path, config_json)
        .context("Failed to write MCP config file")?;
    
    println!("✓ MCP configuration written to: {}", output_path.display());
    println!("  Environment: {}", env_name);
    println!("  CLI binary: {}", cli_binary_path.display());
    
    if args.passphrase.is_some() {
        println!("  Note: Set WALLET_PASSPHRASE environment variable before using MCP server");
    }
    
    Ok(())
}

/// Check MCP server status
pub fn check_mcp_status() -> Result<()> {
    println!("MCP Server Status");
    println!("==================");
    
    // Check CLI binary
    match get_cli_binary_path() {
        Ok(path) => {
            println!("✓ CLI binary: {}", path.display());
            if !path.exists() {
                println!("  ⚠ Warning: Binary path does not exist!");
            }
        }
        Err(e) => {
            println!("✗ Failed to get CLI binary path: {}", e);
        }
    }
    
    // Check MCP server
    match get_mcp_server_dir() {
        Ok(dir) => {
            let dist_file = dir.join("dist").join("index.js");
            if dist_file.exists() {
                println!("✓ MCP server: {}", dist_file.display());
            } else {
                println!("✗ MCP server not built: {}", dist_file.display());
                println!("  Run 'alkanes-cli mcp install' to build it");
            }
        }
        Err(e) => {
            println!("✗ Failed to get MCP server directory: {}", e);
        }
    }
    
    // Check Node.js
    match check_nodejs() {
        Ok(true) => {
            let output = Command::new("node")
                .arg("--version")
                .output()
                .ok();
            if let Some(output) = output {
                if let Ok(version) = str::from_utf8(&output.stdout) {
                    println!("✓ Node.js: {}", version.trim());
                }
            }
        }
        Ok(false) => {
            println!("✗ Node.js: Not installed");
        }
        Err(e) => {
            println!("✗ Node.js check failed: {}", e);
        }
    }
    
    // Check npm
    match check_npm() {
        Ok(true) => {
            let output = Command::new("npm")
                .arg("--version")
                .output()
                .ok();
            if let Some(output) = output {
                if let Ok(version) = str::from_utf8(&output.stdout) {
                    println!("✓ npm: {}", version.trim());
                }
            }
        }
        Ok(false) => {
            println!("✗ npm: Not installed");
        }
        Err(e) => {
            println!("✗ npm check failed: {}", e);
        }
    }
    
    // Check MCP config file
    let home = env::var("HOME").ok();
    if let Some(home) = home {
        let config_path = PathBuf::from(home).join(".cursor").join("mcp.json");
        if config_path.exists() {
            println!("✓ MCP config: {}", config_path.display());
            
            // Try to read and show if alkanes-cli is configured
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(config_json) = serde_json::from_str::<Value>(&content) {
                    if let Some(servers) = config_json.get("mcpServers") {
                        if let Some(alkanes_cli) = servers.get("alkanes-cli") {
                            println!("  ✓ alkanes-cli server configured");
                        } else {
                            println!("  ⚠ alkanes-cli server not configured");
                            println!("    Run 'alkanes-cli mcp configure' to configure it");
                        }
                    }
                }
            }
        } else {
            println!("✗ MCP config: Not found at {}", config_path.display());
            println!("  Run 'alkanes-cli mcp configure' to create it");
        }
    }
    
    Ok(())
}

/// Complete setup (install + configure + verify)
pub fn setup_mcp(args: &DeezelCommands, force: bool) -> Result<()> {
    println!("Setting up MCP server...\n");
    
    // Step 1: Install
    println!("[1/3] Installing MCP server...");
    install_mcp_server(force)?;
    println!();
    
    // Step 2: Configure
    println!("[2/3] Configuring MCP server...");
    generate_mcp_config(args, None, None)?;
    println!();
    
    // Step 3: Verify
    println!("[3/3] Verifying setup...");
    check_mcp_status()?;
    println!();
    
    println!("✓ MCP server setup complete!");
    println!("\nNext steps:");
    println!("1. Set WALLET_PASSPHRASE environment variable (if using passphrase)");
    println!("2. Restart Cursor to load the MCP server");
    println!("3. The alkanes-cli tools will be available in Cursor");
    
    Ok(())
}

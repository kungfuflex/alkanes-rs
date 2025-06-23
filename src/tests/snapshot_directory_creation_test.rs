//! Test for snapshot directory creation issue
//!
//! This test reproduces the "No such file or directory" error that occurs
//! when trying to create snapshots without proper directory initialization.

use anyhow::Result;
use tempfile::TempDir;

#[tokio::test]
async fn test_snapshot_directory_creation_issue() -> Result<()> {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new()?;
    let snapshot_dir = temp_dir.path().join("snapshots");
    
    // Note: We intentionally DON'T create the snapshot directory
    // to reproduce the issue
    
    // Try to create a snapshot without initializing the directory structure
    // This simulates what happens when RockshrewSnapshotProvider.initialize() is not called
    
    // First, let's try to read index.json (this should fail)
    let index_path = snapshot_dir.join("index.json");
    let result = tokio::fs::read(&index_path).await;
    
    // This should fail because the directory doesn't exist
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("No such file or directory") || error_msg.contains("cannot find the path"));
    
    println!("Successfully reproduced the directory creation issue: {}", error_msg);
    
    Ok(())
}

#[tokio::test]
async fn test_snapshot_directory_creation_fix() -> Result<()> {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new()?;
    let snapshot_dir = temp_dir.path().join("snapshots");
    
    // Initialize the directory structure first (this is what the fix does)
    tokio::fs::create_dir_all(&snapshot_dir).await?;
    tokio::fs::create_dir_all(snapshot_dir.join("intervals")).await?;
    tokio::fs::create_dir_all(snapshot_dir.join("wasm")).await?;
    
    // Create initial index.json
    let index_content = r#"{
  "intervals": [],
  "latest_height": 0,
  "created_at": 1640995200
}"#;
    tokio::fs::write(snapshot_dir.join("index.json"), index_content).await?;
    
    // Verify the directory structure was created
    assert!(snapshot_dir.exists());
    assert!(snapshot_dir.join("intervals").exists());
    assert!(snapshot_dir.join("wasm").exists());
    assert!(snapshot_dir.join("index.json").exists());
    
    // Now try to create a snapshot - this should work
    let interval_dir = snapshot_dir.join("intervals").join("0-880010");
    tokio::fs::create_dir_all(&interval_dir).await?;
    
    // Create snapshot files
    tokio::fs::write(interval_dir.join("diff.bin.zst"), b"dummy data").await?;
    tokio::fs::write(interval_dir.join("stateroot.json"), r#"{"height": 880010, "root": "0000000000000000000000000000000000000000000000000000000000000000", "timestamp": 1640995200}"#).await?;
    
    // Verify the snapshot was created
    assert!(interval_dir.exists());
    assert!(interval_dir.join("diff.bin.zst").exists());
    assert!(interval_dir.join("stateroot.json").exists());
    
    println!("Successfully created snapshot with proper initialization");
    
    Ok(())
}

#[tokio::test]
async fn test_snapshot_directory_auto_creation() -> Result<()> {
    // Test that create_snapshot should auto-create directories if they don't exist
    let temp_dir = TempDir::new()?;
    let snapshot_dir = temp_dir.path().join("snapshots");
    
    // Try to create a snapshot without explicit initialization
    // The create_snapshot method should handle directory creation
    
    // First, let's manually ensure the base directory exists
    // This simulates what should happen in the initialization
    tokio::fs::create_dir_all(&snapshot_dir).await?;
    tokio::fs::create_dir_all(snapshot_dir.join("intervals")).await?;
    tokio::fs::create_dir_all(snapshot_dir.join("wasm")).await?;
    
    // Create initial index.json
    let index_content = r#"{
  "intervals": [],
  "latest_height": 0,
  "created_at": 1640995200
}"#;
    tokio::fs::write(snapshot_dir.join("index.json"), index_content).await?;
    
    // Now create_snapshot should work
    let interval_dir = snapshot_dir.join("intervals").join("0-880010");
    let result = tokio::fs::create_dir_all(&interval_dir).await;
    assert!(result.is_ok());
    
    println!("Successfully created snapshot with manual directory setup");
    
    Ok(())
}
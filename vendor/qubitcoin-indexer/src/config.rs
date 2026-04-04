//! Configuration parsing for indexer modules.
//!
//! Supports:
//! - `-loadindexer=label:path` command-line arguments
//! - `indexer.toml` manifest files

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Indexer layer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexerLayer {
    /// Secondary indexer: processes raw blocks, no dependencies on other indexers.
    Secondary,
    /// Tertiary indexer: depends on secondary (or other tertiary) indexers.
    Tertiary,
}

/// Configuration for a single indexer module.
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    /// Human-readable label (e.g. "alkanes", "brc20").
    pub label: String,
    /// Path to the `.wasm` binary.
    pub wasm_path: PathBuf,
    /// Whether to compute SMT state roots.
    pub smt_enabled: bool,
    /// Block height at which this indexer begins processing.
    /// Blocks below this height are skipped. Default: 0 (process all blocks).
    pub start_height: u32,
    /// Indexer layer (secondary or tertiary).
    pub layer: IndexerLayer,
    /// Labels of indexers this one depends on (tertiary only).
    /// A tertiary indexer starts processing only once all its dependencies
    /// have reached at least the current block height.
    pub depends_on: Vec<String>,
}

/// Manifest file format (`indexer.toml`).
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerManifest {
    pub label: String,
    pub sha256: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub installed_at: String,
    #[serde(default)]
    pub smt: bool,
    /// Block height at which this indexer begins processing (default: 0).
    #[serde(default)]
    pub start_height: u32,
    /// Indexer layer: "secondary" (default) or "tertiary".
    #[serde(default = "default_layer_str")]
    pub layer: String,
    /// Labels of indexers this one depends on (tertiary only).
    #[serde(default)]
    pub depends_on: Vec<String>,
}

fn default_layer_str() -> String {
    "secondary".to_string()
}

/// Parse `-loadindexer=label:path` and `-loadtertiary=label:path:dep1,dep2`
/// arguments.
///
/// Returns a list of indexer configs. Each `-loadindexer` argument has the
/// format `label:path` where `path` points to a `.wasm` file, or `label:dir`
/// where `dir` contains an `indexer.toml` manifest.
///
/// `-loadtertiary` arguments use `label:path:dep1,dep2` where the third
/// colon-separated segment is a comma-separated list of dependency labels.
///
/// When loading from a directory with an `indexer.toml`, the manifest fields
/// `start_height`, `layer`, and `depends_on` are used.
pub fn parse_load_indexer_args(args: &[&str]) -> Vec<IndexerConfig> {
    parse_indexer_args_with_layer(args, IndexerLayer::Secondary)
}

/// Parse `-loadtertiary=label:path:dep1,dep2` arguments.
pub fn parse_load_tertiary_args(args: &[&str]) -> Vec<IndexerConfig> {
    parse_indexer_args_with_layer(args, IndexerLayer::Tertiary)
}

fn parse_indexer_args_with_layer(args: &[&str], default_layer: IndexerLayer) -> Vec<IndexerConfig> {
    let mut configs = Vec::new();

    for arg in args {
        // Split into at most 3 parts: label:path[:deps]
        let parts: Vec<&str> = arg.splitn(3, ':').collect();
        if parts.len() < 2 {
            continue;
        }

        let label = parts[0];
        let path_str = parts[1];
        // For tertiary: third part is comma-separated dependency labels.
        // For secondary: third part is start_height (numeric).
        let cli_deps: Vec<String>;
        let cli_start_height: u32;
        if parts.len() >= 3 && default_layer == IndexerLayer::Tertiary {
            cli_deps = parts[2]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            cli_start_height = 0;
        } else if parts.len() >= 3 && default_layer == IndexerLayer::Secondary {
            cli_deps = vec![];
            cli_start_height = parts[2].trim().parse::<u32>().unwrap_or(0);
        } else {
            cli_deps = vec![];
            cli_start_height = 0;
        };

        let path = PathBuf::from(path_str);

        if path.is_dir() {
            // Look for indexer.toml manifest.
            let manifest_path = path.join("indexer.toml");
            if manifest_path.exists() {
                match load_manifest(&manifest_path) {
                    Ok(manifest) => {
                        let wasm_path = path.join("program.wasm");
                        let layer = parse_layer_str(&manifest.layer, default_layer);
                        let depends_on = if manifest.depends_on.is_empty() {
                            cli_deps.clone()
                        } else {
                            manifest.depends_on
                        };
                        configs.push(IndexerConfig {
                            label: manifest.label,
                            wasm_path,
                            smt_enabled: manifest.smt,
                            start_height: manifest.start_height,
                            layer,
                            depends_on,
                        });
                    }
                    Err(e) => {
                        tracing::error!(
                            label = label,
                            path = %manifest_path.display(),
                            error = %e,
                            "failed to load indexer manifest"
                        );
                    }
                }
            } else {
                // Directory without manifest — look for program.wasm.
                let wasm_path = path.join("program.wasm");
                configs.push(IndexerConfig {
                    label: label.to_string(),
                    wasm_path,
                    smt_enabled: false,
                    start_height: cli_start_height,
                    layer: default_layer,
                    depends_on: cli_deps.clone(),
                });
            }
        } else {
            // Direct path to .wasm file.
            configs.push(IndexerConfig {
                label: label.to_string(),
                wasm_path: path,
                smt_enabled: false,
                start_height: 0,
                layer: default_layer,
                depends_on: cli_deps.clone(),
            });
        }
    }

    configs
}

fn parse_layer_str(s: &str, default: IndexerLayer) -> IndexerLayer {
    match s.to_lowercase().as_str() {
        "tertiary" => IndexerLayer::Tertiary,
        "secondary" => IndexerLayer::Secondary,
        _ => default,
    }
}

/// Load an `indexer.toml` manifest.
fn load_manifest(path: &Path) -> Result<IndexerManifest, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read manifest: {}", e))?;
    toml::from_str(&content).map_err(|e| format!("parse manifest: {}", e))
}

/// Write an `indexer.toml` manifest.
pub fn write_manifest(path: &Path, manifest: &IndexerManifest) -> Result<(), String> {
    let content =
        toml::to_string_pretty(manifest).map_err(|e| format!("serialize manifest: {}", e))?;
    std::fs::write(path, content).map_err(|e| format!("write manifest: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manifest(label: &str) -> IndexerManifest {
        IndexerManifest {
            label: label.to_string(),
            sha256: "abc123".to_string(),
            source_url: "".to_string(),
            installed_at: "".to_string(),
            smt: false,
            start_height: 0,
            layer: "secondary".to_string(),
            depends_on: vec![],
        }
    }

    #[test]
    fn test_parse_direct_wasm_path() {
        let args = vec!["test:/path/to/module.wasm"];
        let configs = parse_load_indexer_args(&args);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].label, "test");
        assert_eq!(configs[0].wasm_path, PathBuf::from("/path/to/module.wasm"));
        assert!(!configs[0].smt_enabled);
        assert_eq!(configs[0].start_height, 0);
        assert_eq!(configs[0].layer, IndexerLayer::Secondary);
        assert!(configs[0].depends_on.is_empty());
    }

    #[test]
    fn test_parse_multiple_indexers() {
        let args = vec![
            "alkanes:/opt/alkanes.wasm",
            "brc20:/opt/brc20.wasm",
        ];
        let configs = parse_load_indexer_args(&args);
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].label, "alkanes");
        assert_eq!(configs[1].label, "brc20");
    }

    #[test]
    fn test_parse_empty_args() {
        let args: Vec<&str> = vec![];
        let configs = parse_load_indexer_args(&args);
        assert!(configs.is_empty());
    }

    #[test]
    fn test_parse_invalid_format_no_colon() {
        let args = vec!["nocolon"];
        let configs = parse_load_indexer_args(&args);
        assert!(configs.is_empty());
    }

    #[test]
    fn test_parse_directory_without_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("program.wasm"), b"fake wasm").unwrap();

        let arg = format!("test:{}", dir.path().display());
        let args = vec![arg.as_str()];
        let configs = parse_load_indexer_args(&args);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].label, "test");
        assert_eq!(configs[0].wasm_path, dir.path().join("program.wasm"));
    }

    #[test]
    fn test_parse_directory_with_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = make_manifest("custom_label");
        manifest.smt = true;
        manifest.start_height = 100;
        manifest.layer = "tertiary".to_string();
        manifest.depends_on = vec!["alkanes".to_string()];
        write_manifest(&dir.path().join("indexer.toml"), &manifest).unwrap();
        std::fs::write(dir.path().join("program.wasm"), b"fake wasm").unwrap();

        let arg = format!("ignored:{}", dir.path().display());
        let args = vec![arg.as_str()];
        let configs = parse_load_indexer_args(&args);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].label, "custom_label");
        assert!(configs[0].smt_enabled);
        assert_eq!(configs[0].start_height, 100);
        assert_eq!(configs[0].layer, IndexerLayer::Tertiary);
        assert_eq!(configs[0].depends_on, vec!["alkanes"]);
    }

    #[test]
    fn test_parse_tertiary_with_deps() {
        let args = vec!["overlay:/path/to/overlay.wasm:alkanes,brc20"];
        let configs = parse_load_tertiary_args(&args);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].label, "overlay");
        assert_eq!(configs[0].layer, IndexerLayer::Tertiary);
        assert_eq!(configs[0].depends_on, vec!["alkanes", "brc20"]);
    }

    #[test]
    fn test_parse_tertiary_single_dep() {
        let args = vec!["derived:/path/to/derived.wasm:base"];
        let configs = parse_load_tertiary_args(&args);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].depends_on, vec!["base"]);
    }

    #[test]
    fn test_manifest_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = make_manifest("test");
        manifest.sha256 = "deadbeef".to_string();
        manifest.source_url = "https://example.com/test.wasm".to_string();
        manifest.installed_at = "2025-01-01".to_string();
        manifest.start_height = 50;
        manifest.depends_on = vec!["foo".to_string(), "bar".to_string()];

        let path = dir.path().join("indexer.toml");
        write_manifest(&path, &manifest).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: IndexerManifest = toml::from_str(&content).unwrap();
        assert_eq!(loaded.label, "test");
        assert_eq!(loaded.sha256, "deadbeef");
        assert_eq!(loaded.start_height, 50);
        assert_eq!(loaded.depends_on, vec!["foo", "bar"]);
    }

    #[test]
    fn test_manifest_defaults() {
        // Minimal TOML — new fields should default correctly.
        let toml_str = r#"
            label = "minimal"
            sha256 = "abc"
        "#;
        let loaded: IndexerManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(loaded.label, "minimal");
        assert_eq!(loaded.start_height, 0);
        assert_eq!(loaded.layer, "secondary");
        assert!(loaded.depends_on.is_empty());
        assert!(!loaded.smt);
    }
}

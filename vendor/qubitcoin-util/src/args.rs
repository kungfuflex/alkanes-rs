//! Command-line argument parser matching Bitcoin Core's style.
//!
//! Maps to: `src/common/args.h` and `src/common/args.cpp` in Bitcoin Core.
//!
//! Supports:
//! - `-key=value` syntax (with leading dash)
//! - `-key` as boolean true
//! - `-nokey` as boolean false (negation prefix)
//! - Multiple values for the same key (e.g., `-connect=a -connect=b`)
//! - Default values via `ArgsManager::set_default`

use std::collections::HashMap;

/// Command-line argument parser matching Bitcoin Core's style.
///
/// Supports: `-key=value`, `-key` (boolean true), `-nokey` (boolean false).
///
/// Port of Bitcoin Core's `ArgsManager`.
pub struct ArgsManager {
    /// Parsed arguments: key -> list of values.
    /// Keys are stored without the leading dash.
    args: HashMap<String, Vec<String>>,
    /// Default values for arguments.
    defaults: HashMap<String, String>,
}

impl ArgsManager {
    /// Create a new empty argument manager.
    pub fn new() -> Self {
        ArgsManager {
            args: HashMap::new(),
            defaults: HashMap::new(),
        }
    }

    /// Parse command-line arguments.
    ///
    /// Arguments are expected in the form:
    /// - `-key=value`: sets key to value
    /// - `-key`: sets key to "" (boolean true)
    /// - `-nokey`: sets key with negation (boolean false)
    /// - Arguments without a leading `-` are ignored (positional args).
    ///
    /// Multiple occurrences of the same key append to the value list.
    /// The first argument (typically the program name) is skipped.
    pub fn parse_args(&mut self, args: &[String]) {
        // Skip the first argument (program name)
        let iter = if args.is_empty() {
            &args[..]
        } else {
            &args[1..]
        };

        for arg in iter {
            if !arg.starts_with('-') {
                // Skip positional arguments
                continue;
            }

            // Strip leading dashes (support both -key and --key)
            let stripped = arg.trim_start_matches('-');
            if stripped.is_empty() {
                continue;
            }

            if let Some(eq_pos) = stripped.find('=') {
                let key = &stripped[..eq_pos];
                let value = &stripped[eq_pos + 1..];
                self.args
                    .entry(key.to_lowercase())
                    .or_insert_with(Vec::new)
                    .push(value.to_string());
            } else {
                // Boolean flag: -key means true, -nokey means false
                let key = stripped.to_lowercase();
                if let Some(rest) = key.strip_prefix("no") {
                    if !rest.is_empty() {
                        // -nokey: insert key with "0" to represent false
                        self.args
                            .entry(rest.to_string())
                            .or_insert_with(Vec::new)
                            .push("0".to_string());
                    }
                } else {
                    // -key: insert with "1" to represent true
                    self.args
                        .entry(key)
                        .or_insert_with(Vec::new)
                        .push("1".to_string());
                }
            }
        }
    }

    /// Set a default value for a key.
    ///
    /// The default is used when [`get_arg`](Self::get_arg) is called and the
    /// key was not explicitly set on the command line.
    pub fn set_default(&mut self, key: &str, value: &str) {
        self.defaults.insert(key.to_lowercase(), value.to_string());
    }

    /// Get the last value for a key, or the default if not set.
    ///
    /// Returns `None` if the key is neither set nor has a default.
    pub fn get_arg(&self, key: &str) -> Option<&str> {
        let key_lower = key.to_lowercase();
        if let Some(values) = self.args.get(&key_lower) {
            values.last().map(|s| s.as_str())
        } else {
            self.defaults.get(&key_lower).map(|s| s.as_str())
        }
    }

    /// Get the last value for a key parsed as an integer, or `None` if not
    /// set or not a valid integer.
    pub fn get_int_arg(&self, key: &str) -> Option<i64> {
        self.get_arg(key).and_then(|v| v.parse::<i64>().ok())
    }

    /// Get the boolean value for a key.
    ///
    /// Returns `true` if the key's last value is "1", "true", or "yes"
    /// (case-insensitive). Returns `false` otherwise, including when the
    /// key is not set.
    pub fn get_bool_arg(&self, key: &str) -> bool {
        match self.get_arg(key) {
            Some(v) => matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"),
            None => false,
        }
    }

    /// Get all values for a key (for multi-value arguments).
    ///
    /// Returns an empty vector if the key is not set.
    pub fn get_args(&self, key: &str) -> Vec<&str> {
        let key_lower = key.to_lowercase();
        self.args
            .get(&key_lower)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Check if a key was explicitly set on the command line.
    pub fn is_set(&self, key: &str) -> bool {
        self.args.contains_key(&key.to_lowercase())
    }
}

impl Default for ArgsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_args(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    // --- Basic parsing tests ---

    #[test]
    fn test_parse_key_value() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&[
            "qubitcoind",
            "-datadir=/home/user/.qubitcoin",
        ]));
        assert_eq!(mgr.get_arg("datadir"), Some("/home/user/.qubitcoin"));
    }

    #[test]
    fn test_parse_boolean_flag() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-daemon"]));
        assert!(mgr.get_bool_arg("daemon"));
    }

    #[test]
    fn test_parse_no_prefix_boolean() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-nodaemon"]));
        assert!(!mgr.get_bool_arg("daemon"));
    }

    #[test]
    fn test_parse_explicit_boolean_values() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-listen=true"]));
        assert!(mgr.get_bool_arg("listen"));

        let mut mgr2 = ArgsManager::new();
        mgr2.parse_args(&make_args(&["qubitcoind", "-listen=false"]));
        assert!(!mgr2.get_bool_arg("listen"));
    }

    #[test]
    fn test_parse_integer_arg() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-port=8333"]));
        assert_eq!(mgr.get_int_arg("port"), Some(8333));
    }

    #[test]
    fn test_parse_negative_integer() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-timeout=-1"]));
        assert_eq!(mgr.get_int_arg("timeout"), Some(-1));
    }

    #[test]
    fn test_parse_non_integer_returns_none() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-port=abc"]));
        assert_eq!(mgr.get_int_arg("port"), None);
    }

    // --- Multiple values tests ---

    #[test]
    fn test_multiple_values() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&[
            "qubitcoind",
            "-connect=127.0.0.1:8333",
            "-connect=192.168.1.1:8333",
            "-connect=10.0.0.1:8333",
        ]));

        let connects = mgr.get_args("connect");
        assert_eq!(connects.len(), 3);
        assert_eq!(connects[0], "127.0.0.1:8333");
        assert_eq!(connects[1], "192.168.1.1:8333");
        assert_eq!(connects[2], "10.0.0.1:8333");

        // get_arg returns the last value
        assert_eq!(mgr.get_arg("connect"), Some("10.0.0.1:8333"));
    }

    // --- Default value tests ---

    #[test]
    fn test_default_value() {
        let mut mgr = ArgsManager::new();
        mgr.set_default("rpcport", "8332");
        mgr.parse_args(&make_args(&["qubitcoind"]));

        assert_eq!(mgr.get_arg("rpcport"), Some("8332"));
        assert_eq!(mgr.get_int_arg("rpcport"), Some(8332));
    }

    #[test]
    fn test_explicit_overrides_default() {
        let mut mgr = ArgsManager::new();
        mgr.set_default("rpcport", "8332");
        mgr.parse_args(&make_args(&["qubitcoind", "-rpcport=18332"]));

        assert_eq!(mgr.get_arg("rpcport"), Some("18332"));
        assert_eq!(mgr.get_int_arg("rpcport"), Some(18332));
    }

    #[test]
    fn test_no_default_returns_none() {
        let mgr = ArgsManager::new();
        assert_eq!(mgr.get_arg("nonexistent"), None);
        assert_eq!(mgr.get_int_arg("nonexistent"), None);
    }

    // --- is_set tests ---

    #[test]
    fn test_is_set() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-daemon", "-port=8333"]));

        assert!(mgr.is_set("daemon"));
        assert!(mgr.is_set("port"));
        assert!(!mgr.is_set("connect"));
    }

    #[test]
    fn test_is_set_not_affected_by_defaults() {
        let mut mgr = ArgsManager::new();
        mgr.set_default("rpcport", "8332");
        mgr.parse_args(&make_args(&["qubitcoind"]));

        // Default values should not show up as "set"
        assert!(!mgr.is_set("rpcport"));
    }

    // --- Case insensitivity tests ---

    #[test]
    fn test_case_insensitive_keys() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-DataDir=/tmp/test"]));
        assert_eq!(mgr.get_arg("datadir"), Some("/tmp/test"));
        assert_eq!(mgr.get_arg("DATADIR"), Some("/tmp/test"));
        assert!(mgr.is_set("datadir"));
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_args() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&[]));
        assert!(!mgr.is_set("anything"));
    }

    #[test]
    fn test_only_program_name() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind"]));
        assert!(!mgr.is_set("anything"));
    }

    #[test]
    fn test_positional_args_ignored() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "some_file.conf", "-daemon"]));
        assert!(mgr.is_set("daemon"));
        assert!(!mgr.is_set("some_file.conf"));
    }

    #[test]
    fn test_double_dash_prefix() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "--datadir=/tmp/test"]));
        assert_eq!(mgr.get_arg("datadir"), Some("/tmp/test"));
    }

    #[test]
    fn test_empty_value() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-key="]));
        assert_eq!(mgr.get_arg("key"), Some(""));
        assert!(mgr.is_set("key"));
    }

    #[test]
    fn test_get_bool_arg_default_false() {
        let mgr = ArgsManager::new();
        assert!(!mgr.get_bool_arg("nonexistent"));
    }

    #[test]
    fn test_get_bool_arg_yes_value() {
        let mut mgr = ArgsManager::new();
        mgr.parse_args(&make_args(&["qubitcoind", "-daemon=yes"]));
        assert!(mgr.get_bool_arg("daemon"));
    }

    #[test]
    fn test_get_args_empty_for_unset() {
        let mgr = ArgsManager::new();
        let result = mgr.get_args("nonexistent");
        assert!(result.is_empty());
    }

    #[test]
    fn test_default_impl() {
        let mgr = ArgsManager::default();
        assert!(!mgr.is_set("anything"));
    }
}

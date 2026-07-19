//! Enhanced logging utilities with colored/treeview/emoji support
//! 
//! This module provides consistent, visually-enhanced logging throughout the protorune indexer.
//! It uses metashrew_core::println! which works in both WASM and native contexts.

use metashrew_core::{println, stdio::{stdout, Write}};

/// Logging level colors and emoji prefixes
pub struct LogStyle {
    pub emoji: &'static str,
    pub prefix: &'static str,
}

impl LogStyle {
    pub const INFO: LogStyle = LogStyle { emoji: "ℹ️", prefix: "INFO" };
    pub const SUCCESS: LogStyle = LogStyle { emoji: "✅", prefix: "SUCCESS" };
    pub const WARNING: LogStyle = LogStyle { emoji: "⚠️", prefix: "WARNING" };
    pub const ERROR: LogStyle = LogStyle { emoji: "❌", prefix: "ERROR" };
    pub const DEBUG: LogStyle = LogStyle { emoji: "🔍", prefix: "DEBUG" };
    pub const BLOCK: LogStyle = LogStyle { emoji: "📦", prefix: "BLOCK" };
    pub const TX: LogStyle = LogStyle { emoji: "💳", prefix: "TX" };
    pub const PROTORUNE: LogStyle = LogStyle { emoji: "🔮", prefix: "PROTORUNE" };
    pub const RUNE: LogStyle = LogStyle { emoji: "🪙", prefix: "RUNE" };
    pub const PROTOSTONE: LogStyle = LogStyle { emoji: "💎", prefix: "PROTOSTONE" };
    pub const BALANCE: LogStyle = LogStyle { emoji: "⚖️", prefix: "BALANCE" };
    pub const BURN: LogStyle = LogStyle { emoji: "🔥", prefix: "BURN" };
}

/// Log a simple info message
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::INFO, format!($($arg)*));
    }};
}

/// Log a success message
#[macro_export]
macro_rules! log_success {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::SUCCESS, format!($($arg)*));
    }};
}

/// Log a warning message
#[macro_export]
macro_rules! log_warning {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::WARNING, format!($($arg)*));
    }};
}

/// Log an error message
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::ERROR, format!($($arg)*));
    }};
}

/// Log a debug message
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            use $crate::logging::{log_message, LogStyle};
            log_message(LogStyle::DEBUG, format!($($arg)*));
        }
    };
}

/// Log a block processing message
#[macro_export]
macro_rules! log_block {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::BLOCK, format!($($arg)*));
    }};
}

/// Log a transaction processing message
#[macro_export]
macro_rules! log_tx {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::TX, format!($($arg)*));
    }};
}

/// Log a protorune-specific message
#[macro_export]
macro_rules! log_protorune {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::PROTORUNE, format!($($arg)*));
    }};
}

/// Log a rune-specific message
#[macro_export]
macro_rules! log_rune {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::RUNE, format!($($arg)*));
    }};
}

/// Log a protostone message
#[macro_export]
macro_rules! log_protostone {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::PROTOSTONE, format!($($arg)*));
    }};
}

/// Log a balance sheet message
#[macro_export]
macro_rules! log_balance {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::BALANCE, format!($($arg)*));
    }};
}

/// Log a burn operation message
#[macro_export]
macro_rules! log_burn {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::BURN, format!($($arg)*));
    }};
}

/// Core logging function used by macros
pub fn log_message(style: LogStyle, message: String) {
    println!("{} [{}] {}", style.emoji, style.prefix, message);
}

/// Log a tree-style hierarchical structure
pub struct LogTree {
    prefix: String,
    lines: Vec<String>,
}

impl LogTree {
    pub fn new(root: String) -> Self {
        Self {
            prefix: String::new(),
            lines: vec![root],
        }
    }

    pub fn add(&mut self, item: String) {
        self.lines.push(format!("{}├─ {}", self.prefix, item));
    }

    pub fn add_last(&mut self, item: String) {
        self.lines.push(format!("{}└─ {}", self.prefix, item));
    }

    pub fn add_subtree<F>(&mut self, label: String, f: F)
    where
        F: FnOnce(&mut LogTree),
    {
        self.lines.push(format!("{}├─ {}", self.prefix, label));
        let old_prefix = self.prefix.clone();
        self.prefix = format!("{}│  ", self.prefix);
        f(self);
        self.prefix = old_prefix;
    }

    pub fn add_last_subtree<F>(&mut self, label: String, f: F)
    where
        F: FnOnce(&mut LogTree),
    {
        self.lines.push(format!("{}└─ {}", self.prefix, label));
        let old_prefix = self.prefix.clone();
        self.prefix = format!("{}   ", self.prefix);
        f(self);
        self.prefix = old_prefix;
    }

    pub fn print(&self) {
        for line in &self.lines {
            println!("{}", line);
        }
    }
}

/// Log a tree structure with a given style
pub fn log_tree(style: LogStyle, tree: &LogTree) {
    println!("{} [{}]", style.emoji, style.prefix);
    tree.print();
}

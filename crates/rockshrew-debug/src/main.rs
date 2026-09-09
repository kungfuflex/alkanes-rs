mod key_builder;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};
use rockshrew_runtime::adapter::RocksDBRuntimeAdapter;
use rockshrew_runtime::KeyValueStoreLike;
use std::io::{stdout, Stdout};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "rockshrew-debug")]
#[command(about = "Debug utilities for Rockshrew indexer")]
struct Cli {
    /// Path to the RocksDB database
    #[arg(long)]
    db_path: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Find the earliest block with a missed reorg by checking append-only processing counts
    FindEarliestReorg {
        /// Stop scanning at this height
        #[arg(long)]
        exit_at: u64,
    },
    /// Rollback the database to a specific block height
    Rollback {
        /// Target height to rollback to
        target_height: u64,

        /// Actually perform the rollback (without this flag, it's a dry run)
        #[arg(long)]
        execute: bool,
    },
    /// List sample keys from the database to understand structure
    ListKeys {
        /// Maximum number of keys to show
        #[arg(long, default_value = "100")]
        limit: usize,
    },
}

struct ReorgDetector {
    adapter: RocksDBRuntimeAdapter,
    start_height: u64,      // Store initial height for progress calculation
    current_height: u64,
    exit_at: u64,
    earliest_reorg: Option<ReorgInfo>,
    blocks_scanned: u64,
    start_time: Instant,
    // Stats for append-only checks
    blocks_compared: u64,
    hash_mismatches: u64,    // Actually "blocks processed multiple times"
    last_local_hash: Option<String>,   // Shows process count
    last_remote_hash: Option<String>,  // Shows "OK" or "REORG!"
}

#[derive(Clone, Debug)]
struct ReorgInfo {
    height: u64,
    local_hash: String,
    remote_hash: String,
}

impl ReorgDetector {
    fn new(db_path: String, exit_at: u64) -> Result<Self> {
        let mut adapter = RocksDBRuntimeAdapter::open_optimized(db_path)?;

        // Find current indexed height from the database
        // The key is "__INTERNAL/height" as per RocksDBStorageAdapter
        let mut current_height = 0u64;

        if let Some(height_bytes) = adapter.get(b"__INTERNAL/height")? {
            if height_bytes.len() >= 4 {
                // Storage uses u32 format
                current_height = u32::from_le_bytes([
                    height_bytes[0], height_bytes[1], height_bytes[2], height_bytes[3],
                ]) as u64;
                println!("Database indexed height: {}", current_height);
            }
        }

        if current_height == 0 {
            anyhow::bail!("Could not find indexed height in database. The __INTERNAL/height key is missing or database is empty.");
        }

        println!("Will scan from height {} down to {}", current_height, exit_at);
        println!("Checking append-only structure for blocks processed multiple times...\n");

        Ok(Self {
            adapter,
            start_height: current_height,  // Store initial height
            current_height,
            exit_at,
            earliest_reorg: None,
            blocks_scanned: 0,
            start_time: Instant::now(),
            blocks_compared: 0,
            hash_mismatches: 0,
            last_local_hash: None,
            last_remote_hash: None,
        })
    }

    fn scan_next_block(&mut self) -> Result<bool> {
        if self.current_height <= self.exit_at {
            return Ok(false); // Done
        }

        let height = self.current_height;

        // Check if this height was processed multiple times (append-only structure)
        // Use shared key builder to ensure consistency
        let length_key = key_builder::build_txid_length_key(height);

        let process_count = match self.adapter.get(&length_key)? {
            Some(bytes) => {
                String::from_utf8_lossy(&bytes).parse::<u32>().unwrap_or(0)
            }
            None => 0,
        };

        // Store for display
        self.last_local_hash = Some(format!("{}", process_count));
        self.last_remote_hash = Some(if process_count > 1 { "REORG!".to_string() } else { "OK".to_string() });

        if process_count > 0 {
            self.blocks_compared += 1;
        }

        // If processed more than once, this is a missed reorg!
        if process_count > 1 {
            self.hash_mismatches += 1;

            // Get the actual txid data from first and second processing
            let first_key = key_builder::build_txid_data_key(height, 0);
            let second_key = key_builder::build_txid_data_key(height, 1);

            let first_data = self.adapter.get(&first_key)?.unwrap_or_default();
            let second_data = self.adapter.get(&second_key)?.unwrap_or_default();

            // Update earliest reorg (we're scanning backwards, so later updates are earlier)
            self.earliest_reorg = Some(ReorgInfo {
                height,
                local_hash: format!("Processed {} times", process_count),
                remote_hash: format!("First: {} bytes, Second: {} bytes", first_data.len(), second_data.len()),
            });
        }

        self.blocks_scanned += 1;
        self.current_height -= 1;

        Ok(true) // Continue scanning
    }

    fn progress(&self) -> f64 {
        let total_blocks = self.start_height.saturating_sub(self.exit_at);
        if total_blocks == 0 {
            return 100.0;
        }
        // Clamp to prevent overflow if blocks_scanned somehow exceeds total
        let scanned = self.blocks_scanned.min(total_blocks);
        (scanned as f64 / total_blocks as f64) * 100.0
    }

    fn blocks_per_second(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }
        self.blocks_scanned as f64 / elapsed
    }

    fn estimated_time_remaining(&self) -> Duration {
        let blocks_remaining = self.current_height.saturating_sub(self.exit_at);
        let bps = self.blocks_per_second();
        if bps == 0.0 {
            return Duration::from_secs(0);
        }
        Duration::from_secs_f64(blocks_remaining as f64 / bps)
    }
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

fn run_tui(detector: &mut ReorgDetector) -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, detector);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    detector: &mut ReorgDetector,
) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Length(3),  // Title
                    Constraint::Length(3),  // Progress bar
                    Constraint::Length(11), // Stats (increased for more lines)
                    Constraint::Length(6),  // Reorg info
                    Constraint::Min(0),     // Help
                ])
                .split(f.area());

            // Title
            let title = Paragraph::new("Rockshrew Reorg Detector")
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            // Progress bar
            let progress = detector.progress();
            let gauge = Gauge::default()
                .block(Block::default().title("Scanning Progress").borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(progress as u16);
            f.render_widget(gauge, chunks[1]);

            // Stats
            let bps = detector.blocks_per_second();
            let eta = format_duration(detector.estimated_time_remaining());
            let elapsed = format_duration(detector.start_time.elapsed());

            let stats = vec![
                Line::from(vec![
                    Span::styled("Current Height: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{}", detector.current_height)),
                ]),
                Line::from(vec![
                    Span::styled("Process Count: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{}", detector.last_local_hash.as_deref().unwrap_or("N/A"))),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{}", detector.last_remote_hash.as_deref().unwrap_or("N/A")),
                        if detector.last_remote_hash.as_deref() == Some("REORG!") {
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Green)
                        }
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Blocks Scanned: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{} ({} with data)",
                        detector.blocks_scanned, detector.blocks_compared)),
                ]),
                Line::from(vec![
                    Span::styled("Missed Reorgs: ", Style::default().fg(Color::Yellow)),
                    Span::styled(format!("{}", detector.hash_mismatches),
                        if detector.hash_mismatches > 0 {
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Green)
                        }),
                ]),
                Line::from(vec![
                    Span::styled("Speed: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{:.2} blocks/sec", bps)),
                ]),
                Line::from(vec![
                    Span::styled("Elapsed: ", Style::default().fg(Color::Yellow)),
                    Span::raw(elapsed),
                ]),
                Line::from(vec![
                    Span::styled("ETA: ", Style::default().fg(Color::Yellow)),
                    Span::raw(eta),
                ]),
            ];

            let stats_widget = Paragraph::new(stats)
                .block(Block::default().title("Statistics").borders(Borders::ALL));
            f.render_widget(stats_widget, chunks[2]);

            // Reorg info
            let reorg_info = if let Some(ref reorg) = detector.earliest_reorg {
                let local_short = format!("{}...{}", &reorg.local_hash[..8], &reorg.local_hash[reorg.local_hash.len()-8..]);
                let remote_short = format!("{}...{}", &reorg.remote_hash[..8], &reorg.remote_hash[reorg.remote_hash.len()-8..]);
                vec![
                    Line::from(vec![
                        Span::styled("⚠ REORG DETECTED!", Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Earliest Height: ", Style::default().fg(Color::Yellow)),
                        Span::raw(format!("{}", reorg.height)),
                    ]),
                    Line::from(vec![
                        Span::styled("Local:  ", Style::default().fg(Color::Yellow)),
                        Span::raw(local_short),
                    ]),
                    Line::from(vec![
                        Span::styled("Remote: ", Style::default().fg(Color::Yellow)),
                        Span::raw(remote_short),
                    ]),
                ]
            } else {
                vec![
                    Line::from(vec![
                        Span::styled("No reorgs detected yet", Style::default().fg(Color::Green)),
                    ]),
                ]
            };

            let reorg_widget = Paragraph::new(reorg_info)
                .block(Block::default().title("Earliest Reorg Found").borders(Borders::ALL));
            f.render_widget(reorg_widget, chunks[3]);

            // Help
            let help = Paragraph::new("Press 'q' to quit")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[4]);
        })?;

        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        // Scan next block
        if !detector.scan_next_block()? {
            // Finished scanning
            std::thread::sleep(Duration::from_secs(1));
            break;
        }
    }

    Ok(())
}

struct RollbackStats {
    keys_deleted: usize,
    keys_scanned: usize,
    target_height: u64,
}

fn perform_rollback(db_path: String, target_height: u64, execute: bool) -> Result<RollbackStats> {
    use rockshrew_runtime::{KeyValueStoreLike, BatchLike};

    let mut adapter = RocksDBRuntimeAdapter::open_optimized(db_path)?;

    println!("Opening database...");
    println!("Target height: {}", target_height);
    println!("Mode: {}", if execute { "EXECUTE" } else { "DRY RUN" });
    println!();

    let mut keys_to_delete = Vec::new();
    let mut keys_scanned = 0;

    // Scan all keys in the database
    println!("Scanning database for keys with height > {}...", target_height);

    // Get an iterator over all keys
    let iter = adapter.db.iterator(rocksdb::IteratorMode::Start);

    for item in iter {
        let (key, _value) = item?;
        keys_scanned += 1;

        if keys_scanned % 10000 == 0 {
            print!("\rScanned {} keys...", keys_scanned);
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        // Parse the key to extract height if present
        // Keys can be in various formats:
        // - /key/height
        // - /blockhash/byheight/height
        // - /state/root/height
        // - /txids/byheight/height/...
        let key_str = String::from_utf8_lossy(&key);

        // Try to extract height from the key
        if let Some(height) = extract_height_from_key(&key_str) {
            if height > target_height {
                keys_to_delete.push(key.to_vec());
            }
        }
    }

    println!("\r{} keys scanned", keys_scanned);
    println!("Found {} keys to delete", keys_to_delete.len());

    if execute {
        println!("\nDeleting keys...");
        let mut batch = adapter.create_batch();

        for (i, key) in keys_to_delete.iter().enumerate() {
            if i % 1000 == 0 && i > 0 {
                print!("\rDeleted {}/{}...", i, keys_to_delete.len());
                std::io::Write::flush(&mut std::io::stdout())?;

                // Write batch periodically to avoid memory issues
                adapter.write(batch)?;
                batch = adapter.create_batch();
            }
            batch.delete(key);
        }

        // Write final batch
        if keys_to_delete.len() > 0 {
            adapter.write(batch)?;
        }

        println!("\r{} keys deleted", keys_to_delete.len());

        // Update indexed height using the correct key and format
        // Key is "__INTERNAL/height" and value is u32 (4 bytes)
        let height_key = b"__INTERNAL/height";
        let height_bytes = (target_height as u32).to_le_bytes();
        adapter.db.put(height_key, &height_bytes)?;
        println!("Updated indexed height to {}", target_height);

    } else {
        println!("\n⚠ DRY RUN - no changes made");
        println!("Run with --execute to actually perform the rollback");
    }

    Ok(RollbackStats {
        keys_deleted: keys_to_delete.len(),
        keys_scanned,
        target_height,
    })
}

/// Extract height from a key path
/// Examples:
/// - "/key/123" -> Some(123)
/// - "/blockhash/byheight/456" -> Some(456)
/// - "/txids/byheight/789/0" -> Some(789)
/// - "/state/root/100" -> Some(100)
fn extract_height_from_key(key: &str) -> Option<u64> {
    // Split by '/' and look for numeric segments
    let parts: Vec<&str> = key.split('/').collect();

    for (i, part) in parts.iter().enumerate() {
        // Skip non-numeric parts
        if let Ok(height) = part.parse::<u64>() {
            // Basic heuristic: if the number is likely a height
            // (reasonable range, not an index like 0, 1, 2...)
            // Check if preceded by "byheight", "root", or if it's after a key name
            if i > 0 {
                let prev = parts[i - 1];
                if prev == "byheight" || prev == "root" || prev.len() > 0 {
                    return Some(height);
                }
            }
        }
    }

    None
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::FindEarliestReorg { exit_at } => {
            let mut detector = ReorgDetector::new(cli.db_path, exit_at)?;
            run_tui(&mut detector)?;

            // Print final results
            println!("\n=== Scan Complete ===");
            println!("Blocks scanned: {}", detector.blocks_scanned);
            println!("Blocks with data: {}", detector.blocks_compared);
            println!("Missed reorgs found: {}", detector.hash_mismatches);

            if let Some(reorg) = detector.earliest_reorg {
                println!("\n⚠ EARLIEST MISSED REORG DETECTED:");
                println!("  Height: {}", reorg.height);
                println!("  {}", reorg.local_hash);
                println!("  {}", reorg.remote_hash);
                println!("\nThis block was processed multiple times without proper rollback.");
                println!("You should rollback to height {} or earlier", reorg.height - 1);
            } else {
                println!("\n✓ No missed reorgs detected in scanned range");
                println!("Each block was processed exactly once");
            }
        }
        Commands::Rollback {
            target_height,
            execute,
        } => {
            println!("=== Rollback Database ===");
            println!();

            let stats = perform_rollback(cli.db_path, target_height, execute)?;

            println!();
            println!("=== Rollback Summary ===");
            println!("Target height: {}", stats.target_height);
            println!("Keys scanned: {}", stats.keys_scanned);
            if execute {
                println!("Keys deleted: {}", stats.keys_deleted);
                println!();
                println!("✓ Rollback complete!");
            } else {
                println!("Keys that would be deleted: {}", stats.keys_deleted);
                println!();
                println!("⚠ This was a dry run. Use --execute to perform the rollback.");
            }
        }
        Commands::ListKeys { limit } => {
            println!("=== Database Keys Sample ===");
            println!();

            let adapter = RocksDBRuntimeAdapter::open_optimized(cli.db_path)?;
            let mut iter = adapter.db.raw_iterator();
            iter.seek_to_first();

            let mut count = 0;
            while iter.valid() && count < limit {
                if let Some(key) = iter.key() {
                    let key_str = String::from_utf8_lossy(key);
                    let value_len = iter.value().map(|v| v.len()).unwrap_or(0);
                    println!("{:4}. {} (value: {} bytes)", count + 1, key_str, value_len);
                    count += 1;
                }
                iter.next();
            }

            println!();
            println!("Displayed {} keys", count);
        }
    }

    Ok(())
}

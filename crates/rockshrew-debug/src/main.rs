use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::Txid;
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
use std::collections::HashSet;
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
    /// Find the earliest block with a missed reorg
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
}

struct ReorgDetector {
    adapter: RocksDBRuntimeAdapter,
    current_height: u64,
    exit_at: u64,
    earliest_reorg: Option<ReorgInfo>,
    blocks_scanned: u64,
    start_time: Instant,
}

#[derive(Clone, Debug)]
struct ReorgInfo {
    height: u64,
    total_txids: usize,
    unique_txids: usize,
    duplicate_count: usize,
}

impl ReorgDetector {
    fn new(db_path: String, exit_at: u64) -> Result<Self> {
        let mut adapter = RocksDBRuntimeAdapter::open_optimized(db_path)?;

        // Find current tip height by scanning the database
        // We look for the key pattern "/txids/byheight/<height>"
        let mut current_height = 0u64;

        // Scan for height keys - look for the highest one
        for h in (exit_at..=10_000_000).rev() {
            let key = format!("/txids/byheight/{}/length", h);
            if let Some(_) = adapter.get(key.as_bytes())? {
                current_height = h;
                break;
            }
        }

        if current_height == 0 {
            anyhow::bail!("Could not find tip height in database");
        }

        Ok(Self {
            adapter,
            current_height,
            exit_at,
            earliest_reorg: None,
            blocks_scanned: 0,
            start_time: Instant::now(),
        })
    }

    fn get_txids_for_height(&mut self, height: u64) -> Result<Vec<Txid>> {
        let mut txids = Vec::new();

        // Read the length
        let length_key = format!("/txids/byheight/{}/length", height);
        let length_bytes = match self.adapter.get(length_key.as_bytes())? {
            Some(bytes) => bytes,
            None => return Ok(txids), // No txids at this height
        };

        if length_bytes.len() < 4 {
            return Ok(txids);
        }

        let length = u32::from_le_bytes([
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
        ]);

        // Read each txid
        for i in 0..length {
            let item_key = format!("/txids/byheight/{}/{}", height, i);
            if let Some(txid_bytes) = self.adapter.get(item_key.as_bytes())? {
                if txid_bytes.len() >= 32 {
                    let mut txid_array = [0u8; 32];
                    txid_array.copy_from_slice(&txid_bytes[..32]);
                    txids.push(Txid::from_byte_array(txid_array));
                }
            }
        }

        Ok(txids)
    }

    fn scan_next_block(&mut self) -> Result<bool> {
        if self.current_height <= self.exit_at {
            return Ok(false); // Done
        }

        let height = self.current_height;

        // Get transaction IDs for this height
        let txids = self.get_txids_for_height(height)?;

        if !txids.is_empty() {
            let total_txids = txids.len();
            let unique_txids: HashSet<_> = txids.iter().collect();
            let unique_count = unique_txids.len();

            // Check for duplicates
            if unique_count < total_txids {
                let duplicate_count = total_txids - unique_count;
                self.earliest_reorg = Some(ReorgInfo {
                    height,
                    total_txids,
                    unique_txids: unique_count,
                    duplicate_count,
                });
            }
        }

        self.blocks_scanned += 1;
        self.current_height -= 1;

        Ok(true) // Continue scanning
    }

    fn progress(&self) -> f64 {
        let total_blocks = self.current_height.saturating_sub(self.exit_at);
        if total_blocks == 0 {
            return 100.0;
        }
        (self.blocks_scanned as f64 / total_blocks as f64) * 100.0
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
                    Constraint::Length(8),  // Stats
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
                    Span::styled("Exit At: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{}", detector.exit_at)),
                ]),
                Line::from(vec![
                    Span::styled("Blocks Scanned: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{}", detector.blocks_scanned)),
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
                vec![
                    Line::from(vec![
                        Span::styled("⚠ REORG DETECTED!", Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Height: ", Style::default().fg(Color::Yellow)),
                        Span::raw(format!("{}", reorg.height)),
                    ]),
                    Line::from(vec![
                        Span::styled("Duplicate TXIDs: ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("{}", reorg.duplicate_count),
                            Style::default().fg(Color::Red)
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Total/Unique: ", Style::default().fg(Color::Yellow)),
                        Span::raw(format!("{}/{}", reorg.total_txids, reorg.unique_txids)),
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

        // Update indexed height
        let height_key = b"/indexed_height";
        let height_bytes = target_height.to_le_bytes();
        adapter.db.put(height_key, height_bytes)?;
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

            if let Some(reorg) = detector.earliest_reorg {
                println!("\n⚠ EARLIEST REORG DETECTED:");
                println!("  Height: {}", reorg.height);
                println!("  Total TXIDs: {}", reorg.total_txids);
                println!("  Unique TXIDs: {}", reorg.unique_txids);
                println!("  Duplicates: {}", reorg.duplicate_count);
            } else {
                println!("\n✓ No reorgs detected in scanned range");
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
    }

    Ok(())
}

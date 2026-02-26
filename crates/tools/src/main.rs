use anyhow::{Context, Result};
use clap::Parser;
use core::{ContentPack, InputJournal, ReplayResult, replay::replay_to_end};
use std::fs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the journal JSON file to replay
    #[arg(short, long)]
    journal: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let journal_data = fs::read_to_string(&args.journal)
        .with_context(|| format!("Failed to read journal file: {}", args.journal))?;
    let journal: InputJournal = serde_json::from_str(&journal_data)
        .with_context(|| "Failed to deserialize journal JSON")?;

    let content = ContentPack::default();

    let result: ReplayResult = replay_to_end(&content, &journal)
        .map_err(|e| anyhow::anyhow!("Replay failed during execution: {:?}", e))?;

    println!("Replay complete.");
    println!("Final Tick: {}", result.final_tick);
    println!("Outcome: {:?}", result.final_outcome);
    println!("Snapshot Hash: {}", result.final_snapshot_hash);

    Ok(())
}

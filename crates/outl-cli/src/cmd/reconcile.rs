//! `outl reconcile` — print orphan log and (eventually) open a TUI for
//! manual resolution.
//!
//! Phase 1 ships a read-only listing. The TUI flow lands once `outl-tui`
//! has its read-write paths.

use crate::workspace_layout::Paths;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Run the `reconcile` subcommand.
pub fn run(path: &Path) -> Result<()> {
    let paths = Paths::at(path.to_path_buf());
    if !paths.orphans.exists() {
        println!("no orphans recorded");
        return Ok(());
    }
    let text = fs::read_to_string(&paths.orphans)
        .with_context(|| format!("reading {}", paths.orphans.display()))?;
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        println!("no orphans recorded");
        return Ok(());
    }
    println!("{} orphan(s) pending manual resolution:", lines.len());
    for line in &lines {
        println!("  {line}");
    }
    println!();
    println!("Interactive resolution lands with `outl-tui` (phase 5+).");
    Ok(())
}

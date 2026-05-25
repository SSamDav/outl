//! `outl doctor` — workspace integrity check.
//!
//! Reports problems without fixing them — `outl reconcile` and editor
//! workflows are the canonical fix paths. Doctor is read-only.

use crate::workspace_layout::{read_config, Paths};
use anyhow::{Context, Result};
use outl_core::storage::{SqliteStorage, Storage};
use outl_md::sidecar::{self, sidecar_path_for};
use std::collections::HashSet;
use std::path::Path;

#[derive(Default)]
struct Findings {
    warnings: usize,
    errors: usize,
}

impl Findings {
    fn warn(&mut self, msg: impl AsRef<str>) {
        self.warnings += 1;
        println!("warn: {}", msg.as_ref());
    }
    fn err(&mut self, msg: impl AsRef<str>) {
        self.errors += 1;
        println!("err:  {}", msg.as_ref());
    }
    fn ok(&self, msg: impl AsRef<str>) {
        println!("ok:   {}", msg.as_ref());
    }
}

/// Run the `doctor` subcommand.
pub fn run(path: &Path) -> Result<()> {
    let paths = Paths::at(path.to_path_buf());
    let cfg =
        read_config(&paths).with_context(|| "workspace config missing — run `outl init` first")?;

    println!("workspace: {}", paths.root.display());
    println!("actor:     {}", cfg.workspace.actor_id);
    println!();

    let mut f = Findings::default();

    // 1. SQLite integrity.
    match SqliteStorage::open(&paths.db).and_then(|s| s.integrity_check()) {
        Ok(s) if s.eq_ignore_ascii_case("ok") => {
            f.ok("log.db PRAGMA integrity_check passed");
        }
        Ok(other) => f.err(format!("log.db integrity_check reported: {other}")),
        Err(e) => f.err(format!("log.db integrity check failed: {e}")),
    }

    // 2. Op log basic stats.
    let known_node_ids: HashSet<outl_core::id::NodeId> = match SqliteStorage::open(&paths.db) {
        Ok(storage) => match storage.all_ops() {
            Ok(ops) => {
                println!("ok:   op log has {} ops", ops.len());
                let mut ids = HashSet::new();
                for op in &ops {
                    let node = match &op.op {
                        outl_core::op::Op::Move { node, .. }
                        | outl_core::op::Op::Edit { node, .. }
                        | outl_core::op::Op::SetProp { node, .. }
                        | outl_core::op::Op::Create { node, .. } => *node,
                    };
                    ids.insert(node);
                }
                ids
            }
            Err(e) => {
                f.err(format!("could not read op log: {e}"));
                HashSet::new()
            }
        },
        Err(e) => {
            f.err(format!("could not open log.db: {e}"));
            HashSet::new()
        }
    };

    // 3. Pages and journals: `.md` ↔ sidecar pairing.
    for dir in [&paths.pages, &paths.journals] {
        if !dir.is_dir() {
            continue;
        }
        let mut md_files = Vec::new();
        let mut sidecar_files = Vec::new();
        for entry in walkdir::WalkDir::new(dir).max_depth(1) {
            let Ok(entry) = entry else {
                continue;
            };
            let p = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if name.starts_with('.') {
                // Sidecar dotfile.
                if name.ends_with(".outl") {
                    sidecar_files.push(p.to_path_buf());
                }
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) == Some("md") {
                md_files.push(p.to_path_buf());
            }
        }

        check_md_files(&mut f, &md_files, &known_node_ids);
        check_orphan_sidecars(&mut f, &sidecar_files, &md_files);
    }

    // 4. Orphan log presence (informational).
    if paths.orphans.exists() {
        let bytes = std::fs::metadata(&paths.orphans)
            .map(|m| m.len())
            .unwrap_or(0);
        if bytes == 0 {
            f.ok("orphans.log is empty");
        } else {
            println!("info: orphans.log has {bytes} bytes — run `outl reconcile` to triage");
        }
    }

    // 5. Lock file warning if held by something else (we can't acquire it).
    match outl_core::WorkspaceLock::acquire(&paths.root) {
        Ok(_lock) => f.ok("workspace lock is free (no other outl process attached)"),
        Err(outl_core::LockError::AlreadyHeld(_)) => {
            f.warn("another outl process is holding the workspace lock");
        }
        Err(e) => f.warn(format!("could not test workspace lock: {e}")),
    }

    println!();
    match (f.errors, f.warnings) {
        (0, 0) => println!("integrity OK"),
        (0, w) => println!("integrity OK with {w} warning(s)"),
        (e, w) => {
            println!("{e} error(s), {w} warning(s) — see lines above");
            // Non-zero exit so scripts can detect failure.
            std::process::exit(1);
        }
    }
    Ok(())
}

fn check_md_files(
    f: &mut Findings,
    md_files: &[std::path::PathBuf],
    known_node_ids: &HashSet<outl_core::id::NodeId>,
) {
    for md in md_files {
        let scp = sidecar_path_for(md);
        if !scp.exists() {
            f.warn(format!(
                "{}: no sidecar (next `outl serve` or TUI commit will create one)",
                md.display()
            ));
            continue;
        }
        match sidecar::read(&scp) {
            Ok(sc) if sc.version == sidecar::SIDECAR_VERSION => {
                // Cross-check each block ID against the op log.
                let mut unknown = 0;
                for b in &sc.blocks {
                    if !known_node_ids.is_empty() && !known_node_ids.contains(&b.id) {
                        unknown += 1;
                    }
                }
                if unknown == 0 {
                    f.ok(format!(
                        "{} (sidecar v{}, {} blocks, all IDs known)",
                        md.display(),
                        sc.version,
                        sc.blocks.len()
                    ));
                } else {
                    f.warn(format!(
                        "{}: {} block id(s) in sidecar not present in op log (workspace partially de-synced)",
                        md.display(),
                        unknown
                    ));
                }
            }
            Ok(sc) => {
                f.warn(format!(
                    "{}: sidecar version {} unsupported by this build",
                    md.display(),
                    sc.version
                ));
            }
            Err(e) => {
                f.err(format!("{}: sidecar unreadable: {e}", md.display()));
            }
        }
    }
}

fn check_orphan_sidecars(
    f: &mut Findings,
    sidecar_files: &[std::path::PathBuf],
    md_files: &[std::path::PathBuf],
) {
    let md_names: HashSet<String> = md_files
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
        .collect();
    for scp in sidecar_files {
        let Some(name) = scp.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Sidecar naming is `.<md_name>.outl`. Strip prefix + suffix.
        let Some(stripped) = name.strip_prefix('.').and_then(|s| s.strip_suffix(".outl")) else {
            continue;
        };
        if !md_names.contains(stripped) {
            f.warn(format!(
                "{}: orphaned sidecar (no matching {} on disk)",
                scp.display(),
                stripped
            ));
        }
    }
}

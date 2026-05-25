//! Workspace-wide derived index.
//!
//! Walks the `pages/` and `journals/` directories, parses each `.md`,
//! and builds in-memory maps the TUI / GUI / mobile can query without
//! re-walking the filesystem:
//!
//! - `slug → PageEntry` (filename without `.md`).
//! - `title → slug` (the `title::` property; falls back to slug).
//! - `slug → Vec<Backlink>` (every block that contains `[[name]]` or
//!   `#name` where `slugify(name) == this`).
//!
//! Rebuild on demand: this is cheap for hundreds of pages, expensive
//! for thousands. The TUI calls `rebuild()` at startup and on a debounce
//! after writes.

use crate::inline::{tokenize, InlineTok};
use crate::parse::{parse, OutlineNode};
use crate::slug::slugify;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One entry in the index — the data we want to know about a page
/// without re-reading its file.
#[derive(Debug, Clone)]
pub struct PageEntry {
    /// Filesystem path, e.g. `pages/avelino.md`.
    pub path: PathBuf,
    /// Slug (filename without extension).
    pub slug: String,
    /// User-visible title (from `title::` property, or `slug` if unset).
    pub title: String,
    /// Optional decoration from the `icon::` property — usually a
    /// single emoji or short string the UI prepends to the title.
    /// `None` when the page has no `icon::` set.
    pub icon: Option<String>,
    /// Whether the file lives in `journals/`.
    pub is_journal: bool,
}

/// One backlink — a block in another page that references this slug.
#[derive(Debug, Clone)]
pub struct Backlink {
    /// Slug of the page containing the reference.
    pub source_slug: String,
    /// Title of the source page.
    pub source_title: String,
    /// Icon of the source page (if any) — propagated so backlink
    /// panels can render the same `<icon> <title>` shape every other
    /// surface uses.
    pub source_icon: Option<String>,
    /// Filesystem path of the source.
    pub source_path: PathBuf,
    /// Block index inside the source page (DFS preorder).
    pub block_index: usize,
    /// Block text snippet (for display).
    pub snippet: String,
}

/// Full workspace index.
#[derive(Debug, Default, Clone)]
pub struct WorkspaceIndex {
    pages: HashMap<String, PageEntry>,
    title_to_slug: HashMap<String, String>,
    backlinks: HashMap<String, Vec<Backlink>>,
}

impl WorkspaceIndex {
    /// Walk `pages/` and `journals/` under `workspace_root`, parse every
    /// `.md`, and return the populated index. Files that fail to parse
    /// are skipped with no error — the index is best-effort.
    ///
    /// Two logical passes (pages metadata first, then backlinks) but
    /// only **one read+parse per file**: the parsed AST is held in a
    /// buffer between passes. Halves the I/O + parsing cost vs the
    /// naive two-pass implementation; verified in `benches/index.rs`.
    pub fn build(workspace_root: &Path) -> Self {
        let mut idx = WorkspaceIndex::default();
        // Buffer of (slug, parsed AST). Populated in pass 1 alongside
        // the `pages` map; consumed in pass 2 for backlink collection.
        // Capacity-hint avoids regrowth on workspaces of any reasonable
        // size.
        let mut parsed_pages: Vec<(String, crate::parse::ParsedPage)> = Vec::with_capacity(64);

        for (dir, is_journal) in [
            (workspace_root.join("pages"), false),
            (workspace_root.join("journals"), true),
        ] {
            if !dir.is_dir() {
                continue;
            }
            for entry in walkdir::WalkDir::new(&dir).max_depth(1) {
                let Ok(entry) = entry else {
                    continue;
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if path.extension().and_then(|x| x.to_str()) != Some("md") {
                    continue;
                }
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
                {
                    continue;
                }
                let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let Ok(text) = std::fs::read_to_string(path) else {
                    continue;
                };
                let parsed = parse(&text);
                let title = parsed
                    .properties
                    .iter()
                    .find(|(k, _)| k == "title")
                    .map(|(_, v)| v.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| slug.to_string());
                let icon = parsed
                    .properties
                    .iter()
                    .find(|(k, _)| k == "icon")
                    .map(|(_, v)| v.trim().to_string())
                    .filter(|s| !s.is_empty());

                idx.pages.insert(
                    slug.to_string(),
                    PageEntry {
                        path: path.to_path_buf(),
                        slug: slug.to_string(),
                        title: title.clone(),
                        icon,
                        is_journal,
                    },
                );
                idx.title_to_slug.insert(title.clone(), slug.to_string());
                parsed_pages.push((slug.to_string(), parsed));
            }
        }

        // Second pass: scan blocks for `[[ref]]` and `#tag`, populate
        // backlinks. Reuses the AST cached in `parsed_pages` so we
        // don't pay another read + parse round-trip.
        for (slug, parsed) in &parsed_pages {
            // Clone is cheap — `PageEntry` is small and `Arc`-less.
            // Avoids holding an immutable borrow of `idx.pages` while
            // we mutate `idx.backlinks` below.
            let Some(entry) = idx.pages.get(slug).cloned() else {
                continue;
            };
            let mut block_idx = 0usize;
            collect_backlinks_recursive(&parsed.blocks, &mut block_idx, &entry, &mut idx);
        }

        idx
    }

    /// Look up a page by its slug.
    pub fn by_slug(&self, slug: &str) -> Option<&PageEntry> {
        self.pages.get(slug)
    }

    /// Look up a page by its `title::` (or slug fallback). Title match
    /// is case-sensitive — use `pages_by_title_prefix` for autocomplete.
    pub fn by_title(&self, title: &str) -> Option<&PageEntry> {
        let slug = self.title_to_slug.get(title)?;
        self.pages.get(slug)
    }

    /// Iterate every page entry in unspecified order.
    pub fn pages(&self) -> impl Iterator<Item = &PageEntry> {
        self.pages.values()
    }

    /// Number of pages indexed.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Backlinks pointing at a given slug. The returned slice may be
    /// empty.
    pub fn backlinks(&self, slug: &str) -> &[Backlink] {
        self.backlinks
            .get(slug)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Titles starting with `prefix` (case-insensitive), best-effort
    /// for autocomplete. Returns at most `limit` results sorted by
    /// title length (shorter first).
    pub fn pages_by_title_prefix(&self, prefix: &str, limit: usize) -> Vec<&PageEntry> {
        let needle = prefix.to_lowercase();
        let mut hits: Vec<&PageEntry> = self
            .pages
            .values()
            .filter(|p| p.title.to_lowercase().starts_with(&needle))
            .collect();
        hits.sort_by_key(|p| (p.title.len(), p.title.clone()));
        hits.truncate(limit);
        hits
    }
}

fn collect_backlinks_recursive(
    blocks: &[OutlineNode],
    cursor: &mut usize,
    source: &PageEntry,
    idx: &mut WorkspaceIndex,
) {
    for b in blocks {
        let block_index = *cursor;
        let snippet = b.text.clone();
        for tok in tokenize(&b.text) {
            match tok {
                InlineTok::PageRef { name } => {
                    let target_slug = slugify(name);
                    push_backlink(idx, &target_slug, source, block_index, &snippet);
                }
                InlineTok::Tag { name } => {
                    let target_slug = slugify(name);
                    push_backlink(idx, &target_slug, source, block_index, &snippet);
                }
                _ => {}
            }
        }
        *cursor += 1;
        collect_backlinks_recursive(&b.children, cursor, source, idx);
    }
}

fn push_backlink(
    idx: &mut WorkspaceIndex,
    target_slug: &str,
    source: &PageEntry,
    block_index: usize,
    snippet: &str,
) {
    // Skip self-references: a page linking to itself is noise.
    if target_slug == source.slug {
        return;
    }
    idx.backlinks
        .entry(target_slug.to_string())
        .or_default()
        .push(Backlink {
            source_slug: source.slug.clone(),
            source_title: source.title.clone(),
            source_icon: source.icon.clone(),
            source_path: source.path.clone(),
            block_index,
            snippet: snippet.to_string(),
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_workspace(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (rel, content) in files {
            let full = dir.path().join(rel);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full, content).unwrap();
        }
        dir
    }

    #[test]
    fn empty_workspace_indexes_to_nothing() {
        let dir = TempDir::new().unwrap();
        let idx = WorkspaceIndex::build(dir.path());
        assert_eq!(idx.page_count(), 0);
    }

    #[test]
    fn pages_get_indexed_by_slug_and_title() {
        let dir = write_workspace(&[
            (
                "pages/avelino.md",
                "title:: Avelino\n\n- some note about me\n",
            ),
            ("pages/projeto.md", "title:: Meu Projeto\n\n- objetivo\n"),
        ]);
        let idx = WorkspaceIndex::build(dir.path());
        assert_eq!(idx.page_count(), 2);
        assert_eq!(idx.by_slug("avelino").unwrap().title, "Avelino");
        assert_eq!(idx.by_title("Meu Projeto").unwrap().slug, "projeto");
    }

    #[test]
    fn missing_title_falls_back_to_slug() {
        let dir = write_workspace(&[("pages/no-title.md", "- bare bullet\n")]);
        let idx = WorkspaceIndex::build(dir.path());
        assert_eq!(idx.by_slug("no-title").unwrap().title, "no-title");
    }

    #[test]
    fn icon_property_is_indexed_and_propagated_to_backlinks() {
        let dir = write_workspace(&[
            (
                "pages/avelino.md",
                "title:: Avelino\nicon:: 🦀\n\n- author\n",
            ),
            (
                "pages/projeto.md",
                "title:: Projeto\nicon:: 🚀\n\n- led by [[Avelino]]\n",
            ),
            // Page without icon — must produce None, not crash.
            ("pages/bare.md", "title:: Bare\n\n- nothing fancy\n"),
        ]);
        let idx = WorkspaceIndex::build(dir.path());

        assert_eq!(idx.by_slug("avelino").unwrap().icon.as_deref(), Some("🦀"));
        assert_eq!(idx.by_slug("projeto").unwrap().icon.as_deref(), Some("🚀"));
        assert_eq!(idx.by_slug("bare").unwrap().icon, None);

        // Backlink to Avelino comes from Projeto — must carry its icon.
        let bls = idx.backlinks("avelino");
        assert_eq!(bls.len(), 1);
        assert_eq!(bls[0].source_slug, "projeto");
        assert_eq!(bls[0].source_icon.as_deref(), Some("🚀"));
    }

    #[test]
    fn empty_icon_is_treated_as_none() {
        // `icon:: ` (no value) shouldn't show up as a present-but-empty
        // icon — the UI would render a stray space.
        let dir = write_workspace(&[("pages/x.md", "title:: X\nicon::\n\n- body\n")]);
        let idx = WorkspaceIndex::build(dir.path());
        assert_eq!(idx.by_slug("x").unwrap().icon, None);
    }

    #[test]
    fn backlinks_are_collected_across_pages() {
        let dir = write_workspace(&[
            ("pages/avelino.md", "title:: Avelino\n\n- I am the author\n"),
            (
                "pages/projeto.md",
                "title:: Projeto\n\n- led by [[Avelino]]\n",
            ),
            (
                "journals/2026-05-24.md",
                "- meeting with [[Avelino]] and #urgent stuff\n",
            ),
        ]);
        let idx = WorkspaceIndex::build(dir.path());
        let bl = idx.backlinks("avelino");
        assert_eq!(bl.len(), 2);
        let slugs: Vec<_> = bl.iter().map(|b| b.source_slug.as_str()).collect();
        assert!(slugs.contains(&"projeto"));
        assert!(slugs.contains(&"2026-05-24"));

        let urgent = idx.backlinks("urgent");
        assert_eq!(urgent.len(), 1);
    }

    #[test]
    fn self_references_are_skipped() {
        let dir = write_workspace(&[(
            "pages/recursive.md",
            "title:: Recursive\n\n- I link to [[Recursive]] myself\n",
        )]);
        let idx = WorkspaceIndex::build(dir.path());
        assert!(idx.backlinks("recursive").is_empty());
    }

    #[test]
    fn journals_are_treated_as_pages_for_lookup() {
        let dir = write_workspace(&[("journals/2026-05-24.md", "- entry\n")]);
        let idx = WorkspaceIndex::build(dir.path());
        let entry = idx.by_slug("2026-05-24").unwrap();
        assert!(entry.is_journal);
    }

    #[test]
    fn title_prefix_lookup() {
        let dir = write_workspace(&[
            ("pages/a.md", "title:: Apple\n\n- a\n"),
            ("pages/b.md", "title:: Apricot\n\n- a\n"),
            ("pages/c.md", "title:: Banana\n\n- a\n"),
        ]);
        let idx = WorkspaceIndex::build(dir.path());
        let hits = idx.pages_by_title_prefix("Ap", 10);
        assert_eq!(hits.len(), 2);
        let names: Vec<_> = hits.iter().map(|p| p.title.as_str()).collect();
        assert!(names.contains(&"Apple"));
        assert!(names.contains(&"Apricot"));
    }
}

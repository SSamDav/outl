//! Navigation: between pages, between journals, between blocks, and
//! inside a block's text. Also `[[ref]]` / `#tag` / date-link
//! resolution.
//!
//! Nothing in this file persists — `nav` only mutates `self.view`,
//! `self.selected`, and `self.cursor_col`. The lifecycle module is
//! the one that touches disk.

use crate::outline_ops::{node_at_path, path_for_index};
use crate::state::{App, Mode, View};
use anyhow::Result;
use chrono::{Duration, Local};
use outl_md::inline::{ref_at_cursor, RefTarget};
use outl_md::reconcile::reconcile_md;
use std::fs;
use std::path::PathBuf;

impl App {
    pub(crate) fn current_path(&self) -> PathBuf {
        match &self.view {
            View::Journal(date) => self
                .workspace_root
                .join("journals")
                .join(format!("{}.md", date.format("%Y-%m-%d"))),
            View::Page(p) => p.clone(),
        }
    }

    pub(crate) fn current_title(&self) -> String {
        let mode_tag = match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert { .. } => "INSERT",
            Mode::Visual { .. } => "VISUAL",
        };
        match &self.view {
            View::Journal(date) => {
                format!("Journal · {} · [{}]", date.format("%A, %Y-%m-%d"), mode_tag)
            }
            View::Page(p) => {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                // Pull title + icon from the workspace index. Falls
                // back to the slug when the index doesn't know about
                // this page yet (just-created file, before the next
                // rebuild). Title is preferred over slug because it's
                // what the user wrote — `Page · CTO` reads better than
                // `Page · cto`.
                let entry = self.index.by_slug(stem);
                let display_name = entry
                    .map(|e| e.title.clone())
                    .unwrap_or_else(|| stem.to_string());
                let icon_prefix = entry
                    .and_then(|e| e.icon.as_deref())
                    .map(|i| format!("{i} "))
                    .unwrap_or_default();
                format!("Page · {icon_prefix}{display_name} · [{mode_tag}]")
            }
        }
    }

    /// Slug of the currently-opened view, used to look up backlinks.
    pub(crate) fn current_slug(&self) -> String {
        self.current_path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string()
    }

    pub(crate) fn go_today(&mut self) -> Result<()> {
        self.view = View::Journal(Local::now().date_naive());
        self.selected = 0;
        self.cursor_col = 0;
        self.ensure_view_file_exists()?;
        self.load_current();
        Ok(())
    }

    pub(crate) fn shift_journal(&mut self, days: i64) -> Result<()> {
        let new_date = match self.view {
            View::Journal(d) => d + Duration::days(days),
            _ => Local::now().date_naive() + Duration::days(days),
        };
        self.view = View::Journal(new_date);
        self.selected = 0;
        self.cursor_col = 0;
        self.ensure_view_file_exists()?;
        self.load_current();
        Ok(())
    }

    pub(crate) fn move_selection(&mut self, delta: i32) {
        if self.flat_len == 0 {
            self.selected = 0;
            self.cursor_col = 0;
            return;
        }
        let cur = self.selected as i32;
        let next = (cur + delta).clamp(0, self.flat_len as i32 - 1) as usize;
        if next != self.selected {
            self.selected = next;
            // Each block has its own cursor context; reset to start.
            self.cursor_col = 0;
        }
    }

    /// Current selected block's text (or empty if no selection).
    pub(crate) fn current_block_text(&self) -> String {
        let Some(path) = path_for_index(&self.page.blocks, self.selected) else {
            return String::new();
        };
        node_at_path(&self.page.blocks, &path)
            .map(|n| n.text.clone())
            .unwrap_or_default()
    }

    pub(crate) fn current_block_char_count(&self) -> usize {
        self.current_block_text().chars().count()
    }

    pub(crate) fn move_cursor_col(&mut self, delta: i32) {
        let max = self.current_block_char_count() as i32;
        let next = (self.cursor_col as i32 + delta).clamp(0, max);
        self.cursor_col = next as usize;
    }

    pub(crate) fn cursor_to_home(&mut self) {
        self.cursor_col = 0;
    }

    pub(crate) fn cursor_to_end(&mut self) {
        self.cursor_col = self.current_block_char_count();
    }

    pub(crate) fn cursor_word_left(&mut self) {
        let text = self.current_block_text();
        let chars: Vec<char> = text.chars().collect();
        let mut i = self.cursor_col;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.cursor_col = i;
    }

    pub(crate) fn cursor_word_right(&mut self) {
        let text = self.current_block_text();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = self.cursor_col;
        while i < len && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        self.cursor_col = i;
    }

    /// If the cursor is sitting on a `[[ref]]`, `#tag`, or `[[YYYY-MM-DD]]`,
    /// open the corresponding page or journal. Returns `true` when an
    /// open happened so the caller can suppress the fallback (entering
    /// Insert mode on Enter).
    pub(crate) fn try_open_under_cursor(&mut self) -> Result<bool> {
        let text = self.current_block_text();
        let Some(target) = ref_at_cursor(&text, self.cursor_col) else {
            return Ok(false);
        };
        match target {
            RefTarget::Journal(date) => {
                self.view = View::Journal(date);
                self.selected = 0;
                self.cursor_col = 0;
                self.ensure_view_file_exists()?;
                self.load_current();
            }
            RefTarget::Page(name) | RefTarget::Tag(name) => {
                self.open_page_by_name(&name)?;
            }
        }
        Ok(true)
    }

    /// Open (or create) the page corresponding to a user-visible name.
    /// Files live under `pages/{slug}.md`; the original `name` is
    /// preserved in the page's `title::` property.
    pub(crate) fn open_page_by_name(&mut self, name: &str) -> Result<()> {
        let slug = outl_md::slug::slugify(name);
        let path = self.workspace_root.join("pages").join(format!("{slug}.md"));
        let created_new = !path.exists();
        if created_new {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            // Seed with title:: <name> + a single empty bullet so the
            // editor has a cursor home.
            let seed = format!("title:: {name}\n\n- \n");
            outl_md::write_atomic(&path, seed.as_bytes())?;
            // Reconcile to establish stable IDs.
            let _ = reconcile_md(
                &mut self.workspace,
                &self.hlc,
                &path,
                Some(&self.orphans_log),
            );
        }
        self.view = View::Page(path);
        self.selected = 0;
        self.cursor_col = 0;
        self.load_current();
        self.refresh_page_list();
        if created_new {
            self.status = format!("created page \"{name}\"");
        }
        Ok(())
    }
}

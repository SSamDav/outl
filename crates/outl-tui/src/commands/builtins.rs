// Each struct here is self-documenting (one `impl SlashCommand` block
// each, name + description on the trait methods). The struct itself
// is just a zero-sized handle.
#![allow(missing_docs)]

//! Built-in slash / palette commands.
//!
//! Each command is a small struct implementing [`SlashCommand`].
//! Adding a new one is ~20 lines + one call in [`register_all`].
//!
//! Convention:
//!
//! - `name()` is lowercase, single word.
//! - `description()` is one short sentence in present-tense English.
//! - Args-less commands (`needs_args = false`) usually toggle UI or
//!   navigate. They run immediately from `/` without a second prompt.
//! - Arg-taking commands explain expected format in `description`.

use anyhow::Result;

use super::{CommandRegistry, SlashCommand};
use crate::state::App;
use crate::theme;

/// Hook for [`CommandRegistry::with_builtins`].
pub(super) fn register_all(reg: &mut CommandRegistry) {
    reg.register(PropBlockCommand);
    reg.register(PropPageCommand);
    reg.register(SearchCommand);
    reg.register(RunCommand);
    reg.register(ThemeCommand);
    reg.register(TodayCommand);
    reg.register(RefreshCommand);
    reg.register(WriteCommand);
    reg.register(HelpCommand);
    reg.register(QuitCommand);
    reg.register(OpenCommand);
}

// ---------------------------------------------------------------------------
// prop-block — add or update a property on the current block
// ---------------------------------------------------------------------------

pub struct PropBlockCommand;
impl SlashCommand for PropBlockCommand {
    fn name(&self) -> &'static str {
        "prop-block"
    }
    fn description(&self) -> &'static str {
        "Set a property on the current block — `prop-block <key> <value>` (empty value deletes)"
    }
    fn needs_args(&self) -> bool {
        true
    }
    fn aliases(&self) -> &'static [&'static str] {
        // `prop` defaults to the block scope — that's the common case.
        &["prop"]
    }
    fn execute(&self, app: &mut App, args: &str) -> Result<bool> {
        let (key, value) = args.split_once(' ').unwrap_or((args, ""));
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            app.status = "usage: prop-block <key> <value>".into();
            return Ok(false);
        }
        app.set_property_on_current_block(key, value);
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// prop-page — add or update a property at page level
// ---------------------------------------------------------------------------

pub struct PropPageCommand;
impl SlashCommand for PropPageCommand {
    fn name(&self) -> &'static str {
        "prop-page"
    }
    fn description(&self) -> &'static str {
        "Set a property on the page itself (`title::`, `icon::`, …) — `prop-page <key> <value>`"
    }
    fn needs_args(&self) -> bool {
        true
    }
    fn execute(&self, app: &mut App, args: &str) -> Result<bool> {
        let (key, value) = args.split_once(' ').unwrap_or((args, ""));
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            app.status = "usage: prop-page <key> <value>".into();
            return Ok(false);
        }
        app.set_property_on_page(key, value);
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// search — workspace-wide block search
// ---------------------------------------------------------------------------

pub struct SearchCommand;
impl SlashCommand for SearchCommand {
    fn name(&self) -> &'static str {
        "search"
    }
    fn description(&self) -> &'static str {
        "Open the workspace search overlay"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["s", "find"]
    }
    fn execute(&self, app: &mut App, _args: &str) -> Result<bool> {
        app.open_search();
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// run — execute the code block under the cursor
// ---------------------------------------------------------------------------

pub struct RunCommand;
impl SlashCommand for RunCommand {
    fn name(&self) -> &'static str {
        "run"
    }
    fn description(&self) -> &'static str {
        "Run the code block under the cursor through outl-exec"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["x", "execute"]
    }
    fn execute(&self, app: &mut App, _args: &str) -> Result<bool> {
        app.run_current_block();
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// theme — switch palette at runtime
// ---------------------------------------------------------------------------

pub struct ThemeCommand;
impl SlashCommand for ThemeCommand {
    fn name(&self) -> &'static str {
        "theme"
    }
    fn description(&self) -> &'static str {
        "Switch the active theme — `theme <preset>`"
    }
    fn needs_args(&self) -> bool {
        true
    }
    fn execute(&self, app: &mut App, args: &str) -> Result<bool> {
        if args.is_empty() {
            app.status = "usage: theme <preset>".into();
            return Ok(false);
        }
        if let Some(t) = theme::by_name(args) {
            let name = t.name;
            app.theme = t;
            app.status = format!("theme: {name}");
        } else {
            app.status = format!("unknown theme: {args}");
        }
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// today — jump to today's journal
// ---------------------------------------------------------------------------

pub struct TodayCommand;
impl SlashCommand for TodayCommand {
    fn name(&self) -> &'static str {
        "today"
    }
    fn description(&self) -> &'static str {
        "Jump to today's journal"
    }
    fn execute(&self, app: &mut App, _args: &str) -> Result<bool> {
        app.go_today()?;
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// refresh — re-read the workspace from disk
// ---------------------------------------------------------------------------

pub struct RefreshCommand;
impl SlashCommand for RefreshCommand {
    fn name(&self) -> &'static str {
        "refresh"
    }
    fn description(&self) -> &'static str {
        "Re-read the workspace from disk (rebuilds index)"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["reload", "r"]
    }
    fn execute(&self, app: &mut App, _args: &str) -> Result<bool> {
        app.refresh_workspace();
        app.status = "refreshed".into();
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// write — force-save the current page
// ---------------------------------------------------------------------------

pub struct WriteCommand;
impl SlashCommand for WriteCommand {
    fn name(&self) -> &'static str {
        "write"
    }
    fn description(&self) -> &'static str {
        "Save the current page to disk"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["w", "save"]
    }
    fn execute(&self, app: &mut App, _args: &str) -> Result<bool> {
        app.save();
        app.status = "saved".into();
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// help — toggle the help popup
// ---------------------------------------------------------------------------

pub struct HelpCommand;
impl SlashCommand for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }
    fn description(&self) -> &'static str {
        "Toggle the help popup"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["h"]
    }
    fn execute(&self, app: &mut App, _args: &str) -> Result<bool> {
        app.show_help = true;
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// quit — close the TUI
// ---------------------------------------------------------------------------

pub struct QuitCommand;
impl SlashCommand for QuitCommand {
    fn name(&self) -> &'static str {
        "quit"
    }
    fn description(&self) -> &'static str {
        "Close the TUI (commits any pending Insert first)"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["q", "exit"]
    }
    fn execute(&self, _app: &mut App, _args: &str) -> Result<bool> {
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// open — open (or create) a page by title
// ---------------------------------------------------------------------------

pub struct OpenCommand;
impl SlashCommand for OpenCommand {
    fn name(&self) -> &'static str {
        "open"
    }
    fn description(&self) -> &'static str {
        "Open (or create) a page by name — `open <name>`"
    }
    fn needs_args(&self) -> bool {
        true
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["o", "new", "n"]
    }
    fn execute(&self, app: &mut App, args: &str) -> Result<bool> {
        if args.is_empty() {
            app.status = "usage: open <page name>".into();
            return Ok(false);
        }
        app.open_page_by_name(args)?;
        Ok(false)
    }
}

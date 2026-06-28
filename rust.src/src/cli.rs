//! CLI entry point and main loop (RFC-0001).
//!
//! Parses command-line arguments, loads configuration, opens the document and
//! drives the `Input -> Buffer -> Viewport -> Renderer` loop. Hotkeys: Ctrl+S
//! save, Ctrl+Q quit (with an unsaved-changes confirmation), Ctrl+O settings,
//! `?` help. Settings priority is CLI flags over the config file. The terminal
//! is set up and torn down via an RAII guard plus a panic hook so it is always
//! restored, even on panic.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use crossterm::cursor::{Hide, Show};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
};
use crossterm::{ExecutableCommand, execute};

use crate::buffer::Buffer;
use crate::config::{self, CursorOnOpen};
use crate::file::Document;
use crate::input::{self, Action, Direction, Event};
use crate::renderer::{Frame, Renderer};
use crate::viewport;

/// Minimalist focus editor.
#[derive(Parser, Debug)]
#[command(name = "be", about = "Minimalist focus editor")]
struct Args {
    /// File to open (created if it does not exist).
    path: PathBuf,
    /// Number of context lines shown above and below the active line.
    #[arg(long)]
    lines: Option<usize>,
    /// Open the file for viewing only; editing and saving are disabled.
    #[arg(long)]
    readonly: bool,
}

/// Loop control after handling an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// Active editor screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Edit,
    Help,
    Settings,
}

/// Editor state driving the main loop.
pub struct Editor {
    doc: Document,
    lines_before: usize,
    lines_after: usize,
    text_width: usize,
    cursor_on_open: CursorOnOpen,
    message: Option<String>,
    pending_quit: bool,
    mode: Mode,
    settings_field: usize,
    /// Last rendered text height (console rows minus the status line); used as
    /// the PageUp/PageDown step.
    viewport_height: usize,
}

impl Editor {
    /// Creates an editor, placing the cursor according to `cursor_on_open`.
    pub fn new(
        mut doc: Document,
        lines_before: usize,
        lines_after: usize,
        text_width: usize,
        cursor_on_open: CursorOnOpen,
    ) -> Self {
        match cursor_on_open {
            CursorOnOpen::Start => doc.buffer_mut().set_cursor(0, 0),
            CursorOnOpen::End => {
                let last = doc.buffer().line_count() - 1;
                doc.buffer_mut().set_cursor(last, usize::MAX);
            }
        }
        Self {
            doc,
            lines_before,
            lines_after,
            text_width,
            cursor_on_open,
            message: None,
            pending_quit: false,
            mode: Mode::Edit,
            settings_field: 0,
            viewport_height: 1,
        }
    }

    /// Sets a transient status message shown on the next frame.
    pub fn set_message(&mut self, message: String) {
        self.message = Some(message);
    }

    /// Applies an editing operation unless the document is readonly.
    fn edit(&mut self, op: impl FnOnce(&mut Buffer)) {
        if self.doc.is_readonly() {
            self.message = Some("cannot edit in readonly mode".to_string());
            return;
        }
        op(self.doc.buffer_mut());
    }

    /// Saves the document, reporting the outcome in the status message.
    fn save(&mut self) {
        match self.doc.save() {
            Ok(()) => self.message = Some("saved".to_string()),
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    /// Handles a quit request, confirming when there are unsaved changes.
    fn quit(&mut self) -> Flow {
        if self.doc.buffer().is_modified() {
            if self.pending_quit {
                return Flow::Quit;
            }
            self.pending_quit = true;
            self.message = Some("Unsaved changes! Press Ctrl+Q again to quit".to_string());
            Flow::Continue
        } else {
            Flow::Quit
        }
    }

    /// Processes a single normalized input event.
    pub fn handle(&mut self, event: Event) -> Flow {
        match self.mode {
            Mode::Edit => self.handle_edit(event),
            Mode::Help => {
                self.handle_help(event);
                Flow::Continue
            }
            Mode::Settings => {
                self.handle_settings(event);
                Flow::Continue
            }
        }
    }

    /// Handles input in the main editing mode.
    fn handle_edit(&mut self, event: Event) -> Flow {
        // A non-quit event cancels a pending quit confirmation.
        if !matches!(event, Event::Action(Action::Quit)) {
            self.pending_quit = false;
        }
        self.message = None;

        match event {
            Event::Insert(c) => self.edit(|b| b.insert_char(c)),
            Event::Newline => self.edit(|b| b.insert_newline()),
            Event::Backspace => self.edit(|b| b.backspace()),
            Event::Delete => self.edit(|b| b.delete()),
            Event::Move(dir) => {
                let page = self.viewport_height.max(1);
                let ctx_up = self.lines_before.max(1);
                let ctx_down = self.lines_after.max(1);
                let b = self.doc.buffer_mut();
                match dir {
                    Direction::Up => b.move_up(),
                    Direction::Down => b.move_down(),
                    Direction::Left => b.move_left(),
                    Direction::Right => b.move_right(),
                    Direction::LineStart => b.move_line_start(),
                    Direction::LineEnd => b.move_line_end(),
                    Direction::PageUp => b.move_up_by(page),
                    Direction::PageDown => b.move_down_by(page),
                    Direction::PageContextUp => b.move_up_by(ctx_up),
                    Direction::PageContextDown => b.move_down_by(ctx_down),
                    Direction::FileStart => b.move_file_start(),
                    Direction::FileEnd => b.move_file_end(),
                }
            }
            Event::Action(Action::Save) => self.save(),
            Event::Action(Action::Quit) => return self.quit(),
            Event::Action(Action::Help) => self.mode = Mode::Help,
            Event::Action(Action::OpenSettings) => self.mode = Mode::Settings,
            Event::Escape => {}
            Event::Resize(_, _) => {}
            Event::Unknown => {}
        }
        Flow::Continue
    }

    /// Handles input while the help overlay is open; Esc or `?` closes it.
    fn handle_help(&mut self, event: Event) {
        if matches!(event, Event::Escape | Event::Action(Action::Help)) {
            self.mode = Mode::Edit;
        }
    }

    /// Handles input in the settings panel.
    ///
    /// Up/Down select a field, Left/Right change it, Esc or Ctrl+O closes.
    /// Changes apply to the current session only.
    fn handle_settings(&mut self, event: Event) {
        match event {
            Event::Escape | Event::Action(Action::OpenSettings) => self.mode = Mode::Edit,
            Event::Move(Direction::Up) => {
                self.settings_field = self.settings_field.saturating_sub(1);
            }
            Event::Move(Direction::Down) => {
                self.settings_field = (self.settings_field + 1).min(3);
            }
            Event::Move(Direction::Left) => self.adjust_setting(false),
            Event::Move(Direction::Right) => self.adjust_setting(true),
            _ => {}
        }
    }

    /// Increments (`up`) or decrements the currently selected setting.
    fn adjust_setting(&mut self, up: bool) {
        match self.settings_field {
            0 => {
                self.lines_before = if up {
                    self.lines_before + 1
                } else {
                    self.lines_before.saturating_sub(1)
                };
            }
            1 => {
                self.lines_after = if up {
                    self.lines_after + 1
                } else {
                    self.lines_after.saturating_sub(1)
                };
            }
            2 => {
                self.text_width = if up {
                    self.text_width + 1
                } else {
                    self.text_width.saturating_sub(1).max(1)
                };
            }
            _ => {
                self.cursor_on_open = match self.cursor_on_open {
                    CursorOnOpen::Start => CursorOnOpen::End,
                    CursorOnOpen::End => CursorOnOpen::Start,
                };
            }
        }
    }

    /// Builds the help overlay content lines.
    fn help_lines(&self) -> Vec<String> {
        let mut lines = vec![
            "be — help".to_string(),
            String::new(),
            "  Ctrl+S      Save".to_string(),
            "  Ctrl+Q      Quit (confirm if unsaved)".to_string(),
            "  Ctrl+O      Settings".to_string(),
            "  Ctrl+H/?    Toggle this help".to_string(),
            "  Arrows      Move cursor".to_string(),
            "  Home/End    Start/end of line".to_string(),
            "  Ctrl+Home   Start of file".to_string(),
            "  Ctrl+End    End of file".to_string(),
            "  PgUp/PgDn   Scroll by one page".to_string(),
            "  Ctrl+PgUp/Dn  Scroll by context height".to_string(),
            "  Enter       Insert line break".to_string(),
            "  Backspace   Delete left".to_string(),
            "  Delete      Delete right".to_string(),
        ];
        if self.doc.is_readonly() {
            lines.push(String::new());
            lines.push("  [readonly] viewing only".to_string());
        }
        lines
    }

    /// Builds the settings panel content lines, marking the selected field.
    fn settings_lines(&self) -> Vec<String> {
        let cursor = match self.cursor_on_open {
            CursorOnOpen::Start => "start",
            CursorOnOpen::End => "end",
        };
        let fields = [
            format!("lines_before: {}", self.lines_before),
            format!("lines_after: {}", self.lines_after),
            format!("text_width: {}", self.text_width),
            format!("cursor_on_open: {cursor}"),
        ];
        let mut lines = vec!["be — settings".to_string(), String::new()];
        for (i, field) in fields.iter().enumerate() {
            let marker = if i == self.settings_field { ">" } else { " " };
            lines.push(format!("{marker} {field}"));
        }
        lines
    }

    /// Builds the current frame and draws it.
    fn render<W: Write>(
        &mut self,
        renderer: &mut Renderer<W>,
        width: u16,
        height: u16,
    ) -> io::Result<()> {
        self.viewport_height = height.saturating_sub(1) as usize;
        match self.mode {
            Mode::Help => {
                return renderer.render_overlay(
                    width,
                    height,
                    &self.help_lines(),
                    "Esc or ? to close",
                );
            }
            Mode::Settings => {
                return renderer.render_overlay(
                    width,
                    height,
                    &self.settings_lines(),
                    "Up/Down select · Left/Right change · Esc to close",
                );
            }
            Mode::Edit => {}
        }

        let cursor = self.doc.buffer().cursor();
        let content_height = height.saturating_sub(1) as usize;
        let screen = viewport::build(
            self.doc.buffer().lines(),
            cursor.line,
            cursor.col,
            content_height,
            self.text_width,
            self.lines_before,
            self.lines_after,
        );
        let file_name = self.doc.file_name();
        let frame = Frame {
            term_width: width,
            height,
            text_width: self.text_width as u16,
            screen: &screen,
            file_name: &file_name,
            modified: self.doc.buffer().is_modified(),
            readonly: self.doc.is_readonly(),
            cursor_line: cursor.line,
            cursor_col: cursor.col,
            message: self.message.as_deref(),
        };
        renderer.render(&frame)
    }
}

/// RAII terminal session: raw mode + alternate screen + hidden cursor.
struct TerminalSession;

impl TerminalSession {
    /// Enters raw mode and the alternate screen, hiding the cursor.
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut out = io::stdout();
        out.execute(EnterAlternateScreen)?;
        out.execute(Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = out.execute(Show);
        let _ = out.execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Installs a panic hook that restores the terminal before reporting a panic.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut out = io::stdout();
        let _ = execute!(out, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        previous(info);
    }));
}

/// Runs the editor: setup, main loop, teardown.
fn run() -> io::Result<()> {
    let args = Args::parse();

    let (cfg, warnings) = config::load_or_create();
    let (lines_before, lines_after) = match args.lines {
        Some(n) => (n, n),
        None => (cfg.lines_before, cfg.lines_after),
    };

    let doc = match Document::open(&args.path, args.readonly) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("be: {e}");
            std::process::exit(1);
        }
    };

    let mut editor = Editor::new(
        doc,
        lines_before,
        lines_after,
        cfg.text_width,
        cfg.cursor_on_open,
    );
    if !warnings.is_empty() {
        editor.set_message(warnings.join("; "));
    }

    install_panic_hook();
    let _session = TerminalSession::enter()?;
    let mut renderer = Renderer::new(io::stdout());

    loop {
        let (width, height) = size()?;
        editor.render(&mut renderer, width, height)?;
        match editor.handle(input::read_event()?) {
            Flow::Continue => {}
            Flow::Quit => break,
        }
    }

    Ok(())
}

/// Process entry point.
pub fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("be: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Builds an editor over a fresh temp file containing `text`.
    fn editor_with(text: &str, readonly: bool) -> (Editor, PathBuf) {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("be_cli_{}_{}", std::process::id(), n));
        fs::write(&path, text).unwrap();
        let doc = Document::open(&path, readonly).unwrap();
        (Editor::new(doc, 3, 3, 80, CursorOnOpen::Start), path)
    }

    #[test]
    fn insert_marks_modified() {
        let (mut ed, path) = editor_with("", false);
        ed.handle(Event::Insert('x'));
        assert!(ed.doc.buffer().is_modified());
        assert_eq!(ed.doc.buffer().lines(), &["x"]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn readonly_blocks_edit_with_message() {
        let (mut ed, path) = editor_with("abc", true);
        ed.handle(Event::Insert('x'));
        assert!(!ed.doc.buffer().is_modified());
        assert_eq!(ed.message.as_deref(), Some("cannot edit in readonly mode"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_clears_modified_and_reports() {
        let (mut ed, path) = editor_with("", false);
        ed.handle(Event::Insert('h'));
        ed.handle(Event::Action(Action::Save));
        assert!(!ed.doc.buffer().is_modified());
        assert_eq!(ed.message.as_deref(), Some("saved"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "h");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn quit_when_clean_quits_immediately() {
        let (mut ed, path) = editor_with("abc", false);
        assert_eq!(ed.handle(Event::Action(Action::Quit)), Flow::Quit);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn quit_when_modified_requires_confirmation() {
        let (mut ed, path) = editor_with("", false);
        ed.handle(Event::Insert('x'));
        assert_eq!(ed.handle(Event::Action(Action::Quit)), Flow::Continue);
        assert!(ed.message.is_some());
        assert_eq!(ed.handle(Event::Action(Action::Quit)), Flow::Quit);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn other_event_cancels_pending_quit() {
        let (mut ed, path) = editor_with("", false);
        ed.handle(Event::Insert('x'));
        assert_eq!(ed.handle(Event::Action(Action::Quit)), Flow::Continue);
        // A movement cancels the pending quit; next quit confirms again.
        ed.handle(Event::Move(Direction::Left));
        assert_eq!(ed.handle(Event::Action(Action::Quit)), Flow::Continue);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn movement_updates_cursor() {
        let (mut ed, path) = editor_with("ab\ncd", false);
        assert_eq!(ed.doc.buffer().cursor().line, 0);
        ed.handle(Event::Move(Direction::Down));
        assert_eq!(ed.doc.buffer().cursor().line, 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn cursor_on_open_end_places_cursor_at_end() {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("be_cli_end_{}_{}", std::process::id(), n));
        fs::write(&path, "ab\ncde").unwrap();
        let doc = Document::open(&path, false).unwrap();
        let ed = Editor::new(doc, 3, 3, 80, CursorOnOpen::End);
        let c = ed.doc.buffer().cursor();
        assert_eq!(c.line, 1);
        assert_eq!(c.col, 3);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn help_toggles_with_question_mark() {
        let (mut ed, path) = editor_with("abc", false);
        assert_eq!(ed.mode, Mode::Edit);
        ed.handle(Event::Action(Action::Help));
        assert_eq!(ed.mode, Mode::Help);
        ed.handle(Event::Action(Action::Help));
        assert_eq!(ed.mode, Mode::Edit);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn help_closes_on_escape_and_ignores_edits() {
        let (mut ed, path) = editor_with("abc", false);
        ed.handle(Event::Action(Action::Help));
        ed.handle(Event::Insert('x'));
        assert!(!ed.doc.buffer().is_modified());
        ed.handle(Event::Escape);
        assert_eq!(ed.mode, Mode::Edit);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn help_shows_readonly_marker() {
        let (ed, path) = editor_with("abc", true);
        let lines = ed.help_lines();
        assert!(lines.iter().any(|l| l.contains("[readonly]")));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn settings_open_navigate_adjust_and_close() {
        let (mut ed, path) = editor_with("abc", false);
        ed.handle(Event::Action(Action::OpenSettings));
        assert_eq!(ed.mode, Mode::Settings);
        assert_eq!(ed.settings_field, 0);

        // Increase lines_before on field 0.
        ed.handle(Event::Move(Direction::Right));
        assert_eq!(ed.lines_before, 4);
        ed.handle(Event::Move(Direction::Left));
        assert_eq!(ed.lines_before, 3);

        // Move to field 1 and adjust lines_after.
        ed.handle(Event::Move(Direction::Down));
        assert_eq!(ed.settings_field, 1);
        ed.handle(Event::Move(Direction::Right));
        assert_eq!(ed.lines_after, 4);

        // Move to field 2 and adjust text_width.
        ed.handle(Event::Move(Direction::Down));
        assert_eq!(ed.settings_field, 2);
        assert_eq!(ed.text_width, 80);
        ed.handle(Event::Move(Direction::Right));
        assert_eq!(ed.text_width, 81);

        // Move to field 3 and toggle cursor_on_open.
        ed.handle(Event::Move(Direction::Down));
        assert_eq!(ed.settings_field, 3);
        assert_eq!(ed.cursor_on_open, CursorOnOpen::Start);
        ed.handle(Event::Move(Direction::Right));
        assert_eq!(ed.cursor_on_open, CursorOnOpen::End);

        ed.handle(Event::Action(Action::OpenSettings));
        assert_eq!(ed.mode, Mode::Edit);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn settings_lines_marks_selected_field() {
        let (mut ed, path) = editor_with("abc", false);
        ed.handle(Event::Action(Action::OpenSettings));
        ed.handle(Event::Move(Direction::Down));
        let lines = ed.settings_lines();
        assert!(lines.iter().any(|l| l.starts_with("> lines_after")));
        let _ = fs::remove_file(path);
    }
}

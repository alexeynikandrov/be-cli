//! Terminal renderer and status line (RFC-0005).
//!
//! The renderer draws the focus fragment described by a [`Layout`]: the active
//! line is shown brightest, context lines dimmed and padding rows blank. The
//! last terminal row is reserved for a status line of the fixed form
//! `[filename] [saved|modified]` on the left and `Ln <l>, Col <c>` on the
//! right; when width is tight the filename is truncated while the position
//! status is preserved. Drawing is batched via crossterm's command queue and
//! flushed once per frame to minimize flicker.

use std::io::{self, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{QueueableCommand, queue};

use crate::viewport::{RowKind, Screen};

/// All view data required to draw a single frame.
pub struct Frame<'a> {
    /// Full terminal width.
    pub term_width: u16,
    pub height: u16,
    /// Desired text column width; the column is centered within the terminal.
    pub text_width: u16,
    pub screen: &'a Screen,
    pub file_name: &'a str,
    pub modified: bool,
    pub readonly: bool,
    /// Whether autosave is enabled (shown as a status-line marker).
    pub autosave: bool,
    /// Zero-based cursor line (for the status line).
    pub cursor_line: usize,
    /// Zero-based cursor column (for the status line).
    pub cursor_col: usize,
    /// Transient message shown on the status row instead of the normal status.
    pub message: Option<&'a str>,
}

/// Truncates `s` to at most `max` characters (character-based, UTF-8 safe).
fn take_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Builds the status line, exactly `width` characters wide.
///
/// Layout: `left` (`[name] [state]` plus optional `[autosave]` and `[readonly]`
/// markers) is left-aligned, the position `Ln L, Col C` is right-aligned, and
/// spaces fill the gap. When the line does not fit, the left part is truncated
/// (with an ellipsis) so the right-aligned position remains visible.
/// `cursor_line` and `cursor_col` are zero-based and shown 1-based.
pub fn status_line(
    width: usize,
    file_name: &str,
    modified: bool,
    readonly: bool,
    autosave: bool,
    cursor_line: usize,
    cursor_col: usize,
) -> String {
    if width == 0 {
        return String::new();
    }

    let state = if modified { "modified" } else { "saved" };
    let mut left = format!("[{file_name}] [{state}]");
    if autosave {
        left.push_str(" [autosave]");
    }
    if readonly {
        left.push_str(" [readonly]");
    }
    let right = format!("Ln {}, Col {}", cursor_line + 1, cursor_col + 1);
    let hint = "Ctrl+H Help";

    let left_len = left.chars().count();

    // Prefer showing the help hint to the left of the position when it fits;
    // otherwise keep just the position so it stays visible on narrow lines.
    let right_with_hint = format!("{hint}  {right}");
    let right = if left_len + 1 + right_with_hint.chars().count() <= width {
        right_with_hint
    } else {
        right
    };

    let right_len = right.chars().count();

    // The position status is wider than the whole line: show its right side.
    if right_len >= width {
        let skip = right_len - width;
        return right.chars().skip(skip).collect();
    }

    // Both parts plus a separating space fit: pad the middle.
    if left_len + 1 + right_len <= width {
        let pad = width - left_len - right_len;
        let mut out = left;
        out.extend(std::iter::repeat_n(' ', pad));
        out.push_str(&right);
        return out;
    }

    // Truncate the left part, keeping one space before the position status.
    let avail = width - right_len - 1;
    let mut left_trunc = take_chars(&left, avail);
    if avail >= 1 && left_trunc.chars().count() == avail {
        // Mark truncation with an ellipsis in the last visible cell.
        left_trunc = take_chars(&left_trunc, avail - 1);
        left_trunc.push('…');
    }
    let mut out = left_trunc;
    out.push(' ');
    out.push_str(&right);
    out
}

/// Fits a transient message to exactly `width` characters (truncate or pad).
pub fn fit_message(width: usize, message: &str) -> String {
    if width == 0 {
        return String::new();
    }
    let len = message.chars().count();
    if len >= width {
        take_chars(message, width)
    } else {
        let mut out = message.to_string();
        out.extend(std::iter::repeat_n(' ', width - len));
        out
    }
}

/// Renders frames to a writer (typically stdout) using crossterm.
pub struct Renderer<W: Write> {
    out: W,
}

impl<W: Write> Renderer<W> {
    /// Creates a renderer writing to `out`.
    pub fn new(out: W) -> Self {
        Self { out }
    }

    /// Enters the alternate screen and hides the cursor.
    pub fn enter_screen(&mut self) -> io::Result<()> {
        self.out.queue(EnterAlternateScreen)?;
        self.out.queue(Hide)?;
        self.out.flush()
    }

    /// Shows the cursor and leaves the alternate screen.
    pub fn leave_screen(&mut self) -> io::Result<()> {
        self.out.queue(Show)?;
        self.out.queue(LeaveAlternateScreen)?;
        self.out.flush()
    }

    /// Prints `text` truncated to `width` characters at column `margin`.
    fn print_at(&mut self, margin: u16, row: u16, text: &str, width: u16) -> io::Result<()> {
        queue!(self.out, MoveTo(margin, row))?;
        self.out.queue(Print(take_chars(text, width as usize)))?;
        Ok(())
    }

    /// Draws one frame and flushes it.
    ///
    /// The text column is `text_width` wide (clamped to the terminal) and
    /// horizontally centered; the status line is aligned to the same column.
    pub fn render(&mut self, frame: &Frame) -> io::Result<()> {
        queue!(self.out, Hide)?;

        let col_w = frame.text_width.min(frame.term_width).max(1);
        let margin = frame.term_width.saturating_sub(col_w) / 2;

        let text_rows = frame.height.saturating_sub(1) as usize;
        for (i, cell) in frame.screen.rows.iter().enumerate().take(text_rows) {
            queue!(self.out, MoveTo(0, i as u16), Clear(ClearType::CurrentLine))?;
            match cell.kind {
                RowKind::Active => {
                    queue!(self.out, SetForegroundColor(Color::White))?;
                    self.print_at(margin, i as u16, &cell.text, col_w)?;
                    queue!(self.out, ResetColor)?;
                }
                RowKind::Context => {
                    queue!(self.out, SetForegroundColor(Color::DarkGrey))?;
                    self.print_at(margin, i as u16, &cell.text, col_w)?;
                    queue!(self.out, ResetColor)?;
                }
                RowKind::Padding => {}
            }
        }

        // The status line spans the full terminal width at the left edge, so
        // feedback (e.g. "saved") stays where the user expects regardless of
        // the centered text column.
        let status = match frame.message {
            Some(msg) => fit_message(frame.term_width as usize, msg),
            None => status_line(
                frame.term_width as usize,
                frame.file_name,
                frame.modified,
                frame.readonly,
                frame.autosave,
                frame.cursor_line,
                frame.cursor_col,
            ),
        };
        queue!(
            self.out,
            MoveTo(0, frame.height.saturating_sub(1)),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(status),
            ResetColor
        )?;

        let cursor_x = (margin
            + frame
                .screen
                .cursor_col
                .min(col_w.saturating_sub(1) as usize) as u16)
            .min(frame.term_width.saturating_sub(1));
        let cursor_y = frame.screen.cursor_row as u16;
        queue!(self.out, MoveTo(cursor_x, cursor_y), Show)?;

        self.out.flush()
    }

    /// Draws a full-screen overlay (help or settings) and flushes it.
    ///
    /// `content` lines are printed from the top (truncated to width); the last
    /// row shows a reversed `footer`. The cursor stays hidden.
    pub fn render_overlay(
        &mut self,
        width: u16,
        height: u16,
        content: &[String],
        footer: &str,
    ) -> io::Result<()> {
        queue!(self.out, Hide)?;
        let text_rows = height.saturating_sub(1) as usize;
        for r in 0..text_rows {
            queue!(self.out, MoveTo(0, r as u16), Clear(ClearType::CurrentLine))?;
            if let Some(line) = content.get(r) {
                self.print_at(0, r as u16, line, width)?;
            }
        }
        queue!(
            self.out,
            MoveTo(0, height.saturating_sub(1)),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(fit_message(width as usize, footer)),
            ResetColor
        )?;
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_shows_help_hint_when_wide() {
        let s = status_line(60, "a.txt", false, false, false, 0, 0);
        assert!(s.contains("Ctrl+H Help"));
        assert!(s.ends_with("Ln 1, Col 1"));
    }

    #[test]
    fn status_zero_width_is_empty() {
        assert_eq!(status_line(0, "a.txt", false, false, false, 0, 0), "");
    }

    #[test]
    fn status_has_exact_width() {
        let s = status_line(40, "notes.txt", true, false, false, 11, 7);
        assert_eq!(s.chars().count(), 40);
    }

    #[test]
    fn status_shows_left_and_right_parts() {
        let s = status_line(40, "notes.txt", true, false, false, 11, 7);
        assert!(s.starts_with("[notes.txt] [modified]"));
        assert!(s.ends_with("Ln 12, Col 8"));
    }

    #[test]
    fn status_saved_state() {
        let s = status_line(40, "a.txt", false, false, false, 0, 0);
        assert!(s.starts_with("[a.txt] [saved]"));
        assert!(s.ends_with("Ln 1, Col 1"));
    }

    #[test]
    fn status_autosave_marker() {
        let s = status_line(60, "a.txt", false, false, true, 0, 0);
        assert!(s.contains("[autosave]"));
    }

    #[test]
    fn status_readonly_marker() {
        let s = status_line(50, "a.txt", false, true, false, 0, 0);
        assert!(s.contains("[readonly]"));
    }

    #[test]
    fn status_truncates_filename_keeping_position() {
        let s = status_line(20, "a-very-long-file-name.txt", false, false, false, 4, 2);
        assert_eq!(s.chars().count(), 20);
        assert!(s.ends_with("Ln 5, Col 3"));
        assert!(s.contains('…'));
    }

    #[test]
    fn status_position_preserved_when_extremely_narrow() {
        let right = "Ln 1, Col 1";
        let s = status_line(5, "name.txt", false, false, false, 0, 0);
        assert_eq!(s.chars().count(), 5);
        // Shows the right side of the position status.
        assert!(right.ends_with(&s));
    }

    #[test]
    fn take_chars_is_utf8_safe() {
        assert_eq!(take_chars("привет", 3), "при");
        assert_eq!(take_chars("ab", 5), "ab");
    }

    #[test]
    fn fit_message_pads_and_truncates() {
        assert_eq!(fit_message(0, "hi"), "");
        assert_eq!(fit_message(5, "hi").chars().count(), 5);
        assert!(fit_message(5, "hi").starts_with("hi"));
        assert_eq!(fit_message(3, "hello"), "hel");
    }
}

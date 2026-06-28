//! Text buffer and cursor model (RFC-0002).
//!
//! The buffer stores text as a list of lines in memory. The cursor holds a
//! `line` and `col` position, where `col` is a character index (UTF-8 safe).
//! Every editing or movement operation keeps the cursor valid:
//! `line` stays in `0..line_count` and `col` stays in `0..=current_line_len`.

// Consumed by later phases (viewport, renderer, cli); allow until wired in.
#![allow(dead_code)]

/// Cursor position within the buffer.
///
/// `line` is a zero-based line index. `col` is a zero-based character index
/// (not a byte offset), so multibyte characters count as a single column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

/// In-memory text buffer with a single cursor and a `modified` flag.
#[derive(Debug, Clone)]
pub struct Buffer {
    lines: Vec<String>,
    cursor: Cursor,
    modified: bool,
}

/// Returns the number of characters in `line`.
fn char_len(line: &str) -> usize {
    line.chars().count()
}

/// Converts a character index into a byte offset within `line`.
///
/// If `col` is beyond the end of the line, the line's byte length is returned.
fn byte_offset(line: &str, col: usize) -> usize {
    line.char_indices()
        .nth(col)
        .map(|(b, _)| b)
        .unwrap_or(line.len())
}

impl Buffer {
    /// Creates a buffer from `text`, splitting on `\n`.
    ///
    /// An empty input yields a single empty line so the buffer always has at
    /// least one line. A trailing newline does not create an extra empty line.
    /// The cursor starts at the beginning and `modified` is `false`.
    pub fn new(text: &str) -> Self {
        let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
        // `split` on a string ending with '\n' yields a trailing empty element;
        // drop it so a saved file round-trips without growing.
        if lines.len() > 1 && lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            cursor: Cursor { line: 0, col: 0 },
            modified: false,
        }
    }

    /// Returns the buffer lines.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Returns the number of lines (always >= 1).
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Returns whether the buffer has unsaved changes.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Marks the buffer as saved (clears the `modified` flag).
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// Serializes the buffer back into a single string joined by `\n`.
    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Character length of the line at the given index (0 if out of range).
    fn line_len(&self, line: usize) -> usize {
        self.lines.get(line).map(|l| char_len(l)).unwrap_or(0)
    }

    /// Re-establishes cursor invariants after a structural change.
    fn clamp_cursor(&mut self) {
        if self.cursor.line >= self.lines.len() {
            self.cursor.line = self.lines.len() - 1;
        }
        let max_col = self.line_len(self.cursor.line);
        if self.cursor.col > max_col {
            self.cursor.col = max_col;
        }
    }

    /// Moves the cursor to `(line, col)`, clamping both into valid ranges.
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        let line = line.min(self.lines.len() - 1);
        let col = col.min(self.line_len(line));
        self.cursor = Cursor { line, col };
    }

    /// Inserts a printable character at the cursor and advances past it.
    ///
    /// A `\n` is treated as a newline insertion.
    pub fn insert_char(&mut self, ch: char) {
        if ch == '\n' {
            self.insert_newline();
            return;
        }
        let line = &mut self.lines[self.cursor.line];
        let at = byte_offset(line, self.cursor.col);
        line.insert(at, ch);
        self.cursor.col += 1;
        self.modified = true;
    }

    /// Splits the current line at the cursor, moving the remainder to a new
    /// line below. The cursor moves to the start of that new line.
    pub fn insert_newline(&mut self) {
        let at = byte_offset(&self.lines[self.cursor.line], self.cursor.col);
        let remainder = self.lines[self.cursor.line].split_off(at);
        self.lines.insert(self.cursor.line + 1, remainder);
        self.cursor.line += 1;
        self.cursor.col = 0;
        self.modified = true;
    }

    /// Deletes the character before the cursor.
    ///
    /// At the start of a line (but not the first line), joins the current line
    /// onto the previous one. At the very start of the buffer this is a no-op.
    pub fn backspace(&mut self) {
        if self.cursor.col > 0 {
            let line = &mut self.lines[self.cursor.line];
            let start = byte_offset(line, self.cursor.col - 1);
            let end = byte_offset(line, self.cursor.col);
            line.replace_range(start..end, "");
            self.cursor.col -= 1;
            self.modified = true;
        } else if self.cursor.line > 0 {
            let current = self.lines.remove(self.cursor.line);
            let prev = self.cursor.line - 1;
            let prev_len = char_len(&self.lines[prev]);
            self.lines[prev].push_str(&current);
            self.cursor.line = prev;
            self.cursor.col = prev_len;
            self.modified = true;
        }
    }

    /// Deletes the character at the cursor.
    ///
    /// At the end of a line (but not the last line), joins the next line onto
    /// the current one. At the very end of the buffer this is a no-op.
    pub fn delete(&mut self) {
        let len = self.line_len(self.cursor.line);
        if self.cursor.col < len {
            let line = &mut self.lines[self.cursor.line];
            let start = byte_offset(line, self.cursor.col);
            let end = byte_offset(line, self.cursor.col + 1);
            line.replace_range(start..end, "");
            self.modified = true;
        } else if self.cursor.line + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor.line + 1);
            self.lines[self.cursor.line].push_str(&next);
            self.modified = true;
        }
    }

    /// Moves the cursor one character left, wrapping to the end of the previous
    /// line at the start of a line.
    pub fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.line_len(self.cursor.line);
        }
    }

    /// Moves the cursor one character right, wrapping to the start of the next
    /// line at the end of a line.
    pub fn move_right(&mut self) {
        let len = self.line_len(self.cursor.line);
        if self.cursor.col < len {
            self.cursor.col += 1;
        } else if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
    }

    /// Moves the cursor up one line, clamping the column to the new line length.
    pub fn move_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.clamp_cursor();
        }
    }

    /// Moves the cursor down one line, clamping the column to the new line length.
    pub fn move_down(&mut self) {
        if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.clamp_cursor();
        }
    }

    /// Moves the cursor to the start of the current line.
    pub fn move_line_start(&mut self) {
        self.cursor.col = 0;
    }

    /// Moves the cursor to the end of the current line.
    pub fn move_line_end(&mut self) {
        self.cursor.col = self.line_len(self.cursor.line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_empty_has_single_line() {
        let b = Buffer::new("");
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.lines(), &[""]);
        assert_eq!(b.cursor(), Cursor { line: 0, col: 0 });
        assert!(!b.is_modified());
    }

    #[test]
    fn new_splits_lines_without_trailing_empty() {
        let b = Buffer::new("a\nb\n");
        assert_eq!(b.lines(), &["a", "b"]);
        assert_eq!(b.to_text(), "a\nb");
    }

    #[test]
    fn new_keeps_internal_blank_lines() {
        let b = Buffer::new("a\n\nb");
        assert_eq!(b.lines(), &["a", "", "b"]);
    }

    #[test]
    fn insert_char_advances_and_marks_modified() {
        let mut b = Buffer::new("");
        b.insert_char('h');
        b.insert_char('i');
        assert_eq!(b.lines(), &["hi"]);
        assert_eq!(b.cursor(), Cursor { line: 0, col: 2 });
        assert!(b.is_modified());
    }

    #[test]
    fn insert_char_is_utf8_safe() {
        let mut b = Buffer::new("");
        b.insert_char('п');
        b.insert_char('р');
        b.insert_char('и');
        b.set_cursor(0, 1);
        b.insert_char('X');
        assert_eq!(b.lines(), &["пXри"]);
        assert_eq!(b.cursor().col, 2);
    }

    #[test]
    fn insert_newline_splits_line() {
        let mut b = Buffer::new("hello");
        b.set_cursor(0, 2);
        b.insert_newline();
        assert_eq!(b.lines(), &["he", "llo"]);
        assert_eq!(b.cursor(), Cursor { line: 1, col: 0 });
    }

    #[test]
    fn backspace_removes_char() {
        let mut b = Buffer::new("abc");
        b.set_cursor(0, 2);
        b.backspace();
        assert_eq!(b.lines(), &["ac"]);
        assert_eq!(b.cursor(), Cursor { line: 0, col: 1 });
    }

    #[test]
    fn backspace_at_line_start_joins_previous() {
        let mut b = Buffer::new("ab\ncd");
        b.set_cursor(1, 0);
        b.backspace();
        assert_eq!(b.lines(), &["abcd"]);
        assert_eq!(b.cursor(), Cursor { line: 0, col: 2 });
    }

    #[test]
    fn backspace_at_buffer_start_is_noop() {
        let mut b = Buffer::new("abc");
        b.backspace();
        assert_eq!(b.lines(), &["abc"]);
        assert_eq!(b.cursor(), Cursor { line: 0, col: 0 });
        assert!(!b.is_modified());
    }

    #[test]
    fn delete_removes_char_at_cursor() {
        let mut b = Buffer::new("abc");
        b.set_cursor(0, 1);
        b.delete();
        assert_eq!(b.lines(), &["ac"]);
        assert_eq!(b.cursor(), Cursor { line: 0, col: 1 });
    }

    #[test]
    fn delete_at_line_end_joins_next() {
        let mut b = Buffer::new("ab\ncd");
        b.set_cursor(0, 2);
        b.delete();
        assert_eq!(b.lines(), &["abcd"]);
        assert_eq!(b.cursor(), Cursor { line: 0, col: 2 });
    }

    #[test]
    fn delete_at_buffer_end_is_noop() {
        let mut b = Buffer::new("abc");
        b.set_cursor(0, 3);
        b.delete();
        assert_eq!(b.lines(), &["abc"]);
        assert!(!b.is_modified());
    }

    #[test]
    fn horizontal_moves_wrap_across_lines() {
        let mut b = Buffer::new("ab\ncd");
        b.set_cursor(0, 2);
        b.move_right();
        assert_eq!(b.cursor(), Cursor { line: 1, col: 0 });
        b.move_left();
        assert_eq!(b.cursor(), Cursor { line: 0, col: 2 });
    }

    #[test]
    fn moves_are_clamped_at_buffer_edges() {
        let mut b = Buffer::new("ab\ncd");
        b.move_left();
        assert_eq!(b.cursor(), Cursor { line: 0, col: 0 });
        b.set_cursor(1, 2);
        b.move_right();
        assert_eq!(b.cursor(), Cursor { line: 1, col: 2 });
    }

    #[test]
    fn vertical_move_clamps_column() {
        let mut b = Buffer::new("longline\nhi");
        b.set_cursor(0, 8);
        b.move_down();
        assert_eq!(b.cursor(), Cursor { line: 1, col: 2 });
        b.move_up();
        assert_eq!(b.cursor(), Cursor { line: 0, col: 2 });
    }

    #[test]
    fn line_start_and_end_moves() {
        let mut b = Buffer::new("hello\nhi");
        b.set_cursor(0, 3);
        b.move_line_start();
        assert_eq!(b.cursor(), Cursor { line: 0, col: 0 });
        b.move_line_end();
        assert_eq!(b.cursor(), Cursor { line: 0, col: 5 });
        // End on a shorter line lands at that line's length.
        b.set_cursor(1, 0);
        b.move_line_end();
        assert_eq!(b.cursor(), Cursor { line: 1, col: 2 });
    }

    #[test]
    fn set_cursor_clamps_out_of_range() {
        let mut b = Buffer::new("ab\ncd");
        b.set_cursor(99, 99);
        assert_eq!(b.cursor(), Cursor { line: 1, col: 2 });
    }

    #[test]
    fn mark_saved_clears_modified() {
        let mut b = Buffer::new("");
        b.insert_char('x');
        assert!(b.is_modified());
        b.mark_saved();
        assert!(!b.is_modified());
    }
}

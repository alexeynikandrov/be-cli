//! Focus viewport with word wrap and centering (RFC-0004).
//!
//! [`build`] lays out the visible text as a column of [`Cell`]s of a fixed
//! height. Logical lines are word-wrapped to the column width, the active line
//! is rendered brightest, a limited context window (`lines_before`/`after`) is
//! dimmed, and everything else is blank padding. The visual row containing the
//! cursor is centered vertically. Being a pure function, it is simply
//! recomputed on resize or configuration change.
//!
//! The cursor model stays line/column based: wrapping only affects layout, so
//! the cursor is mapped onto the wrapped segment it falls in.

use crate::wrap::wrap_line;

/// How a visual row should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Part of the active (cursor) line, rendered brightest.
    Active,
    /// Part of a context line around the active line, rendered dimmed.
    Context,
    /// Empty filler used for centering and at file edges.
    Padding,
}

/// One visual row of the viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub kind: RowKind,
    pub text: String,
}

impl Cell {
    fn padding() -> Self {
        Self {
            kind: RowKind::Padding,
            text: String::new(),
        }
    }
}

/// Computed viewport: one [`Cell`] per visual text row plus the cursor's
/// position within the text column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub rows: Vec<Cell>,
    /// Row index (within `rows`) where the terminal cursor sits.
    pub cursor_row: usize,
    /// Column (0-based, within the text column) where the cursor sits.
    pub cursor_col: usize,
}

/// Builds the focus viewport.
///
/// * `lines` — buffer lines (at least one).
/// * `cursor_line` / `cursor_col` — zero-based cursor position.
/// * `height` — number of text rows available (excludes the status line).
/// * `width` — text column width to wrap to (at least 1).
/// * `before` / `after` — context window in logical lines.
///
/// The visual row holding the cursor is centered vertically. Rows that map to
/// the active line are [`RowKind::Active`], context lines within the window are
/// [`RowKind::Context`], and everything else is [`RowKind::Padding`].
pub fn build(
    lines: &[String],
    cursor_line: usize,
    cursor_col: usize,
    height: usize,
    width: usize,
    before: usize,
    after: usize,
) -> Screen {
    if height == 0 {
        return Screen {
            rows: Vec::new(),
            cursor_row: 0,
            cursor_col: 0,
        };
    }

    let width = width.max(1);

    // Wrap the active line and locate the cursor's segment.
    let active_segs = wrap_line(&lines[cursor_line], width);
    let mut cursor_seg = active_segs.len() - 1;
    for (idx, seg) in active_segs.iter().enumerate() {
        if cursor_col < seg.end() {
            cursor_seg = idx;
            break;
        }
    }
    let cursor_col_in = (cursor_col - active_segs[cursor_seg].start).min(width);

    // Build the full focus window top-to-bottom, tracking the cursor row.
    let mut seq: Vec<Cell> = Vec::new();

    for k in (1..=before).rev() {
        if let Some(li) = cursor_line.checked_sub(k) {
            for seg in wrap_line(&lines[li], width) {
                seq.push(Cell {
                    kind: RowKind::Context,
                    text: seg.text,
                });
            }
        }
    }

    let active_base = seq.len();
    for seg in &active_segs {
        seq.push(Cell {
            kind: RowKind::Active,
            text: seg.text.clone(),
        });
    }
    let cursor_index = active_base + cursor_seg;

    for k in 1..=after {
        let li = cursor_line + k;
        if li < lines.len() {
            for seg in wrap_line(&lines[li], width) {
                seq.push(Cell {
                    kind: RowKind::Context,
                    text: seg.text,
                });
            }
        }
    }

    // Center the cursor row vertically, clipping/padding as needed.
    let center = (height - 1) / 2;
    let mut rows = Vec::with_capacity(height);
    for r in 0..height {
        let idx = cursor_index as isize + (r as isize - center as isize);
        if idx >= 0 && (idx as usize) < seq.len() {
            rows.push(seq[idx as usize].clone());
        } else {
            rows.push(Cell::padding());
        }
    }

    Screen {
        rows,
        cursor_row: center,
        cursor_col: cursor_col_in,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn kinds(s: &Screen) -> Vec<RowKind> {
        s.rows.iter().map(|c| c.kind).collect()
    }

    fn texts(s: &Screen) -> Vec<&str> {
        s.rows.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn empty_height_yields_no_rows() {
        let s = build(&lines(&["a"]), 0, 0, 0, 40, 2, 2);
        assert!(s.rows.is_empty());
    }

    #[test]
    fn single_short_line_centered() {
        let s = build(&lines(&["hello"]), 0, 0, 5, 40, 2, 2);
        assert_eq!(s.cursor_row, 2);
        assert_eq!(s.cursor_col, 0);
        assert_eq!(kinds(&s)[2], RowKind::Active);
        assert_eq!(texts(&s), vec!["", "", "hello", "", ""]);
    }

    #[test]
    fn cursor_col_maps_within_line() {
        let s = build(&lines(&["hello"]), 0, 3, 5, 40, 2, 2);
        assert_eq!(s.cursor_col, 3);
    }

    #[test]
    fn context_lines_are_dimmed_around_active() {
        let s = build(&lines(&["a", "b", "c", "d", "e"]), 2, 0, 5, 40, 1, 1);
        // window: context "b", active "c", context "d"; centered.
        assert_eq!(texts(&s), vec!["", "b", "c", "d", ""]);
        assert_eq!(
            kinds(&s),
            vec![
                RowKind::Padding,
                RowKind::Context,
                RowKind::Active,
                RowKind::Context,
                RowKind::Padding,
            ]
        );
    }

    #[test]
    fn top_of_file_pads_above() {
        let s = build(&lines(&["a", "b", "c"]), 0, 0, 5, 40, 2, 2);
        assert_eq!(texts(&s), vec!["", "", "a", "b", "c"]);
        assert_eq!(s.cursor_row, 2);
        assert_eq!(s.rows[2].kind, RowKind::Active);
    }

    #[test]
    fn active_line_wraps_into_multiple_rows() {
        // "aaaa bbbb cccc" wrapped at width 5 -> "aaaa ", "bbbb ", "cccc".
        let s = build(&lines(&["aaaa bbbb cccc"]), 0, 0, 7, 5, 1, 1);
        // Cursor at col 0 -> first segment is the cursor row, centered at row 3.
        assert_eq!(s.cursor_row, 3);
        assert_eq!(s.rows[3].text, "aaaa ");
        assert_eq!(s.rows[3].kind, RowKind::Active);
        // The following segments of the same line are also Active.
        assert_eq!(s.rows[4].text, "bbbb ");
        assert_eq!(s.rows[4].kind, RowKind::Active);
        assert_eq!(s.rows[5].text, "cccc");
        assert_eq!(s.rows[5].kind, RowKind::Active);
    }

    #[test]
    fn cursor_in_second_wrapped_segment() {
        // width 5: segments "aaaa " (0..5), "bbbb " (5..10), "cccc" (10..14).
        // cursor_col 6 -> second segment, column 1.
        let s = build(&lines(&["aaaa bbbb cccc"]), 0, 6, 7, 5, 1, 1);
        assert_eq!(s.rows[s.cursor_row].text, "bbbb ");
        assert_eq!(s.cursor_col, 1);
    }

    #[test]
    fn height_one_shows_cursor_row_only() {
        let s = build(&lines(&["a", "b", "c"]), 1, 0, 1, 40, 2, 2);
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0].text, "b");
        assert_eq!(s.cursor_row, 0);
    }

    #[test]
    fn context_beyond_window_is_padding() {
        // before/after 1, so only neighbors shown; far lines are padding.
        let s = build(&lines(&["a", "b", "c", "d", "e"]), 2, 0, 7, 40, 1, 1);
        assert_eq!(texts(&s), vec!["", "", "b", "c", "d", "", ""]);
    }
}

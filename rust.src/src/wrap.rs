//! Word wrapping for the fixed text column.
//!
//! [`wrap_line`] splits a logical line into contiguous visual segments no wider
//! than the column. Breaks are preferred after a space; words longer than the
//! column are hard-broken. Segments partition the line by character offset
//! (no characters are dropped), so a cursor column maps unambiguously onto a
//! segment.

/// One visual segment of a wrapped line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Character offset of this segment's first character within the line.
    pub start: usize,
    pub text: String,
}

impl Segment {
    /// Character offset just past this segment's last character.
    pub fn end(&self) -> usize {
        self.start + self.text.chars().count()
    }
}

/// Wraps `line` into segments at most `width` characters wide.
///
/// `width` is treated as at least 1. An empty line yields a single empty
/// segment so it still occupies one visual row.
pub fn wrap_line(line: &str, width: usize) -> Vec<Segment> {
    let width = width.max(1);
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();

    if n == 0 {
        return vec![Segment {
            start: 0,
            text: String::new(),
        }];
    }

    let mut segments = Vec::new();
    let mut i = 0;
    while i < n {
        if n - i <= width {
            segments.push(Segment {
                start: i,
                text: chars[i..n].iter().collect(),
            });
            break;
        }

        let limit = i + width;
        // Prefer to break right after the last space inside the window.
        let mut end = limit;
        for j in (i..limit).rev() {
            if chars[j] == ' ' {
                // Don't break at the very first position (would loop forever).
                if j + 1 > i {
                    end = j + 1;
                    break;
                }
            }
        }

        segments.push(Segment {
            start: i,
            text: chars[i..end].iter().collect(),
        });
        i = end;
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(segs: &[Segment]) -> Vec<String> {
        segs.iter().map(|s| s.text.clone()).collect()
    }

    #[test]
    fn empty_line_is_one_empty_segment() {
        let s = wrap_line("", 10);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].start, 0);
        assert_eq!(s[0].text, "");
    }

    #[test]
    fn short_line_fits_in_one_segment() {
        let s = wrap_line("hello", 10);
        assert_eq!(texts(&s), vec!["hello"]);
    }

    #[test]
    fn line_equal_to_width_is_one_segment() {
        let s = wrap_line("abcde", 5);
        assert_eq!(texts(&s), vec!["abcde"]);
    }

    #[test]
    fn wraps_at_space() {
        let s = wrap_line("hello world", 7);
        // "hello " (6, breaks after the space) then "world".
        assert_eq!(texts(&s), vec!["hello ", "world"]);
        assert_eq!(s[1].start, 6);
    }

    #[test]
    fn long_word_is_hard_broken() {
        let s = wrap_line("abcdefghij", 4);
        assert_eq!(texts(&s), vec!["abcd", "efgh", "ij"]);
        assert_eq!(s[1].start, 4);
        assert_eq!(s[2].start, 8);
    }

    #[test]
    fn segments_partition_all_characters() {
        let line = "the quick brown fox jumps";
        let s = wrap_line(line, 9);
        let joined: String = s.iter().map(|seg| seg.text.clone()).collect();
        assert_eq!(joined, line);
        // Offsets are contiguous.
        for w in s.windows(2) {
            assert_eq!(w[0].end(), w[1].start);
        }
    }

    #[test]
    fn utf8_is_handled_by_characters() {
        let s = wrap_line("привет мир", 7);
        assert_eq!(texts(&s), vec!["привет ", "мир"]);
    }
}

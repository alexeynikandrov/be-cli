//! Terminal input layer (RFC-0003).
//!
//! The terminal is switched to raw mode for the lifetime of a [`RawModeGuard`],
//! which restores the previous state on drop (including on panic). Raw key and
//! resize events from crossterm are normalized into a small [`Event`] enum so
//! the rest of the editor never deals with raw escape sequences. Unknown input
//! maps to [`Event::Unknown`] and never panics.

use std::io;

use crossterm::event::{
    Event as CtEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read as ct_read,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

/// Cursor movement direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    /// Start of the current line (Home).
    LineStart,
    /// End of the current line (End).
    LineEnd,
}

/// High-level editor action bound to a hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Save,
    Quit,
    OpenSettings,
    Help,
}

/// A normalized input event consumed by the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Insert a printable character.
    Insert(char),
    /// Insert a line break (Enter).
    Newline,
    /// Delete the character before the cursor (Backspace).
    Backspace,
    /// Delete the character at the cursor (Delete).
    Delete,
    /// Move the cursor.
    Move(Direction),
    /// Trigger an editor action.
    Action(Action),
    /// Escape key (e.g. close an overlay).
    Escape,
    /// Terminal was resized to `(width, height)`.
    Resize(u16, u16),
    /// Unrecognized or ignored input; never causes a crash.
    Unknown,
}

/// Maps a crossterm key event into a normalized [`Event`].
///
/// Release events are ignored (reported as [`Event::Unknown`]) so platforms
/// that emit both press and release do not double-process a keystroke.
fn map_key(key: KeyEvent) -> Event {
    if key.kind == KeyEventKind::Release {
        return Event::Unknown;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char(c) if ctrl => match c.to_ascii_lowercase() {
            's' => Event::Action(Action::Save),
            'q' => Event::Action(Action::Quit),
            'o' => Event::Action(Action::OpenSettings),
            _ => Event::Unknown,
        },
        KeyCode::Char('?') => Event::Action(Action::Help),
        KeyCode::Char(c) => Event::Insert(c),
        KeyCode::Enter => Event::Newline,
        KeyCode::Backspace => Event::Backspace,
        KeyCode::Delete => Event::Delete,
        KeyCode::Up => Event::Move(Direction::Up),
        KeyCode::Down => Event::Move(Direction::Down),
        KeyCode::Left => Event::Move(Direction::Left),
        KeyCode::Right => Event::Move(Direction::Right),
        KeyCode::Home => Event::Move(Direction::LineStart),
        KeyCode::End => Event::Move(Direction::LineEnd),
        KeyCode::Esc => Event::Escape,
        _ => Event::Unknown,
    }
}

/// Maps any crossterm event into a normalized [`Event`].
fn map_event(event: CtEvent) -> Event {
    match event {
        CtEvent::Key(key) => map_key(key),
        CtEvent::Resize(w, h) => Event::Resize(w, h),
        _ => Event::Unknown,
    }
}

/// Blocks until the next input event and returns it normalized.
pub fn read_event() -> io::Result<Event> {
    Ok(map_event(ct_read()?))
}

/// RAII guard that keeps the terminal in raw mode while alive.
///
/// Raw mode is enabled on [`RawModeGuard::enter`] and disabled on drop, so the
/// terminal is always restored on normal exit, early return, or panic.
pub struct RawModeGuard {
    _private: (),
}

impl RawModeGuard {
    /// Enables raw mode and returns a guard that restores it on drop.
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self { _private: () })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    /// Builds a press key event with the given code and modifiers.
    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn printable_char_inserts() {
        assert_eq!(
            map_key(press(KeyCode::Char('a'), KeyModifiers::NONE)),
            Event::Insert('a')
        );
    }

    #[test]
    fn shifted_char_inserts_as_is() {
        assert_eq!(
            map_key(press(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Event::Insert('A')
        );
    }

    #[test]
    fn ctrl_keys_map_to_actions() {
        assert_eq!(
            map_key(press(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            Event::Action(Action::Save)
        );
        assert_eq!(
            map_key(press(KeyCode::Char('Q'), KeyModifiers::CONTROL)),
            Event::Action(Action::Quit)
        );
        assert_eq!(
            map_key(press(KeyCode::Char('o'), KeyModifiers::CONTROL)),
            Event::Action(Action::OpenSettings)
        );
    }

    #[test]
    fn question_mark_opens_help() {
        assert_eq!(
            map_key(press(KeyCode::Char('?'), KeyModifiers::NONE)),
            Event::Action(Action::Help)
        );
    }

    #[test]
    fn unknown_ctrl_combo_is_unknown() {
        assert_eq!(
            map_key(press(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Event::Unknown
        );
    }

    #[test]
    fn editing_keys_map() {
        assert_eq!(
            map_key(press(KeyCode::Enter, KeyModifiers::NONE)),
            Event::Newline
        );
        assert_eq!(
            map_key(press(KeyCode::Backspace, KeyModifiers::NONE)),
            Event::Backspace
        );
        assert_eq!(
            map_key(press(KeyCode::Delete, KeyModifiers::NONE)),
            Event::Delete
        );
    }

    #[test]
    fn arrow_keys_map_to_moves() {
        assert_eq!(
            map_key(press(KeyCode::Up, KeyModifiers::NONE)),
            Event::Move(Direction::Up)
        );
        assert_eq!(
            map_key(press(KeyCode::Down, KeyModifiers::NONE)),
            Event::Move(Direction::Down)
        );
        assert_eq!(
            map_key(press(KeyCode::Left, KeyModifiers::NONE)),
            Event::Move(Direction::Left)
        );
        assert_eq!(
            map_key(press(KeyCode::Right, KeyModifiers::NONE)),
            Event::Move(Direction::Right)
        );
    }

    #[test]
    fn home_end_map_to_line_moves() {
        assert_eq!(
            map_key(press(KeyCode::Home, KeyModifiers::NONE)),
            Event::Move(Direction::LineStart)
        );
        assert_eq!(
            map_key(press(KeyCode::End, KeyModifiers::NONE)),
            Event::Move(Direction::LineEnd)
        );
    }

    #[test]
    fn escape_maps() {
        assert_eq!(
            map_key(press(KeyCode::Esc, KeyModifiers::NONE)),
            Event::Escape
        );
    }

    #[test]
    fn release_event_is_ignored() {
        let key = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        assert_eq!(map_key(key), Event::Unknown);
    }

    #[test]
    fn function_key_is_unknown() {
        assert_eq!(
            map_key(press(KeyCode::F(1), KeyModifiers::NONE)),
            Event::Unknown
        );
    }

    #[test]
    fn resize_event_maps() {
        assert_eq!(map_event(CtEvent::Resize(80, 24)), Event::Resize(80, 24));
    }

    #[test]
    fn paste_event_is_unknown() {
        assert_eq!(map_event(CtEvent::Paste("x".into())), Event::Unknown);
    }
}

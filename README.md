# bbe-cli

A minimal Linux terminal editor focused on distraction-free writing and note-taking.

## Status

- RFC: `docs/rfcs/0001.md`
- RFC status: `review`
- Implementation language (current preference): `Hare` *(provisional; may return to Rust)*

## Product idea (MVP)

- Focused viewport: active line centered, with limited context above/below.
- Minimal editing flow: open, edit, save, quit.
- Status line:
  - left: `[filename] [saved|modified]`
  - right: `Ln <line>, Col <col>`
- Shortcuts:
  - `Ctrl+S` save
  - `Ctrl+Q` quit (with unsaved changes confirmation)
  - `Ctrl+O` settings
  - `?` help

## Planned settings

- `lines_before`
- `lines_after`
- `cursor_on_open`: `start | end`

Post-MVP ideas include theme customization, custom keybindings, and full-file preview mode with scrolling.

## Repository structure

- `docs/rfcs/` — RFC documents and index
- `AGENTS.md` — project workflow and agent rules

## Development notes

RFC lifecycle operations should be handled via `rfc-cli` (status, links, dependencies), not by manual edits to `docs/rfcs/.index.json`.

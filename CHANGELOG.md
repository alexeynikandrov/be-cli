# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `make release` target building an optimized binary.
- Configurable, horizontally centered text column (`text_width`, default 80)
  with word wrap for long lines; adjustable from the settings panel and
  persisted to the config file.
- Home and End keys move the cursor to the start and end of the current line.
- PageUp and PageDown move the cursor by one console height; Ctrl+PageUp and
  Ctrl+PageDown move it by the configured context height (`lines_before` /
  `lines_after`).
- Ctrl+Home moves the cursor to the start of the file.
- Ctrl+End moves the cursor to the end of the file.
- Autosave: when enabled (config key `autosave`, default off) the file is
  saved automatically every `autosave_interval` minutes (default 5) and on
  Ctrl+Q (which then quits without the unsaved-changes confirmation). Both
  options are also adjustable from the settings panel.
- Help is now toggled with Ctrl+H or Ctrl+?, and the status line shows a
  `Ctrl+H Help` hint on the right when there is room. The plain `?` key now
  inserts a literal question mark.

### Fixed

- Saving no longer strips the file's trailing newline; the original
  trailing-newline state is now preserved on save.
- The status line (and transient messages such as "saved") again span the
  full terminal width at the left edge instead of shifting into the centered
  text column, so save feedback stays visible on wide terminals.

### Changed

- Advanced RFC-0001..0008 to status `implemented`.

## [0.1.0] - 2026-06-28

### Added

- Created `README.md` with project overview and current RFC-driven scope.
- Created `CHANGELOG.md`.
- Rust implementation of the `be` minimalist focus editor under `rust.src/`
  (package `be`), covering the MVP RFCs:
  - Line-based text buffer and UTF-8-safe cursor model (RFC-0002).
  - File I/O with auto-create, save, and readonly handling (RFC-0006).
  - crossterm-based terminal input with normalized events (RFC-0003).
  - Focus viewport with vertical centering and context window (RFC-0004).
  - ANSI renderer and fixed-format status line (RFC-0005).
  - Settings and auto-created TOML config file (RFC-0007).
  - clap-based CLI and main editing loop with save/quit/help/settings
    hotkeys and unsaved-changes confirmation (RFC-0001).
  - Help overlay and in-session settings panel (RFC-0008).
- `Makefile` with `build`, `test`, `lint`, `fmt`, and `check` targets.

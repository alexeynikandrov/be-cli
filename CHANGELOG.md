# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-06-29

### Fixed

- Active line is now drawn with the terminal's default foreground color instead
  of a hardcoded white, so the line being edited stays readable on light
  terminal backgrounds (e.g. the default macOS Terminal theme).

## [0.2.0] - 2026-06-28

### Added

- GitHub Actions CI workflow (`.github/workflows/ci.yml`) running build, test,
  and lint on pull requests, and a release workflow
  (`.github/workflows/release.yml`) building cross-platform binaries on `v*`
  tags. Both are adapted to the `rust.src/` project layout and the `be` binary.
- Dual `MIT OR Apache-2.0` license (`LICENSE-MIT`, `LICENSE-APACHE`), declared
  in `Cargo.toml` and documented in `README.md`. License files are bundled into
  release archives.
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
- `make install` target installing the optimized binary into
  `~/.local/bin` (overridable via `INSTALL_DIR`).
- The status line shows an `[autosave]` marker when autosave is enabled.
- Settings changed in the Ctrl+O panel are now persisted to the config file
  when the panel is closed, instead of applying only to the current session.
- Autosave: when enabled (config key `autosave`, default off) the file is
  saved automatically every `autosave_interval` minutes (default 5) and on
  Ctrl+Q (which then quits without the unsaved-changes confirmation). Both
  options are also adjustable from the settings panel.
- Help is now toggled with Ctrl+H or Ctrl+?, and the status line shows a
  `Ctrl+H Help` hint on the right when there is room. The plain `?` key now
  inserts a literal question mark.

### Fixed

- The cursor now blinks: a blinking block style is set on startup and the
  terminal's default cursor style is restored on exit.

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

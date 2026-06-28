# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `make release` target building an optimized binary.

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

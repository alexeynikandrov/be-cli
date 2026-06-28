//! Settings and configuration file (RFC-0007).
//!
//! Configuration lives at `$XDG_CONFIG_HOME/be/config.toml` (falling back to
//! `~/.config/be/config.toml`) and is auto-created with defaults on first run.
//! Three parameters are supported: `lines_before`, `lines_after` and
//! `cursor_on_open` (`"start"` | `"end"`). Invalid values are replaced by safe
//! defaults and reported as warnings for the UI to surface. The file/default
//! layer produced here is later merged with session edits and CLI flags by the
//! main loop (CLI > session > file > defaults).
//!
//! Only this fixed key set is read/written, so a tiny hand-rolled parser is
//! used instead of pulling in a general TOML dependency.

use std::fs;
use std::path::{Path, PathBuf};

/// Where the cursor starts when a file is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorOnOpen {
    Start,
    End,
}

impl CursorOnOpen {
    fn as_str(self) -> &'static str {
        match self {
            CursorOnOpen::Start => "start",
            CursorOnOpen::End => "end",
        }
    }
}

/// Editor settings (the file/default layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub lines_before: usize,
    pub lines_after: usize,
    pub text_width: usize,
    pub cursor_on_open: CursorOnOpen,
    pub autosave: bool,
    pub autosave_interval: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lines_before: 3,
            lines_after: 3,
            text_width: 80,
            cursor_on_open: CursorOnOpen::Start,
            autosave: false,
            autosave_interval: 5,
        }
    }
}

impl Config {
    /// Serializes the config into the TOML form written to disk.
    pub fn to_toml(self) -> String {
        format!(
            "# be editor configuration\n\
             lines_before = {}\n\
             lines_after = {}\n\
             text_width = {}\n\
             cursor_on_open = \"{}\"\n\
             autosave = {}\n\
             autosave_interval = {}\n",
            self.lines_before,
            self.lines_after,
            self.text_width,
            self.cursor_on_open.as_str(),
            self.autosave,
            self.autosave_interval,
        )
    }
}

/// Strips an optional surrounding pair of double quotes from `value`.
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

/// Parses the config text, returning the config and any validation warnings.
///
/// Unknown keys are ignored. Invalid values fall back to the corresponding
/// default and add a warning.
pub fn parse(content: &str) -> (Config, Vec<String>) {
    let mut config = Config::default();
    let mut warnings = Vec::new();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim());

        match key {
            "lines_before" => match value.parse::<usize>() {
                Ok(n) => config.lines_before = n,
                Err(_) => warnings.push(format!(
                    "invalid lines_before '{value}', using {}",
                    config.lines_before
                )),
            },
            "lines_after" => match value.parse::<usize>() {
                Ok(n) => config.lines_after = n,
                Err(_) => warnings.push(format!(
                    "invalid lines_after '{value}', using {}",
                    config.lines_after
                )),
            },
            "text_width" => match value.parse::<usize>() {
                Ok(n) if n >= 1 => config.text_width = n,
                _ => warnings.push(format!(
                    "invalid text_width '{value}', using {}",
                    config.text_width
                )),
            },
            "cursor_on_open" => match value.to_ascii_lowercase().as_str() {
                "start" => config.cursor_on_open = CursorOnOpen::Start,
                "end" => config.cursor_on_open = CursorOnOpen::End,
                _ => warnings.push(format!(
                    "invalid cursor_on_open '{value}', using {}",
                    config.cursor_on_open.as_str()
                )),
            },
            "autosave" => match value.to_ascii_lowercase().as_str() {
                "true" => config.autosave = true,
                "false" => config.autosave = false,
                _ => warnings.push(format!(
                    "invalid autosave '{value}', using {}",
                    config.autosave
                )),
            },
            "autosave_interval" => match value.parse::<usize>() {
                Ok(n) if n >= 1 => config.autosave_interval = n,
                _ => warnings.push(format!(
                    "invalid autosave_interval '{value}', using {}",
                    config.autosave_interval
                )),
            },
            _ => {}
        }
    }

    (config, warnings)
}

/// Resolves the config file path from the relevant environment variables.
///
/// Prefers `$XDG_CONFIG_HOME/be/config.toml`, falling back to
/// `<home>/.config/be/config.toml`. Returns `None` if neither is available.
pub fn resolve_path(xdg_config_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    if let Some(xdg) = xdg_config_home.filter(|s| !s.is_empty()) {
        return Some(Path::new(xdg).join("be").join("config.toml"));
    }
    home.filter(|s| !s.is_empty())
        .map(|h| Path::new(h).join(".config").join("be").join("config.toml"))
}

/// Writes `config` to `path`, creating the parent directory if needed.
///
/// Returns an error message on failure (the caller surfaces it in the UI).
pub fn save_at(path: &Path, config: Config) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return Err(format!(
            "cannot create config dir '{}': {e}",
            parent.display()
        ));
    }
    fs::write(path, config.to_toml())
        .map_err(|e| format!("cannot write config '{}': {e}", path.display()))
}

/// Loads the config from `path`, creating it with defaults if it is missing.
///
/// Returns the config plus warnings (invalid values, or I/O problems that fall
/// back to defaults). I/O failures never abort the editor; defaults are used.
pub fn load_or_create_at(path: &Path) -> (Config, Vec<String>) {
    if path.exists() {
        match fs::read_to_string(path) {
            Ok(content) => parse(&content),
            Err(e) => (
                Config::default(),
                vec![format!("cannot read config '{}': {e}", path.display())],
            ),
        }
    } else {
        let config = Config::default();
        let mut warnings = Vec::new();
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            warnings.push(format!(
                "cannot create config dir '{}': {e}",
                parent.display()
            ));
            return (config, warnings);
        }
        if let Err(e) = fs::write(path, config.to_toml()) {
            warnings.push(format!("cannot write config '{}': {e}", path.display()));
        }
        (config, warnings)
    }
}

/// Loads the config using the environment, creating defaults on first run.
///
/// If no config path can be resolved, defaults are returned with a warning.
pub fn load_or_create() -> (Config, Vec<String>) {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var("HOME").ok();
    match resolve_path(xdg.as_deref(), home.as_deref()) {
        Some(path) => load_or_create_at(&path),
        None => (
            Config::default(),
            vec!["cannot resolve config path (no XDG_CONFIG_HOME or HOME)".to_string()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("be_cfg_{}_{}", std::process::id(), n))
    }

    #[test]
    fn default_values() {
        let c = Config::default();
        assert_eq!(c.lines_before, 3);
        assert_eq!(c.lines_after, 3);
        assert_eq!(c.text_width, 80);
        assert_eq!(c.cursor_on_open, CursorOnOpen::Start);
        assert!(!c.autosave);
        assert_eq!(c.autosave_interval, 5);
    }

    #[test]
    fn parse_valid_config() {
        let (c, w) = parse("lines_before = 1\nlines_after = 5\ncursor_on_open = \"end\"\n");
        assert_eq!(c.lines_before, 1);
        assert_eq!(c.lines_after, 5);
        assert_eq!(c.cursor_on_open, CursorOnOpen::End);
        assert!(w.is_empty());
    }

    #[test]
    fn parse_ignores_comments_blanks_and_unknown_keys() {
        let (c, w) = parse("# comment\n\n  lines_before = 2 \nunknown = 9\n");
        assert_eq!(c.lines_before, 2);
        assert_eq!(c.lines_after, 3); // default kept
        assert!(w.is_empty());
    }

    #[test]
    fn parse_invalid_number_warns_and_defaults() {
        let (c, w) = parse("lines_before = abc\n");
        assert_eq!(c.lines_before, 3);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("lines_before"));
    }

    #[test]
    fn parse_invalid_cursor_warns_and_defaults() {
        let (c, w) = parse("cursor_on_open = \"middle\"\n");
        assert_eq!(c.cursor_on_open, CursorOnOpen::Start);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("cursor_on_open"));
    }

    #[test]
    fn parse_cursor_is_case_insensitive() {
        let (c, _) = parse("cursor_on_open = \"END\"\n");
        assert_eq!(c.cursor_on_open, CursorOnOpen::End);
    }

    #[test]
    fn toml_roundtrips_through_parse() {
        let original = Config {
            lines_before: 4,
            lines_after: 1,
            text_width: 72,
            cursor_on_open: CursorOnOpen::End,
            autosave: true,
            autosave_interval: 10,
        };
        let (parsed, w) = parse(&original.to_toml());
        assert_eq!(parsed, original);
        assert!(w.is_empty());
    }

    #[test]
    fn parse_text_width_valid_and_invalid() {
        let (c, w) = parse("text_width = 60\n");
        assert_eq!(c.text_width, 60);
        assert!(w.is_empty());
        let (c, w) = parse("text_width = 0\n");
        assert_eq!(c.text_width, 80); // default kept
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("text_width"));
    }

    #[test]
    fn parse_autosave_valid_and_invalid() {
        let (c, w) = parse("autosave = true\nautosave_interval = 10\n");
        assert!(c.autosave);
        assert_eq!(c.autosave_interval, 10);
        assert!(w.is_empty());
        let (c, w) = parse("autosave = maybe\nautosave_interval = 0\n");
        assert!(!c.autosave); // default kept
        assert_eq!(c.autosave_interval, 5); // default kept
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn save_at_roundtrips_through_load() {
        let dir = temp_dir();
        let path = dir.join("be").join("config.toml");
        let cfg = Config {
            lines_before: 2,
            lines_after: 6,
            text_width: 64,
            cursor_on_open: CursorOnOpen::End,
            autosave: true,
            autosave_interval: 7,
        };
        save_at(&path, cfg).unwrap();
        let (loaded, w) = load_or_create_at(&path);
        assert_eq!(loaded, cfg);
        assert!(w.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_path_prefers_xdg() {
        let p = resolve_path(Some("/x/cfg"), Some("/home/u")).unwrap();
        assert_eq!(p, PathBuf::from("/x/cfg/be/config.toml"));
    }

    #[test]
    fn resolve_path_falls_back_to_home() {
        let p = resolve_path(None, Some("/home/u")).unwrap();
        assert_eq!(p, PathBuf::from("/home/u/.config/be/config.toml"));
    }

    #[test]
    fn resolve_path_empty_xdg_falls_back() {
        let p = resolve_path(Some(""), Some("/home/u")).unwrap();
        assert_eq!(p, PathBuf::from("/home/u/.config/be/config.toml"));
    }

    #[test]
    fn resolve_path_none_when_unset() {
        assert_eq!(resolve_path(None, None), None);
    }

    #[test]
    fn load_or_create_creates_then_loads() {
        let dir = temp_dir();
        let path = dir.join("be").join("config.toml");
        // First run: file is created with defaults.
        let (c1, w1) = load_or_create_at(&path);
        assert!(path.exists());
        assert_eq!(c1, Config::default());
        assert!(w1.is_empty());
        // Second run: existing file is loaded.
        let (c2, w2) = load_or_create_at(&path);
        assert_eq!(c2, Config::default());
        assert!(w2.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_create_reads_existing_values() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "lines_before = 7\ncursor_on_open = \"end\"\n").unwrap();
        let (c, w) = load_or_create_at(&path);
        assert_eq!(c.lines_before, 7);
        assert_eq!(c.cursor_on_open, CursorOnOpen::End);
        assert!(w.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}

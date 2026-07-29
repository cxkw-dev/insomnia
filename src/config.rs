//! Configuration: command-line flags, human durations, and the optional
//! config file (`~/.config/insomnia/config.toml`) that drives the dashboard
//! cards and sky toggles.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::power::Kind;
use crate::sky;

// --- Command-line flags ---

#[derive(Debug, Default)]
pub struct Config {
    pub display: bool,
    pub idle: bool,
    pub system: bool,
    pub disk: bool,
    pub timeout: Option<Duration>,
    pub quiet: bool,
    pub config_path: Option<String>,
}

impl Config {
    pub fn kinds(&self) -> Vec<Kind> {
        let mut v = Vec::new();
        if self.display {
            v.push(Kind::Display);
        }
        if self.idle {
            v.push(Kind::Idle);
        }
        if self.system {
            v.push(Kind::System);
        }
        if self.disk {
            v.push(Kind::Disk);
        }
        v
    }
}

pub enum Outcome {
    Run(Config),
    Help,
    Version,
}

pub fn parse<I: Iterator<Item = String>>(mut args: I) -> Result<Outcome, String> {
    let mut cfg = Config::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Outcome::Help),
            "-V" | "--version" => return Ok(Outcome::Version),
            "--quiet" => cfg.quiet = true,
            "--config" => {
                let v = args
                    .next()
                    .ok_or("--config needs a path, e.g. --config ./insomnia.toml")?;
                cfg.config_path = Some(v);
            }
            "-t" | "--for" => {
                let v = args
                    .next()
                    .ok_or("-t needs a duration, e.g. -t 45m, -t 2h, -t 300")?;
                cfg.timeout = Some(parse_duration(&v)?);
            }
            s if s.starts_with('-') && s.len() > 1 && !s.starts_with("--") => {
                let chars: Vec<char> = s[1..].chars().collect();
                for (i, c) in chars.iter().enumerate() {
                    match c {
                        'd' => cfg.display = true,
                        'i' => cfg.idle = true,
                        's' => cfg.system = true,
                        'm' => cfg.disk = true,
                        'q' => cfg.quiet = true,
                        't' => {
                            if i != chars.len() - 1 {
                                return Err("-t must come last in a flag group".into());
                            }
                            let v = args
                                .next()
                                .ok_or("-t needs a duration, e.g. -t 45m, -t 2h, -t 300")?;
                            cfg.timeout = Some(parse_duration(&v)?);
                        }
                        _ => return Err(format!("unknown flag -{c} (see insomnia --help)")),
                    }
                }
            }
            s => return Err(format!("unexpected argument '{s}' (see insomnia --help)")),
        }
    }
    if !(cfg.display || cfg.idle || cfg.system || cfg.disk) {
        cfg.display = true;
        cfg.idle = true;
    }
    Ok(Outcome::Run(cfg))
}

// --- The config file ---

/// The optional TOML config: docker toggle, sky toggles, and custom cards.
/// Everything defaults to a sensible value, so no file is required at all.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileConfig {
    pub docker: DockerConfig,
    /// `[sky]` — per-flyer toggles, e.g. `raider = false`. Unlisted = on.
    pub sky: HashMap<String, bool>,
    /// `[[card]]` blocks — custom dashboard cards fed by shell commands.
    #[serde(rename = "card")]
    pub cards: Vec<CardConfig>,
    pub dashboard: DashboardConfig,
}

/// The `[dashboard]` table. `combined = true` folds every card — custom and
/// docker alike — into the status box itself, each as a section under a
/// header in that card's glyph and accent, so the whole app is one box.
/// The box expands responsively up to the renderer's comfortable reading
/// width. Title, glyph, accent, and side are then unused.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DashboardConfig {
    pub combined: bool,
    pub title: Option<String>,
    pub glyph: Option<String>,
    pub accent: Option<String>,
    pub side: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DockerConfig {
    pub enabled: bool,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// One custom dashboard card. `command` runs through `sh -c` on `interval`
/// seconds; each stdout line becomes a row. Lines may be plain text,
/// `ok|name|detail` / `warn|…` / `bad|…` / `off|…` for a health-dotted
/// two-column row, or `hdr|accent|text` for a section header in that
/// accent's color.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardConfig {
    pub title: String,
    pub command: String,
    #[serde(default)]
    pub glyph: Option<String>,
    #[serde(default = "default_interval")]
    pub interval: u64,
    #[serde(default)]
    pub accent: Option<String>,
    /// `"left"` floats the card to the left of the status card when the
    /// terminal is wide enough for three columns; the default is `"right"`.
    #[serde(default)]
    pub side: Option<String>,
}

fn default_interval() -> u64 {
    10
}

/// Accent names accepted by `accent = "…"` on a card.
pub const ACCENTS: &[(&str, (u8, u8, u8))] = &[
    ("violet", (167, 139, 250)),
    ("pink", (244, 114, 182)),
    ("sky", (125, 211, 252)),
    ("blue", (59, 130, 246)),
    ("emerald", (52, 211, 153)),
    ("gold", (251, 191, 36)),
    ("red", (248, 113, 113)),
    ("slate", (148, 163, 184)),
    ("coral", (217, 119, 87)),
    ("teal", (16, 163, 127)),
];

pub fn accent_rgb(name: &str) -> Option<(u8, u8, u8)> {
    ACCENTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, rgb)| *rgb)
}

/// Load the config file. An explicit `--config` path must exist; the default
/// path is allowed to be absent and yields the defaults.
pub fn load_file(explicit: Option<&str>) -> Result<FileConfig, String> {
    let (path, required) = match explicit {
        Some(p) => (PathBuf::from(p), true),
        None => (default_config_path(), false),
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !required => {
            return Ok(FileConfig::default())
        }
        Err(e) => return Err(format!("could not read {}: {e}", path.display())),
    };
    parse_file(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse_file(text: &str) -> Result<FileConfig, String> {
    let cfg: FileConfig = toml::from_str(text).map_err(|e| e.to_string())?;
    for card in &cfg.cards {
        if card.title.trim().is_empty() {
            return Err("a [[card]] is missing its title".into());
        }
        if card.command.trim().is_empty() {
            return Err(format!("card '{}' is missing its command", card.title));
        }
        if let Some(a) = &card.accent {
            if accent_rgb(a).is_none() {
                let known: Vec<&str> = ACCENTS.iter().map(|(n, _)| *n).collect();
                return Err(format!(
                    "card '{}': unknown accent '{a}' (try one of {})",
                    card.title,
                    known.join(", ")
                ));
            }
        }
        if let Some(sd) = &card.side {
            if sd != "left" && sd != "right" {
                return Err(format!(
                    "card '{}': unknown side '{sd}' (left or right)",
                    card.title
                ));
            }
        }
    }
    if let Some(a) = &cfg.dashboard.accent {
        if accent_rgb(a).is_none() {
            let known: Vec<&str> = ACCENTS.iter().map(|(n, _)| *n).collect();
            return Err(format!(
                "[dashboard]: unknown accent '{a}' (try one of {})",
                known.join(", ")
            ));
        }
    }
    if let Some(sd) = &cfg.dashboard.side {
        if sd != "left" && sd != "right" {
            return Err(format!("[dashboard]: unknown side '{sd}' (left or right)"));
        }
    }
    for key in cfg.sky.keys() {
        if !sky::names().any(|n| n == key) {
            let known: Vec<&str> = sky::names().collect();
            return Err(format!(
                "[sky]: unknown flyer '{key}' (the sky holds {})",
                known.join(", ")
            ));
        }
    }
    Ok(cfg)
}

/// `$XDG_CONFIG_HOME/insomnia/config.toml`, or `~/.config/insomnia/config.toml`.
pub fn default_config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    base.join("insomnia").join("config.toml")
}

// --- Durations ---

/// Ten years, in seconds. Longer than any vigil — and comfortably inside the
/// range where `Instant + Duration` stays safe.
const MAX_SECS: u64 = 10 * 365 * 24 * 3600;

pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return Err("empty duration".into());
    }
    // Bare number = seconds, matching caffeinate -t.
    if s.chars().all(|c| c.is_ascii_digit()) {
        let secs: u64 = s.parse().map_err(|_| format!("bad duration '{s}'"))?;
        return finish(secs);
    }
    let mut total: u64 = 0;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            let n: u64 = num
                .parse()
                .map_err(|_| format!("bad duration '{s}' (try 90s, 45m, 2h, 1h30m)"))?;
            num.clear();
            let mult = match c {
                'h' => 3600,
                'm' => 60,
                's' => 1,
                _ => return Err(format!("unknown unit '{c}' in '{s}' (use h, m, or s)")),
            };
            total = n
                .checked_mul(mult)
                .and_then(|v| total.checked_add(v))
                .ok_or("duration too long — insomnia tops out at ten years")?;
        }
    }
    if !num.is_empty() {
        return Err(format!(
            "trailing number without unit in '{s}' (use h, m, or s)"
        ));
    }
    finish(total)
}

fn finish(secs: u64) -> Result<Duration, String> {
    if secs == 0 {
        return Err("duration must be greater than zero".into());
    }
    if secs > MAX_SECS {
        return Err("duration too long — insomnia tops out at ten years".into());
    }
    Ok(Duration::from_secs(secs))
}

pub fn human(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    if sec > 0 && h == 0 {
        parts.push(format!("{sec}s"));
    }
    if parts.is_empty() {
        parts.push("0s".into());
    }
    parts.join(" ")
}

pub fn help() -> String {
    let violet = "\x1b[38;2;167;139;250m";
    let pink = "\x1b[38;2;244;114;182m";
    let slate = "\x1b[38;2;148;163;184m";
    let dim = "\x1b[38;2;100;116;139m";
    let bold = "\x1b[1m";
    let r = "\x1b[0m";
    format!(
        "\
{violet}☾ insomnia{r} {dim}v{}{r} — keep your mac awake, beautifully

{pink}{bold}usage{r}  insomnia [flags]

{pink}{bold}flags{r}
  {bold}-d{r}           {slate}keep the display awake{r}
  {bold}-i{r}           {slate}keep the system awake while idle{r}
  {bold}-s{r}           {slate}prevent sleep entirely, even lid-closed (AC power only){r}
  {bold}-m{r}           {slate}keep disks from idle-sleeping{r}
  {bold}-t{r} <time>    {slate}stay awake for a duration — 300, 90s, 45m, 2h, 1h30m (or --for){r}
  {bold}-q{r}, --quiet  {slate}skip the art, hold quietly instead{r}
  {bold}--config{r} <p> {slate}config file (default ~/.config/insomnia/config.toml){r}
  {bold}-h{r}, --help   {slate}this screen{r}
  {bold}-V{r}, --version

{dim}with no flags, insomnia runs as -d -i: display and system stay awake.
quit with q, esc, or ctrl-c — power assertions release instantly.
the config file adds dashboard cards (docker, or any shell command)
and toggles the ships in the sky — see the readme for the format.{r}
",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("300").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("45m").unwrap(), Duration::from_secs(2700));
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(5400));
        assert!(parse_duration("0").is_err());
        assert!(parse_duration("5x").is_err());
        assert!(parse_duration("90").is_ok());
        // Absurd values error instead of overflowing.
        assert!(parse_duration("18446744073709551615").is_err());
        assert!(parse_duration("10000000000000000h").is_err());
        assert!(parse_duration("87600h").is_ok()); // exactly ten years
    }

    #[test]
    fn empty_config_is_default() {
        let cfg = parse_file("").unwrap();
        assert!(cfg.docker.enabled);
        assert!(cfg.cards.is_empty());
        assert!(cfg.sky.is_empty());
    }

    #[test]
    fn full_config_parses() {
        let cfg = parse_file(
            r#"
[docker]
enabled = false

[sky]
raider = false

[[card]]
title = "todos"
command = "echo 'ok|hello|world'"
glyph = "⌖"
accent = "violet"
interval = 30
"#,
        )
        .unwrap();
        assert!(!cfg.docker.enabled);
        assert_eq!(cfg.sky.get("raider"), Some(&false));
        assert_eq!(cfg.cards.len(), 1);
        assert_eq!(cfg.cards[0].interval, 30);
    }

    #[test]
    fn bad_configs_error_helpfully() {
        assert!(parse_file("[sky]\ndragon = true").is_err());
        assert!(parse_file("[[card]]\ntitle = \"x\"\ncommand = \"\"").is_err());
        assert!(
            parse_file("[[card]]\ntitle = \"x\"\ncommand = \"echo\"\naccent = \"mauve\"").is_err()
        );
        assert!(parse_file("nonsense = 1").is_err());
    }
}

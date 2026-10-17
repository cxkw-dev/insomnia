//! Configuration: command-line flags and the optional config file
//! (`~/.config/insomnia/config.toml`) that drives the dashboard cards, the
//! sky, and the menu bar's account grouping.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::duration;
use crate::power::Kind;
use crate::sky;
use crate::theme::{accent_names, accent_rgb, ansi, Rgb, ANSI_RESET};

// --- Command-line flags ---

#[derive(Debug, Default)]
pub struct Flags {
    /// What to hold awake; never empty after [`parse`].
    pub kinds: Vec<Kind>,
    pub timeout: Option<Duration>,
    pub quiet: bool,
    pub config_path: Option<String>,
}

pub enum Outcome {
    Run(Flags),
    Help,
    Version,
}

const NEEDS_DURATION: &str = "-t needs a duration, e.g. -t 45m, -t 2h, -t 300";

pub fn parse<I: Iterator<Item = String>>(mut args: I) -> Result<Outcome, String> {
    fn hold(kind: Kind, kinds: &mut Vec<Kind>) {
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    let mut flags = Flags::default();
    while let Some(arg) = args.next() {
        if let Some(path) = config_arg(&arg, &mut args) {
            flags.config_path = Some(path?);
            continue;
        }
        match arg.as_str() {
            "-h" | "--help" => return Ok(Outcome::Help),
            "-V" | "--version" => return Ok(Outcome::Version),
            "--quiet" => flags.quiet = true,
            "-t" | "--for" => {
                let v = args.next().ok_or(NEEDS_DURATION)?;
                flags.timeout = Some(duration::parse(&v)?);
            }
            s if s.starts_with('-') && s.len() > 1 && !s.starts_with("--") => {
                let chars: Vec<char> = s[1..].chars().collect();
                for (i, c) in chars.iter().enumerate() {
                    match c {
                        'd' => hold(Kind::Display, &mut flags.kinds),
                        'i' => hold(Kind::Idle, &mut flags.kinds),
                        's' => hold(Kind::System, &mut flags.kinds),
                        'm' => hold(Kind::Disk, &mut flags.kinds),
                        'q' => flags.quiet = true,
                        't' => {
                            if i != chars.len() - 1 {
                                return Err("-t must come last in a flag group".into());
                            }
                            let v = args.next().ok_or(NEEDS_DURATION)?;
                            flags.timeout = Some(duration::parse(&v)?);
                        }
                        _ => return Err(format!("unknown flag -{c} (see insomnia --help)")),
                    }
                }
            }
            s => return Err(format!("unexpected argument '{s}' (see insomnia --help)")),
        }
    }
    if flags.kinds.is_empty() {
        flags.kinds = vec![Kind::Display, Kind::Idle];
    }
    flags.kinds.sort_by_key(|k| *k as u8);
    Ok(Outcome::Run(flags))
}

/// `--config <path>` or `--config=<path>`, shared by `insomnia` and
/// `insomnia-snapshot`. `None` when `arg` isn't the config flag at all.
pub fn config_arg(
    arg: &str,
    rest: &mut impl Iterator<Item = String>,
) -> Option<Result<String, String>> {
    if let Some(path) = arg.strip_prefix("--config=") {
        return Some(Ok(path.to_string()));
    }
    (arg == "--config").then(|| {
        rest.next()
            .ok_or_else(|| "--config needs a path, e.g. --config ./insomnia.toml".into())
    })
}

// --- The config file ---

/// The optional TOML config: `[docker]`, `[sky]`, `[dashboard]`,
/// `[accounts]`, and `[[card]]` blocks. Everything defaults to a sensible
/// value, so no file is required at all.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileConfig {
    pub docker: DockerConfig,
    pub sky: SkyConfig,
    /// `[[card]]` blocks — custom dashboard cards fed by shell commands.
    #[serde(rename = "card")]
    pub cards: Vec<CardConfig>,
    pub dashboard: DashboardConfig,
    pub accounts: AccountsConfig,
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

/// `[sky]`: `motion = false` starts with a still sky; every other key is a
/// flyer toggle from [`sky::FLYERS`] (unlisted flyers stay on).
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "HashMap<String, bool>")]
pub struct SkyConfig {
    pub motion: bool,
    pub flyers: HashMap<String, bool>,
}

impl Default for SkyConfig {
    fn default() -> Self {
        HashMap::new().into()
    }
}

impl From<HashMap<String, bool>> for SkyConfig {
    fn from(mut flyers: HashMap<String, bool>) -> Self {
        let motion = flyers.remove("motion").unwrap_or(true);
        Self { motion, flyers }
    }
}

/// `[dashboard]`. `combined = true` (the default) shows every card as a
/// section of one scrollable frame under the session; `false` keeps the
/// separate boxes, which then honor each card's glyph, accent, and side.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DashboardConfig {
    pub combined: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self { combined: true }
    }
}

/// `[accounts]`: how the menu bar app groups AI accounts. Each entry is a
/// case-insensitive substring of an account row's name, so
/// `personal = ["me@example.com"]` claims that login wherever it appears.
/// Explicit entries win; unlisted accounts fall back to the plan in their
/// detail (org, business, enterprise, team → enterprise; anything else →
/// personal). The terminal UI ignores this table.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccountsConfig {
    pub personal: Vec<String>,
    /// `business` is the old name for this key and still accepted.
    #[serde(alias = "business")]
    pub enterprise: Vec<String>,
}

/// One custom dashboard card. `command` runs through `sh -c` every
/// `interval` seconds; each stdout line becomes a row in the row protocol
/// (see [`crate::collect::parse_row`]).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardConfig {
    pub title: String,
    pub command: String,
    #[serde(default = "default_interval")]
    pub interval: u64,
    /// Separate-card layout only: the title glyph.
    #[serde(default)]
    pub glyph: Option<String>,
    /// Separate-card layout only: the box's accent.
    #[serde(default)]
    pub accent: Option<Accent>,
    /// Separate-card layout only: `left` floats the card left of the status
    /// card when the terminal fits three columns.
    #[serde(default)]
    pub side: Side,
}

fn default_interval() -> u64 {
    10
}

const MIN_INTERVAL: u64 = 2;

/// A named accent from [`crate::theme::ACCENTS`]; anything else is a config
/// error that lists the valid names.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(try_from = "String")]
pub struct Accent(pub Rgb);

impl TryFrom<String> for Accent {
    type Error = String;

    fn try_from(name: String) -> Result<Self, String> {
        accent_rgb(&name)
            .map(Accent)
            .ok_or_else(|| format!("unknown accent '{name}' (try one of {})", accent_names()))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    #[default]
    Right,
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
        if card.interval < MIN_INTERVAL {
            return Err(format!(
                "card '{}': interval must be at least {MIN_INTERVAL} seconds",
                card.title
            ));
        }
    }
    for name in cfg.accounts.personal.iter().chain(&cfg.accounts.enterprise) {
        if name.trim().is_empty() {
            return Err("[accounts]: entries must not be empty".into());
        }
    }
    for key in cfg.sky.flyers.keys() {
        if !sky::names().any(|n| n == key) {
            let known: Vec<&str> = sky::names().collect();
            return Err(format!(
                "[sky]: unknown flyer '{key}' (the sky holds {}, plus motion)",
                known.join(", ")
            ));
        }
    }
    Ok(cfg)
}

/// `$XDG_CONFIG_HOME/insomnia/config.toml`, or `~/.config/insomnia/config.toml`.
fn default_config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    base.join("insomnia").join("config.toml")
}

pub fn help() -> String {
    let violet = ansi((167, 139, 250));
    let pink = ansi((244, 114, 182));
    let slate = ansi((148, 163, 184));
    let dim = ansi((100, 116, 139));
    let bold = "\x1b[1m";
    let r = ANSI_RESET;
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
and toggles the sky's comets and shooting stars — see the readme.{r}
",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(args: &[&str]) -> Flags {
        match parse(args.iter().map(|s| s.to_string())).unwrap() {
            Outcome::Run(f) => f,
            _ => panic!("expected a run"),
        }
    }

    #[test]
    fn flags_combine_and_default() {
        assert_eq!(flags(&[]).kinds, [Kind::Display, Kind::Idle]);
        let f = flags(&["-di", "-t", "90m"]);
        assert_eq!(f.kinds, [Kind::Display, Kind::Idle]);
        assert_eq!(f.timeout, Some(Duration::from_secs(5400)));
        assert_eq!(flags(&["-mdt", "5"]).kinds, [Kind::Display, Kind::Disk]);
        assert_eq!(
            flags(&["--config=x.toml"]).config_path.as_deref(),
            Some("x.toml")
        );
        assert_eq!(
            flags(&["--config", "y.toml"]).config_path.as_deref(),
            Some("y.toml")
        );
        assert!(parse(["-td".to_string()].into_iter()).is_err());
        assert!(parse(["--config".to_string()].into_iter()).is_err());
    }

    #[test]
    fn empty_config_is_default() {
        let cfg = parse_file("").unwrap();
        assert!(cfg.docker.enabled);
        assert!(cfg.cards.is_empty());
        assert!(cfg.sky.motion);
        assert!(cfg.sky.flyers.is_empty());
        assert!(cfg.dashboard.combined);
        assert!(cfg.accounts.personal.is_empty());
    }

    #[test]
    fn accounts_group_by_name() {
        let cfg = parse_file(
            "[accounts]\npersonal = [\"me@example.com\"]\nenterprise = [\"acme\", \"work\"]",
        )
        .unwrap();
        assert_eq!(cfg.accounts.personal, ["me@example.com"]);
        assert_eq!(cfg.accounts.enterprise, ["acme", "work"]);
        let old = parse_file("[accounts]\nbusiness = [\"acme\"]").unwrap();
        assert_eq!(old.accounts.enterprise, ["acme"]);
        assert!(parse_file("[accounts]\npersonal = [\" \"]").is_err());
        assert!(parse_file("[accounts]\nhobby = [\"x\"]").is_err());
    }

    #[test]
    fn shipped_example_config_parses() {
        let cfg = parse_file(include_str!("../../cards/config.example.toml")).unwrap();
        assert_eq!(cfg.cards.len(), 1);
        assert!(cfg.accounts.personal.is_empty());
    }

    #[test]
    fn full_config_parses() {
        let cfg = parse_file(
            r#"
[docker]
enabled = false

[sky]
comets = false

[[card]]
title = "todos"
command = "echo 'ok|hello|world'"
glyph = "⌖"
accent = "violet"
side = "left"
interval = 30
"#,
        )
        .unwrap();
        assert!(!cfg.docker.enabled);
        assert_eq!(cfg.sky.flyers.get("comets"), Some(&false));
        assert_eq!(cfg.cards.len(), 1);
        assert_eq!(cfg.cards[0].interval, 30);
        assert_eq!(cfg.cards[0].accent, Some(Accent((167, 139, 250))));
        assert_eq!(cfg.cards[0].side, Side::Left);
    }

    #[test]
    fn motion_is_separate_from_flyer_toggles() {
        let cfg = parse_file("[sky]\nmotion = false\ncomets = true\n[dashboard]\ncombined = false")
            .unwrap();
        assert!(!cfg.sky.motion);
        assert!(!cfg.sky.flyers.contains_key("motion"));
        assert!(sky::resolve(&cfg.sky.flyers)[0]);
        assert!(!cfg.dashboard.combined);
    }

    #[test]
    fn bad_configs_error_helpfully() {
        assert!(parse_file("[sky]\ndragon = true").is_err());
        assert!(parse_file("[[card]]\ntitle = \"x\"\ncommand = \"\"").is_err());
        assert!(parse_file("[[card]]\ntitle = \"x\"\ncommand = \"echo\"\ninterval = 1").is_err());
        let err = parse_file("[[card]]\ntitle = \"x\"\ncommand = \"echo\"\naccent = \"mauve\"")
            .unwrap_err();
        assert!(
            err.contains("unknown accent 'mauve'") && err.contains("violet"),
            "{err}"
        );
        assert!(parse_file("[[card]]\ntitle = \"x\"\ncommand = \"echo\"\nside = \"up\"").is_err());
        assert!(parse_file("[dashboard]\nside = \"left\"").is_err());
        assert!(parse_file("nonsense = 1").is_err());
    }
}

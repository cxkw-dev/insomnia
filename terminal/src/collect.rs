//! Background data sources: everything slow — pmset, docker, custom card
//! commands — runs off the render thread and streams updates to the UI, so
//! the starfield never stutters on a stalled subprocess.
//!
//! Each source gets its own thread: a hanging command only ever delays its
//! own card, never the others and never the sky.

use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde::Serialize;

use crate::config::{accent_rgb, CardConfig};
use crate::power::PowerStatus;

const BASE_POLL: Duration = Duration::from_secs(10);
/// A custom card never renders more rows than this, however chatty its command.
const MAX_ROWS: usize = 32;

/// Ordered so that sorting puts the rows needing attention first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Bad,
    Warn,
    Good,
    /// Present but dormant — a steady slate dot instead of a pulsing one.
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerState {
    Running,
    Paused,
    Stopped,
    Restarting,
    Unknown,
}

impl ContainerState {
    pub fn from_docker(state: &str) -> Self {
        match state {
            "running" => Self::Running,
            "paused" => Self::Paused,
            "exited" | "dead" | "created" => Self::Stopped,
            "restarting" => Self::Restarting,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Container {
    pub state: ContainerState,
    pub name: String,
    pub status: String,
    pub health: Health,
}

/// One row of a custom card, parsed from a line of command output. The
/// default value is a blank spacer row: no dot, no text, no meter.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CardRow {
    pub container_state: Option<ContainerState>,
    pub health: Option<Health>,
    pub name: String,
    pub detail: Option<String>,
    /// Section headers (`hdr|accent|text`) carry their accent color here.
    pub accent: Option<(u8, u8, u8)>,
    /// `hdr` rows are visual section dividers. Other accent-only rows are
    /// card headers synthesized by the combined dashboard.
    pub section: bool,
    /// Utilization meters (`bar|accent|label|percent|detail`) carry a value
    /// from 0–100 here. The optional detail is typically a reset time.
    pub percent: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct CardData {
    pub rows: Vec<CardRow>,
}

pub enum Update {
    Power(PowerStatus),
    /// None: docker isn't installed or the daemon is down.
    Docker(Option<Vec<Container>>),
    /// None: the card's command failed. The index matches the config order.
    Card(usize, Option<CardData>),
}

/// Spawn one thread per data source, all reporting into a single channel.
/// Threads exit on their own once the receiver is dropped.
pub fn spawn(docker: bool, cards: Vec<CardConfig>) -> Receiver<Update> {
    let (tx, rx) = mpsc::channel();
    {
        let tx = tx.clone();
        thread::spawn(move || loop {
            if tx.send(Update::Power(PowerStatus::poll())).is_err() {
                break;
            }
            thread::sleep(BASE_POLL);
        });
    }
    // Docker gets its own thread: a wedged daemon can hang `docker ps` for a
    // long time, and that must never stall the power readings.
    if docker {
        let tx = tx.clone();
        thread::spawn(move || loop {
            if tx.send(Update::Docker(poll_docker())).is_err() {
                break;
            }
            thread::sleep(BASE_POLL);
        });
    }
    for (i, card) in cards.into_iter().enumerate() {
        let tx = tx.clone();
        thread::spawn(move || {
            let every = Duration::from_secs(card.interval.max(2));
            loop {
                if tx.send(Update::Card(i, poll_card(&card))).is_err() {
                    break;
                }
                thread::sleep(every);
            }
        });
    }
    rx
}

fn poll_docker() -> Option<Vec<Container>> {
    let out = Command::new("docker")
        .args([
            "ps",
            "--all",
            "--format",
            "{{.Names}}\t{{.State}}\t{{.Status}}",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut containers: Vec<Container> = stdout.lines().filter_map(parse_container).collect();
    containers.sort_by_key(|c| c.health);
    Some(containers)
}

fn parse_container(line: &str) -> Option<Container> {
    let mut parts = line.split('\t');
    let name = parts.next()?.to_string();
    let state = parts.next()?;
    let status = parts.next().unwrap_or_default();
    let health = if status.contains("(unhealthy)") || state == "dead" {
        Health::Bad
    } else if state == "exited" {
        if status.starts_with("Exited (0)") {
            Health::Off
        } else {
            Health::Bad
        }
    } else if state == "created" {
        Health::Off
    } else if state == "running" && !status.contains("health: starting") {
        Health::Good
    } else {
        Health::Warn
    };
    // Show lifecycle state explicitly and retain health checks and exit codes.
    let label = if state == "exited" { "stopped" } else { state };
    let status = status.to_lowercase();
    let status = if status.is_empty() {
        label.to_string()
    } else if status.starts_with(label) {
        status
    } else {
        format!("{label} · {status}")
    };
    Some(Container {
        state: ContainerState::from_docker(state),
        name,
        status,
        health,
    })
}

pub fn poll_card(cfg: &CardConfig) -> Option<CardData> {
    let out = Command::new("sh")
        .args(["-c", &cfg.command])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let rows = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(MAX_ROWS)
        .map(parse_row)
        .collect();
    Some(CardData { rows })
}

/// The card row protocol: `ok|name|detail`, `warn|name`, `bad|name|detail`,
/// and `off|name|detail` get a health dot and columns; `hdr|accent|text` is
/// a colored section header; `bar|accent|label|percent|detail` is an inline
/// utilization meter; anything else is a plain text row.
fn parse_row(line: &str) -> CardRow {
    let meter: Vec<&str> = line.splitn(5, '|').map(str::trim).collect();
    if meter.first() == Some(&"bar") && meter.len() >= 4 {
        if let (Some(rgb), Ok(percent)) = (accent_rgb(meter[1]), meter[3].parse::<u8>()) {
            if percent <= 100 {
                return CardRow {
                    name: meter[2].to_string(),
                    detail: meter
                        .get(4)
                        .filter(|d| !d.is_empty())
                        .map(|d| d.to_string()),
                    accent: Some(rgb),
                    percent: Some(percent),
                    ..CardRow::default()
                };
            }
        }
    }
    let parts: Vec<&str> = line.splitn(3, '|').map(str::trim).collect();
    if parts.first() == Some(&"hdr") && parts.len() == 3 {
        if let Some(rgb) = accent_rgb(parts[1]) {
            return CardRow {
                name: parts[2].to_string(),
                accent: Some(rgb),
                section: true,
                ..CardRow::default()
            };
        }
    }
    let health = match parts.first().copied() {
        Some("ok") => Some(Health::Good),
        Some("warn") => Some(Health::Warn),
        Some("bad") => Some(Health::Bad),
        Some("off") => Some(Health::Off),
        _ => None,
    };
    match health {
        Some(h) if parts.len() >= 2 => CardRow {
            health: Some(h),
            name: parts[1].to_string(),
            detail: parts
                .get(2)
                .filter(|d| !d.is_empty())
                .map(|d| d.to_string()),
            ..CardRow::default()
        },
        _ => CardRow {
            name: line.trim().to_string(),
            ..CardRow::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_with_health_and_columns() {
        let r = parse_row("ok|api|up 3 days");
        assert_eq!(r.health, Some(Health::Good));
        assert_eq!(r.name, "api");
        assert_eq!(r.detail.as_deref(), Some("up 3 days"));

        let r = parse_row("bad|worker");
        assert_eq!(r.health, Some(Health::Bad));
        assert_eq!(r.detail, None);
    }

    #[test]
    fn header_rows_take_an_accent() {
        let r = parse_row("hdr|coral|✻ claude");
        assert_eq!(r.health, None);
        assert_eq!(r.accent, Some((217, 119, 87)));
        assert_eq!(r.name, "✻ claude");
        assert!(r.section);
        assert_eq!(r.percent, None);

        // An unknown accent falls back to a plain text row.
        let r = parse_row("hdr|mauve|x");
        assert_eq!(r.accent, None);
        assert_eq!(r.name, "hdr|mauve|x");
    }

    #[test]
    fn bar_rows_carry_utilization_and_reset_detail() {
        let r = parse_row("bar|teal|weekly|72|resets Sun 03:49");
        assert_eq!(r.health, None);
        assert_eq!(r.name, "weekly");
        assert_eq!(r.detail.as_deref(), Some("resets Sun 03:49"));
        assert_eq!(r.accent, Some((16, 163, 127)));
        assert!(!r.section);
        assert_eq!(r.percent, Some(72));

        // Invalid percentages and accents remain visible as plain text.
        assert_eq!(parse_row("bar|teal|weekly|101").percent, None);
        assert_eq!(parse_row("bar|mauve|weekly|42").percent, None);
    }

    #[test]
    fn off_rows_are_dormant() {
        let r = parse_row("off|work|not logged in");
        assert_eq!(r.health, Some(Health::Off));
        assert_eq!(r.detail.as_deref(), Some("not logged in"));
    }

    #[test]
    fn plain_rows_pass_through() {
        let r = parse_row("just some text");
        assert_eq!(r.health, None);
        assert_eq!(r.name, "just some text");

        // A lone "ok" with no name is treated as plain text, not a health row.
        let r = parse_row("ok");
        assert_eq!(r.health, None);
        assert_eq!(r.name, "ok");
    }

    #[test]
    fn containers_parse_and_classify() {
        let c = parse_container("web\trunning\tUp 3 hours (healthy)").unwrap();
        assert_eq!(c.health, Health::Good);
        assert_eq!(c.status, "running · up 3 hours (healthy)");

        let c = parse_container("db\trunning\tUp 2 minutes (unhealthy)").unwrap();
        assert_eq!(c.health, Health::Bad);
        assert_eq!(c.status, "running · up 2 minutes (unhealthy)");

        let c = parse_container("job\trunning\tUp 1 second (health: starting)").unwrap();
        assert_eq!(c.health, Health::Warn);
        assert_eq!(c.status, "running · up 1 second (health: starting)");

        for (state, status, label, health) in [
            (
                "running",
                "Up 3 hours",
                "running · up 3 hours",
                Health::Good,
            ),
            (
                "exited",
                "Exited (0) 2 hours ago",
                "stopped · exited (0) 2 hours ago",
                Health::Off,
            ),
            (
                "exited",
                "Exited (1) 2 hours ago",
                "stopped · exited (1) 2 hours ago",
                Health::Bad,
            ),
            (
                "paused",
                "Up 3 hours (Paused)",
                "paused · up 3 hours (paused)",
                Health::Warn,
            ),
            (
                "restarting",
                "Restarting (1) 5 seconds ago",
                "restarting (1) 5 seconds ago",
                Health::Warn,
            ),
            ("created", "Created", "created", Health::Off),
            ("dead", "Dead", "dead", Health::Bad),
        ] {
            let c = parse_container(&format!("worker\t{state}\t{status}")).unwrap();
            assert_eq!(c.status, label);
            assert_eq!(c.health, health);
        }

        assert!(parse_container("garbage-no-tabs").is_none());
    }
}

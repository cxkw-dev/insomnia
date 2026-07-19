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

use crate::config::CardConfig;
use crate::power::PowerStatus;

const BASE_POLL: Duration = Duration::from_secs(10);
/// A custom card never renders more rows than this, however chatty its command.
const MAX_ROWS: usize = 32;

/// Ordered so that sorting puts the rows needing attention first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Health {
    Bad,
    Warn,
    Good,
}

#[derive(Clone, Debug)]
pub struct Container {
    pub name: String,
    pub status: String,
    pub health: Health,
}

/// One row of a custom card, parsed from a line of command output.
#[derive(Clone, Debug, PartialEq)]
pub struct CardRow {
    pub health: Option<Health>,
    pub name: String,
    pub detail: Option<String>,
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
        .args(["ps", "--format", "{{.Names}}\t{{.State}}\t{{.Status}}"])
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
    let health = if status.contains("(unhealthy)") || state == "exited" || state == "dead" {
        Health::Bad
    } else if state == "running" && !status.contains("health: starting") {
        Health::Good
    } else {
        Health::Warn
    };
    // The dot carries health in the UI, so drop the "(healthy)" parenthetical.
    let status = status.split(" (").next().unwrap_or(status).to_lowercase();
    Some(Container {
        name,
        status,
        health,
    })
}

fn poll_card(cfg: &CardConfig) -> Option<CardData> {
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

/// The card row protocol: `ok|name|detail`, `warn|name`, `bad|name|detail`
/// get a health dot and columns; anything else is a plain text row.
fn parse_row(line: &str) -> CardRow {
    let parts: Vec<&str> = line.splitn(3, '|').map(str::trim).collect();
    let health = match parts.first().copied() {
        Some("ok") => Some(Health::Good),
        Some("warn") => Some(Health::Warn),
        Some("bad") => Some(Health::Bad),
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
        },
        _ => CardRow {
            health: None,
            name: line.trim().to_string(),
            detail: None,
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
        assert_eq!(c.status, "up 3 hours");

        let c = parse_container("db\trunning\tUp 2 minutes (unhealthy)").unwrap();
        assert_eq!(c.health, Health::Bad);

        let c = parse_container("job\trunning\tUp 1 second (health: starting)").unwrap();
        assert_eq!(c.health, Health::Warn);

        assert!(parse_container("garbage-no-tabs").is_none());
    }
}

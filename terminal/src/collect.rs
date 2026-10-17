//! Background data sources: everything slow — pmset, docker, custom card
//! commands — runs off the render thread and streams updates to the UI, so
//! the starfield never stutters on a stalled subprocess.
//!
//! Each source gets its own thread: a hanging command only ever delays its
//! own card, never the others and never the sky, and a command that runs
//! past its deadline is killed along with everything it started.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::config::CardConfig;
use crate::power::PowerStatus;
use crate::theme::{accent_rgb, Rgb};

const BASE_POLL: Duration = Duration::from_secs(10);
/// A custom card shows at most this many of its command's rows; the rest
/// collapse into one "+N more rows" line.
pub const MAX_ROWS: usize = 64;
/// Meter values from here up read as a warning, and from [`BAD_PCT`] up as
/// critical — the same cutoffs the AI usage script uses for its dots.
pub const WARN_PCT: u8 = 75;
pub const BAD_PCT: u8 = 90;

/// Ordered so that sorting puts the rows needing attention first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Bad,
    Warn,
    Good,
    /// Present but dormant — a steady slate dot.
    Off,
}

/// How urgent a row is, from its own health and its meter, if either needs
/// attention.
pub fn severity(percent: Option<u8>, health: Option<Health>) -> Option<Health> {
    if percent >= Some(BAD_PCT) || health == Some(Health::Bad) {
        Some(Health::Bad)
    } else if percent >= Some(WARN_PCT) || health == Some(Health::Warn) {
        Some(Health::Warn)
    } else {
        None
    }
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

/// One dashboard row, parsed from a line of card output (or built from a
/// container). The default value is a blank spacer row. Serialized as-is
/// for the menu bar app, which reads `health`, `name`, `detail`, `accent`,
/// `section`, and `percent`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CardRow {
    pub container_state: Option<ContainerState>,
    pub health: Option<Health>,
    pub name: String,
    pub detail: Option<String>,
    /// The accent of a header or meter row.
    pub accent: Option<Rgb>,
    /// An `hdr|accent|text` row: a group header within a card, such as one
    /// provider in the AI usage card.
    #[serde(rename = "section")]
    pub header: bool,
    /// A `bar|accent|label|percent|detail` meter's 0–100 value. The detail
    /// is typically a reset time.
    pub percent: Option<u8>,
}

impl CardRow {
    /// A spacer: no dot, no text, no meter — just vertical room.
    pub fn is_blank(&self) -> bool {
        self.health.is_none()
            && self.percent.is_none()
            && !self.header
            && self.name.trim().is_empty()
    }
}

pub enum Update {
    Power(PowerStatus),
    /// None: docker isn't installed or the daemon is down.
    Docker(Option<Vec<Container>>),
    /// The card at this config index: its rows, or why the run failed.
    Card(usize, Result<Vec<CardRow>, String>),
}

/// Spawn one thread per data source, all reporting into a single channel.
/// Each thread exits after its next poll once the receiver is dropped.
/// Card commands get `timeout` to finish before they are killed.
pub fn spawn(docker: bool, cards: Vec<CardConfig>, timeout: Duration) -> Receiver<Update> {
    let (tx, rx) = mpsc::channel();
    poll_every(&tx, BASE_POLL, || Update::Power(PowerStatus::poll()));
    // Docker gets its own thread: a wedged daemon can hang `docker ps` for a
    // long time, and that must never stall the power readings.
    if docker {
        poll_every(&tx, BASE_POLL, || Update::Docker(poll_docker()));
    }
    for (i, card) in cards.into_iter().enumerate() {
        let every = Duration::from_secs(card.interval);
        poll_every(&tx, every, move || {
            Update::Card(i, poll_card(&card, timeout))
        });
    }
    rx
}

fn poll_every(
    tx: &Sender<Update>,
    every: Duration,
    mut poll: impl FnMut() -> Update + Send + 'static,
) {
    let tx = tx.clone();
    thread::spawn(move || {
        while tx.send(poll()).is_ok() {
            thread::sleep(every);
        }
    });
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

/// Run a card's command and parse its stdout into rows. The command runs in
/// its own process group, so on timeout everything it started — `curl`,
/// subshells — is killed with it rather than orphaned.
pub fn poll_card(cfg: &CardConfig, timeout: Duration) -> Result<Vec<CardRow>, String> {
    let mut child = Command::new("sh")
        .args(["-c", &cfg.command])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("could not start: {e}"))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait_or_kill(&mut child, timeout)
        .ok_or_else(|| format!("timed out after {}s", timeout.as_secs()))?;
    let stdout = stdout.join().unwrap_or_default();
    if !status.success() {
        let stderr = stderr.join().unwrap_or_default();
        let why = stderr.lines().map(str::trim).find(|l| !l.is_empty());
        return Err(match (status.code(), why) {
            (Some(code), Some(why)) => format!("exit {code}: {why}"),
            (Some(code), None) => format!("exit {code}"),
            (None, _) => "killed by a signal".into(),
        });
    }
    Ok(parse_rows(&stdout))
}

/// Read a pipe to the end on its own thread, so a chatty command can never
/// fill the pipe and deadlock against our wait.
fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn wait_or_kill(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    extern "C" {
        fn killpg(pgrp: i32, sig: i32) -> i32;
    }
    const SIGKILL: i32 = 9;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                // SAFETY: plain syscall; the group id is our child's pid,
                // which `process_group(0)` made a group leader.
                unsafe { killpg(child.id() as i32, SIGKILL) };
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn parse_rows(stdout: &str) -> Vec<CardRow> {
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut rows: Vec<CardRow> = lines.iter().take(MAX_ROWS).map(|l| parse_row(l)).collect();
    if lines.len() > MAX_ROWS {
        rows.push(CardRow {
            name: format!("+{} more rows", lines.len() - MAX_ROWS),
            ..CardRow::default()
        });
    }
    rows
}

/// The card row protocol, one row per line of stdout:
///
/// - `ok|name|detail`, `warn|…`, `bad|…`, `off|…` — a health row (detail
///   optional)
/// - `hdr|accent|text` — a group header in that accent
/// - `bar|accent|label|percent|detail` — a 0–100 meter (detail optional)
///
/// Anything else, including a malformed protocol line, is plain text.
pub fn parse_row(line: &str) -> CardRow {
    let text = || CardRow {
        name: line.trim().to_string(),
        ..CardRow::default()
    };
    let Some((tag, rest)) = line.split_once('|') else {
        return text();
    };
    let detail = |d: Option<&str>| d.map(str::trim).filter(|d| !d.is_empty()).map(String::from);
    match tag.trim() {
        "bar" => {
            let f: Vec<&str> = rest.splitn(4, '|').map(str::trim).collect();
            let percent = f.get(2).and_then(|p| p.parse::<f32>().ok());
            match (f.first().and_then(|a| accent_rgb(a)), percent) {
                (Some(accent), Some(p)) if (0.0..=100.0).contains(&p) && f.len() >= 3 => CardRow {
                    name: f[1].to_string(),
                    detail: detail(f.get(3).copied()),
                    accent: Some(accent),
                    percent: Some(p.round() as u8),
                    ..CardRow::default()
                },
                _ => text(),
            }
        }
        "hdr" => match rest.split_once('|') {
            Some((accent, name)) => match accent_rgb(accent.trim()) {
                Some(accent) => CardRow {
                    name: name.trim().to_string(),
                    accent: Some(accent),
                    header: true,
                    ..CardRow::default()
                },
                None => text(),
            },
            None => text(),
        },
        tag => {
            let health = match tag {
                "ok" => Health::Good,
                "warn" => Health::Warn,
                "bad" => Health::Bad,
                "off" => Health::Off,
                _ => return text(),
            };
            let (name, d) = match rest.split_once('|') {
                Some((name, d)) => (name, Some(d)),
                None => (rest, None),
            };
            if name.trim().is_empty() {
                return text();
            }
            CardRow {
                health: Some(health),
                name: name.trim().to_string(),
                detail: detail(d),
                ..CardRow::default()
            }
        }
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
        assert!(r.header);
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
        assert!(!r.header);
        assert_eq!(r.percent, Some(72));
        assert_eq!(parse_row("bar|teal|weekly|72.6").percent, Some(73));

        // Invalid percentages and accents remain visible as plain text.
        assert_eq!(parse_row("bar|teal|weekly|101").percent, None);
        assert_eq!(parse_row("bar|mauve|weekly|42").percent, None);
        assert_eq!(parse_row("bar|teal|weekly").name, "bar|teal|weekly");
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
        assert_eq!(parse_row("ok|").health, None);
        assert_eq!(parse_row("ok|a|b|c").detail.as_deref(), Some("b|c"));
    }

    fn card(command: &str) -> CardConfig {
        CardConfig {
            title: "t".into(),
            command: command.into(),
            interval: 10,
            glyph: None,
            accent: None,
            side: Default::default(),
        }
    }

    #[test]
    fn card_commands_report_failures_and_time_out() {
        let ok = poll_card(&card("echo 'ok|a|b'; echo plain"), Duration::from_secs(5)).unwrap();
        assert_eq!(ok.len(), 2);
        let err = poll_card(&card("echo nope >&2; exit 3"), Duration::from_secs(5)).unwrap_err();
        assert_eq!(err, "exit 3: nope");
        let started = Instant::now();
        let err = poll_card(&card("sleep 30 & sleep 30"), Duration::from_millis(300)).unwrap_err();
        assert!(err.starts_with("timed out"), "{err}");
        // The background `sleep` held stdout too; killing the whole group is
        // what lets the reader finish instead of waiting it out.
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn long_output_is_capped_with_a_visible_remainder() {
        let out: String = (0..MAX_ROWS + 5).map(|i| format!("row {i}\n")).collect();
        let rows = parse_rows(&out);
        assert_eq!(rows.len(), MAX_ROWS + 1);
        assert_eq!(rows.last().unwrap().name, "+5 more rows");
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

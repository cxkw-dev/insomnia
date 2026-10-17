//! One-shot card data for the native menu bar app. The CLI and app share the
//! same config loader, command runner, and row parser.

use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use insomnia::{collect, config};
use serde::Serialize;

/// Per-card deadline. The app gives the whole snapshot 20s, and cards run
/// in parallel, so this leaves room to report a slow card instead of being
/// killed with nothing to say.
const CARD_TIMEOUT: Duration = Duration::from_secs(15);

const USAGE: &str = "usage: insomnia-snapshot [--config <path>]";

#[derive(Serialize)]
struct SnapshotCard<'a> {
    title: &'a str,
    rows: Vec<collect::CardRow>,
    error: Option<String>,
}

#[derive(Serialize)]
struct Snapshot<'a> {
    cards: Vec<SnapshotCard<'a>>,
    accounts: &'a config::AccountsConfig,
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut explicit = None;
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        match config::config_arg(&arg, &mut args) {
            Some(Ok(path)) => explicit = Some(path),
            Some(Err(e)) => return fail(&e),
            None => return fail(USAGE),
        }
    }

    let cfg = match config::load_file(explicit.as_deref()) {
        Ok(cfg) => cfg,
        Err(e) => return fail(&e),
    };
    let cards = thread::scope(|scope| {
        let runs: Vec<_> = cfg
            .cards
            .iter()
            .map(|card| (card, scope.spawn(|| collect::poll_card(card, CARD_TIMEOUT))))
            .collect();
        runs.into_iter()
            .map(|(card, run)| {
                let result = run.join().unwrap_or_else(|_| Err("crashed".into()));
                let (rows, error) = match result {
                    Ok(rows) => (rows, None),
                    Err(e) => (Vec::new(), Some(format!("Card command failed: {e}"))),
                };
                SnapshotCard {
                    title: &card.title,
                    rows,
                    error,
                }
            })
            .collect()
    });
    let snapshot = Snapshot {
        cards,
        accounts: &cfg.accounts,
    };
    match serde_json::to_writer(std::io::stdout(), &snapshot) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("insomnia-snapshot: {e}");
            ExitCode::FAILURE
        }
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("insomnia-snapshot: {message}");
    ExitCode::from(2)
}

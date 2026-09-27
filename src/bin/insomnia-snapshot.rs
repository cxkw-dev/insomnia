//! One-shot card data for the native menu bar app. The CLI and app share the
//! same config loader, command runner, and row parser.

use std::process::ExitCode;

use insomnia::{collect, config};
use serde::Serialize;

#[derive(Serialize)]
struct SnapshotCard<'a> {
    title: &'a str,
    rows: Vec<collect::CardRow>,
    error: Option<&'static str>,
}

#[derive(Serialize)]
struct Snapshot<'a> {
    cards: Vec<SnapshotCard<'a>>,
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let explicit = match args.next() {
        None => None,
        Some(flag) if flag == "--config" => match args.next() {
            Some(path) => Some(path),
            None => {
                eprintln!("insomnia-snapshot: --config needs a path");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!("usage: insomnia-snapshot [--config path]");
            return ExitCode::from(2);
        }
    };
    if args.next().is_some() {
        eprintln!("usage: insomnia-snapshot [--config path]");
        return ExitCode::from(2);
    }

    let cfg = match config::load_file(explicit.as_deref()) {
        Ok(cfg) => cfg,
        Err(error) => {
            eprintln!("insomnia-snapshot: {error}");
            return ExitCode::from(2);
        }
    };
    let cards = cfg
        .cards
        .iter()
        .map(|card| match collect::poll_card(card) {
            Some(data) => SnapshotCard {
                title: &card.title,
                rows: data.rows,
                error: None,
            },
            None => SnapshotCard {
                title: &card.title,
                rows: Vec::new(),
                error: Some("Card command failed"),
            },
        })
        .collect();
    match serde_json::to_writer(std::io::stdout(), &Snapshot { cards }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("insomnia-snapshot: {error}");
            ExitCode::FAILURE
        }
    }
}

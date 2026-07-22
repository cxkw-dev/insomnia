// Renders one frame to an in-memory backend and prints it, for eyeballing the
// layout without a live terminal: cargo run --example preview [width height tick]
use std::time::Duration;

use insomnia::collect::{CardRow, Health};
use insomnia::power::{Kind, PowerStatus};
use insomnia::sky;
use insomnia::ui::{draw, SideCard, ViewState};
use ratatui::{backend::TestBackend, Terminal};

fn main() {
    let mut args = std::env::args().skip(1);
    let w: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(110);
    let h: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(40);
    let tick: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(7);

    let plain = |name: &str| CardRow {
        health: None,
        name: name.into(),
        detail: None,
        accent: None,
    };
    let status = |health, name: &str, detail: &str| CardRow {
        health: Some(health),
        name: name.into(),
        detail: Some(detail.into()),
        accent: None,
    };
    let hdr = |accent: (u8, u8, u8), name: &str| CardRow {
        health: None,
        name: name.into(),
        detail: None,
        accent: Some(accent),
    };

    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    let state = ViewState {
        tick,
        elapsed: Duration::from_secs(2537),
        total: Some(Duration::from_secs(7200)),
        remaining: Some(Duration::from_secs(4663)),
        kinds: vec![Kind::Display, Kind::Idle],
        power: PowerStatus {
            on_ac: true,
            percent: Some(100),
        },
        // Mirrors `[dashboard] combined = true`: one pre-folded card — section
        // headers, long account names, details wide enough to force growth —
        // rendered inside the status box.
        cards: vec![SideCard {
            title: "dashboard".into(),
            glyph: "✦".into(),
            accent: (148, 163, 184),
            rows: vec![
                hdr((148, 163, 184), "✦ ai usage"),
                hdr((217, 119, 87), "✻ claude"),
                status(Health::Good, "▸ cxkw.dev", "5h 9% · wk 42%"),
                hdr((16, 163, 127), "◎ openai codex"),
                status(Health::Warn, "▸ cxkw.dev", "pro · 5h 12% · wk 78%"),
                status(Health::Off, "  andy.nguyen", "not logged in"),
                hdr((167, 139, 250), "⧉ copilot"),
                status(Health::Good, "▸ andy-nguyen-cxkw", "biz · unlimited"),
                plain(""),
                hdr((59, 130, 246), "≋ docker"),
                status(Health::Warn, "worker", "restarting"),
                status(Health::Good, "web", "up 3 hours"),
                status(Health::Good, "postgres", "up 3 hours"),
                status(Health::Good, "redis", "up 3 hours"),
                status(Health::Good, "builder", "up 2 hours"),
            ],
            summary: "2 sections".into(),
            left: false,
        }],
        merged: true,
        sky: vec![true; sky::FLYERS.len()],
    };
    terminal.draw(|f| draw(f, &state)).unwrap();

    let buf = terminal.backend().buffer();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        println!("{}", line.trim_end());
    }
}

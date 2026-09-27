// Renders one frame to an in-memory backend and prints it, for eyeballing the
// layout without a live terminal: cargo run --example preview [width height tick]
use std::time::Duration;

use insomnia::cards::SideCard;
use insomnia::collect::{CardRow, Health};
use insomnia::power::{Kind, PowerStatus};
use insomnia::sky;
use insomnia::ui::{draw, ViewState};
use ratatui::{backend::TestBackend, Terminal};

fn main() {
    let mut args = std::env::args().skip(1);
    let w: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(110);
    let h: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(40);
    let tick: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(7);
    let html = args.next().as_deref() == Some("--html");

    let plain = |name: &str| CardRow {
        name: name.into(),
        ..CardRow::default()
    };
    let status = |health, name: &str, detail: &str| CardRow {
        health: Some(health),
        name: name.into(),
        detail: Some(detail.into()),
        ..CardRow::default()
    };
    let container = |state: &str, health, name: &str, detail: &str| CardRow {
        container_state: Some(insomnia::collect::ContainerState::from_docker(state)),
        ..status(health, name, detail)
    };
    let hdr = |accent: (u8, u8, u8), name: &str| CardRow {
        name: name.into(),
        accent: Some(accent),
        section: true,
        ..CardRow::default()
    };
    let group = |accent: (u8, u8, u8), name: &str| CardRow {
        name: name.into(),
        accent: Some(accent),
        ..CardRow::default()
    };
    let bar = |accent: (u8, u8, u8), name: &str, percent, detail: &str| CardRow {
        name: name.into(),
        detail: Some(detail.into()),
        accent: Some(accent),
        percent: Some(percent),
        ..CardRow::default()
    };

    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    let mut state = ViewState {
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
                group((148, 163, 184), "✦ ai usage"),
                hdr((217, 119, 87), "✻ claude"),
                status(
                    Health::Warn,
                    "▸ work account",
                    "enterprise · $25 left · active",
                ),
                bar((217, 119, 87), "monthly spend", 87, "$175 / $200"),
                status(Health::Good, "  personal account", "42% used"),
                bar((217, 119, 87), "5h window", 9, "reset in 2h · today 14:30"),
                bar((217, 119, 87), "weekly", 42, "reset in 3d · Sun 03:49"),
                bar(
                    (217, 119, 87),
                    "Fable weekly",
                    100,
                    "reset in 2d · Sat 11:59",
                ),
                // Blank rows between services: what `space_sections` inserts.
                plain(""),
                hdr((16, 163, 127), "◎ codex"),
                status(Health::Bad, "▸ personal account", "pro · 96% used · active"),
                bar((16, 163, 127), "weekly", 96, "reset in 4d · Mon 09:00"),
                status(
                    Health::Warn,
                    "  work account",
                    "business · 95% used · stale 1d",
                ),
                bar(
                    (16, 163, 127),
                    "monthly credits",
                    95,
                    "billing in 1d · Fri 19:00",
                ),
                plain(""),
                hdr((167, 139, 250), "⧉ copilot"),
                status(Health::Good, "▸ example team", "biz · unlimited"),
                plain(""),
                group((59, 130, 246), "≋ docker"),
                container("restarting", Health::Warn, "worker", "restarting"),
                container(
                    "running",
                    Health::Good,
                    "web",
                    "running · up 3 hours (healthy)",
                ),
                container(
                    "running",
                    Health::Good,
                    "postgres",
                    "running · up 3 hours (healthy)",
                ),
                container("running", Health::Good, "redis", "running · up 3 hours"),
                container(
                    "exited",
                    Health::Off,
                    "builder",
                    "stopped · exited (0) 2 hours ago",
                ),
                container(
                    "paused",
                    Health::Warn,
                    "scheduler",
                    "paused · up 2 hours (paused)",
                ),
            ],
            summary: "2 sections".into(),
            left: false,
        }],
        merged: true,
        sky: vec![true; sky::FLYERS.len()],
        motion: true,
        navigation: Default::default(),
    };
    let dashboard = insomnia::dashboard::Dashboard::from_cards(&state.cards, true);
    state.navigation.selected = dashboard.entries().nth(2).map(|e| e.key.clone());
    terminal.draw(|f| draw(f, &state)).unwrap();

    let buf = terminal.backend().buffer();
    if html {
        print_html(buf);
        return;
    }
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        println!("{}", line.trim_end());
    }
}

// Export the actual ratatui cells for color/layout inspection in a browser.
// This uses the same draw path as the terminal, with only fixture data.
fn print_html(buf: &ratatui::buffer::Buffer) {
    use ratatui::style::{Color, Modifier};
    fn css(color: Color, fallback: &str) -> String {
        match color {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => fallback.into(),
        }
    }
    println!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>insomnia preview</title><style>body{{margin:0;background:#11121d}}pre{{margin:0;font:14px/1.65 ui-monospace,Menlo,monospace;white-space:pre}}span{{display:inline-block;width:1ch}}</style><pre aria-label=\"insomnia terminal preview with sample data\">");
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            let symbol = cell
                .symbol()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            print!(
                "<span style=\"color:{};background:{};font-weight:{}\">{}</span>",
                css(cell.fg, "#e2e8f0"),
                css(cell.bg, "#11121d"),
                if cell.modifier.contains(Modifier::BOLD) {
                    "bold"
                } else {
                    "normal"
                },
                symbol
            );
        }
        println!();
    }
    println!("</pre></html>");
}

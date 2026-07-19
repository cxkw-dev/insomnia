use std::process::exit;

use insomnia::config::{self, Config, Outcome};
use insomnia::power::{self, Kind};
use insomnia::ui;

fn main() {
    let outcome = match config::parse(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("insomnia: {e}");
            exit(2);
        }
    };
    let cfg = match outcome {
        Outcome::Help => {
            print!("{}", config::help());
            return;
        }
        Outcome::Version => {
            println!("insomnia {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Outcome::Run(cfg) => cfg,
    };
    let file_cfg = match config::load_file(cfg.config_path.as_deref()) {
        Ok(fc) => fc,
        Err(e) => {
            eprintln!("insomnia: {e}");
            exit(2);
        }
    };

    let kinds = cfg.kinds();
    let assertions: Vec<power::Assertion> = match kinds.iter().map(|k| power::create(*k)).collect()
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("insomnia: {e}");
            exit(1);
        }
    };

    if cfg.quiet {
        quiet_hold(&cfg, &kinds);
    } else {
        match ui::run(&cfg, &kinds, &file_cfg) {
            Ok(elapsed) => println!(
                "\x1b[38;2;167;139;250m☾ insomnia\x1b[0m released — awake for {}. sleep well.",
                ui::fmt_hms(elapsed)
            ),
            Err(e) => {
                eprintln!("insomnia: terminal error: {e}");
                exit(1);
            }
        }
    }
    drop(assertions);
}

fn quiet_hold(cfg: &Config, kinds: &[Kind]) {
    let names = kinds
        .iter()
        .map(|k| k.label())
        .collect::<Vec<_>>()
        .join(" + ");
    let until = match cfg.timeout {
        Some(d) => format!("for {}", config::human(d)),
        None => "until you ctrl-c".into(),
    };
    println!("\x1b[38;2;167;139;250m☾ insomnia\x1b[0m — keeping {names} awake {until}");
    match cfg.timeout {
        Some(d) => {
            std::thread::sleep(d);
            println!(
                "\x1b[38;2;167;139;250m☾ insomnia\x1b[0m released after {}. sleep well.",
                config::human(d)
            );
        }
        None => loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        },
    }
}

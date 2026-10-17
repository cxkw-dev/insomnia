use std::process::ExitCode;

use insomnia::config::{self, Flags, Outcome};
use insomnia::duration;
use insomnia::power;
use insomnia::theme::{ansi, ANSI_RESET, VIOLET_RGB};
use insomnia::ui;

fn main() -> ExitCode {
    let flags = match config::parse(std::env::args().skip(1)) {
        Ok(Outcome::Run(flags)) => flags,
        Ok(Outcome::Help) => {
            print!("{}", config::help());
            return ExitCode::SUCCESS;
        }
        Ok(Outcome::Version) => {
            println!("insomnia {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(e) => return fail(&e, 2),
    };
    let file_cfg = match config::load_file(flags.config_path.as_deref()) {
        Ok(fc) => fc,
        Err(e) => return fail(&e, 2),
    };
    // Held until main returns; dropping them is what releases the Mac.
    let _assertions: Vec<power::Assertion> =
        match flags.kinds.iter().map(|k| power::create(*k)).collect() {
            Ok(a) => a,
            Err(e) => return fail(&e, 1),
        };

    if flags.quiet {
        quiet_hold(&flags);
        return ExitCode::SUCCESS;
    }
    match ui::run(&flags, &file_cfg) {
        Ok(elapsed) => {
            println!(
                "{} RELEASED — AWAKE FOR {}. SLEEP WELL.",
                wordmark(),
                duration::clock(elapsed)
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(&format!("terminal error: {e}"), 1),
    }
}

fn fail(message: &str, code: u8) -> ExitCode {
    eprintln!("insomnia: {message}");
    ExitCode::from(code)
}

fn wordmark() -> String {
    format!("{}☾ INSOMNIA{ANSI_RESET}", ansi(VIOLET_RGB))
}

fn quiet_hold(flags: &Flags) {
    let names = flags
        .kinds
        .iter()
        .map(|k| k.label().to_uppercase())
        .collect::<Vec<_>>()
        .join(" + ");
    let until = match flags.timeout {
        Some(d) => format!("FOR {}", duration::human(d).to_uppercase()),
        None => "UNTIL YOU CTRL-C".into(),
    };
    println!("{} — KEEPING {names} AWAKE {until}", wordmark());
    match flags.timeout {
        Some(d) => {
            std::thread::sleep(d);
            println!(
                "{} RELEASED AFTER {}. SLEEP WELL.",
                wordmark(),
                duration::human(d).to_uppercase()
            );
        }
        None => loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        },
    }
}

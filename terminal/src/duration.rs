//! Durations as people say them (`90s`, `45m`, `1h30m`) and as the UI shows
//! them (`01:17:43`, `1h 30m`).

use std::time::Duration;

/// Ten years, in seconds. Longer than any vigil — and comfortably inside the
/// range where `Instant + Duration` stays safe.
const MAX_SECS: u64 = 10 * 365 * 24 * 3600;
const TOO_LONG: &str = "duration too long — insomnia tops out at ten years";

/// A bare number is seconds (matching `caffeinate -t`); otherwise any run of
/// `<n>h`, `<n>m`, `<n>s`.
pub fn parse(s: &str) -> Result<Duration, String> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return Err("empty duration".into());
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        let secs: u64 = s.parse().map_err(|_| format!("bad duration '{s}'"))?;
        return finish(secs);
    }
    let mut total: u64 = 0;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let n: u64 = num
            .parse()
            .map_err(|_| format!("bad duration '{s}' (try 90s, 45m, 2h, 1h30m)"))?;
        num.clear();
        let unit = match c {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return Err(format!("unknown unit '{c}' in '{s}' (use h, m, or s)")),
        };
        total = n
            .checked_mul(unit)
            .and_then(|v| total.checked_add(v))
            .ok_or(TOO_LONG)?;
    }
    if !num.is_empty() {
        return Err(format!(
            "trailing number without unit in '{s}' (use h, m, or s)"
        ));
    }
    finish(total)
}

fn finish(secs: u64) -> Result<Duration, String> {
    if secs == 0 {
        return Err("duration must be greater than zero".into());
    }
    if secs > MAX_SECS {
        return Err(TOO_LONG.into());
    }
    Ok(Duration::from_secs(secs))
}

/// `1h 30m`, `45s`, `1h 0m 5s` — every unit from the largest non-zero one.
pub fn human(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 || (h > 0 && sec > 0) {
        parts.push(format!("{m}m"));
    }
    if sec > 0 || parts.is_empty() {
        parts.push(format!("{sec}s"));
    }
    parts.join(" ")
}

/// `HH:MM:SS`, for the live timers.
pub fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_way_people_say_it() {
        assert_eq!(parse("300").unwrap(), Duration::from_secs(300));
        assert_eq!(parse("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse("45m").unwrap(), Duration::from_secs(2700));
        assert_eq!(parse("1h30m").unwrap(), Duration::from_secs(5400));
        assert!(parse("0").is_err());
        assert!(parse("5x").is_err());
        assert!(parse("90").is_ok());
        // Absurd values error instead of overflowing.
        assert!(parse("18446744073709551615").is_err());
        assert!(parse("10000000000000000h").is_err());
        assert!(parse("87600h").is_ok()); // exactly ten years
    }

    #[test]
    fn human_keeps_every_unit_that_matters() {
        assert_eq!(human(Duration::from_secs(45)), "45s");
        assert_eq!(human(Duration::from_secs(5400)), "1h 30m");
        assert_eq!(human(Duration::from_secs(3605)), "1h 0m 5s");
        assert_eq!(human(Duration::from_secs(3600)), "1h");
        assert_eq!(human(Duration::ZERO), "0s");
        assert_eq!(clock(Duration::from_secs(4663)), "01:17:43");
    }
}

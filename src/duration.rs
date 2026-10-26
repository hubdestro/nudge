//! Human-friendly durations: `90`, `90s`, `2m`, `1h30m`, `1h 30m`.

use std::time::Duration;

pub fn parse(s: &str) -> Result<Duration, String> {
    let err = || format!("invalid duration {s:?} (examples: 45s, 2m, 1h30m)");
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    if compact.is_empty() {
        return Err(err());
    }
    if let Ok(secs) = compact.parse::<u64>() {
        return Ok(Duration::from_secs(secs));
    }
    let mut total = 0u64;
    let mut num = String::new();
    let mut chars = compact.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let mut unit = c.to_string();
        while let Some(&next) = chars.peek() {
            if next.is_ascii_alphabetic() {
                unit.push(next);
                chars.next();
            } else {
                break;
            }
        }
        let n: u64 = num.parse().map_err(|_| err())?;
        let mult = match unit.as_str() {
            "s" | "sec" | "secs" | "second" | "seconds" => 1,
            "m" | "min" | "mins" | "minute" | "minutes" => 60,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
            _ => return Err(err()),
        };
        total = total.checked_add(n.checked_mul(mult).ok_or_else(err)?).ok_or_else(err)?;
        num.clear();
    }
    if !num.is_empty() {
        return Err(err());
    }
    Ok(Duration::from_secs(total))
}

/// `1h 30m`, `2m`, `45s`, `1m 5s`.
pub fn format(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    let mut parts = vec![];
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    if sec > 0 || parts.is_empty() {
        parts.push(format!("{sec}s"));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        let secs = |s| parse(s).unwrap().as_secs();
        assert_eq!(secs("90"), 90);
        assert_eq!(secs("90s"), 90);
        assert_eq!(secs("2m"), 120);
        assert_eq!(secs("1h30m"), 5400);
        assert_eq!(secs("1h 30m"), 5400);
        assert_eq!(secs("2 hours"), 7200);
        assert_eq!(secs("1H"), 3600);
        assert_eq!(secs("1m5s"), 65);
    }

    #[test]
    fn rejects() {
        for bad in ["", "m", "5x", "1.5h", "10m5", "-5m", "h1"] {
            assert!(parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn formats() {
        let f = |s| format(Duration::from_secs(s));
        assert_eq!(f(0), "0s");
        assert_eq!(f(45), "45s");
        assert_eq!(f(120), "2m");
        assert_eq!(f(65), "1m 5s");
        assert_eq!(f(5400), "1h 30m");
        assert_eq!(f(3601), "1h 1s");
    }
}

//! Copy formatting that must match the UI (DESIGN.md §1 Type, PRD §7 Voice).

use crate::types::{MS_PER_DAY, Timestamp};

/// Decimal units (GB = 10⁹). One decimal under 100 ("12.8 GB"), none above ("412 GB").
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} {}", if bytes == 1 { "byte" } else { "bytes" });
    }
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 999.95 && u < UNITS.len() - 1 {
        v /= 1000.0;
        u += 1;
    }
    if v < 99.95 {
        format!("{v:.1} {}", UNITS[u])
    } else {
        format!("{v:.0} {}", UNITS[u])
    }
}

/// "1,912"
pub fn format_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// "3 days", "8 months", "2 years"
pub fn duration_phrase(ms: i64) -> String {
    let days = (ms / MS_PER_DAY).max(0);
    let plural = |n: i64, unit: &str| {
        if n == 1 {
            format!("1 {unit}")
        } else {
            format!("{n} {unit}s")
        }
    };
    if days < 1 {
        let hours = (ms / 3_600_000).max(0);
        if hours < 1 {
            return "less than an hour".into();
        }
        return plural(hours, "hour");
    }
    if days < 45 {
        return plural(days, "day");
    }
    if days < 365 {
        return plural((days as f64 / 30.44).round() as i64, "month");
    }
    plural((days as f64 / 365.25).floor() as i64, "year")
}

/// "9 days ago", "today", "8 months ago"
pub fn age_phrase(now: Timestamp, then: Timestamp) -> String {
    let d = now - then;
    if (0..MS_PER_DAY).contains(&d) {
        return "today".into();
    }
    if d < 0 {
        return "just now".into();
    }
    format!("{} ago", duration_phrase(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_match_design() {
        assert_eq!(format_bytes(12_800_000_000), "12.8 GB");
        assert_eq!(format_bytes(412_000_000_000), "412 GB");
        assert_eq!(format_bytes(41_000_000), "41.0 MB");
        assert_eq!(format_bytes(999), "999 bytes");
        assert_eq!(format_bytes(2_100_000_000), "2.1 GB");
    }

    #[test]
    fn counts_and_ages() {
        assert_eq!(format_count(1912), "1,912");
        assert_eq!(format_count(1_240_000), "1,240,000");
        assert_eq!(duration_phrase(9 * MS_PER_DAY), "9 days");
        assert_eq!(duration_phrase(240 * MS_PER_DAY), "8 months");
        assert_eq!(duration_phrase(800 * MS_PER_DAY), "2 years");
        assert_eq!(age_phrase(10 * MS_PER_DAY, MS_PER_DAY), "9 days ago");
    }
}

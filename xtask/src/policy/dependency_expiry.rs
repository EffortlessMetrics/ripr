use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const EXPIRY_MARKER: &str = "expires:";

pub(crate) fn check_dependency_suppression_expiry(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let today = utc_date_string();
    check_dependency_suppression_expiry_text(&text, &today)
}

fn check_dependency_suppression_expiry_text(text: &str, today: &str) -> Result<(), String> {
    let mut in_advisories = false;
    let mut in_ignore = false;
    let mut checked = 0usize;

    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_advisories = trimmed == "[advisories]";
            in_ignore = false;
            continue;
        }
        if in_advisories && trimmed.starts_with("ignore = [") {
            in_ignore = true;
            continue;
        }
        if in_advisories && in_ignore && trimmed.starts_with(']') {
            in_ignore = false;
            continue;
        }
        if !in_advisories || !in_ignore || trimmed.starts_with('#') {
            continue;
        }

        let Some(quote) = trimmed
            .chars()
            .next()
            .filter(|quote| *quote == '"' || *quote == '\'')
        else {
            return Err(format!(
                "deny.toml:{}: malformed advisory ignore entry",
                index + 1
            ));
        };
        let Some(remainder) = trimmed.get(1..) else {
            return Err(format!(
                "deny.toml:{}: malformed advisory ignore entry",
                index + 1
            ));
        };
        let Some(id_end) = remainder.find(quote) else {
            return Err(format!(
                "deny.toml:{}: malformed advisory ignore entry",
                index + 1
            ));
        };
        let Some(id) = trimmed.get(1..id_end + 1) else {
            return Err(format!(
                "deny.toml:{}: malformed advisory ignore entry",
                index + 1
            ));
        };
        if !id.starts_with("RUSTSEC-") {
            continue;
        }
        checked += 1;

        let Some(marker_start) = trimmed.find(EXPIRY_MARKER) else {
            return Err(format!(
                "deny.toml:{}: advisory {id} is missing an `{EXPIRY_MARKER} YYYY-MM-DD` expiry",
                index + 1
            ));
        };
        let Some(expiry) = trimmed
            .get(marker_start + EXPIRY_MARKER.len()..)
            .map(str::trim)
        else {
            return Err(format!(
                "deny.toml:{}: advisory {id} has an invalid expiry",
                index + 1
            ));
        };
        if !is_iso_date(expiry) {
            return Err(format!(
                "deny.toml:{}: advisory {id} has invalid expiry `{expiry}`",
                index + 1
            ));
        }
        if expiry <= today {
            return Err(format!(
                "deny.toml:{}: advisory {id} expiry `{expiry}` is not after today `{today}`",
                index + 1
            ));
        }
    }

    if checked == 0 {
        return Err("deny.toml: [advisories].ignore contains no RUSTSEC entries".to_string());
    }
    Ok(())
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return false;
    }
    let Some(year) = parse_digits(bytes, 0, 4) else {
        return false;
    };
    let Some(month) = parse_digits(bytes, 5, 7) else {
        return false;
    };
    let Some(day) = parse_digits(bytes, 8, 10) else {
        return false;
    };
    if year == 0 {
        return false;
    }
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days_in_month).contains(&day)
}

fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100))
}

fn parse_digits(bytes: &[u8], start: usize, end: usize) -> Option<u16> {
    let mut value = 0u16;
    for &byte in bytes.get(start..end)? {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u16::from(byte - b'0');
    }
    Some(value)
}

fn utc_date_string() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{check_dependency_suppression_expiry, check_dependency_suppression_expiry_text};

    #[test]
    fn checks_a_real_policy_file_using_the_current_utc_date() {
        let path = std::env::temp_dir().join(format!(
            "ripr-dependency-expiry-{}.toml",
            std::process::id()
        ));
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 9999-12-31\n]\n";
        let write_result = fs::write(&path, text);
        assert!(
            write_result.is_ok(),
            "failed to write test policy: {write_result:?}"
        );

        let result = check_dependency_suppression_expiry(&path);
        let remove_result = fs::remove_file(&path);
        assert!(
            remove_result.is_ok(),
            "failed to remove test policy: {remove_result:?}"
        );
        assert!(result.is_ok(), "real policy check failed: {result:?}");
    }

    #[test]
    fn reports_a_missing_policy_file() {
        let path = std::env::temp_dir().join(format!(
            "ripr-dependency-expiry-missing-{}.toml",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let result = check_dependency_suppression_expiry(Path::new(&path));
        assert!(result.is_err());
    }

    #[test]
    fn rejects_a_malformed_advisory_entry() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("malformed"))
        );
    }

    #[test]
    fn rejects_an_invalid_expiry_date() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-13-01\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("invalid expiry"))
        );
    }

    #[test]
    fn rejects_a_short_expiry_date() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-07\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("invalid expiry"))
        );
    }

    #[test]
    fn rejects_a_literal_string_without_expiry() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-10-01\n  'RUSTSEC-2025-0002'\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(result.err().is_some_and(
            |message| message.contains("RUSTSEC-2025-0002") && message.contains("missing")
        ));
    }

    #[test]
    fn closes_ignore_array_with_trailing_comment() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-10-01\n] # keep this list bounded\n";
        assert!(matches!(
            check_dependency_suppression_expiry_text(text, "2026-07-28"),
            Ok(())
        ));
    }

    #[test]
    fn rejects_an_impossible_calendar_date() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-02-31\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-01-01");
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("invalid expiry"))
        );
    }

    #[test]
    fn accepts_a_leap_year_expiry_date() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2028-02-29\n]\n";
        assert!(matches!(
            check_dependency_suppression_expiry_text(text, "2026-01-01"),
            Ok(())
        ));
    }

    #[test]
    fn rejects_expiry_with_trailing_text() {
        let text =
            "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-10-01 review\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("invalid expiry"))
        );
    }

    #[test]
    fn rejects_an_ignore_list_without_rustsec_entries() {
        let text = "[advisories]\nignore = [\n  \"other-advisory\"\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("contains no RUSTSEC entries"))
        );
    }

    #[test]
    fn accepts_future_expiry_for_each_advisory() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-10-01\n]\n";
        assert!(matches!(
            check_dependency_suppression_expiry_text(text, "2026-07-28"),
            Ok(())
        ));
    }

    #[test]
    fn rejects_advisory_without_expiry() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\"\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(result.is_err());
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("missing"))
        );
    }

    #[test]
    fn rejects_expired_advisory() {
        let text = "[advisories]\nignore = [\n  \"RUSTSEC-2025-0001\", # expires: 2026-07-28\n]\n";
        let result = check_dependency_suppression_expiry_text(text, "2026-07-28");
        assert!(result.is_err());
        assert!(
            result
                .err()
                .is_some_and(|message| message.contains("not after today"))
        );
    }
}

//! Producer-version freshness for on-disk start-here packets.
//!
//! A 0.10 packet has no `ripr_version`, so `doctor` and `first-pr --check`
//! cannot tell it from a current packet. Missing or mismatched values are
//! `stale_evidence`; they are not schema violations on additive `0.1`.

use crate::output::start_here_state::START_HERE_STALE_EVIDENCE;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub(crate) const START_HERE_RIPR_VERSION_FIELD: &str = "ripr_version";

pub(crate) fn producing_ripr_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StartHereVersionFreshness {
    Current,
    Missing,
    Mismatch { recorded: String },
    Unreadable,
}

pub(crate) fn start_here_json_version_freshness(json_path: &Path) -> StartHereVersionFreshness {
    let Ok(text) = fs::read_to_string(json_path) else {
        return StartHereVersionFreshness::Unreadable;
    };
    let Ok(packet) = serde_json::from_str::<Value>(&text) else {
        return StartHereVersionFreshness::Unreadable;
    };
    start_here_packet_version_freshness(&packet)
}

pub(crate) fn start_here_packet_version_freshness(packet: &Value) -> StartHereVersionFreshness {
    match packet
        .get(START_HERE_RIPR_VERSION_FIELD)
        .and_then(Value::as_str)
    {
        Some(recorded) if recorded == producing_ripr_version() => {
            StartHereVersionFreshness::Current
        }
        Some(recorded) => StartHereVersionFreshness::Mismatch {
            recorded: recorded.to_string(),
        },
        None => StartHereVersionFreshness::Missing,
    }
}

/// Human detail for a stale packet, including the `stale_evidence` state name.
/// `None` when the packet was written by this ripr.
pub(crate) fn start_here_version_stale_detail(
    freshness: &StartHereVersionFreshness,
) -> Option<String> {
    match freshness {
        StartHereVersionFreshness::Current => None,
        StartHereVersionFreshness::Missing => Some(format!(
            "{START_HERE_STALE_EVIDENCE}; missing {START_HERE_RIPR_VERSION_FIELD}"
        )),
        StartHereVersionFreshness::Mismatch { recorded } => Some(format!(
            "{START_HERE_STALE_EVIDENCE}; recorded {START_HERE_RIPR_VERSION_FIELD} {recorded}, this ripr is {}",
            producing_ripr_version()
        )),
        StartHereVersionFreshness::Unreadable => Some(format!(
            "{START_HERE_STALE_EVIDENCE}; start-here.json is missing or unreadable"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn current_version_is_fresh() {
        let packet = json!({ START_HERE_RIPR_VERSION_FIELD: producing_ripr_version() });
        assert_eq!(
            start_here_packet_version_freshness(&packet),
            StartHereVersionFreshness::Current
        );
        assert_eq!(
            start_here_version_stale_detail(&StartHereVersionFreshness::Current),
            None
        );
    }

    #[test]
    fn missing_version_is_stale_evidence() -> Result<(), String> {
        let packet = json!({ "schema_version": "0.1", "tool": "ripr" });
        let freshness = start_here_packet_version_freshness(&packet);
        assert_eq!(freshness, StartHereVersionFreshness::Missing);
        let detail = start_here_version_stale_detail(&freshness)
            .ok_or_else(|| "missing version should be stale".to_string())?;
        assert!(detail.contains(START_HERE_STALE_EVIDENCE), "{detail}");
        assert!(detail.contains("missing ripr_version"), "{detail}");
        Ok(())
    }

    #[test]
    fn other_version_is_stale_evidence() -> Result<(), String> {
        let recorded = if producing_ripr_version() == "0.10.0" {
            "0.9.0"
        } else {
            "0.10.0"
        };
        let packet = json!({ START_HERE_RIPR_VERSION_FIELD: recorded });
        let freshness = start_here_packet_version_freshness(&packet);
        assert_eq!(
            freshness,
            StartHereVersionFreshness::Mismatch {
                recorded: recorded.to_string()
            }
        );
        let detail = start_here_version_stale_detail(&freshness)
            .ok_or_else(|| "mismatched version should be stale".to_string())?;
        assert!(detail.contains(START_HERE_STALE_EVIDENCE), "{detail}");
        assert!(detail.contains(recorded), "{detail}");
        assert!(detail.contains(producing_ripr_version()), "{detail}");
        Ok(())
    }

    #[test]
    fn missing_file_is_unreadable_stale_evidence() -> Result<(), String> {
        let freshness = start_here_json_version_freshness(Path::new(
            "/nonexistent-ripr-start-here-packet.json",
        ));
        assert_eq!(freshness, StartHereVersionFreshness::Unreadable);
        let detail = start_here_version_stale_detail(&freshness)
            .ok_or_else(|| "unreadable packet should be stale".to_string())?;
        assert!(detail.contains(START_HERE_STALE_EVIDENCE), "{detail}");
        Ok(())
    }
}

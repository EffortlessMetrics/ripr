pub const START_HERE_ACTIONABLE_GAP: &str = "actionable_gap";
pub const START_HERE_CLEAN: &str = "clean";
pub const START_HERE_NO_ACTIONABLE_GAP: &str = "no_actionable_gap";
pub const START_HERE_MISSING_ARTIFACTS: &str = "missing_artifacts";
pub const START_HERE_STALE_EVIDENCE: &str = "stale_evidence";
pub const START_HERE_WRONG_ROOT: &str = "wrong_root";
pub const START_HERE_LANGUAGE_DISABLED: &str = "language_disabled";
pub const START_HERE_ADAPTER_UNAVAILABLE: &str = "adapter_unavailable";
pub const START_HERE_PREVIEW_DISABLED: &str = "preview_disabled";
pub const START_HERE_PREVIEW_LIMITED: &str = "preview_limited";
pub const START_HERE_MALFORMED_ARTIFACT: &str = "malformed_artifact";
pub const START_HERE_TIMEOUT_PARTIAL: &str = "timeout_partial";
pub const START_HERE_SERVER_UNAVAILABLE: &str = "server_unavailable";
pub const START_HERE_UNSUPPORTED_SCHEMA: &str = "unsupported_schema";
pub const START_HERE_UNSAFE_PATH: &str = "unsafe_path";
pub const START_HERE_UNSAFE_COMMAND: &str = "unsafe_command";

pub const START_HERE_OUTPUT_STATES: &[&str] = &[
    START_HERE_ACTIONABLE_GAP,
    START_HERE_CLEAN,
    START_HERE_NO_ACTIONABLE_GAP,
    START_HERE_MISSING_ARTIFACTS,
    START_HERE_STALE_EVIDENCE,
    START_HERE_WRONG_ROOT,
    START_HERE_LANGUAGE_DISABLED,
    START_HERE_ADAPTER_UNAVAILABLE,
    START_HERE_PREVIEW_DISABLED,
    START_HERE_PREVIEW_LIMITED,
    START_HERE_MALFORMED_ARTIFACT,
    START_HERE_TIMEOUT_PARTIAL,
    START_HERE_SERVER_UNAVAILABLE,
    START_HERE_UNSUPPORTED_SCHEMA,
    START_HERE_UNSAFE_PATH,
    START_HERE_UNSAFE_COMMAND,
];

pub fn normalize_start_here_output_state(state: &str) -> &'static str {
    match state {
        "top_gap" | "actionable" | "actionable_gap" => START_HERE_ACTIONABLE_GAP,
        "empty_diff" | "clean" => START_HERE_CLEAN,
        "no_action" | "no_actionable_gap" | "no_actionable_seam" => START_HERE_NO_ACTIONABLE_GAP,
        "missing_artifact" | "missing_required_input" | "blocked_artifact" => {
            START_HERE_MISSING_ARTIFACTS
        }
        "stale_artifact" | "stale_input" | "stale" => START_HERE_STALE_EVIDENCE,
        "wrong_root" => START_HERE_WRONG_ROOT,
        "disabled_language" | "language_disabled" => START_HERE_LANGUAGE_DISABLED,
        "adapter_unavailable" | "preview_adapter_unavailable" => START_HERE_ADAPTER_UNAVAILABLE,
        "preview_disabled" => START_HERE_PREVIEW_DISABLED,
        "preview_limited" | "static_limit_only" | "report_only_static_limit" => {
            START_HERE_PREVIEW_LIMITED
        }
        "malformed_artifact" | "malformed" => START_HERE_MALFORMED_ARTIFACT,
        "timeout" | "timed_out" | "timeout_partial" => START_HERE_TIMEOUT_PARTIAL,
        "server_unavailable" | "server_missing" => START_HERE_SERVER_UNAVAILABLE,
        "unsupported_schema" => START_HERE_UNSUPPORTED_SCHEMA,
        "unsafe_path" => START_HERE_UNSAFE_PATH,
        "unsafe_command" => START_HERE_UNSAFE_COMMAND,
        _ => START_HERE_MISSING_ARTIFACTS,
    }
}

pub fn start_here_output_state_is_known(state: &str) -> bool {
    START_HERE_OUTPUT_STATES.contains(&state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_no_output_and_fail_closed_states() {
        for (raw, expected) in [
            ("empty_diff", START_HERE_CLEAN),
            ("no_action", START_HERE_NO_ACTIONABLE_GAP),
            ("missing_artifact", START_HERE_MISSING_ARTIFACTS),
            ("blocked_artifact", START_HERE_MISSING_ARTIFACTS),
            ("stale_artifact", START_HERE_STALE_EVIDENCE),
            ("wrong_root", START_HERE_WRONG_ROOT),
            ("disabled_language", START_HERE_LANGUAGE_DISABLED),
            ("adapter_unavailable", START_HERE_ADAPTER_UNAVAILABLE),
            ("preview_disabled", START_HERE_PREVIEW_DISABLED),
            ("preview_limited", START_HERE_PREVIEW_LIMITED),
            ("malformed_artifact", START_HERE_MALFORMED_ARTIFACT),
            ("timeout", START_HERE_TIMEOUT_PARTIAL),
            ("server_missing", START_HERE_SERVER_UNAVAILABLE),
            ("unsupported_schema", START_HERE_UNSUPPORTED_SCHEMA),
            ("unsafe_path", START_HERE_UNSAFE_PATH),
            ("unsafe_command", START_HERE_UNSAFE_COMMAND),
        ] {
            assert_eq!(normalize_start_here_output_state(raw), expected);
            assert!(start_here_output_state_is_known(expected));
        }
    }
}

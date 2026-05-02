use crate::app::{Mode, OutputFormat};

pub(super) fn parse_mode(value: &str) -> Result<Mode, String> {
    match value {
        "instant" => Ok(Mode::Instant),
        "draft" => Ok(Mode::Draft),
        "fast" => Ok(Mode::Fast),
        "deep" => Ok(Mode::Deep),
        "ready" => Ok(Mode::Ready),
        _ => Err(format!("unknown mode {value:?}")),
    }
}

pub(super) fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "human" | "text" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        "github" => Ok(OutputFormat::Github),
        _ => Err(format!("unknown format {value:?}")),
    }
}

pub(super) fn expect_value<'a>(
    args: &'a [String],
    idx: usize,
    flag: &str,
) -> Result<&'a str, String> {
    args.get(idx)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("missing value for {flag}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    struct ModeScenario {
        given_mode: &'static str,
        then_result: Result<Mode, String>,
    }

    struct FormatScenario {
        given_format: &'static str,
        then_result: Result<OutputFormat, String>,
    }

    #[test]
    fn given_mode_strings_when_parse_mode_then_returns_expected_result() {
        let scenarios = [
            ModeScenario {
                given_mode: "instant",
                then_result: Ok(Mode::Instant),
            },
            ModeScenario {
                given_mode: "draft",
                then_result: Ok(Mode::Draft),
            },
            ModeScenario {
                given_mode: "fast",
                then_result: Ok(Mode::Fast),
            },
            ModeScenario {
                given_mode: "deep",
                then_result: Ok(Mode::Deep),
            },
            ModeScenario {
                given_mode: "ready",
                then_result: Ok(Mode::Ready),
            },
            ModeScenario {
                given_mode: "slow",
                then_result: Err("unknown mode \"slow\"".to_string()),
            },
        ];

        for scenario in scenarios {
            let actual = parse_mode(scenario.given_mode);
            assert_eq!(
                actual, scenario.then_result,
                "mode scenario failed for given={:?}",
                scenario.given_mode
            );
        }
    }

    #[test]
    fn given_format_strings_when_parse_format_then_returns_expected_result() {
        let scenarios = [
            FormatScenario {
                given_format: "human",
                then_result: Ok(OutputFormat::Human),
            },
            FormatScenario {
                given_format: "text",
                then_result: Ok(OutputFormat::Human),
            },
            FormatScenario {
                given_format: "json",
                then_result: Ok(OutputFormat::Json),
            },
            FormatScenario {
                given_format: "github",
                then_result: Ok(OutputFormat::Github),
            },
            FormatScenario {
                given_format: "xml",
                then_result: Err("unknown format \"xml\"".to_string()),
            },
        ];

        for scenario in scenarios {
            let actual = parse_format(scenario.given_format);
            assert_eq!(
                actual, scenario.then_result,
                "format scenario failed for given={:?}",
                scenario.given_format
            );
        }
    }

    #[test]
    fn given_args_and_index_when_expect_value_then_returns_value_or_missing_error() {
        let values = args(&["--diff", "sample.diff"]);

        let when_value_is_present = expect_value(&values, 1, "--diff");
        assert_eq!(when_value_is_present, Ok("sample.diff"));

        let when_value_is_missing = expect_value(&values, 2, "--diff");
        assert_eq!(
            when_value_is_missing,
            Err("missing value for --diff".to_string())
        );
    }

    proptest! {
        #[test]
        fn given_arbitrary_non_mode_string_when_parse_mode_then_returns_unknown_mode(value in "\\PC*") {
            prop_assume!(!matches!(value.as_str(), "instant" | "draft" | "fast" | "deep" | "ready"));
            let actual = parse_mode(value.as_str());
            prop_assert_eq!(actual, Err(format!("unknown mode {value:?}")));
        }

        #[test]
        fn given_arbitrary_non_format_string_when_parse_format_then_returns_unknown_format(value in "\\PC*") {
            prop_assume!(!matches!(value.as_str(), "human" | "text" | "json" | "github"));
            let actual = parse_format(value.as_str());
            prop_assert_eq!(actual, Err(format!("unknown format {value:?}")));
        }

        #[test]
        fn given_argument_list_when_expect_value_then_index_behavior_matches_slice_get(
            values in proptest::collection::vec("\\PC*", 0..12),
            idx in 0usize..15,
            flag in "\\PC*"
        ) {
            let actual = expect_value(&values, idx, flag.as_str());
            let expected = values
                .get(idx)
                .map(|s| s.as_str())
                .ok_or_else(|| format!("missing value for {}", flag));
            prop_assert_eq!(actual, expected);
        }
    }
}

use super::super::rust_index::extract_identifier_tokens;
use crate::domain::ProbeFamily;

pub fn expected_sinks(text: &str, family: &ProbeFamily) -> Vec<String> {
    let mut sinks = Vec::new();
    match family {
        ProbeFamily::Predicate => {
            sinks.extend(["branch result".to_string(), "returned value".to_string()])
        }
        ProbeFamily::ReturnValue => {
            sinks.extend(["return value".to_string(), "assigned field".to_string()])
        }
        ProbeFamily::ErrorPath => sinks.extend(["error variant".to_string(), "Result".to_string()]),
        ProbeFamily::CallDeletion => {
            sinks.extend(["call effect".to_string(), "returned value".to_string()])
        }
        ProbeFamily::FieldConstruction => sinks.extend(
            extract_identifier_tokens(text)
                .into_iter()
                .take(4)
                .map(|t| format!("field:{t}")),
        ),
        ProbeFamily::SideEffect => sinks.extend([
            "published event".to_string(),
            "persisted state".to_string(),
            "mock expectation".to_string(),
        ]),
        ProbeFamily::MatchArm => {
            sinks.extend(["selected variant".to_string(), "arm result".to_string()])
        }
        ProbeFamily::StaticUnknown => sinks.push("unknown sink".to_string()),
    }
    sinks.sort();
    sinks.dedup();
    sinks
}

pub fn required_oracles(text: &str, family: &ProbeFamily) -> Vec<String> {
    let mut out = oracle_templates::family_oracles(family);
    out.extend(oracle_templates::uppercase_token_oracles(text));
    out.sort();
    out.dedup();
    out
}

mod oracle_templates {
    use super::*;

    pub(super) fn family_oracles(family: &ProbeFamily) -> Vec<String> {
        match family {
            ProbeFamily::Predicate => vec![
                "boundary input".to_string(),
                "exact assertion on branch output".to_string(),
            ],
            ProbeFamily::ReturnValue => {
                vec!["exact or property assertion on returned value".to_string()]
            }
            ProbeFamily::ErrorPath => vec!["exact error variant assertion".to_string()],
            ProbeFamily::CallDeletion => {
                vec!["assertion that notices removed call behavior".to_string()]
            }
            ProbeFamily::FieldConstruction => vec!["field or whole-struct assertion".to_string()],
            ProbeFamily::SideEffect => {
                vec!["mock, event, persisted-state, or metric assertion".to_string()]
            }
            ProbeFamily::MatchArm => {
                vec!["input selecting changed match arm and exact assertion".to_string()]
            }
            ProbeFamily::StaticUnknown => vec!["manual review or real mutation".to_string()],
        }
    }

    pub(super) fn uppercase_token_oracles(text: &str) -> Vec<String> {
        extract_identifier_tokens(text)
            .into_iter()
            .take(3)
            .filter(|token| token.chars().any(|c| c.is_uppercase()))
            .map(|token| format!("assertion mentioning {token}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expectations_functions_are_callable() {
        let sinks = expected_sinks("if x > 5", &ProbeFamily::Predicate);
        assert!(!sinks.is_empty());

        let oracles = required_oracles("if x > 5", &ProbeFamily::Predicate);
        assert!(!oracles.is_empty());
    }

    #[test]
    fn expectations_functions_cover_every_probe_family() {
        let cases = [
            (ProbeFamily::Predicate, "branch result"),
            (ProbeFamily::ReturnValue, "return value"),
            (ProbeFamily::ErrorPath, "error variant"),
            (ProbeFamily::CallDeletion, "call effect"),
            (ProbeFamily::FieldConstruction, "field:status"),
            (ProbeFamily::SideEffect, "published event"),
            (ProbeFamily::MatchArm, "selected variant"),
            (ProbeFamily::StaticUnknown, "unknown sink"),
        ];

        for (family, sink) in cases {
            assert!(
                expected_sinks("status: Status::Ready", &family)
                    .iter()
                    .any(|candidate| candidate == sink),
                "{} did not include expected sink {sink}",
                family.as_str()
            );
            assert!(
                !required_oracles("Status::Ready", &family).is_empty(),
                "{} had no required oracle",
                family.as_str()
            );
        }
    }

    #[test]
    fn required_oracles_preserve_family_default_strings() {
        assert_eq!(
            required_oracles("total", &ProbeFamily::ReturnValue),
            vec!["exact or property assertion on returned value"]
        );
        assert_eq!(
            required_oracles("currency", &ProbeFamily::ErrorPath),
            vec!["exact error variant assertion"]
        );
    }

    #[test]
    fn required_oracles_preserve_uppercase_token_window_and_sorting() {
        assert_eq!(
            required_oracles("Alpha Beta Gamma Zeta", &ProbeFamily::Predicate),
            vec![
                "assertion mentioning Alpha",
                "assertion mentioning Beta",
                "assertion mentioning Gamma",
                "boundary input",
                "exact assertion on branch output",
            ]
        );
        assert_eq!(
            required_oracles("111 222 333 Zeta", &ProbeFamily::StaticUnknown),
            vec!["manual review or real mutation"]
        );
    }
}

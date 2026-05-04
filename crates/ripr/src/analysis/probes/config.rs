use super::super::rust_index::extract_identifier_tokens;
use crate::domain::{DeltaKind, ProbeFamily};

pub fn delta_for_family(family: &ProbeFamily) -> DeltaKind {
    match family {
        ProbeFamily::Predicate | ProbeFamily::MatchArm => DeltaKind::Control,
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => DeltaKind::Effect,
        ProbeFamily::ReturnValue | ProbeFamily::ErrorPath | ProbeFamily::FieldConstruction => {
            DeltaKind::Value
        }
        ProbeFamily::StaticUnknown => DeltaKind::Unknown,
    }
}

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
    let mut out = Vec::new();
    match family {
        ProbeFamily::Predicate => {
            out.push("boundary input".to_string());
            out.push("exact assertion on branch output".to_string());
        }
        ProbeFamily::ReturnValue => {
            out.push("exact or property assertion on returned value".to_string())
        }
        ProbeFamily::ErrorPath => out.push("exact error variant assertion".to_string()),
        ProbeFamily::CallDeletion => {
            out.push("assertion that notices removed call behavior".to_string())
        }
        ProbeFamily::FieldConstruction => out.push("field or whole-struct assertion".to_string()),
        ProbeFamily::SideEffect => {
            out.push("mock, event, persisted-state, or metric assertion".to_string())
        }
        ProbeFamily::MatchArm => {
            out.push("input selecting changed match arm and exact assertion".to_string())
        }
        ProbeFamily::StaticUnknown => out.push("manual review or real mutation".to_string()),
    }
    for token in extract_identifier_tokens(text).into_iter().take(3) {
        if token.chars().any(|c| c.is_uppercase()) {
            out.push(format!("assertion mentioning {token}"));
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_functions_are_callable() {
        let family = ProbeFamily::Predicate;
        let delta = delta_for_family(&family);
        assert_eq!(delta, DeltaKind::Control);

        let sinks = expected_sinks("if x > 5", &ProbeFamily::Predicate);
        assert!(!sinks.is_empty());

        let oracles = required_oracles("if x > 5", &ProbeFamily::Predicate);
        assert!(!oracles.is_empty());
    }
}

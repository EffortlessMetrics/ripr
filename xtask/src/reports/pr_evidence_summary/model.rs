use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct JsonInput {
    pub(super) state: InputState,
    pub(super) value: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InputState {
    Present,
    Missing,
    Invalid(String),
}

impl std::fmt::Display for InputState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Present => f.write_str("present"),
            Self::Missing => f.write_str("missing"),
            Self::Invalid(err) => write!(
                f,
                "invalid: {}",
                crate::reports::pr_evidence_summary::util::md_escape(err)
            ),
        }
    }
}

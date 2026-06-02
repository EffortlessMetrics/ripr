use crate::reports::pr_evidence_summary::model::{InputState, JsonInput};
use crate::reports::pr_evidence_summary::util::first_line;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub(super) fn load_json(repo: &Path, relative: &str) -> JsonInput {
    let path = repo.join(relative);
    let Ok(text) = fs::read_to_string(&path) else {
        return JsonInput {
            state: InputState::Missing,
            value: None,
        };
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => JsonInput {
            state: InputState::Present,
            value: Some(value),
        },
        Err(err) => JsonInput {
            state: InputState::Invalid(first_line(&err.to_string())),
            value: None,
        },
    }
}

pub(super) fn file_state(repo: &Path, relative: &str) -> &'static str {
    if repo.join(relative).exists() {
        "present"
    } else {
        "missing"
    }
}

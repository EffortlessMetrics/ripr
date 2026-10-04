//! The generated workflow's PR guidance annotations.
//!
//! `ripr init --ci github` used to emit one `::warning` workflow command
//! per placed review card from an inline jq program. `ripr reports
//! ci-packet` prints the same lines from this renderer. The workflow
//! command is encoded here, never through a TSV or shell round-trip, so a
//! backslash, tab, CR, or LF in a path or message reaches GitHub's decoder
//! as the original bytes (#4089). An annotation names the repair start only
//! when that field is present; the literal string "null" stays absent. The
//! brief command is not interpolated: it points at the runner's checkout.

use super::jq::{Fail, add, at, each_at, index, interpolated, truthy};
use serde_json::Value;

/// One `::warning` line per placed comment in a `comments.json` document,
/// each ending in a newline. A value whose type breaks the retired jq
/// program stops the output there, as jq did, and returns the lines
/// printed so far with the error.
pub(crate) fn render_workflow_annotations(document: &Value) -> Result<String, (String, String)> {
    let mut out = String::new();
    let comments = each_at(document, "comments").map_err(|Fail| type_error(&out))?;
    for comment in comments {
        match annotation(comment) {
            Ok(Some(line)) => {
                out.push_str(&line);
                out.push('\n');
            }
            Ok(None) => {}
            Err(Fail) => return Err(type_error(&out)),
        }
    }
    Ok(out)
}

fn type_error(printed: &str) -> (String, String) {
    (
        printed.to_string(),
        "comments.json holds a value whose type the annotation format cannot encode".to_string(),
    )
}

fn annotation(comment: &Value) -> Result<Option<String>, Fail> {
    let path = at(comment, "placement.path")?;
    let line = at(comment, "placement.line")?;
    if !(truthy(path) && truthy(line)) {
        return Ok(None);
    }
    let repair = at(comment, "llm_guidance.repair_command")?;
    let repair = if truthy(repair) {
        repair.clone()
    } else {
        Value::String(String::new())
    };
    let reason = index(comment, "reason")?;
    let reason = if truthy(reason) {
        reason.clone()
    } else {
        Value::String("RIPR targeted test guidance".to_string())
    };
    let suffix = if repair != "" && repair != "null" {
        add(Value::from(" Start the repair: "), repair)?
    } else {
        Value::String(String::new())
    };
    let message = add(reason, suffix)?;
    let path = escape_property(string(path)?);
    let line = escape_property(&interpolated(line));
    let message = escape_data(string(&message)?);
    Ok(Some(format!(
        "::warning file={path},line={line},title=RIPR targeted test guidance::{message}"
    )))
}

/// `gsub` takes strings only.
fn string(value: &Value) -> Result<&str, Fail> {
    value.as_str().ok_or(Fail)
}

fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

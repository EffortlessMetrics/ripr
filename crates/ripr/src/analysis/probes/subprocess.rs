use super::super::rust_index::{RustIndex, find_owner_function};
use crate::domain::ProbeFamily;
use std::path::Path;

/// Commands whose bounded adapter shape is understood by the static analyzer.
/// Keep this list deliberately small: an unknown or dynamic command must stay
/// on the ordinary exposure path until a fixture-backed boundary is added.
const ALLOWED_COMMANDS: &[&str] = &["curl"];

/// Classify a changed line as an observable outbound effect only when the
/// owning function is a bounded, literal-command adapter.
///
/// This is intentionally deny-by-default. The owner must contain exactly one
/// literal allowlisted command, argument construction, a timeout bound, and
/// captured process output. Shell dispatch, dynamic command names, and missing
/// bounds do not qualify.
pub(super) fn bounded_subprocess_family(
    index: &RustIndex,
    path: &Path,
    line: usize,
    changed_text: &str,
) -> Option<ProbeFamily> {
    if !looks_like_subprocess_builder_line(changed_text) {
        return None;
    }

    let owner = find_owner_function(index, path, line)?;
    let body = owner.body.as_str();
    let invocation = body.split("Command::new(").collect::<Vec<_>>();
    if invocation.len() != 2 {
        return None;
    }

    let command = invocation[1].trim_start().strip_prefix('"')?;
    let command = command.split_once('"')?.0;
    if !ALLOWED_COMMANDS.contains(&command) {
        return None;
    }

    let has_arguments = body.contains(".arg(") || body.contains(".args(");
    let has_timeout =
        body.contains("--max-time") || body.contains("timeout_sec") || body.contains(".timeout(");
    let captures_output = body.contains(".output(")
        || body.contains(".status(")
        || (body.contains(".spawn(") && body.contains("wait_for_child_output_files"));
    let cleans_up_receipts =
        body.contains("remove_output_files(") && body.contains(".kill(") && body.contains(".wait(");
    let handles_errors =
        body.contains("is_err()") || body.contains("if let Err") || body.contains("return Err(");
    let invokes_shell = ["sh -c", "bash -c", "cmd /c", "\"-c\"", ".shell("]
        .iter()
        .any(|marker| body.contains(marker));

    (has_arguments
        && has_timeout
        && captures_output
        && cleans_up_receipts
        && handles_errors
        && !invokes_shell)
        .then_some(ProbeFamily::SideEffect)
}

fn looks_like_subprocess_builder_line(text: &str) -> bool {
    [
        "Command::new(",
        ".arg(",
        ".args(",
        ".output(",
        ".status(",
        ".spawn(",
        "--max-time",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::diff::{ChangedFile, ChangedLine};
    use crate::analysis::rust_index::{FileFacts, FunctionFact};
    use crate::domain::SymbolId;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn index_with_body(body: &str) -> RustIndex {
        let path = PathBuf::from("src/adapter.rs");
        let function = FunctionFact {
            id: SymbolId("adapter::send".to_owned()),
            name: "send".to_owned(),
            file: path.clone(),
            start_line: 1,
            end_line: 20,
            body: body.to_owned(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        };
        RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path,
                    functions: vec![function],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        }
    }

    #[test]
    fn bounded_literal_curl_adapter_is_an_observable_effect() {
        let index = index_with_body(
            r#"
let mut command = ProcessCommand::new("curl");
command.arg("--max-time").arg(timeout_sec.to_string());
let mut child = command.spawn()?;
if write_config_result.is_err() {
    let _ = child.kill();
    let _ = child.wait();
    remove_output_files(&stdout_path, &stderr_path);
    return Err(error);
}
let output = wait_for_child_output_files(child, &stdout_path, &stderr_path, timeout_sec)?;
if output.status.success() { receipt_written = true; }
remove_output_files(&stdout_path, &stderr_path);
"#,
        );

        assert_eq!(
            bounded_subprocess_family(
                &index,
                Path::new("src/adapter.rs"),
                3,
                "command.arg(\"--max-time\");"
            ),
            Some(ProbeFamily::SideEffect)
        );
    }

    #[test]
    fn dynamic_or_shell_commands_remain_unclassified() {
        for body in [
            r#"let command = ProcessCommand::new(command_name); command.arg("--max-time").output()?;"#,
            r#"let command = ProcessCommand::new("curl"); command.arg("-c").arg(script).arg("--max-time").output()?;"#,
            r#"let command = ProcessCommand::new("curl"); command.arg(url).output()?;"#,
        ] {
            let index = index_with_body(body);
            assert_eq!(
                bounded_subprocess_family(
                    &index,
                    Path::new("src/adapter.rs"),
                    3,
                    "command.arg(url);"
                ),
                None,
                "unsafe or unbounded adapter was classified as bounded: {body}"
            );
        }
    }

    #[test]
    fn probes_for_file_emits_side_effect_for_bounded_adapter_line() {
        let path = PathBuf::from("src/adapter.rs");
        let index = index_with_body(
            r#"
let mut command = ProcessCommand::new("curl");
command.arg("--max-time").arg(timeout_sec.to_string());
let mut child = command.spawn()?;
if write_config_result.is_err() {
    let _ = child.kill();
    let _ = child.wait();
    remove_output_files(&stdout_path, &stderr_path);
    return Err(error);
}
let output = wait_for_child_output_files(child, &stdout_path, &stderr_path, timeout_sec)?;
if output.status.success() { receipt_written = true; }
remove_output_files(&stdout_path, &stderr_path);
"#,
        );
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "command.arg(\"--max-time\");".to_owned(),
            }],
            removed_lines: Vec::new(),
        };

        let probes = super::super::diff::probes_for_file(Path::new("workspace"), &changed, &index);

        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].family, ProbeFamily::SideEffect);
        assert_eq!(probes[0].location.line, 3);
    }
}

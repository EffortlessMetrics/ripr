use crate::cli::commands_context::ensure_command_root;
use crate::cli::parse::expect_value;
use crate::output;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Options {
    pub(super) root: PathBuf,
    pub(super) result: PathBuf,
}

pub(super) fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut root = PathBuf::from(".");
    let mut result = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                let value = expect_value(args, i, "--root")?;
                if value.trim().is_empty() {
                    return Err("swarm ingest --root requires a non-empty path".to_string());
                }
                root = PathBuf::from(value);
            }
            "--result" => {
                i += 1;
                let value = expect_value(args, i, "--result")?;
                if value.trim().is_empty() {
                    return Err("swarm ingest --result requires a non-empty path".to_string());
                }
                result = Some(PathBuf::from(value));
            }
            "--format" => {
                i += 1;
                let value = expect_value(args, i, "--format")?;
                if value != "json" {
                    return Err(format!(
                        "unknown swarm ingest format {value:?}; expected `json`"
                    ));
                }
            }
            "--json" => {}
            other => return Err(format!("unknown swarm ingest argument {other:?}")),
        }
        i += 1;
    }

    Ok(Options {
        root,
        result: result.ok_or_else(|| "swarm ingest requires --result <path>".to_string())?,
    })
}

pub(super) fn run(options: Options) -> Result<(), String> {
    ensure_command_root(&options.root, "swarm ingest")?;
    let result_path = validate_result_path(&options.root, &options.result)?;
    let contents = std::fs::read_to_string(&result_path).map_err(|err| {
        format!(
            "read swarm ingest --result {} failed: {err}",
            options.result.display()
        )
    })?;
    let rendered = output::swarm_ingest::render_swarm_ingest_json(
        &contents,
        &output::outcome::display_path(&options.result),
    )?;
    print!("{rendered}");
    Ok(())
}

fn validate_result_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize swarm ingest root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize swarm ingest --result {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "swarm ingest --result {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_result_and_format() {
        assert_eq!(
            parse_options(&args(&[
                "--root",
                ".",
                "--result",
                "target/ripr/workflow/agent-result.json",
                "--format",
                "json",
            ])),
            Ok(Options {
                root: PathBuf::from("."),
                result: PathBuf::from("target/ripr/workflow/agent-result.json"),
            })
        );
        assert_eq!(
            parse_options(&args(&["--format", "md"])),
            Err("unknown swarm ingest format \"md\"; expected `json`".to_string())
        );
        assert_eq!(
            parse_options(&args(&[])),
            Err("swarm ingest requires --result <path>".to_string())
        );
    }
}

//! Repository configuration commands.

use crate::cli::help;
use crate::cli::suggest::unknown_argument;
use crate::config::load_for_root;
use std::path::{Path, PathBuf};

pub(in crate::cli) fn config(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_config_help();
        return Ok(());
    }

    let Some((subcommand, rest)) = args.split_first() else {
        return Err("config requires subcommand `validate`".to_string());
    };
    if subcommand != "validate" {
        return Err(format!(
            "unknown config subcommand {subcommand:?}; expected `validate`"
        ));
    }

    println!("{}", validate_config(&parse_validate_root(rest)?)?);
    Ok(())
}

fn parse_validate_root(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => Ok(PathBuf::from(".")),
        [flag] if flag == "--root" => Err("missing value for --root".to_string()),
        [flag, value] if flag == "--root" => Ok(PathBuf::from(value)),
        [flag, _value, extra, ..] if flag == "--root" => {
            Err(unknown_argument("config validate", extra))
        }
        [other, ..] => Err(unknown_argument("config validate", other)),
    }
}

fn validate_config(root: &Path) -> Result<&'static str, String> {
    if !root.is_dir() {
        return Err(format!(
            "config validate root {} is not a directory",
            root.display()
        ));
    }
    load_for_root(root)?;
    Ok("✓ ripr.toml valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG_FILE_NAME;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ripr-config-{label}-{nonce}"))
    }

    #[test]
    fn validate_accepts_a_valid_root_config() -> Result<(), String> {
        let root = temp_dir("valid");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        fs::write(
            root.join(CONFIG_FILE_NAME),
            "[analysis]\nmode = \"draft\"\ninclude_unchanged_tests = true\n",
        )
        .map_err(|error| error.to_string())?;

        let result = validate_config(&root);
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if result? != "✓ ripr.toml valid" {
            return Err("valid configuration returned the wrong success message".to_string());
        }
        Ok(())
    }

    #[test]
    fn validate_accepts_a_missing_config_using_built_in_defaults() -> Result<(), String> {
        let root = temp_dir("missing");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;

        let result = validate_config(&root);
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if result? != "✓ ripr.toml valid" {
            return Err("missing configuration returned the wrong success message".to_string());
        }
        Ok(())
    }

    #[test]
    fn validate_rejects_missing_and_file_roots() -> Result<(), String> {
        let missing = temp_dir("missing-root");
        let missing_error = validate_config(&missing).err().ok_or_else(|| {
            "a nonexistent validation root must not use built-in defaults".to_string()
        })?;
        if !missing_error.contains("is not a directory") {
            return Err(format!("unexpected missing-root error: {missing_error}"));
        }

        let file_root = temp_dir("file-root");
        fs::write(&file_root, "not a directory\n").map_err(|error| error.to_string())?;
        let file_error = validate_config(&file_root)
            .err()
            .ok_or_else(|| "a file validation root must not use built-in defaults".to_string())?;
        fs::remove_file(&file_root).map_err(|error| error.to_string())?;
        if !file_error.contains("is not a directory") {
            return Err(format!("unexpected file-root error: {file_error}"));
        }
        Ok(())
    }

    #[test]
    fn validate_returns_the_loader_error_for_invalid_toml() -> Result<(), String> {
        let root = temp_dir("invalid");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        fs::write(root.join(CONFIG_FILE_NAME), "[analysis\n").map_err(|error| error.to_string())?;

        let result = validate_config(&root);
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        let error = match result {
            Ok(message) => {
                return Err(format!(
                    "invalid TOML unexpectedly passed validation with {message}"
                ));
            }
            Err(error) => error,
        };
        if !error.contains("ripr.toml") || !error.contains("invalid ripr.toml") {
            return Err(format!("expected a path-qualified TOML error, got {error}"));
        }
        Ok(())
    }

    #[test]
    fn validate_parser_rejects_unknown_arguments_and_missing_root() -> Result<(), String> {
        let cases = [
            (
                args(&["validate", "--root"]),
                "missing value for --root".to_string(),
            ),
            (
                args(&["validate", "--wat"]),
                "unknown config validate argument \"--wat\". Run `ripr config validate --help`."
                    .to_string(),
            ),
            (
                args(&["validate", "--root", "/tmp/workspace", "extra"]),
                "unknown config validate argument \"extra\". Run `ripr config validate --help`."
                    .to_string(),
            ),
        ];
        for (input, expected) in cases {
            let actual = config(&input);
            if actual != Err(expected.clone()) {
                return Err(format!("expected {expected:?}, got {actual:?}"));
            }
        }
        Ok(())
    }
}

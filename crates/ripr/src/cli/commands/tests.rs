use super::{check, context, doctor, explain, lsp};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn check_requires_values_for_value_flags() {
    assert_eq!(
        check(&args(&["--diff"])),
        Err("missing value for --diff".to_string())
    );
    assert_eq!(
        check(&args(&["--mode"])),
        Err("missing value for --mode".to_string())
    );
}

#[test]
fn command_help_branches_return_ok() {
    assert_eq!(check(&args(&["--help"])), Ok(()));
    assert_eq!(explain(&args(&["--help"])), Ok(()));
    assert_eq!(context(&args(&["--help"])), Ok(()));
    assert_eq!(doctor(&args(&["--help"])), Ok(()));
    assert_eq!(lsp(&args(&["--help"])), Ok(()));
}

#[test]
fn context_rejects_invalid_max_related_tests() {
    let result = context(&args(&[
        "--at",
        "probe:file.rs:1:predicate",
        "--max-related-tests",
        "many",
    ]));
    assert!(matches!(
        result,
        Err(message) if message.starts_with("invalid --max-related-tests:")
    ));
}

#[test]
fn doctor_requires_root_value() {
    assert_eq!(
        doctor(&args(&["--root"])),
        Err("missing value for --root".to_string())
    );
}

#[test]
fn doctor_rejects_unknown_arguments() {
    assert_eq!(
        doctor(&args(&["--verbose"])),
        Err("unknown doctor argument \"--verbose\"".to_string())
    );
}

#[test]
fn doctor_accepts_default_root() {
    assert_eq!(doctor(&args(&[])), Ok(()));
}

#[test]
fn lsp_version_returns_ok() {
    assert_eq!(lsp(&args(&["--version"])), Ok(()));
}

#[test]
fn lsp_rejects_unknown_arguments() {
    assert_eq!(
        lsp(&args(&["--bad"])),
        Err("unknown lsp argument \"--bad\"".to_string())
    );
}

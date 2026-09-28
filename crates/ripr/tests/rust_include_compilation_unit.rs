use ripr::{CheckInput, Mode};
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempRepo(PathBuf);

impl TempRepo {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("ripr-{label}-{stamp}"));
        fs::create_dir_all(path.join("src"))?;
        fs::write(
            path.join("Cargo.toml"),
            "[package]\nname='include-fixture'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn parent_source(include: bool) -> String {
    let implementation = if include {
        "include!(\"parser_fragment.rs\");"
    } else {
        r#"
impl Parser {
    fn clamp(&self, value: i32) -> i32 {
        if value > self.limit { self.limit } else { value }
    }
}
"#
    };
    format!(
        r#"
struct Parser {{ limit: i32 }}
{implementation}

#[cfg(test)]
mod tests {{
    use super::Parser;

    #[test]
    fn clamps_to_private_parent_state() {{
        let parser = Parser {{ limit: 10 }};
        assert_eq!(parser.clamp(11), 10);
    }}
}}
"#
    )
}

fn semantic_inventory(
    output: &ripr::CheckOutput,
    file: &Path,
) -> BTreeSet<(String, String, String)> {
    output
        .findings
        .iter()
        .filter(|finding| finding.probe.location.file.ends_with(file))
        .map(|finding| {
            (
                finding.probe.family.as_str().to_string(),
                finding.probe.expression.clone(),
                finding.class.as_str().to_string(),
            )
        })
        .collect()
}

#[test]
fn public_repo_analysis_preserves_literal_include_compilation_unit_and_attribution()
-> Result<(), Box<dyn Error>> {
    let included = TempRepo::new("public-include")?;
    let inline = TempRepo::new("public-inline")?;
    fs::write(included.path().join("src/lib.rs"), parent_source(true))?;
    fs::write(
        included.path().join("src/parser_fragment.rs"),
        r#"
impl Parser {
    fn clamp(&self, value: i32) -> i32 {
        if value > self.limit { self.limit } else { value }
    }
}
"#,
    )?;
    fs::write(inline.path().join("src/lib.rs"), parent_source(false))?;

    let included_output = ripr::app::check_workspace_repo(CheckInput {
        root: included.path().to_path_buf(),
        mode: Mode::Ready,
        ..CheckInput::default()
    })?;
    let inline_output = ripr::app::check_workspace_repo(CheckInput {
        root: inline.path().to_path_buf(),
        mode: Mode::Ready,
        ..CheckInput::default()
    })?;

    let fragment_path = Path::new("src/parser_fragment.rs");
    let fragment_findings = included_output
        .findings
        .iter()
        .filter(|finding| finding.probe.location.file.ends_with(fragment_path))
        .collect::<Vec<_>>();
    assert!(
        !fragment_findings.is_empty(),
        "the public repo route must inventory fragment probes"
    );
    assert!(fragment_findings.iter().all(|finding| {
        finding.probe.location.line > 0
            && finding
                .probe
                .owner
                .as_ref()
                .is_some_and(|owner| owner.0.starts_with("src/lib.rs::impl Parser::clamp"))
    }));
    assert!(
        fragment_findings
            .iter()
            .any(|finding| finding.probe.location.line == 4),
        "the branch probe must retain its exact line in the included fragment"
    );
    assert!(fragment_findings.iter().any(|finding| {
        finding
            .related_tests
            .iter()
            .any(|test| test.name == "clamps_to_private_parent_state")
    }));

    assert_eq!(
        semantic_inventory(&included_output, fragment_path),
        semantic_inventory(&inline_output, Path::new("src/lib.rs")),
        "inline and include representations must agree modulo source location"
    );
    Ok(())
}

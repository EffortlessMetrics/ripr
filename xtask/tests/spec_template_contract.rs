use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempRoot(PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_root() -> Result<TempRoot, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("ripr-spec-template-{}-{nonce}", std::process::id()));
    fs::create_dir_all(root.join("docs/specs")).map_err(|error| error.to_string())?;
    Ok(TempRoot(root))
}

fn materialize_template(root: &Path) -> Result<(), String> {
    let template = fs::read_to_string("../docs/templates/SPEC_TEMPLATE.md")
        .map_err(|error| error.to_string())?;
    let materialized = template
        .replace("RIPR-SPEC-NNNN: Title", "RIPR-SPEC-9999: Template contract")
        .replace("YYYY-MM-DD", "2026-08-27")
        .replace("-\n", "- template contract evidence\n");
    fs::write(
        root.join("docs/specs/RIPR-SPEC-9999-template-contract.md"),
        materialized,
    )
    .map_err(|error| error.to_string())
}

#[test]
fn canonical_template_materializes_into_a_valid_spec() -> Result<(), String> {
    let root = temp_root()?;
    materialize_template(&root.0)?;
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("check-spec-format")
        .current_dir(&root.0)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "canonical template failed the real spec validator:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

use ripr::{CheckInput, OutputFormat, check_workspace};
use std::path::PathBuf;

fn sample_input() -> CheckInput {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let diff_file = manifest_dir.join("examples/library.diff");

    CheckInput {
        root: manifest_dir,
        diff_file: Some(diff_file),
        format: OutputFormat::Json,
        ..CheckInput::default()
    }
}

fn main() -> Result<(), String> {
    let output = check_workspace(sample_input())?;

    println!("schema: {}", output.schema_version);
    println!("findings: {}", output.findings.len());
    for finding in &output.findings {
        println!("- {}: {}", finding.id, finding.class.as_str());
    }

    Ok(())
}

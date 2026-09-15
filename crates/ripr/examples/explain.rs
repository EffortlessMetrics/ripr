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
    let input = sample_input();
    let finding_id = check_workspace(input.clone())?
        .findings
        .first()
        .map(|finding| finding.id.clone())
        .ok_or_else(|| "sample diff produced no findings".to_string())?;
    let explanation = ripr::explain_finding_with_input(input, &finding_id)?;
    println!("{explanation}");
    Ok(())
}

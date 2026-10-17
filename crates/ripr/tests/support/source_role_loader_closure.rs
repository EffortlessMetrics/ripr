//! Run the retained loader against disposable archive inputs, including omissions.
//! Included by source_role_corpus_package; also runs with rustc --test.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const CORPUS: &str = "tests/data/source-role-corpus";
const INPUTS: [&str; 10] = [
    "cfg_variants/Cargo.toml.fixture",
    "cfg_variants/src/lib.rs",
    "explicit_test_target/Cargo.toml.fixture",
    "explicit_test_target/src/lib.rs",
    "explicit_test_target/tests/custom/smoke_case.rs",
    "naming_lookalike/Cargo.toml.fixture",
    "naming_lookalike/src/lib.rs",
    "naming_lookalike/src/price_test.rs",
    "production_lib/Cargo.toml.fixture",
    "production_lib/src/lib.rs",
];
const CASES: [&str; 4] = [
    "cfg_variants",
    "explicit_test_target",
    "naming_lookalike",
    "production_lib",
];

// Exit 42 can only originate at the real loader, before this probe's oracle.
// An oracle read failure after a checkout fallback must not count as rejection.
const PROBE: &str = r#"
#[path = "source_role_fixture.rs"]
mod fixture;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let case = match fixture::SourceRoleCase::load(&args[0]) {
        Ok(case) => case,
        Err(error) => {
            eprintln!("LOADER_ERROR: {error}");
            std::process::exit(42);
        }
    };
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/source-role-corpus/cases").join(&args[0]);
    assert!(!case.root().starts_with(&source), "loader reused retained inputs");
    assert_eq!(case.files().len(), args.len() - 1, "loader file closure");
    for input in &args[1..] {
        let relative = if input == "Cargo.toml.fixture" { "Cargo.toml" } else { input };
        let destination = case.root().join(relative);
        assert!(case.files().contains(&destination), "loader omitted {input}");
        assert_eq!(std::fs::read(&destination)?, std::fs::read(source.join(input))?,
            "loader changed bytes for {input}");
    }
    assert!(!case.root().join("Cargo.toml.fixture").exists());
    let root = case.root().to_path_buf();
    drop(case);
    assert!(!root.exists(), "loader leaked materialized case");
    println!("LOADED {} {}", args[0], args.len() - 1);
    Ok(())
}
"#;

struct Extraction(PathBuf);

impl Drop for Extraction {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_probe(root: &Path, name: &str) -> Result<Output, Box<dyn Error>> {
    let prefix = format!("{name}/");
    Ok(
        Command::new(root.join(format!("probe{}", std::env::consts::EXE_SUFFIX)))
            .current_dir(root)
            .arg(name)
            .args(
                INPUTS
                    .iter()
                    .filter_map(|input| input.strip_prefix(&prefix)),
            )
            .output()?,
    )
}

fn require_complete(root: &Path, name: &str) -> Result<(), Box<dyn Error>> {
    let output = run_probe(root, name)?;
    assert!(
        output.status.success(),
        "complete {name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).starts_with(&format!("LOADED {name} ")));
    Ok(())
}

#[test]
fn archive_inputs_are_the_runtime_loader_closure() -> Result<(), Box<dyn Error>> {
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let extraction = Extraction(std::env::temp_dir().join(format!(
        "ripr-loader-closure-{}-{stamp}",
        std::process::id()
    )));
    let root = &extraction.0;
    fs::create_dir(root)?;
    for input in INPUTS.map(|input| format!("{CORPUS}/cases/{input}")) {
        let destination = root.join(&input);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(archive.join(input), destination)?;
    }
    // Support documents have no presence requirement in this runtime check.
    for support in ["README.md", "SPEC.md"] {
        let relative = Path::new(CORPUS).join(support);
        if archive.join(&relative).is_file() {
            fs::copy(archive.join(&relative), root.join(relative))?;
        }
    }
    // Compile the unchanged archived loader once, bound to this extraction.
    fs::copy(
        archive.join("tests/support/source_role_fixture.rs"),
        root.join("source_role_fixture.rs"),
    )?;
    fs::write(root.join("probe.rs"), PROBE)?;
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compiled = Command::new(compiler)
        .current_dir(root)
        .env("CARGO_MANIFEST_DIR", root)
        .args(["--edition=2024", "probe.rs", "-o"])
        .arg(root.join(format!("probe{}", std::env::consts::EXE_SUFFIX)))
        .output()?;
    assert!(
        compiled.status.success(),
        "compile retained loader: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    for name in CASES {
        require_complete(root, name)?;
    }
    for input in INPUTS {
        let name = input.split('/').next().ok_or("input has no case")?;
        let missing = root.join(CORPUS).join("cases").join(input);
        let held = missing.with_file_name("omitted-input");
        fs::rename(&missing, &held)?;
        let output = run_probe(root, name);
        fs::rename(held, &missing)?;
        let output = output?;
        let reason = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(42), "missing {input}: {reason}");
        assert!(
            reason.contains(&format!(
                "LOADER_ERROR: materialize required source-role input {}:",
                missing.display()
            )),
            "missing {input} failed for the wrong reason: {reason}"
        );
        eprintln!("REJECTED {input}: {}", reason.trim());
        require_complete(root, name)?;
    }
    // Documentation is retained support, not a runtime-loaded case input.
    for support in ["README.md", "SPEC.md"] {
        let path = root.join(CORPUS).join(support);
        if !path.exists() {
            eprintln!("NONLOADED {support}: absent; no runtime presence requirement");
            continue;
        }
        let held = path.with_file_name("omitted-support");
        fs::rename(&path, &held)?;
        let result = CASES
            .into_iter()
            .try_for_each(|name| require_complete(root, name));
        fs::rename(held, path)?;
        result?;
        eprintln!("NONLOADED {support}: 4/4 cases still load");
    }
    Ok(())
}

use super::{OutputFormat, sample_finding};
use crate::app::check::python_test_note_possible;
use crate::app::{CheckInput, Mode, check_workspace};
use crate::domain::{ExposureClass, Finding, StageEvidence, StageState};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn finding(file: &str, class: ExposureClass, reach: StageState) -> Finding {
    let mut finding = sample_finding(file, 1);
    finding.class = class;
    finding.ripr.reach = StageEvidence::new(reach, crate::domain::Confidence::Medium, "reach");
    finding
}

/// #6340: the walk gate mirrors `render_all_no_path_disclosure`, including its
/// reach-evidence guard, so a run the renderer would suppress never walks.
#[test]
fn python_test_note_gate_matches_the_renderer_eligibility() {
    use ExposureClass::*;
    let no_path = |class, reach| finding("src/lib.rs", class, reach);
    assert!(python_test_note_possible(&[no_path(
        NoStaticPath,
        StageState::No
    )]));
    assert!(python_test_note_possible(&[
        no_path(NoStaticPath, StageState::No),
        no_path(InfectionUnknown, StageState::No),
    ]));
    assert!(!python_test_note_possible(&[]), "no findings");
    assert!(
        !python_test_note_possible(&[no_path(InfectionUnknown, StageState::No)]),
        "no no_static_path finding"
    );
    assert!(
        !python_test_note_possible(&[finding("app.py", NoStaticPath, StageState::No)]),
        "no_static_path outside a Rust file"
    );
    for class in [Exposed, WeaklyExposed, ReachableUnrevealed] {
        assert!(
            !python_test_note_possible(&[
                no_path(NoStaticPath, StageState::No),
                no_path(class.clone(), StageState::No)
            ]),
            "{class:?} suppresses the note"
        );
    }
    assert!(
        !python_test_note_possible(&[
            no_path(NoStaticPath, StageState::No),
            no_path(StaticUnknown, StageState::Yes),
        ]),
        "a finding with reach: yes suppresses the note"
    );
}

fn temp_root(name: &str) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-py-note-{name}-{stamp}"));
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create root: {err}"))?;
    std::fs::create_dir_all(root.join("tests")).map_err(|err| format!("create tests: {err}"))?;
    Ok(root)
}

/// #6340: only a human-rendered run walks the repository for Python tests; a
/// JSON run (LSP refreshes and automation) never pays for it.
#[test]
fn python_tests_are_discovered_only_for_human_output() -> Result<(), String> {
    let root = temp_root("format")?;
    let write = |rel: &str, text: &str| {
        std::fs::write(root.join(rel), text).map_err(|err| format!("write {rel}: {err}"))
    };
    write("src/lib.rs", "pub fn f(x: i32) -> bool { x >= 1 }\n")?;
    write("tests/test_f.py", "def test_f():\n    pass\n")?;
    write(
        "example.diff",
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n-pub fn f(x: i32) -> bool { x > 1 }\n+pub fn f(x: i32) -> bool { x >= 1 }\n",
    )?;
    let run = |format: OutputFormat| {
        check_workspace(CheckInput {
            root: root.clone(),
            diff_file: Some(root.join("example.diff")),
            mode: Mode::Draft,
            format,
            ..CheckInput::default()
        })
    };
    let human = run(OutputFormat::Human)?;
    assert!(
        human
            .findings
            .iter()
            .any(|finding| finding.class == ExposureClass::NoStaticPath),
        "fixture precondition: a no_static_path finding"
    );
    let python = human
        .unlinked_python_tests
        .ok_or_else(|| "human output must record the Python tests".to_string())?;
    assert_eq!(python.count, 1);
    assert_eq!(python.example, "tests/test_f.py");
    let python = Some(python);
    assert_eq!(
        run(OutputFormat::HumanFull)?.unlinked_python_tests,
        python,
        "human-full renders the same note"
    );
    assert_eq!(
        run(OutputFormat::Json)?.unlinked_python_tests,
        None,
        "machine output must skip the walk"
    );
    Ok(())
}

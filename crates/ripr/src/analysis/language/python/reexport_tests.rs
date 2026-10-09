//! Package re-export reach (`reexports.rs`), established end to end through
//! `analyze_diff` on on-disk workspaces shaped like the real projects that
//! exposed the false `no_static_path`: `humanize` (`import humanize` +
//! `from .time import naturaldelta`) and `more-itertools`
//! (`import more_itertools as mi` + `from .more import *` with `__all__`).

use super::*;
use std::path::{Path, PathBuf};

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create_dir_all({}): {err}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|err| format!("write({}): {err}", path.display()))
}

fn unique_tempdir(label: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("clock: {err}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-python-reexport-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root)
        .map_err(|err| format!("create_dir_all({}): {err}", root.display()))?;
    Ok(root)
}

/// Writes `files`, analyzes one added line, removes the workspace, and
/// returns the single finding for that line.
fn analyze_one_line(
    label: &str,
    files: &[(&str, &str)],
    changed: &str,
    line: usize,
) -> Result<Finding, String> {
    let root = unique_tempdir(label)?;
    let written = files
        .iter()
        .try_for_each(|(path, contents)| write_file(&root.join(path), contents));
    let text = files
        .iter()
        .find(|(path, _)| *path == changed)
        .and_then(|(_, contents)| contents.lines().nth(line.saturating_sub(1)))
        .unwrap_or_default()
        .to_string();
    let options = AnalysisOptions {
        root: root.clone(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: false,
        perl_facts_path: None,
        git_timeout: None,
        git_candidate: None,
        production_like_targets: Default::default(),
        test_harnesses: Vec::new(),
        resolved_subject_identity: None,
        open_rust_index_paths: Default::default(),
    };
    let changed_files = vec![ChangedFile {
        path: PathBuf::from(changed),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line,
            new_side_line: line,
            text,
        }],
        removed_lines: Vec::new(),
    }];
    let result = written.and_then(|()| {
        PythonAdapter.analyze_diff(&options, &OraclePolicy::default(), &changed_files)
    });
    let cleanup = std::fs::remove_dir_all(&root);
    let result = result?;
    cleanup.map_err(|err| format!("remove_dir_all({}): {err}", root.display()))?;
    let mut findings = result.findings;
    if findings.len() != 1 {
        return Err(format!(
            "{label}: expected exactly one finding, got {}: {:?}",
            findings.len(),
            findings
                .iter()
                .map(|f| &f.probe.expression)
                .collect::<Vec<_>>()
        ));
    }
    findings
        .pop()
        .ok_or_else(|| format!("{label}: finding vanished"))
}

fn related_names(finding: &Finding) -> Vec<&str> {
    finding
        .related_tests
        .iter()
        .map(|test| test.name.as_str())
        .collect()
}

const TIME_PY: &str = "def naturaldelta(days):\n    years = round(days / 365)\n    return str(years)\n\n\ndef other(days):\n    return str(days)\n";

#[test]
fn package_attribute_call_through_init_reexport_reaches_owner() -> Result<(), String> {
    let finding = analyze_one_line(
        "humanize-shape",
        &[
            ("src/humanize/time.py", TIME_PY),
            (
                "src/humanize/__init__.py",
                "from .time import naturaldelta\n\n__all__ = [\"naturaldelta\"]\n",
            ),
            (
                "tests/test_time.py",
                "import humanize\n\n\ndef test_naturaldelta():\n    assert humanize.naturaldelta(730) == \"2\"\n",
            ),
        ],
        "src/humanize/time.py",
        2,
    )?;
    if finding.class == ExposureClass::NoStaticPath {
        return Err(format!(
            "`humanize.naturaldelta(` through the package re-export must reach the owner; evidence: {:?}",
            finding.evidence
        ));
    }
    if related_names(&finding) != ["test_naturaldelta"] {
        return Err(format!(
            "expected the calling test to be related, got {:?}",
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn from_package_import_through_init_reexport_carries_module_identity() -> Result<(), String> {
    let finding = analyze_one_line(
        "from-package",
        &[
            ("src/humanize/time.py", TIME_PY),
            (
                "src/humanize/__init__.py",
                "from humanize.time import naturaldelta\n",
            ),
            (
                "tests/test_time.py",
                "from humanize import naturaldelta\n\n\ndef test_naturaldelta():\n    assert naturaldelta(730) == \"2\"\n",
            ),
        ],
        "src/humanize/time.py",
        2,
    )?;
    // Without the package path as module identity, the exact-value test is
    // identity-less and the finding reads weakly exposed with a
    // "strengthen existing test" repair: a wrong actionable signal.
    if finding.class != ExposureClass::Exposed || related_names(&finding) != ["test_naturaldelta"] {
        return Err(format!(
            "`from humanize import naturaldelta` must carry owner-module identity through the re-export, got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn star_reexport_with_all_reaches_owner_through_module_alias() -> Result<(), String> {
    let finding = analyze_one_line(
        "star-all",
        &[
            (
                "more_itertools/more.py",
                "__all__ = ['one']\n\n\ndef one(items):\n    if len(items) != 1:\n        raise ValueError('expected one')\n    return items[0]\n",
            ),
            (
                "more_itertools/__init__.py",
                "from .more import *  # noqa\n",
            ),
            (
                "tests/test_more.py",
                "import unittest\n\nimport more_itertools as mi\n\n\nclass OneTests(unittest.TestCase):\n    def test_one(self):\n        self.assertEqual(mi.one([7]), 7)\n",
            ),
        ],
        "more_itertools/more.py",
        5,
    )?;
    if finding.class == ExposureClass::NoStaticPath || related_names(&finding) != ["test_one"] {
        return Err(format!(
            "`mi.one(` through `from .more import *` must reach the owner, got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn star_reexport_excluded_by_all_stays_unreached() -> Result<(), String> {
    let finding = analyze_one_line(
        "star-all-excludes",
        &[
            (
                "pkg/more.py",
                "__all__ = ['two']\n\n\ndef one(items):\n    if len(items) != 1:\n        raise ValueError('expected one')\n    return items[0]\n\n\ndef two():\n    return 2\n",
            ),
            ("pkg/__init__.py", "from .more import *\n"),
            (
                "tests/test_more.py",
                "import pkg\n\n\ndef test_one():\n    assert pkg.one([7]) == 7\n",
            ),
        ],
        "pkg/more.py",
        5,
    )?;
    if finding.class != ExposureClass::NoStaticPath {
        return Err(format!(
            "`__all__` omits `one`, so the star re-export must not relate `pkg.one(`; got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn package_import_of_a_sibling_name_stays_unrelated() -> Result<(), String> {
    let finding = analyze_one_line(
        "sibling-name",
        &[
            ("src/humanize/time.py", TIME_PY),
            (
                "src/humanize/__init__.py",
                "from .time import naturaldelta, other\n",
            ),
            (
                "tests/test_time.py",
                "import humanize\n\n\ndef test_other():\n    assert humanize.other(3) == \"3\"\n",
            ),
        ],
        "src/humanize/time.py",
        2,
    )?;
    if finding.class != ExposureClass::NoStaticPath || !finding.related_tests.is_empty() {
        return Err(format!(
            "a test calling only `humanize.other(` must not relate `naturaldelta`; got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn renamed_reexport_is_not_followed() -> Result<(), String> {
    let finding = analyze_one_line(
        "renamed",
        &[
            ("src/humanize/time.py", TIME_PY),
            (
                "src/humanize/__init__.py",
                "from .time import naturaldelta as delta\n",
            ),
            (
                "tests/test_time.py",
                "import humanize\n\n\ndef test_delta():\n    assert humanize.naturaldelta(730) == \"2\"\n",
            ),
        ],
        "src/humanize/time.py",
        2,
    )?;
    if finding.class != ExposureClass::NoStaticPath {
        return Err(format!(
            "a renamed re-export does not bind `humanize.naturaldelta`; got {:?}",
            finding.class
        ));
    }
    Ok(())
}

#[test]
fn repo_mode_relates_package_attribute_calls_through_init_reexport() -> Result<(), String> {
    let root = unique_tempdir("repo-mode")?;
    let written = [
        ("src/humanize/time.py", TIME_PY),
        ("src/humanize/__init__.py", "from .time import naturaldelta\n"),
        (
            "tests/test_time.py",
            "import humanize\n\n\ndef test_naturaldelta():\n    assert humanize.naturaldelta(730) == \"2\"\n",
        ),
    ]
    .iter()
    .try_for_each(|(path, contents)| write_file(&root.join(path), contents));
    let result = written.and_then(|()| {
        PythonAdapter::analyze_repo_with_limit(
            &root,
            repo::RepoWorkingSetLimit {
                limit: 800,
                source: repo::RepoWorkingSetCapSource::Default,
            },
        )
    });
    let cleanup = std::fs::remove_dir_all(&root);
    let result = result?;
    cleanup.map_err(|err| format!("remove_dir_all({}): {err}", root.display()))?;
    let owner_findings: Vec<&Finding> = result
        .findings
        .iter()
        .filter(|finding| {
            finding
                .probe
                .owner
                .as_ref()
                .is_some_and(|owner| owner.0.ends_with("::naturaldelta"))
        })
        .collect();
    if owner_findings.is_empty() {
        return Err("expected repo-mode findings for `naturaldelta`".to_string());
    }
    for finding in owner_findings {
        if finding.class == ExposureClass::NoStaticPath
            || related_names(finding) != ["test_naturaldelta"]
        {
            return Err(format!(
                "repo mode must relate `humanize.naturaldelta(` like diff mode; got {:?} with {:?}",
                finding.class,
                related_names(finding)
            ));
        }
    }
    Ok(())
}

#[test]
fn shadowed_package_alias_stays_unrelated() -> Result<(), String> {
    let finding = analyze_one_line(
        "shadowed-alias",
        &[
            ("src/humanize/time.py", TIME_PY),
            (
                "src/humanize/__init__.py",
                "from .time import naturaldelta\n",
            ),
            (
                "tests/test_time.py",
                "import humanize\n\n\ndef test_naturaldelta(humanize):\n    assert humanize.naturaldelta(730) == \"2\"\n",
            ),
        ],
        "src/humanize/time.py",
        2,
    )?;
    if finding.class != ExposureClass::NoStaticPath {
        return Err(format!(
            "a fixture parameter named `humanize` shadows the package; got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn all_mentioned_only_in_a_docstring_does_not_filter_star_exports() -> Result<(), String> {
    let finding = analyze_one_line(
        "all-in-docstring",
        &[
            (
                "pkg/more.py",
                "\"\"\"This module intentionally has no __all__.\"\"\"\n\n\ndef one(items):\n    if len(items) != 1:\n        raise ValueError('expected one')\n    return items[0]\n",
            ),
            ("pkg/__init__.py", "from .more import *\n"),
            (
                "tests/test_more.py",
                "import pkg\n\n\ndef test_one():\n    assert pkg.one([7]) == 7\n",
            ),
        ],
        "pkg/more.py",
        5,
    )?;
    if finding.class == ExposureClass::NoStaticPath {
        return Err("a docstring mention of `__all__` is not a declaration".to_string());
    }
    Ok(())
}

#[test]
fn quoted_name_outside_all_does_not_satisfy_all() -> Result<(), String> {
    let finding = analyze_one_line(
        "quoted-elsewhere",
        &[
            (
                "pkg/more.py",
                "__all__ = ['two']\n\n\ndef one(items):\n    if len(items) != 1:\n        raise ValueError('one')\n    return items[0]\n\n\ndef two():\n    return 2\n",
            ),
            ("pkg/__init__.py", "from .more import *\n"),
            (
                "tests/test_more.py",
                "import pkg\n\n\ndef test_one():\n    assert pkg.one([7]) == 7\n",
            ),
        ],
        "pkg/more.py",
        5,
    )?;
    if finding.class != ExposureClass::NoStaticPath {
        return Err(format!(
            "`'one'` quoted in a message is not an `__all__` entry; got {:?}",
            finding.class
        ));
    }
    Ok(())
}

#[test]
fn a_later_initializer_binding_of_the_same_name_is_not_credited() -> Result<(), String> {
    let finding = analyze_one_line(
        "rebound-name",
        &[
            ("pkg/a.py", "def f():\n    value = 1\n    return value\n"),
            ("pkg/b.py", "def f():\n    return 2\n"),
            ("pkg/__init__.py", "from .a import f\nfrom .b import f\n"),
            (
                "tests/test_pkg.py",
                "import pkg\n\n\ndef test_f():\n    assert pkg.f() == 2\n",
            ),
        ],
        "pkg/a.py",
        2,
    )?;
    if finding.class != ExposureClass::NoStaticPath || !finding.related_tests.is_empty() {
        return Err(format!(
            "`pkg.f` is bound to `b.f`; `a.f` must not gain package identity, got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn an_initializer_assignment_that_replaces_the_name_is_not_credited() -> Result<(), String> {
    for (label, init) in [
        ("assign-rebind", "from .a import f\n\nf = lambda: 2\n"),
        (
            "conditional-rebind",
            "from .a import f\n\ntry:\n    import fast\nexcept ImportError:\n    pass\nelse:\n    f = fast.f\n",
        ),
        ("del-rebind", "from .a import f\n\ndel f\n"),
    ] {
        let finding = analyze_one_line(
            label,
            &[
                ("pkg/a.py", "def f():\n    value = 1\n    return value\n"),
                ("pkg/__init__.py", init),
                (
                    "tests/test_pkg.py",
                    "import pkg\n\n\ndef test_f():\n    assert pkg.f() == 2\n",
                ),
            ],
            "pkg/a.py",
            2,
        )?;
        if finding.class != ExposureClass::NoStaticPath || !finding.related_tests.is_empty() {
            return Err(format!(
                "{label}: the initializer rebinds `f`, so `a.f` must not gain package identity, got {:?} with {:?}",
                finding.class,
                related_names(&finding)
            ));
        }
    }
    Ok(())
}

#[test]
fn an_initializer_binding_of_another_name_keeps_the_reexport() -> Result<(), String> {
    let finding = analyze_one_line(
        "other-binding",
        &[
            ("pkg/a.py", "def f():\n    value = 1\n    return value\n"),
            (
                "pkg/__init__.py",
                "from .a import f\n\n__version__ = \"1.0\"\ng = f\n",
            ),
            (
                "tests/test_pkg.py",
                "import pkg\n\n\ndef test_f():\n    assert pkg.f() == 1\n",
            ),
        ],
        "pkg/a.py",
        2,
    )?;
    if related_names(&finding) != ["test_f"] {
        return Err(format!(
            "bindings of other names must not block the re-export of `f`, got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

#[test]
fn an_imported_replacement_of_all_fails_closed() -> Result<(), String> {
    let finding = analyze_one_line(
        "all-imported",
        &[
            (
                "pkg/more.py",
                "__all__ = ['one']\nfrom pkg.names import __all__\n\n\ndef one(items):\n    if len(items) != 1:\n        raise ValueError('expected one')\n    return items[0]\n",
            ),
            ("pkg/names.py", "__all__ = ['two']\n"),
            ("pkg/__init__.py", "from .more import *\n"),
            (
                "tests/test_more.py",
                "import pkg\n\n\ndef test_one():\n    assert pkg.one([7]) == 7\n",
            ),
        ],
        "pkg/more.py",
        6,
    )?;
    if finding.class != ExposureClass::NoStaticPath {
        return Err(format!(
            "`__all__` is replaced by an import, so the star re-export must not relate `pkg.one(`; got {:?} with {:?}",
            finding.class,
            related_names(&finding)
        ));
    }
    Ok(())
}

/// Runs repo mode over `files` and requires that no finding in `pkg/a.py`
/// gains a related test.
fn repo_mode_a_stays_unrelated(label: &str, files: &[(&str, &str)]) -> Result<(), String> {
    let root = unique_tempdir(label)?;
    let written = files
        .iter()
        .try_for_each(|(path, contents)| write_file(&root.join(path), contents));
    let result = written.and_then(|()| {
        PythonAdapter::analyze_repo_with_limit(
            &root,
            repo::RepoWorkingSetLimit {
                limit: 800,
                source: repo::RepoWorkingSetCapSource::Default,
            },
        )
    });
    let cleanup = std::fs::remove_dir_all(&root);
    let result = result?;
    cleanup.map_err(|err| format!("remove_dir_all({}): {err}", root.display()))?;
    let a_findings: Vec<&Finding> = result
        .findings
        .iter()
        .filter(|finding| finding.probe.location.file.ends_with("a.py"))
        .collect();
    if a_findings.is_empty() {
        return Err(format!(
            "{label}: expected repo-mode findings for `pkg/a.py`"
        ));
    }
    for finding in a_findings {
        if !finding.related_tests.is_empty() {
            return Err(format!(
                "{label}: `pkg.f` is not bound to `a.f`; it must not gain package identity, got {:?} with {:?}",
                finding.class,
                related_names(finding)
            ));
        }
    }
    Ok(())
}

const A_PY: &str = "def f():\n    value = 1\n    return value\n";
const TEST_PKG_PY: &str = "import pkg\n\n\ndef test_f():\n    assert pkg.f() == 2\n";

/// Repo mode analyzes one production file at a time; the star sources of each
/// initializer are loaded up front so `b.f` is known while `a.py` is analyzed.
#[test]
fn repo_mode_competing_star_exports_are_not_credited() -> Result<(), String> {
    repo_mode_a_stays_unrelated(
        "repo-star-conflict",
        &[
            ("pkg/a.py", A_PY),
            ("pkg/b.py", "def f():\n    return 2\n"),
            ("pkg/__init__.py", "from .a import *\nfrom .b import *\n"),
            ("tests/test_pkg.py", TEST_PKG_PY),
        ],
    )
}

#[test]
fn repo_mode_initializer_assignment_that_replaces_the_name_is_not_credited() -> Result<(), String> {
    repo_mode_a_stays_unrelated(
        "repo-assign-rebind",
        &[
            ("pkg/a.py", A_PY),
            ("pkg/__init__.py", "from .a import f\n\nf = lambda: 2\n"),
            ("tests/test_pkg.py", TEST_PKG_PY),
        ],
    )
}

const UTILS_PY: &str = "def _expand(args, env=True):\n    if env:\n        return [a.upper() for a in args]\n    return list(args)\n";

/// #4560, the click shape: `import click` then `click.utils._expand_args(...)`.
/// The package `__init__.py` does not re-export the helper; the test reaches
/// it through the submodule attribute path.
#[test]
fn package_import_reaches_owner_through_submodule_attribute_path() -> Result<(), String> {
    for (label, test_source) in [
        (
            "submodule-plain",
            "import click\n\n\ndef test_expand():\n    assert click.utils._expand([\"a\"]) == [\"A\"]\n",
        ),
        (
            "submodule-alias",
            "import click as c\n\n\ndef test_expand():\n    assert c.utils._expand([\"a\"]) == [\"A\"]\n",
        ),
        (
            "submodule-from",
            "from click import _internal\n\n\ndef test_expand():\n    assert _internal.utils._expand([\"a\"]) == [\"A\"]\n",
        ),
    ] {
        let internal = label == "submodule-from";
        let owner_path = if internal {
            "src/click/_internal/utils.py"
        } else {
            "src/click/utils.py"
        };
        let finding = analyze_one_line(
            label,
            &[
                (owner_path, UTILS_PY),
                ("src/click/__init__.py", "from .core import main\n"),
                ("src/click/core.py", "def main():\n    return 0\n"),
                ("tests/test_utils.py", test_source),
            ],
            owner_path,
            2,
        )?;
        if related_names(&finding) != ["test_expand"] {
            return Err(format!(
                "{label}: the submodule attribute call must relate the test, got {:?} ({:?})",
                related_names(&finding),
                finding.class
            ));
        }
    }
    Ok(())
}

/// #4560, the dateutil shape: the owner lives in a package initializer
/// (`src/dateutil/zoneinfo/__init__.py`) and the test calls it through
/// `from dateutil import zoneinfo`. The file stem is `__init__`, so only the
/// dotted module path identifies it.
#[test]
fn package_initializer_owner_is_reached_through_its_module_import() -> Result<(), String> {
    let finding = analyze_one_line(
        "init-owner",
        &[
            (
                "src/dateutil/zoneinfo/__init__.py",
                "def get_zonefile_instance(new_instance=False):\n    if new_instance:\n        return 1\n    return 0\n",
            ),
            ("src/dateutil/__init__.py", ""),
            (
                "tests/test_tz.py",
                "from dateutil import zoneinfo\n\n\ndef test_new_instance():\n    assert zoneinfo.get_zonefile_instance(new_instance=True) == 1\n",
            ),
        ],
        "src/dateutil/zoneinfo/__init__.py",
        2,
    )?;
    if related_names(&finding) != ["test_new_instance"] {
        return Err(format!(
            "the package initializer owner must relate its module-import caller, got {:?} ({:?})",
            related_names(&finding),
            finding.class
        ));
    }
    Ok(())
}

/// Discriminating negative for #4560: the dotted path must equal the owner's
/// module path. `click.other._expand(` names a different module, and a local
/// that shadows the package alias is not the import.
#[test]
fn submodule_attribute_path_to_another_module_stays_unreached() -> Result<(), String> {
    for (label, test_source) in [
        (
            "submodule-other",
            "import click\n\n\ndef test_expand():\n    assert click.other._expand([\"a\"]) == [\"A\"]\n",
        ),
        (
            "submodule-shadow",
            "import click\n\n\ndef test_expand(click):\n    assert click.utils._expand([\"a\"]) == [\"A\"]\n",
        ),
    ] {
        let finding = analyze_one_line(
            label,
            &[
                ("src/click/utils.py", UTILS_PY),
                ("src/click/other.py", UTILS_PY),
                ("src/click/__init__.py", ""),
                ("tests/test_utils.py", test_source),
            ],
            "src/click/utils.py",
            2,
        )?;
        if finding.class != ExposureClass::NoStaticPath {
            return Err(format!(
                "{label}: expected no_static_path, got {:?} with {:?}",
                finding.class,
                related_names(&finding)
            ));
        }
    }
    Ok(())
}

const SHARED_CALC_PY: &str = "def price(amount):\n    return amount - 1\n\n\nclass Calculator:\n    def total(self, amount):\n        return amount * 2\n";

/// #4566 end to end: `a/src/shared` and `b/src/shared` both import as
/// `shared`. A strong test inside `a` relates to a change in `a`'s code; the
/// same test inside `b` exercises `b`'s code and must neither make that change
/// `exposed` nor supply the module-identity relation. One row per import shape
/// that carries module identity: `from M import f`, `from M import Class` +
/// method call, `import M as m` + `m.f(`, `from P import m` + `m.f(` or
/// `m.Class()`, `import P` + `P.m.f(` (#4560), and a package re-export `from shared import f`. A module
/// binding never reaches `exposed` for a free function (its identity rule
/// wants `from M import f`), so those rows check their
/// `import_alias_call` relation instead. `syntactic_call` is a name-level
/// relation for every row and is not identity.
#[test]
fn same_named_src_package_in_another_project_lends_no_exposure() -> Result<(), String> {
    let reexport_init = "from .calc import price\n";
    let rows: [(&str, &str, usize, &str); 7] = [
        (
            "from-module",
            "from shared.calc import price\n\n\ndef test_price():\n    assert price(10) == 9\n",
            2,
            "",
        ),
        (
            "class-method",
            "from shared.calc import Calculator\n\n\ndef test_total():\n    assert Calculator().total(3) == 6\n",
            7,
            "",
        ),
        (
            "module-alias",
            "import shared.calc as calc\n\n\ndef test_price():\n    assert calc.price(10) == 9\n",
            2,
            "import_alias_call",
        ),
        (
            "package-submodule",
            "from shared import calc\n\n\ndef test_price():\n    assert calc.price(10) == 9\n",
            2,
            "import_alias_call",
        ),
        (
            "package-dotted-path",
            "import shared\n\n\ndef test_price():\n    assert shared.calc.price(10) == 9\n",
            2,
            "import_alias_call",
        ),
        (
            "class-via-package-submodule",
            "from shared import calc\n\n\ndef test_total():\n    assert calc.Calculator().total(3) == 6\n",
            7,
            "",
        ),
        (
            "package-reexport",
            "from shared import price\n\n\ndef test_price():\n    assert price(10) == 9\n",
            2,
            "",
        ),
    ];
    for (label, test_source, line, identity_relation) in rows {
        for (test_project, own_project) in [("a", true), ("b", false)] {
            let test_path = format!("{test_project}/tests/test_calc.py");
            let finding = analyze_one_line(
                &format!("{label}-{test_project}"),
                &[
                    ("a/src/shared/calc.py", SHARED_CALC_PY),
                    ("a/src/shared/__init__.py", reexport_init),
                    ("b/src/shared/calc.py", SHARED_CALC_PY),
                    ("b/src/shared/__init__.py", reexport_init),
                    (&test_path, test_source),
                ],
                "a/src/shared/calc.py",
                line,
            )?;
            let exposed = finding.class == ExposureClass::Exposed;
            let identity_related = !identity_relation.is_empty()
                && finding.evidence.iter().any(|line| {
                    line.starts_with(&format!("related_test_relation: {identity_relation} "))
                });
            let ok = if own_project {
                (exposed
                    || matches!(
                        label,
                        "module-alias" | "package-submodule" | "package-dotted-path"
                    ))
                    && (identity_relation.is_empty() || identity_related)
            } else {
                !exposed && !identity_related
            };
            if !ok {
                return Err(format!(
                    "{label}: test in `{test_project}` gave {:?}; related {:?}; evidence {:?}",
                    finding.class,
                    related_names(&finding),
                    finding.evidence
                ));
            }
        }
    }
    Ok(())
}

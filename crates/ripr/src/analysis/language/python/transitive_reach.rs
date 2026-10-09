//! Bounded same-class transitive-reach check for Python `no_static_path`.
//!
//! When the direct-call classifier finds no related test, this module names a
//! limitation if a test constructs or calls into the owner's class and a
//! bounded `self.` / `cls.` path of depth 1..=5 may reach the changed method.
//!
//! Fail-closed (RIPR-SPEC-0201 / #4765):
//! - classification stays `no_static_path`;
//! - the witness is never added to `related_tests`;
//! - `getattr`, `super()`, nested functions/lambdas, and other-module classes
//!   do not create edges;
//! - function-to-helper / cross-module façade paths are out of scope (#4568).

use super::related_tests::{
    body_calls_method_on_owner_bound_receiver, contains_call_name,
    import_source_module_matches_owner, owner_module_paths,
};
use super::{PythonOwner, PythonTest};
use crate::domain::{
    ExposureClass, Finding, LIMITATION_ANALYZER_ROUTE_PREFIX,
    LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX, LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
    LIMITATION_NON_CLAIM_PREFIX, OwnerKind, StaticLimitKind, StopReason,
    TRANSITIVE_REACH_WITNESS_PREFIX,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

const MAX_TRANSITIVE_DEPTH: usize = 5;
const ANALYZER_ROUTE: &str = "analysis/python-same-class-transitive-reach";

pub(super) const PYTHON_TRANSITIVE_REACH_MESSAGE: &str = "ripr saw a Python test that constructs or calls into this owner's class; \
a bounded same-class method path may lead here, but the preview adapter does \
not fully trace it. This is not a coverage assessment — ripr cannot confirm \
or deny that the change is observed.";

const NO_STATICALLY_REACHABLE_PYTHON_PATH: &str = "No statically reachable Python test path was found; a test may construct or call into the owner's class through a method path ripr does not fully trace.";

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonTransitiveWitness {
    test_name: String,
    test_file: PathBuf,
    test_line: usize,
    entry_symbol: String,
    other_test_count: usize,
}

/// Attach a named Python transitive-reach limitation after classification.
/// Never changes `class` or `related_tests`.
pub(super) fn apply_python_no_static_path_limit(
    finding: &mut Finding,
    owner: &PythonOwner,
    owners: &[PythonOwner],
    tests: &[PythonTest],
) {
    if !(finding.class == ExposureClass::NoStaticPath
        && finding.related_tests.is_empty()
        && finding.static_limit_kind.is_none())
    {
        return;
    }
    let Some(witness) = find_python_transitive_witness(owner, owners, tests) else {
        return;
    };
    finding.static_limit_kind = Some(StaticLimitKind::PythonTransitiveReachUnresolved);
    finding
        .stop_reasons
        .push(StopReason::TransitiveReachUnresolved);
    for line in &mut finding.missing {
        if line.starts_with("No Python test references") {
            *line = NO_STATICALLY_REACHABLE_PYTHON_PATH.to_string();
        }
    }
    finding
        .evidence
        .push(PYTHON_TRANSITIVE_REACH_MESSAGE.to_string());
    finding.evidence.push(witness_pointer(&witness));
    finding
        .evidence
        .extend(limitation_detail_lines(&witness, &owner.name));
}

fn find_python_transitive_witness(
    owner: &PythonOwner,
    owners: &[PythonOwner],
    tests: &[PythonTest],
) -> Option<PythonTransitiveWitness> {
    let class = owner_class_name(owner)?;
    if !method_reaches_owner_from_another_method(owner, owners) {
        return None;
    }
    let methods = same_class_methods(owner, owners);
    let graph = method_graph(&methods);
    let mut witnesses: Vec<(PathBuf, usize, String, String)> = Vec::new();
    for test in tests {
        let locals = import_provenanced_class_locals(test, owner, class);
        if locals.is_empty() {
            continue;
        }
        let mut entry: Option<String> = None;
        for local in &locals {
            if contains_call_name(&test.body_text, local) {
                match &entry {
                    Some(current) if current.as_str() <= class => {}
                    _ => entry = Some(class.to_string()),
                }
            }
            for method in &methods {
                if method.name == owner.name {
                    continue;
                }
                if body_calls_method_on_owner_bound_receiver(&test.body_text, local, &method.name)
                    && reaches_owner(&method.name, &owner.name, &graph)
                {
                    match &entry {
                        Some(current) if current.as_str() <= method.name.as_str() => {}
                        _ => entry = Some(method.name.clone()),
                    }
                }
            }
        }
        if let Some(symbol) = entry {
            witnesses.push((test.file.clone(), test.line, test.name.clone(), symbol));
        }
    }
    if witnesses.is_empty() {
        return None;
    }
    witnesses.sort();
    let other_test_count = witnesses.len() - 1;
    let (test_file, test_line, test_name, entry_symbol) = witnesses.into_iter().next()?;
    Some(PythonTransitiveWitness {
        test_name,
        test_file,
        test_line,
        entry_symbol,
        other_test_count,
    })
}

fn owner_class_name(owner: &PythonOwner) -> Option<&str> {
    if !matches!(
        owner.owner_kind,
        Some(OwnerKind::Method | OwnerKind::ClassMethod)
    ) {
        return None;
    }
    owner
        .qualified_name
        .rsplit_once('.')
        .map(|(class, _)| class)
        .filter(|class| !class.is_empty())
}

fn same_class_methods<'a>(
    owner: &'a PythonOwner,
    owners: &'a [PythonOwner],
) -> Vec<&'a PythonOwner> {
    let Some(class) = owner_class_name(owner) else {
        return Vec::new();
    };
    owners
        .iter()
        .filter(|other| {
            other.file == owner.file
                && owner_class_name(other) == Some(class)
                && matches!(
                    other.owner_kind,
                    Some(OwnerKind::Method | OwnerKind::ClassMethod)
                )
        })
        .collect()
}

fn method_graph<'a>(methods: &'a [&'a PythonOwner]) -> HashMap<&'a str, &'a [String]> {
    methods
        .iter()
        .map(|method| (method.name.as_str(), method.same_class_callees.as_slice()))
        .collect()
}

fn method_reaches_owner_from_another_method(owner: &PythonOwner, owners: &[PythonOwner]) -> bool {
    let methods = same_class_methods(owner, owners);
    let graph = method_graph(&methods);
    methods
        .iter()
        .any(|method| method.name != owner.name && reaches_owner(&method.name, &owner.name, &graph))
}

fn reaches_owner(start: &str, owner: &str, graph: &HashMap<&str, &[String]>) -> bool {
    if start == owner || start.is_empty() || owner.is_empty() {
        return false;
    }
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start, 0usize));
    seen.insert(start);
    while let Some((current, depth)) = queue.pop_front() {
        if depth >= MAX_TRANSITIVE_DEPTH {
            continue;
        }
        let Some(callees) = graph.get(current) else {
            continue;
        };
        for callee in *callees {
            if callee == owner {
                return true;
            }
            if !seen.insert(callee.as_str()) {
                continue;
            }
            if graph.contains_key(callee.as_str()) {
                queue.push_back((callee.as_str(), depth + 1));
            }
        }
    }
    false
}

/// Class names the test may construct or call, only when the import identifies
/// the owner's module (full dotted path, not a coincidental file stem).
fn import_provenanced_class_locals(
    test: &PythonTest,
    owner: &PythonOwner,
    class: &str,
) -> Vec<String> {
    let owner_modules = owner_module_paths(&owner.file);
    let mut locals = Vec::new();
    for import in &test.imports {
        if import.imported == class
            && !import.alias.is_empty()
            && import_source_module_matches_owner(import, owner, &test.file)
            && !locals.contains(&import.alias)
        {
            locals.push(import.alias.clone());
        }
        if import.source_module.is_empty()
            && !import.alias.is_empty()
            && (owner_modules.contains(&import.imported)
                || owner.reexport_modules.contains(&import.imported))
        {
            let qualified = format!("{}.{class}", import.alias);
            if !locals.contains(&qualified) {
                locals.push(qualified);
            }
        }
    }
    locals
}

fn witness_pointer(witness: &PythonTransitiveWitness) -> String {
    let location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    let others = match witness.other_test_count {
        0 => String::new(),
        1 => " (and 1 other test)".to_string(),
        n => format!(" (and {n} other tests)"),
    };
    format!(
        "{}`{}` ({}) calls `{}`, an entry point that may lead here{}. \
         Inspect it to judge whether this change is observed.",
        TRANSITIVE_REACH_WITNESS_PREFIX, witness.test_name, location, witness.entry_symbol, others
    )
}

fn limitation_detail_lines(witness: &PythonTransitiveWitness, owner_name: &str) -> [String; 4] {
    let location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    [
        format!(
            "{}test `{}` ({}) -> entry `{}`",
            LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
            witness.test_name,
            location,
            witness.entry_symbol
        ),
        format!(
            "{}entry `{}` -> owner `{}` through a bounded same-class Python method path",
            LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX, witness.entry_symbol, owner_name
        ),
        format!("{}{ANALYZER_ROUTE}", LIMITATION_ANALYZER_ROUTE_PREFIX),
        format!(
            "{}named limitation only; ripr cannot confirm or deny that this path observes the change",
            LIMITATION_NON_CLAIM_PREFIX
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::language::python::classify::classify_change;
    use crate::analysis::language::python::owners_tests::{extract_owners, extract_tests};
    use crate::domain::StopReason;
    use std::path::Path;

    const TABLE_SOURCE: &str = r#"
class Table:
    def __init__(self):
        self.columns = [1, 2]

    def __rich_console__(self, console, options):
        return self._calculate_column_widths()

    def _calculate_column_widths(self):
        return self._get_padding_width(0)

    def _get_padding_width(self, column_index):
        if column_index == len(self.columns) - 1:
            return 0
        return 1
"#;

    fn classify_owner(
        source: &str,
        tests: &str,
        owner_file: &str,
        test_file: &str,
        line_text: &str,
    ) -> Result<Finding, String> {
        let file = Path::new(owner_file);
        let line = source
            .lines()
            .position(|line| line == line_text)
            .ok_or_else(|| format!("fixture source must contain the changed line: {line_text}"))?
            + 1;
        let owners = extract_owners(file, source);
        let tests = extract_tests(Path::new(test_file), tests);
        classify_change(file, line, line_text, &owners, &tests)
            .ok_or_else(|| "behavioral fixture must produce a finding".to_string())
    }

    fn padding_finding(tests: &str) -> Result<Finding, String> {
        classify_owner(
            TABLE_SOURCE,
            tests,
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == len(self.columns) - 1:",
        )
    }

    fn assert_named_limitation(finding: &Finding) {
        assert_eq!(finding.class, ExposureClass::NoStaticPath);
        assert!(finding.related_tests.is_empty());
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::PythonTransitiveReachUnresolved)
        );
        assert!(
            finding
                .stop_reasons
                .contains(&StopReason::TransitiveReachUnresolved)
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line == PYTHON_TRANSITIVE_REACH_MESSAGE)
        );
        assert!(
            finding
                .missing
                .iter()
                .all(|line| !line.contains("No Python test references")),
            "witnessed limitation must not claim no tests were found: {:?}",
            finding.missing
        );
    }

    fn assert_silent(finding: &Finding) {
        assert_eq!(finding.class, ExposureClass::NoStaticPath);
        assert!(finding.related_tests.is_empty());
        assert_eq!(finding.static_limit_kind, None);
        assert!(
            !finding
                .stop_reasons
                .contains(&StopReason::TransitiveReachUnresolved)
        );
    }

    #[test]
    fn construction_plus_self_path_names_limitation() -> Result<(), String> {
        let finding = padding_finding(
            "from src.table import Table\n\ndef test_print_table():\n    table = Table()\n    assert table is not None\n",
        )?;
        assert_named_limitation(&finding);
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.contains("entry `Table`")),
            "construction witness should name the class: {:?}",
            finding.evidence
        );
        Ok(())
    }

    #[test]
    fn import_alias_construction_names_limitation() -> Result<(), String> {
        let finding = padding_finding(
            "from src.table import Table as T\n\ndef test_print_table():\n    table = T()\n    assert table is not None\n",
        )?;
        assert_named_limitation(&finding);
        Ok(())
    }

    #[test]
    fn module_import_construction_names_limitation() -> Result<(), String> {
        let finding = padding_finding(
            "import src.table as table\n\ndef test_print_table():\n    value = table.Table()\n    assert value is not None\n",
        )?;
        assert_named_limitation(&finding);
        Ok(())
    }

    #[test]
    fn bound_method_alias_inside_class_is_an_edge() -> Result<(), String> {
        let source = r#"
class Table:
    def render(self):
        helper = self._get_padding_width
        return helper(0)

    def _get_padding_width(self, column_index):
        if column_index == 0:
            return 0
        return 1
"#;
        let finding = classify_owner(
            source,
            "from src.table import Table\n\ndef test_print_table():\n    table = Table()\n    assert table is not None\n",
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == 0:",
        )?;
        assert_named_limitation(&finding);
        Ok(())
    }

    #[test]
    fn classmethod_cls_call_is_an_edge() -> Result<(), String> {
        let source = r#"
class Factory:
    @classmethod
    def create(cls):
        return cls.build()

    @classmethod
    def build(cls):
        if True:
            return 1
        return 0
"#;
        let finding = classify_owner(
            source,
            "from src.factory import Factory\n\ndef test_create():\n    obj = Factory.create()\n    assert obj == 1\n",
            "src/factory.py",
            "tests/test_factory.py",
            "        if True:",
        )?;
        assert_named_limitation(&finding);
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.contains("entry `create`")),
            "classmethod call should be the entry, not mere construction: {:?}",
            finding.evidence
        );
        Ok(())
    }

    #[test]
    fn import_without_construct_or_call_stays_silent() -> Result<(), String> {
        let finding = padding_finding(
            "from src.table import Table\n\ndef test_placeholder():\n    assert True\n",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn same_named_other_module_class_stays_silent() -> Result<(), String> {
        let finding = padding_finding(
            "from other.models import Table\n\ndef test_other():\n    table = Table()\n    assert table is not None\n",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn construction_without_self_path_stays_silent() -> Result<(), String> {
        let source = r#"
class Table:
    def public(self):
        return 1

    def _get_padding_width(self, column_index):
        if column_index == 0:
            return 0
        return 1
"#;
        let finding = classify_owner(
            source,
            "from src.table import Table\n\ndef test_print_table():\n    table = Table()\n    assert table is not None\n",
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == 0:",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn getattr_and_foreign_receiver_are_not_edges() -> Result<(), String> {
        let source = r#"
class Table:
    def render(self, other):
        getattr(self, "_get_padding_width")(0)
        other._get_padding_width(0)
        return 1

    def _get_padding_width(self, column_index):
        if column_index == 0:
            return 0
        return 1
"#;
        let finding = classify_owner(
            source,
            "from src.table import Table\n\ndef test_print_table():\n    table = Table()\n    assert table is not None\n",
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == 0:",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn nested_inner_function_self_call_is_not_an_edge() -> Result<(), String> {
        let source = r#"
class Table:
    def render(self):
        def inner():
            return self._get_padding_width(0)
        return inner()

    def _get_padding_width(self, column_index):
        if column_index == 0:
            return 0
        return 1
"#;
        let finding = classify_owner(
            source,
            "from src.table import Table\n\ndef test_print_table():\n    table = Table()\n    assert table is not None\n",
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == 0:",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn lambda_self_call_is_not_an_edge() -> Result<(), String> {
        let source = r#"
class Table:
    def render(self):
        helper = lambda: self._get_padding_width(0)
        return helper()

    def _get_padding_width(self, column_index):
        if column_index == 0:
            return 0
        return 1
"#;
        let finding = classify_owner(
            source,
            "from src.table import Table\n\ndef test_print_table():\n    table = Table()\n    assert table is not None\n",
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == 0:",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn depth_five_names_limitation_and_depth_six_stays_silent() -> Result<(), String> {
        let chain = |hops: usize| {
            let mut body = String::from(
                "class Box:\n    @classmethod\n    def m0(cls):\n        return cls.m1()\n",
            );
            for index in 1..hops {
                body.push_str(&format!(
                    "    @classmethod\n    def m{index}(cls):\n        return cls.m{}()\n",
                    index + 1
                ));
            }
            body.push_str(&format!(
                "    @classmethod\n    def m{hops}(cls):\n        if True:\n            return 1\n        return 0\n"
            ));
            body
        };
        // Call the classmethod entry without constructing `Box(`, so the depth
        // bound is measured from `m0` rather than from a shorter suffix.
        let tests = "from src.box import Box\n\ndef test_box():\n    assert Box.m0() == 1\n";
        let depth_five = classify_owner(
            &chain(5),
            tests,
            "src/box.py",
            "tests/test_box.py",
            "        if True:",
        )?;
        assert_named_limitation(&depth_five);

        let depth_six = classify_owner(
            &chain(6),
            tests,
            "src/box.py",
            "tests/test_box.py",
            "        if True:",
        )?;
        assert_silent(&depth_six);
        Ok(())
    }

    #[test]
    fn direct_owner_call_stays_related_not_this_limitation() -> Result<(), String> {
        let finding = classify_owner(
            TABLE_SOURCE,
            "from src.table import Table\n\ndef test_padding():\n    table = Table()\n    assert table._get_padding_width(0) == 1\n",
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == len(self.columns) - 1:",
        )?;
        assert!(!finding.related_tests.is_empty());
        assert_ne!(
            finding.static_limit_kind,
            Some(StaticLimitKind::PythonTransitiveReachUnresolved)
        );
        Ok(())
    }

    #[test]
    fn cross_module_function_facade_without_class_construction_stays_silent() -> Result<(), String>
    {
        let source = r#"
class PreparedRequest:
    def prepare_body(self, data):
        if data:
            return data
        return None
"#;
        let finding = classify_owner(
            source,
            "import requests\n\ndef test_post():\n    requests.post(\"https://example.test\", data=b\"x\")\n",
            "src/models.py",
            "tests/test_requests.py",
            "        if data:",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn no_tests_stay_silent() -> Result<(), String> {
        let finding = padding_finding("")?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn staticmethod_does_not_contribute_self_edges() -> Result<(), String> {
        let source = r#"
class Table:
    @staticmethod
    def render():
        return Table()._get_padding_width(0)

    def _get_padding_width(self, column_index):
        if column_index == 0:
            return 0
        return 1
"#;
        let finding = classify_owner(
            source,
            "from src.table import Table\n\ndef test_print_table():\n    table = Table()\n    assert table is not None\n",
            "src/table.py",
            "tests/test_table.py",
            "        if column_index == 0:",
        )?;
        assert_silent(&finding);
        Ok(())
    }

    #[test]
    fn wire_message_uses_may_language_and_not_coverage() {
        assert!(PYTHON_TRANSITIVE_REACH_MESSAGE.contains("may"));
        assert!(PYTHON_TRANSITIVE_REACH_MESSAGE.contains("not a coverage assessment"));
        assert!(!PYTHON_TRANSITIVE_REACH_MESSAGE.contains("covers"));
        assert!(!PYTHON_TRANSITIVE_REACH_MESSAGE.contains("tested"));
        assert!(!PYTHON_TRANSITIVE_REACH_MESSAGE.contains("reaches the change"));
    }

    #[test]
    fn limitation_detail_names_edges_route_and_non_claim() {
        let witness = PythonTransitiveWitness {
            test_name: "test_print_table".to_string(),
            test_file: PathBuf::from("tests/test_table.py"),
            test_line: 4,
            entry_symbol: "Table".to_string(),
            other_test_count: 0,
        };
        let detail = limitation_detail_lines(&witness, "_get_padding_width");
        assert_eq!(
            detail[0],
            "limitation_last_established_edge: test `test_print_table` (tests/test_table.py:4) -> entry `Table`"
        );
        assert_eq!(
            detail[1],
            "limitation_first_unresolved_edge: entry `Table` -> owner `_get_padding_width` through a bounded same-class Python method path"
        );
        assert_eq!(
            detail[2],
            "limitation_analyzer_route: analysis/python-same-class-transitive-reach"
        );
        assert!(detail[3].starts_with("limitation_non_claim: "));
        assert!(detail[3].contains("cannot confirm or deny"));
    }

    #[test]
    fn extract_same_class_callees_from_control_flow_and_skips_staticmethod() {
        let owners = extract_owners(
            Path::new("src/table.py"),
            r#"
class Table:
    def render(self):
        if True:
            return self._calculate_column_widths()
        return 0

    def _calculate_column_widths(self):
        return self._get_padding_width(0)

    @staticmethod
    def normalize(width):
        return self._get_padding_width(width)
"#,
        );
        assert_eq!(
            owners
                .iter()
                .find(|owner| owner.name == "render")
                .map(|owner| &owner.same_class_callees),
            Some(&vec!["_calculate_column_widths".to_string()])
        );
        assert_eq!(
            owners
                .iter()
                .find(|owner| owner.name == "_calculate_column_widths")
                .map(|owner| &owner.same_class_callees),
            Some(&vec!["_get_padding_width".to_string()])
        );
        assert_eq!(
            owners
                .iter()
                .find(|owner| owner.name == "normalize")
                .map(|owner| owner.same_class_callees.is_empty()),
            Some(true)
        );
    }
}

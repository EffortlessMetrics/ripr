//! Test registration is separate from execution and failure propagation.
//!
//! Conditions and runtime rebinding are not evaluated. Recognized controls
//! therefore withhold credit even when a conditional skip might be inactive.

use super::{PythonImport, PythonTest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeclarationSite {
    Function,
    Class,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PythonActivationControl {
    name: String,
    site: DeclarationSite,
}

pub(super) fn activation_control(test: &PythonTest) -> Option<&str> {
    test.activation_controls
        .iter()
        .find(|control| match (control.name.as_str(), control.site) {
            ("unittest.expectedFailure", _)
            | (
                "unittest.skip" | "unittest.skipIf" | "unittest.skipUnless",
                DeclarationSite::Class,
            ) => test.framework == "unittest",
            _ => true,
        })
        .map(|control| control.name.as_str())
}

pub(super) fn activation_controls(
    decorators: &[String],
    definition_imports: &[PythonImport],
    site: DeclarationSite,
) -> Vec<PythonActivationControl> {
    decorators
        .iter()
        .filter_map(|decorator| resolve_control(decorator, definition_imports))
        .map(|name| PythonActivationControl { name, site })
        .collect()
}

fn resolve_control(decorator: &str, imports: &[PythonImport]) -> Option<String> {
    let (root, tail) = decorator.split_once('.').unwrap_or((decorator, ""));
    // Avoid resolving ordinary fixture/parametrization metadata at every query.
    if !tail.is_empty()
        && !matches!(
            tail.rsplit('.').next(),
            Some("skip" | "skipIf" | "skipUnless" | "expectedFailure" | "skipif" | "xfail")
        )
    {
        return None;
    }
    let Some(import) = imports.iter().rev().find(|import| import.alias == root) else {
        // Unresolved canonical spellings retain conservative unestablished
        // activation; an explicit different import must never be bypassed.
        return is_control_name(decorator).then(|| decorator.to_string());
    };
    let mut resolved = if import.source_module.is_empty() {
        import.imported.clone()
    } else {
        format!("{}.{}", import.source_module, import.imported)
    };
    if !tail.is_empty() {
        resolved.push('.');
        resolved.push_str(tail);
    }
    is_control_name(&resolved).then_some(resolved)
}

fn is_control_name(name: &str) -> bool {
    matches!(
        name,
        "unittest.skip"
            | "unittest.skipIf"
            | "unittest.skipUnless"
            | "unittest.expectedFailure"
            | "pytest.mark.skip"
            | "pytest.mark.skipif"
            | "pytest.mark.xfail"
    )
}

use super::super::rust_index::{RustIndex, find_owner_function};
use super::config::{expected_sinks, required_oracles};
use super::family::family_for_probe_shape;
use super::sanitize_path;
use crate::domain::{DeltaKind, Probe, ProbeId, SourceLocation};
use std::path::Path;

pub fn probes_for_repo_file(root: &Path, path: &Path, index: &RustIndex) -> Vec<Probe> {
    let mut probes = Vec::new();
    let Some(facts) = index.files.get(path) else {
        return probes;
    };

    for shape in &facts.probe_shapes {
        let Some(family) = family_for_probe_shape(&shape.kind) else {
            continue;
        };

        let owner = find_owner_function(index, path, shape.start_line).map(|f| f.id.clone());

        let id = ProbeId(format!(
            "repo-probe:{}:{}:{}",
            sanitize_path(path),
            shape.start_line,
            family.as_str()
        ));

        let expected_sinks = expected_sinks(&shape.text, &family);
        let required_oracles = required_oracles(&shape.text, &family);

        probes.push(Probe {
            id,
            location: SourceLocation::new(root.join(path), shape.start_line, 1),
            owner,
            family,
            delta: DeltaKind::Unknown,
            before: None,
            after: Some(shape.text.clone()),
            expression: shape.text.clone(),
            expected_sinks,
            required_oracles,
        });
    }

    probes
}

#[cfg(test)]
mod tests {
    use super::super::super::rust_index::{
        FileFacts, FunctionFact, PROBE_SHAPE_ERROR_PATH, ProbeShapeFact, RustIndex,
    };
    use super::*;
    use crate::domain::{DeltaKind, ProbeFamily, SymbolId};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    #[test]
    fn probes_for_repo_file_emits_known_shape_with_owner() {
        let path = PathBuf::from("src/lib.rs");
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    functions: vec![FunctionFact {
                        id: SymbolId("auth::authenticate".to_string()),
                        name: "authenticate".to_string(),
                        file: path.clone(),
                        start_line: 1,
                        end_line: 6,
                        body:
                            "fn authenticate() -> Result<(), AuthError> { Err(AuthError::Revoked) }"
                                .to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        is_test: false,
                        attrs: vec![],
                    }],
                    probe_shapes: vec![
                        ProbeShapeFact {
                            start_line: 4,
                            end_line: 4,
                            start_byte: 48,
                            kind: PROBE_SHAPE_ERROR_PATH.to_string(),
                            text: "Err(AuthError::Revoked)".to_string(),
                        },
                        ProbeShapeFact {
                            start_line: 5,
                            end_line: 5,
                            start_byte: 80,
                            kind: "opaque_shape".to_string(),
                            text: "opaque".to_string(),
                        },
                    ],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_repo_file(Path::new("workspace"), &path, &index);

        assert_eq!(probes.len(), 1);
        let probe = &probes[0];
        assert_eq!(probe.id.0, "repo-probe:src_lib.rs:4:error_path");
        assert_eq!(probe.family, ProbeFamily::ErrorPath);
        assert_eq!(probe.delta, DeltaKind::Unknown);
        assert_eq!(
            probe.owner,
            Some(SymbolId("auth::authenticate".to_string()))
        );
        assert_eq!(probe.before, None);
        assert_eq!(probe.after, Some("Err(AuthError::Revoked)".to_string()));
        assert!(
            probe
                .required_oracles
                .iter()
                .any(|oracle| oracle == "exact error variant assertion")
        );
    }

    #[test]
    fn probes_for_repo_file_returns_empty_for_unknown_path() {
        let probes = probes_for_repo_file(
            Path::new("workspace"),
            Path::new("src/missing.rs"),
            &RustIndex::default(),
        );
        assert!(probes.is_empty());
    }
}

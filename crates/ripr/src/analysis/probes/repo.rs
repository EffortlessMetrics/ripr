use super::super::rust_index::{RustIndex, find_owner_function};
use super::expectations::{expected_sinks, required_oracles};
use super::family::family_for_probe_shape;
use super::ids::{normalize_expression, repo_probe_id};
use crate::domain::{DeltaKind, Probe, SourceLocation};
use std::collections::HashMap;
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

        // #3284: harness-role functions never enter the production
        // subject inventory. A cfg(test)-module helper inside a
        // production file still contributes probe shapes to the file
        // facts; an owner carrying the test/evidence role is skipped,
        // mirroring the diff path and the seam inventory.
        let owner_function = find_owner_function(index, path, shape.start_line);
        if owner_function.is_some_and(|function| function.source_role.is_evidence_role()) {
            continue;
        }
        // Include facts keep their fragment path for source locations, while
        // `facts::resolve_repository_local_includes` rebases the symbol
        // identity to the parent compilation unit. Normalize that identity
        // here so repo probes remain stable across host path separators.
        let owner = owner_function.map(|function| {
            let mut owner = function.id.clone();
            owner.0 = owner.0.replace('\\', "/");
            owner
        });
        let norm_expr = normalize_expression(&shape.text);
        // Ordinal 1 here; post-hoc dedup below handles collisions.
        let id = repo_probe_id(path, &family, owner.as_ref(), &norm_expr, 1);

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

    // Post-hoc collision de-dup: if two probes got the same id, append .2, .3, …
    // to the 2nd+ occurrences.
    let mut seen: HashMap<String, u32> = HashMap::new();
    for probe in probes.iter_mut() {
        let count = seen.entry(probe.id.0.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            probe.id.0 = format!("{}.{}", probe.id.0, count);
        }
    }

    probes
}

#[cfg(test)]
mod tests {
    use super::super::super::rust_index::{
        FileFacts, FunctionFact, PROBE_SHAPE_ERROR_PATH, ProbeShapeFact, RustIndex,
    };
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
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
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
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
        assert_eq!(probe.id.0, "repo-probe:src_lib.rs:error_path:3bf8c64c");
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

    #[test]
    fn probes_for_included_file_keep_fragment_location_and_parent_owner() {
        let fragment = PathBuf::from("src/parser_fragment.rs");
        let index = RustIndex {
            files: BTreeMap::from([(
                fragment.clone(),
                FileFacts {
                    path: fragment.clone(),
                    functions: vec![FunctionFact {
                        id: SymbolId(r"src\lib.rs::impl Parser::clamp".to_string()),
                        name: "clamp".to_string(),
                        file: fragment.clone(),
                        start_line: 1,
                        end_line: 4,
                        body: "fn clamp(&self, value: i32) -> i32 { value }".to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 2,
                        end_line: 2,
                        start_byte: 36,
                        kind: PROBE_SHAPE_ERROR_PATH.to_string(),
                        text: "value > self.limit".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_repo_file(Path::new("workspace"), &fragment, &index);

        assert_eq!(probes.len(), 1);
        assert_eq!(
            probes[0].location.file,
            PathBuf::from("workspace/src/parser_fragment.rs")
        );
        assert_eq!(probes[0].location.line, 2);
        assert_eq!(
            probes[0].owner,
            Some(SymbolId("src/lib.rs::impl Parser::clamp".to_string()))
        );
    }
}

#[cfg(test)]
mod cfg_test_leak_tests {
    use super::probes_for_repo_file;
    use std::path::Path;

    #[test]
    fn cfg_test_module_probe_shapes_seed_no_repo_probes() -> Result<(), String> {
        // #3284: harness-only control flow inside an inline #[cfg(test)]
        // module must never enter the production subject inventory. The
        // repo path iterates probe_shapes without an owner-role check, so
        // a cfg(test) helper's shapes leak as repo findings.
        let root = std::env::temp_dir().join(format!(
            "ripr-repo-leak-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='repo-leak'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn price(amount: i32) -> i32 {\n    if amount > 100 { amount - 10 } else { amount }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    fn check_price(result: i32, expected: i32) {\n        if result != expected {\n            panic!(\"price mismatch\");\n        }\n    }\n\n    #[test]\n    fn price_at_boundary() {\n        check_price(price(100), 90);\n    }\n}\n",
        )
        .map_err(|error| error.to_string())?;
        let index =
            crate::analysis::facts::build_index(&root, &[std::path::PathBuf::from("src/lib.rs")])
                .map_err(|error| error.to_string())?;
        let lib = Path::new("src/lib.rs");
        let probes = probes_for_repo_file(&root, lib, &index);
        let leaked: Vec<_> = probes
            .iter()
            .filter(|probe| probe.expression.contains("result != expected"))
            .collect();
        assert!(
            leaked.is_empty(),
            "cfg(test) helper probe shapes must not seed repo probes: {leaked:?}"
        );
        assert!(
            probes
                .iter()
                .any(|probe| probe.expression.contains("amount > 100")),
            "production shapes still seed repo probes"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
}

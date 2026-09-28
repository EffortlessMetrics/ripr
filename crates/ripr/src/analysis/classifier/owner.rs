use crate::analysis::rust_index::{FunctionSummary, RustIndex};
use crate::domain::Probe;

pub(in crate::analysis) fn resolve_owner_function<'index>(
    probe: &Probe,
    index: &'index RustIndex,
) -> Option<&'index FunctionSummary> {
    let owner = probe.owner.as_ref()?;
    index
        .functions
        .iter()
        .find(|function| same_symbol_id(&function.id.0, &owner.0))
}

/// Symbol IDs begin with a source path. Include rebasing can produce the
/// probe identity through a normalized path while the indexed function was
/// parsed with the host separator; compare the identity canonically without
/// changing source-location paths or any path-scoping rules.
///
/// Compare borrowed bytes instead of allocating two normalized strings for
/// every candidate in the owner scan. Replacing ASCII separators is length
/// preserving, including in UTF-8 IDs. The length check prevents a zipped
/// prefix from comparing equal to a longer identity. No cache is required.
fn same_symbol_id(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left.bytes().zip(right.bytes()).all(|(left, right)| {
            left == right || matches!((left, right), (b'\\', b'/') | (b'/', b'\\'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::rust_index::FunctionFact;
    use crate::domain::{DeltaKind, ProbeFamily, ProbeId, SourceLocation, SymbolId};
    use std::path::PathBuf;

    fn include_owner_fixture() -> (RustIndex, Probe) {
        let owner = FunctionFact {
            id: SymbolId(r"src\lib.rs::impl Parser::clamp".to_string()),
            name: "clamp".to_string(),
            file: PathBuf::from("src/parser_fragment.rs"),
            start_line: 1,
            end_line: 4,
            body: "fn clamp() -> i32 { 1 }".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:include-owner".to_string()),
            location: SourceLocation::new("workspace/src/parser_fragment.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::impl Parser::clamp".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Unknown,
            before: None,
            after: Some("value > self.limit".to_string()),
            expression: "value > self.limit".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };

        (index, probe)
    }

    #[test]
    fn normalized_include_owner_resolves_native_index_identity() {
        let (index, probe) = include_owner_fixture();
        assert_eq!(
            resolve_owner_function(&probe, &index).map(|function| function.name.as_str()),
            Some("clamp")
        );
    }

    #[test]
    fn symbol_identity_matches_allocating_reference_exhaustively() {
        // 341 strings, 116,281 ordered pairs. This oracle intentionally keeps
        // the previous allocation-based implementation, independently of the
        // new comparator. Include multi-byte UTF-8 and both separators.
        let mut values = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..4 {
            let mut next = Vec::new();
            for prefix in &frontier {
                for suffix in ["a", "/", "\\", "é"] {
                    next.push(format!("{prefix}{suffix}"));
                }
            }
            values.extend(next.iter().cloned());
            frontier = next;
        }
        assert_eq!(values.len(), 341);
        for left in &values {
            for right in &values {
                assert_eq!(
                    same_symbol_id(left, right),
                    left.replace('\\', "/") == right.replace('\\', "/"),
                    "identity comparison drift: {left:?} vs {right:?}"
                );
            }
        }
    }

    #[test]
    fn symbol_identity_preserves_full_paths_suffixes_and_unicode() {
        for (left, right, expected) in [
            (r"src\lib.rs::clamp", "src/lib.rs::clamp", true),
            (r"src\café.rs::値", "src/café.rs::値", true),
            ("src/lib.rs::clamp", "src/lib.rs::clamp_more", false),
            (
                "crates/a/src/lib.rs::clamp",
                "crates/b/src/lib.rs::clamp",
                false,
            ),
            (
                "src/lib.rs::first::clamp",
                "src/lib.rs::other::clamp",
                false,
            ),
            ("src/lib.rs::Clamp", "src/lib.rs::clamp", false),
            ("src//lib.rs::clamp", "src/lib.rs::clamp", false),
            ("src/./lib.rs::clamp", "src/lib.rs::clamp", false),
            ("src/café.rs::値", "src/cafe\u{301}.rs::値", false),
            ("", "", true),
            ("", "/", false),
        ] {
            assert_eq!(
                same_symbol_id(left, right),
                expected,
                "{left:?} vs {right:?}"
            );
            assert_eq!(same_symbol_id(right, left), expected, "reverse comparison");
        }
    }

    #[test]
    fn normalized_lookup_preserves_first_match_before_later_exact_match() {
        let (mut index, probe) = include_owner_fixture();
        let mut later = index.functions.clone();
        for function in &mut later {
            function.id.0 = "src/lib.rs::impl Parser::clamp".to_string();
            function.name = "later_exact_match".to_string();
        }
        index.functions.extend(later);
        assert_eq!(index.functions.len(), 2);
        // An exact-spelling lookup before a normalized lookup would choose
        // the wrong row. Keep the original first canonical-match contract.
        assert_eq!(
            resolve_owner_function(&probe, &index).map(|function| function.name.as_str()),
            Some("clamp")
        );
    }

    #[test]
    fn absent_and_missing_owner_remain_unresolved() {
        let (index, mut probe) = include_owner_fixture();
        probe.owner = None;
        assert!(resolve_owner_function(&probe, &index).is_none());
        probe.owner = Some(SymbolId("src/lib.rs::impl Parser::missing".to_string()));
        assert!(resolve_owner_function(&probe, &index).is_none());
        assert!(resolve_owner_function(&probe, &RustIndex::default()).is_none());
    }

    #[test]
    fn large_uncached_index_keeps_exact_owner_among_same_named_functions() {
        let (mut index, probe) = include_owner_fixture();
        let mut unrelated = Vec::new();
        for ordinal in 0..4096 {
            for mut function in index.functions.iter().cloned() {
                function.id.0 =
                    format!("crates/unrelated_{ordinal}/src/lib.rs::impl Parser::clamp");
                unrelated.push(function);
            }
        }
        unrelated.append(&mut index.functions);
        index.functions = unrelated;
        assert_eq!(index.functions.len(), 4097);
        // A fresh in-memory index has no persistent cache or base proof.
        // The source-location file is the include fragment, not the owner
        // compilation unit, so location-based narrowing is also invalid.
        assert_eq!(
            resolve_owner_function(&probe, &index).map(|function| function.id.0.as_str()),
            Some(r"src\lib.rs::impl Parser::clamp")
        );
    }
}

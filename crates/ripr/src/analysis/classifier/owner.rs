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
        .find(|function| normalize_symbol_id(&function.id.0) == normalize_symbol_id(&owner.0))
}

/// Symbol IDs begin with a source path. Include rebasing can produce the
/// probe identity through a normalized path while the indexed function was
/// parsed with the host separator; compare the identity canonically without
/// changing source-location paths or any path-scoping rules.
fn normalize_symbol_id(id: &str) -> String {
    id.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::rust_index::FunctionFact;
    use crate::domain::{DeltaKind, ProbeFamily, ProbeId, SourceLocation, SymbolId};
    use std::path::PathBuf;

    #[test]
    fn normalized_include_owner_resolves_native_index_identity() {
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

        assert_eq!(
            resolve_owner_function(&probe, &index).map(|function| function.name.as_str()),
            Some("clamp")
        );
    }
}

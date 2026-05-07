use crate::domain::{OracleKind, OracleStrength, SymbolId};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RustIndex {
    pub files: BTreeMap<PathBuf, FileFacts>,
    pub tests: Vec<TestFact>,
    pub functions: Vec<FunctionFact>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileFacts {
    pub path: PathBuf,
    pub functions: Vec<FunctionFact>,
    pub tests: Vec<TestFact>,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub probe_shapes: Vec<ProbeShapeFact>,
    /// Original file source text. Held so `analysis/value-extraction-v2`
    /// can scan for top-level `const`/`static` declarations without
    /// re-reading the file at evidence-build time. Not part of any
    /// cached envelope (the cache stores `ClassifiedSeam` only).
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionFact {
    pub id: SymbolId,
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub is_test: bool,
    /// Attribute syntax lines (e.g., `#[rstest]`, `#[case(100, 100)]`,
    /// `#[test]`) captured from the AST `attrs()` iterator. Used by
    /// `analysis/value-extraction-v2` to read rstest case parameters
    /// without re-reading the file. The lexical fallback path
    /// populates this as empty.
    pub attrs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TestFact {
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub calls: Vec<CallFact>,
    pub assertions: Vec<OracleFact>,
    pub literals: Vec<LiteralFact>,
    /// Attribute syntax lines on the test fn. Mirrors
    /// `FunctionFact.attrs`. Carries `#[rstest]` and `#[case(...)]` for
    /// case-driven tests so value resolution can map case literals to
    /// test parameters.
    pub attrs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OracleFact {
    pub line: usize,
    pub text: String,
    pub kind: OracleKind,
    pub strength: OracleStrength,
    pub observed_tokens: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallFact {
    pub line: usize,
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReturnFact {
    pub line: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiteralFact {
    pub line: usize,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProbeShapeFact {
    pub start_line: usize,
    pub end_line: usize,
    /// Byte offset of the shape's start within the source file. Populated
    /// by the parser-backed summarizer; the lexical fallback emits no
    /// probe shapes at all, so this stays accurate.
    pub start_byte: usize,
    pub kind: String,
    pub text: String,
}

pub type FunctionSummary = FunctionFact;
pub type TestSummary = TestFact;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_index_default_has_empty_fact_sets() {
        let index = RustIndex::default();
        assert!(index.files.is_empty());
        assert!(index.tests.is_empty());
        assert!(index.functions.is_empty());
    }

    #[test]
    fn file_facts_default_has_empty_collections() {
        let facts = FileFacts::default();
        assert!(facts.path.as_os_str().is_empty());
        assert!(facts.functions.is_empty());
        assert!(facts.tests.is_empty());
        assert!(facts.calls.is_empty());
        assert!(facts.returns.is_empty());
        assert!(facts.literals.is_empty());
        assert!(facts.probe_shapes.is_empty());
    }

    #[test]
    fn fact_types_clone_and_compare_equal_for_simple_samples() {
        let call = CallFact {
            line: 1,
            name: "test_fn".to_string(),
            text: "test_fn()".to_string(),
        };
        let call_cloned = call.clone();
        assert_eq!(call, call_cloned);

        let ret = ReturnFact {
            line: 2,
            text: "return Ok(())".to_string(),
        };
        let ret_cloned = ret.clone();
        assert_eq!(ret, ret_cloned);

        let lit = LiteralFact {
            line: 3,
            value: "42".to_string(),
        };
        let lit_cloned = lit.clone();
        assert_eq!(lit, lit_cloned);
    }

    #[test]
    fn probe_shape_fact_preserves_span_kind_text_and_start_byte() {
        let shape = ProbeShapeFact {
            start_line: 10,
            end_line: 12,
            start_byte: 256,
            kind: "predicate".to_string(),
            text: "x > 0".to_string(),
        };
        assert_eq!(shape.start_line, 10);
        assert_eq!(shape.end_line, 12);
        assert_eq!(shape.start_byte, 256);
        assert_eq!(shape.kind, "predicate");
        assert_eq!(shape.text, "x > 0");
    }
}

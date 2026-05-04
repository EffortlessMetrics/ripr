use crate::domain::SymbolId;
use std::path::Path;

use super::super::facts::FileFacts;

pub trait RustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String>;

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact>;
}

#[derive(Clone, Debug, Default)]
pub struct LexicalRustSyntaxAdapter;

#[derive(Clone, Debug, Default)]
pub struct RaRustSyntaxAdapter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRange {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxNodeFact {
    pub file: std::path::PathBuf,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub owner: Option<SymbolId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn text_range_preserves_coordinates() {
        let range = TextRange {
            start_line: 5,
            start_column: 10,
            end_line: 8,
            end_column: 20,
        };
        assert_eq!(range.start_line, 5);
        assert_eq!(range.start_column, 10);
        assert_eq!(range.end_line, 8);
        assert_eq!(range.end_column, 20);
    }

    #[test]
    fn syntax_node_fact_preserves_file_kind_span_text_and_owner() {
        let owner = SymbolId("test::function".to_string());
        let node = SyntaxNodeFact {
            file: PathBuf::from("src/lib.rs"),
            kind: "function".to_string(),
            start_line: 10,
            end_line: 20,
            text: "fn test() {}".to_string(),
            owner: Some(owner.clone()),
        };
        assert_eq!(node.file, PathBuf::from("src/lib.rs"));
        assert_eq!(node.kind, "function");
        assert_eq!(node.start_line, 10);
        assert_eq!(node.end_line, 20);
        assert_eq!(node.text, "fn test() {}");
        assert_eq!(node.owner, Some(owner));
    }

    #[test]
    fn syntax_node_fact_equality_distinguishes_owner() {
        let node1 = SyntaxNodeFact {
            file: PathBuf::from("src/lib.rs"),
            kind: "function".to_string(),
            start_line: 10,
            end_line: 20,
            text: "fn test() {}".to_string(),
            owner: Some(SymbolId("a".to_string())),
        };
        let node2 = SyntaxNodeFact {
            file: PathBuf::from("src/lib.rs"),
            kind: "function".to_string(),
            start_line: 10,
            end_line: 20,
            text: "fn test() {}".to_string(),
            owner: Some(SymbolId("b".to_string())),
        };
        assert_ne!(node1, node2);
    }
}

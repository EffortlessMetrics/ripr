use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn new(file: impl Into<PathBuf>, line: usize, column: usize) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProbeId(pub String);

impl fmt::Display for ProbeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub String);

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{ProbeId, SourceLocation, SymbolId};
    use std::path::PathBuf;

    #[test]
    fn source_location_new_sets_file_line_and_column() {
        let location = SourceLocation::new("src/lib.rs", 42, 7);

        assert_eq!(location.file, PathBuf::from("src/lib.rs"));
        assert_eq!(location.line, 42);
        assert_eq!(location.column, 7);
    }

    #[test]
    fn ids_display_underlying_stable_identifier() {
        let probe = ProbeId("probe:src_lib_rs:42:error_path".to_string());
        let symbol = SymbolId("symbol:crate::pricing::calculate".to_string());

        assert_eq!(probe.to_string(), "probe:src_lib_rs:42:error_path");
        assert_eq!(symbol.to_string(), "symbol:crate::pricing::calculate");
    }
}

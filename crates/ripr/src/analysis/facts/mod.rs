mod build;
mod model;

pub use build::build_index;
pub(crate) use build::build_index_from_loaded_files_with_cache;
pub use model::{
    CallFact, FileFacts, FunctionFact, FunctionSummary, LiteralFact, OracleFact, ProbeShapeFact,
    ReturnFact, RustIndex, TestFact, TestSummary,
};

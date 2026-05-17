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
        .find(|function| &function.id == owner)
}

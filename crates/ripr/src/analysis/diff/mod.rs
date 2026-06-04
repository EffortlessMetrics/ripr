mod load;
mod model;
mod parse;
mod path;

pub use load::{load_diff, load_diff_range};
#[allow(
    unused_imports,
    reason = "ChangedLine is re-exported for use by probes.rs and other external modules; not used within diff module itself."
)]
pub use model::{ChangedFile, ChangedLine};
pub use parse::parse_unified_diff;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub changed_rust_files: usize,
    pub probes: usize,
    pub findings: usize,
    pub exposed: usize,
    pub weakly_exposed: usize,
    pub reachable_unrevealed: usize,
    pub no_static_path: usize,
    pub infection_unknown: usize,
    pub propagation_unknown: usize,
    pub static_unknown: usize,
}

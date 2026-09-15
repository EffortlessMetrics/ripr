pub mod fragment_checks;

pub struct Span {
    pub file: String,
    pub line: u32,
}

pub struct Fragment {
    pub crate_name: String,
    pub crate_root: Option<String>,
    pub spans: Vec<Span>,
}

pub fn expected_root(crate_name: &str) -> String {
    format!("crates/{crate_name}/src/lib.rs")
}

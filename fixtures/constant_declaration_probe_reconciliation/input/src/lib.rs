pub(crate) const OBSERVATION_SCHEMA_GENERATION: u32 = 3;

pub(crate) const LIMIT: usize = compute_limit(64);

fn compute_limit(base: usize) -> usize {
    base.max(1)
}

pub static ACTIVE_NAME: &str = "observation";

pub fn dispatch_event(kind: &str) {
    let rendered = render_label(kind);
    log_line(&rendered);
}

pub struct CoverageRow {
    pub total: usize,
    pub skipped: usize,
}

fn render_label(kind: &str) -> String {
    format!("event:{kind}")
}

fn log_line(line: &str) {
    println!("{line}");
}

#[cfg(test)]
mod observations {
    use super::OBSERVATION_SCHEMA_GENERATION;

    #[test]
    fn observation_schema_generation_matches_policy() {
        assert_eq!(OBSERVATION_SCHEMA_GENERATION, 3);
    }
}

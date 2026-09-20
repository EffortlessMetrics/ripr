#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttemptStatus {
    Ready,
    Tampered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptVerdict {
    pub status: AttemptStatus,
    pub violations: Vec<String>,
    pub display_paths: Vec<String>,
}

pub fn receipt_binding_matches(
    current: &AttemptVerdict,
    recorded: &AttemptVerdict,
) -> bool {
    current.status == recorded.status && current.violations == recorded.violations
}

// Unrelated owner that shares `status`/`verdict` vocabulary but never
// reaches the receipt binding sink.
pub struct ArtifactRecord {
    pub status: String,
    pub verdict_note: String,
}

pub fn artifact_reaches_terminal(record: &ArtifactRecord) -> bool {
    record.status == "terminal"
}

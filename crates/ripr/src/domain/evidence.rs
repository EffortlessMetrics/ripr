#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OracleStrength {
    Strong,
    Medium,
    Weak,
    Smoke,
    None,
    Unknown,
}

impl OracleStrength {
    pub fn as_str(&self) -> &'static str {
        match self {
            OracleStrength::Strong => "strong",
            OracleStrength::Medium => "medium",
            OracleStrength::Weak => "weak",
            OracleStrength::Smoke => "smoke",
            OracleStrength::None => "none",
            OracleStrength::Unknown => "unknown",
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            OracleStrength::Strong => 5,
            OracleStrength::Medium => 4,
            OracleStrength::Weak => 3,
            OracleStrength::Smoke => 2,
            OracleStrength::Unknown => 1,
            OracleStrength::None => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageState {
    Yes,
    Weak,
    No,
    Unknown,
    Opaque,
    NotApplicable,
}

impl StageState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StageState::Yes => "yes",
            StageState::Weak => "weak",
            StageState::No => "no",
            StageState::Unknown => "unknown",
            StageState::Opaque => "opaque",
            StageState::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
            Confidence::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageEvidence {
    pub state: StageState,
    pub confidence: Confidence,
    pub summary: String,
}

impl StageEvidence {
    pub fn new(state: StageState, confidence: Confidence, summary: impl Into<String>) -> Self {
        Self {
            state,
            confidence,
            summary: summary.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealEvidence {
    pub observe: StageEvidence,
    pub discriminate: StageEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiprEvidence {
    pub reach: StageEvidence,
    pub infect: StageEvidence,
    pub propagate: StageEvidence,
    pub reveal: RevealEvidence,
}

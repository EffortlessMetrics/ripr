//! Adapter-neutral identities and observations used by convergence ports.

use std::collections::{BTreeMap, BTreeSet};

/// Stable repository identity; mutable refs and current commits are observations.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RepositoryId(pub String);

/// Adapter-issued handle for an isolated repository that may receive objects.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DisposableRepositoryId(pub String);

/// Stable Git object identity, independent of a checkout path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObjectId(pub String);

/// Stable content digest for receipts and retained artifacts.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContentDigest(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryRole {
    Source,
    Swarm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConvergenceDirection {
    SwarmToSource,
    SourceToSwarm,
}

/// Closed evidence state. Absence and unavailable instrumentation never pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceState {
    Passed,
    Failed,
    Rejected,
    NotProven,
    InstrumentFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPair {
    pub source: RepositoryId,
    pub swarm: RepositoryId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitObservation {
    pub commit: ObjectId,
    pub tree: ObjectId,
    pub parents: Vec<ObjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefObservation {
    pub repository: RepositoryId,
    pub name: String,
    pub target: ObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestObservation {
    pub number: u64,
    pub base: ObjectId,
    pub head: ObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckObservation {
    pub name: String,
    pub subject: ObjectId,
    pub state: EvidenceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewObservation {
    pub subject: ObjectId,
    pub unresolved_threads: u64,
    pub state: EvidenceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactObservation {
    pub stable_id: String,
    pub digest: ContentDigest,
    pub state: EvidenceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectionObservation {
    pub required_checks: BTreeSet<String>,
    pub allowed_merge_methods: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionRequest {
    pub program: String,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub timeout_millis: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionObservation {
    pub state: EvidenceState,
    pub exit_code: Option<i32>,
    pub stdout_digest: ContentDigest,
    pub stderr_digest: ContentDigest,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalTime(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseObservation {
    pub owner: String,
    pub heartbeat: LogicalTime,
    pub expires_at: LogicalTime,
}

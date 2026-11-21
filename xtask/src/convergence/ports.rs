//! Capability-bounded ports consumed by convergence domain/application code.

use super::types::{
    ArtifactObservation, CheckObservation, CommitObservation, ContentDigest,
    DisposableRepositoryId, ExecutionObservation, ExecutionRequest, LeaseObservation, LogicalTime,
    ObjectId, ProtectionObservation, PullRequestObservation, RefObservation, RepositoryId,
    RepositoryPair, ReviewObservation,
};

pub trait GitObservation {
    type Error;

    fn commit(
        &self,
        repository: &RepositoryId,
        id: &ObjectId,
    ) -> Result<CommitObservation, Self::Error>;
    fn is_ancestor(
        &self,
        repository: &RepositoryId,
        ancestor: &ObjectId,
        descendant: &ObjectId,
    ) -> Result<bool, Self::Error>;
}

/// Construction is deliberately separate from read-only graph observation.
pub trait DisposableGitObjectConstruction {
    type Error;

    fn write_tree(
        &mut self,
        repository: &DisposableRepositoryId,
        entries: &[(String, ObjectId)],
    ) -> Result<ObjectId, Self::Error>;
    fn write_commit(
        &mut self,
        repository: &DisposableRepositoryId,
        tree: &ObjectId,
        parents: &[ObjectId],
    ) -> Result<ObjectId, Self::Error>;
}

pub trait GitHubObservation {
    type Error;

    fn reference(
        &self,
        repository: &RepositoryId,
        name: &str,
    ) -> Result<RefObservation, Self::Error>;
    fn pull_request(
        &self,
        repository: &RepositoryId,
        number: u64,
    ) -> Result<PullRequestObservation, Self::Error>;
    fn checks(
        &self,
        repository: &RepositoryId,
        subject: &ObjectId,
    ) -> Result<Vec<CheckObservation>, Self::Error>;
    fn reviews(
        &self,
        repository: &RepositoryId,
        number: u64,
    ) -> Result<ReviewObservation, Self::Error>;
    fn artifacts(
        &self,
        repository: &RepositoryId,
        subject: &ObjectId,
    ) -> Result<Vec<ArtifactObservation>, Self::Error>;
    fn protection(
        &self,
        repository: &RepositoryId,
        branch: &str,
    ) -> Result<ProtectionObservation, Self::Error>;
}

/// Candidate transport cannot administer settings, publish releases, or use registries.
pub trait CandidateTransport {
    type Error;

    fn create_candidate_ref(
        &mut self,
        repository: &RepositoryId,
        name: &str,
        expected_old: Option<&ObjectId>,
        target: &ObjectId,
    ) -> Result<RefObservation, Self::Error>;
    fn open_candidate_pull_request(
        &mut self,
        repository: &RepositoryId,
        base: &str,
        head: &str,
        title: &str,
        body: &str,
    ) -> Result<PullRequestObservation, Self::Error>;
    fn merge_expected_head(
        &mut self,
        repository: &RepositoryId,
        number: u64,
        expected_head: &ObjectId,
    ) -> Result<ObjectId, Self::Error>;
}

pub trait ReceiptStore {
    type Error;

    fn load(&self, digest: &ContentDigest) -> Result<Vec<u8>, Self::Error>;
    fn store(&mut self, canonical_bytes: &[u8]) -> Result<ContentDigest, Self::Error>;
}

pub trait IsolatedExecutor {
    type Error;

    fn execute(&mut self, request: &ExecutionRequest) -> Result<ExecutionObservation, Self::Error>;
}

pub trait ClockObservation {
    fn now(&self) -> LogicalTime;
}

pub trait LeaseObservationPort {
    type Error;

    fn lease(&self, key: &str) -> Result<Option<LeaseObservation>, Self::Error>;
}

pub trait RepositoryPairLoader {
    type Error;

    fn load_pair(&self, profile: &str) -> Result<RepositoryPair, Self::Error>;
}

pub trait SemanticRegistryLoader {
    type Registry;
    type Error;

    fn load_registry(&self, pair: &RepositoryPair) -> Result<Self::Registry, Self::Error>;
}

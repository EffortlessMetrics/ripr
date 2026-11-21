use ripr::{RiprProviderContractErrorCodeV1, RiprRepositorySnapshotV1, RiprSourceViewV1};

const UPPERCASE_TREE_ID: &str = "git-tree:0123456789ABCDEF0123456789ABCDEF01234567";
const UPPERCASE_TREE_DIGEST: &str =
    "sha256:59299de1e10c7ffc01ed0d7d72f6db2b2d3e12c405d2f95697fe13fbe8148317";

#[test]
fn git_tree_snapshot_rejects_noncanonical_uppercase_object_id() {
    let snapshot = RiprRepositorySnapshotV1 {
        repository_id: "EffortlessMetrics/ripr-swarm".into(),
        snapshot_id: UPPERCASE_TREE_ID.into(),
        source_view: RiprSourceViewV1::GitTree,
        source_digest: UPPERCASE_TREE_DIGEST.into(),
    };

    assert_eq!(
        snapshot.validate().err().map(|error| error.code),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );
}

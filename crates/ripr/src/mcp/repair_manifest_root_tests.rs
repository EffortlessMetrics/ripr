//! #5744: genuine publication must reopen only in its selected Unix root.

use super::*;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

struct ManifestRoots {
    root: FixtureRoot,
    parent: FixtureRoot,
    decoy: PathBuf,
    id: RepairAttemptId,
}

fn manifest_roots(label: &str) -> Result<ManifestRoots, String> {
    let parent = fixture_root(label)?;
    let literal = parent.join("team\\repo 'quoted'");
    std::fs::create_dir(&literal).map_err(|error| error.to_string())?;
    let (root, id) = prepare_at(FixtureRoot(literal))?;
    let decoy = parent.join("team").join("repo 'quoted'");
    std::fs::create_dir_all(&decoy).map_err(|error| error.to_string())?;
    git(&decoy, &["init"])?;
    write(&decoy.join("identity"), "different repository")?;
    Ok(ManifestRoots {
        root,
        parent,
        decoy,
        id,
    })
}

fn manifest_path(root: &Path, id: &RepairAttemptId) -> PathBuf {
    root.join("target/ripr/repair-attempts")
        .join(id.as_str())
        .join("attempt.json")
}

fn assert_distinct_roots(fixture: &ManifestRoots) -> Result<(), String> {
    assert_eq!(fixture.root.parent(), Some(&*fixture.parent));
    let selected = std::fs::metadata(&*fixture.root).map_err(|error| error.to_string())?;
    let decoy = std::fs::metadata(&fixture.decoy).map_err(|error| error.to_string())?;
    assert_ne!((selected.dev(), selected.ino()), (decoy.dev(), decoy.ino()));
    Ok(())
}

fn copy_attempt(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir(destination).map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_attempt(&entry.path(), &target)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), target).map_err(|error| error.to_string())?;
        } else {
            return Err("published fixture contained a non-file/non-directory entry".to_string());
        }
    }
    Ok(())
}

#[test]
fn durable_literal_unix_manifest_reopens_in_selected_root() -> Result<(), String> {
    let fixture = manifest_roots("manifest-root-positive")?;
    assert_distinct_roots(&fixture)?;
    let path = manifest_path(&fixture.root, &fixture.id);
    let before = std::fs::read(&path).map_err(|error| error.to_string())?;
    let manifest = load_repair_attempt_manifest(&fixture.root, &fixture.id)?;
    let canonical = fixture
        .root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    assert_eq!(manifest.root.as_bytes(), canonical.as_os_str().as_bytes());
    assert_eq!(manifest.state, RepairAttemptState::AwaitingEdit);
    assert_eq!(manifest.repair_attempt_id, fixture.id);
    assert_eq!(
        std::fs::read(&path).map_err(|error| error.to_string())?,
        before
    );
    Ok(())
}

#[test]
fn durable_literal_unix_manifest_copy_refuses_slash_decoy() -> Result<(), String> {
    let fixture = manifest_roots("manifest-root-decoy-control")?;
    assert_distinct_roots(&fixture)?;
    let original = manifest_path(&fixture.root, &fixture.id);
    let original_bytes = std::fs::read(&original).map_err(|error| error.to_string())?;
    let selected = load_repair_attempt_manifest(&fixture.root, &fixture.id)?;
    assert_eq!(selected.state, RepairAttemptState::AwaitingEdit);
    let decoy_store = fixture.decoy.join("target/ripr/repair-attempts");
    std::fs::create_dir_all(&decoy_store).map_err(|error| error.to_string())?;
    // Copy authentic retained bytes; never alter the serialized root or commitment.
    copy_attempt(
        original
            .parent()
            .ok_or_else(|| "missing attempt directory".to_string())?,
        &decoy_store.join(fixture.id.as_str()),
    )?;
    let copied = manifest_path(&fixture.decoy, &fixture.id);
    assert_eq!(
        std::fs::read(&copied).map_err(|error| error.to_string())?,
        original_bytes
    );
    let refusal = load_repair_attempt_manifest(&fixture.decoy, &fixture.id)
        .err()
        .ok_or_else(|| "authentic attempt copy was admitted in a different root".to_string())?;
    assert_eq!(
        refusal,
        "repair attempt manifest root does not match selected repository"
    );
    assert_eq!(
        std::fs::read(&original).map_err(|error| error.to_string())?,
        original_bytes
    );
    assert_eq!(
        std::fs::read(fixture.decoy.join("identity")).map_err(|error| error.to_string())?,
        b"different repository"
    );
    Ok(())
}

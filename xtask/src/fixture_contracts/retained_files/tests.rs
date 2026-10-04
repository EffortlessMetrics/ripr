use super::*;
use std::path::PathBuf;

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new() -> Self {
        Self(crate::tests::temp_dir("retained-file-containment"))
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        if let Ok(()) = fs::remove_dir_all(&self.0) {}
    }
}

fn descriptor(path: &str, bytes: &[u8]) -> Value {
    serde_json::json!({"path": path, "bytes": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes))})
}

#[test]
fn retained_file_verifies_ordinary_and_relocated_roots() -> Result<(), String> {
    let fixture = FixtureRoot::new();
    let root = fixture.0.join("original");
    fs::create_dir(&root).map_err(|error| error.to_string())?;
    let bytes = b"retained bytes";
    fs::write(root.join("asset.bin"), bytes).map_err(|error| error.to_string())?;
    let entry = descriptor("asset.bin", bytes);
    verify_file(&root, &entry)?;
    let relocated = fixture.0.join("relocated");
    fs::rename(&root, &relocated).map_err(|error| error.to_string())?;
    verify_file(&relocated, &entry)
}

#[cfg(unix)]
#[test]
fn retained_file_accepts_contained_links_and_root_alias() -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let fixture = FixtureRoot::new();
    let root = fixture.0.join("case");
    let data = root.join("data");
    fs::create_dir_all(&data).map_err(|error| error.to_string())?;
    let bytes = b"retained bytes";
    fs::write(data.join("asset.bin"), bytes).map_err(|error| error.to_string())?;
    symlink("data/asset.bin", root.join("file-link.bin")).map_err(|error| error.to_string())?;
    symlink("data", root.join("parent-link")).map_err(|error| error.to_string())?;
    let alias = fixture.0.join("root-alias");
    symlink(&root, &alias).map_err(|error| error.to_string())?;
    for selected_root in [&root, &alias] {
        for path in ["data/asset.bin", "file-link.bin", "parent-link/asset.bin"] {
            verify_file(selected_root, &descriptor(path, bytes))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn retained_file_rejects_outside_file_and_parent_links() -> Result<(), String> {
    use std::os::unix::fs::symlink;

    for mode in ["file", "parent"] {
        let fixture = FixtureRoot::new();
        let root = fixture.0.join("case");
        let data = root.join("data");
        let outside = fixture.0.join("case-neighbor");
        fs::create_dir_all(&data).map_err(|error| error.to_string())?;
        fs::create_dir(&outside).map_err(|error| error.to_string())?;
        let bytes = b"matching bytes must not grant local custody";
        fs::write(data.join("asset.bin"), bytes).map_err(|error| error.to_string())?;
        fs::write(outside.join("asset.bin"), bytes).map_err(|error| error.to_string())?;
        let entry = descriptor("data/asset.bin", bytes);
        verify_file(&root, &entry)?;
        if mode == "file" {
            fs::remove_file(data.join("asset.bin")).map_err(|error| error.to_string())?;
            symlink(outside.join("asset.bin"), data.join("asset.bin"))
                .map_err(|error| error.to_string())?;
        } else {
            fs::remove_dir_all(&data).map_err(|error| error.to_string())?;
            symlink(&outside, &data).map_err(|error| error.to_string())?;
        }
        let error = verify_file(&root, &entry)
            .err()
            .ok_or_else(|| format!("accepted outside {mode} link with matching bytes"))?;
        assert!(error.contains("escapes fixture root"), "{mode}: {error}");
    }
    Ok(())
}

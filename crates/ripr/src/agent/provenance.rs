use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("read artifact {} failed: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut rendered = String::from("sha256:");
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_provenance_test_dir(name: &str) -> Result<std::path::PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock before epoch: {err}"))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        Ok(dir)
    }

    #[test]
    fn sha256_file_hashes_artifact_bytes() -> Result<(), String> {
        let dir = unique_provenance_test_dir("sha256")?;
        let path = dir.join("artifact.json");
        std::fs::write(&path, "abc\n").map_err(|err| format!("write artifact: {err}"))?;

        assert_eq!(
            sha256_file(&path)?,
            "sha256:edeaaff3f1774ad2888673770c6d64097e391bc362d7d6fb34982ddf0efd18cb"
        );

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir: {err}"))?;
        Ok(())
    }
}

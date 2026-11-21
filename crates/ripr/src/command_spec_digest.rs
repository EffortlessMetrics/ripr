use crate::domain::{CommandSpec, CommandSpecDigest};
use sha2::{Digest, Sha256};

impl CommandSpecDigest for CommandSpec {
    fn command_spec_sha256(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        let digest = Sha256::digest(bytes);
        Ok(format!("sha256:{digest:x}"))
    }
}

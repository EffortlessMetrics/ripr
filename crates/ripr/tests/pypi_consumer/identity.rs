//! Wheel and payload identity oracles for the #4626 consumer journey.
//!
//! `--version` success is never sufficient. The installed executable must be
//! the recorded wheel payload, named `ripr-rs` (not the unrelated `ripr`
//! distribution), and pip/uv consumers must agree on the digest.

use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) const DISTRIBUTION: &str = "ripr-rs";
pub(crate) const EXECUTABLE: &str = "ripr";
pub(crate) const WHEEL_NORMALIZED_PREFIX: &str = "ripr_rs-";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WheelIdentity {
    pub(crate) filename: String,
    pub(crate) wheel_sha256: String,
    pub(crate) payload_sha256: String,
    pub(crate) version: String,
    pub(crate) tag: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstalledPayload {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.is_empty() {
        return Err(format!(
            "{} is empty; a wheel payload cannot be empty",
            path.display()
        ));
    }
    Ok(sha256_bytes(&bytes))
}

pub(crate) fn parse_wheel_filename(
    filename: &str,
    expected_version: &str,
) -> Result<String, String> {
    if filename.contains("ripr-") && !filename.starts_with(WHEEL_NORMALIZED_PREFIX) {
        return Err(format!(
            "wheel filename `{filename}` selects the unrelated `ripr` distribution; expected `{WHEEL_NORMALIZED_PREFIX}{expected_version}-*.whl`"
        ));
    }
    let prefix = format!("{WHEEL_NORMALIZED_PREFIX}{expected_version}-");
    let Some(rest) = filename.strip_prefix(&prefix) else {
        return Err(format!(
            "wheel filename `{filename}` does not bind version `{expected_version}` under distribution `{DISTRIBUTION}`"
        ));
    };
    let Some(tag) = rest.strip_suffix(".whl") else {
        return Err(format!(
            "wheel filename `{filename}` is not a `.whl` archive"
        ));
    };
    if tag.is_empty() || tag.contains('/') || (tag.contains("any") && tag.ends_with("-any")) {
        return Err(format!(
            "wheel tag `{tag}` is not a platform-specific native tag; py3-none-any cannot carry the ripr executable"
        ));
    }
    Ok(tag.to_string())
}

pub(crate) fn require_matching_payload(
    expected: &WheelIdentity,
    installed: &InstalledPayload,
) -> Result<(), String> {
    if installed.sha256 != expected.payload_sha256 {
        return Err(format!(
            "installed payload at {} digest {} does not match wheel payload {}; a substituted PATH binary cannot hide this",
            installed.path, installed.sha256, expected.payload_sha256
        ));
    }
    Ok(())
}

pub(crate) fn require_matching_consumer_payloads(
    pip: &InstalledPayload,
    uv: &InstalledPayload,
) -> Result<(), String> {
    if pip.sha256 != uv.sha256 {
        return Err(format!(
            "pip payload {} and uv payload {} disagree; the same wheel must install the same executable",
            pip.sha256, uv.sha256
        ));
    }
    Ok(())
}

pub(crate) fn version_only_evidence_is_insufficient(
    has_version: bool,
    has_subjects: bool,
) -> Result<(), String> {
    if has_version && !has_subjects {
        return Err(
            "`--version` alone is insufficient; the journey must assert nonzero subjects or a precise typed limitation"
                .to_string(),
        );
    }
    if !has_version {
        return Err("installed command/version was not recorded".to_string());
    }
    Ok(())
}

pub(crate) fn require_error<T>(result: Result<T, String>, what: &str) -> Result<String, String> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => Err(format!("{what} unexpectedly succeeded")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_wheel_filename_binds_ripr_rs_and_version() -> Result<(), String> {
        let tag = parse_wheel_filename("ripr_rs-0.11.0-py3-none-linux_x86_64.whl", "0.11.0")?;
        if tag != "py3-none-linux_x86_64" {
            return Err(format!("unexpected wheel tag `{tag}`"));
        }
        Ok(())
    }

    #[test]
    fn unrelated_ripr_distribution_filename_is_rejected() -> Result<(), String> {
        let error = require_error(
            parse_wheel_filename("ripr-0.11.0-py3-none-linux_x86_64.whl", "0.11.0"),
            "unrelated PyPI project must not pass",
        )?;
        if !error.contains("unrelated `ripr` distribution") {
            return Err(format!("unexpected error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn version_mismatch_and_purelib_any_tag_are_rejected() -> Result<(), String> {
        let version = require_error(
            parse_wheel_filename("ripr_rs-9.9.9-py3-none-linux_x86_64.whl", "0.11.0"),
            "stale version must not pass",
        )?;
        if !version.contains("does not bind version `0.11.0`") {
            return Err(format!("unexpected version error: {version}"));
        }
        let any = require_error(
            parse_wheel_filename("ripr_rs-0.11.0-py3-none-any.whl", "0.11.0"),
            "any tag must not carry a native executable",
        )?;
        if !any.contains("py3-none-any") {
            return Err(format!("unexpected any-tag error: {any}"));
        }
        Ok(())
    }

    #[test]
    fn substituted_payload_digest_is_rejected() -> Result<(), String> {
        let wheel = WheelIdentity {
            filename: "ripr_rs-0.11.0-py3-none-linux_x86_64.whl".to_string(),
            wheel_sha256: "aa".to_string(),
            payload_sha256: "deadbeef".to_string(),
            version: "0.11.0".to_string(),
            tag: "py3-none-linux_x86_64".to_string(),
        };
        let planted = InstalledPayload {
            path: "/tmp/planted/ripr".to_string(),
            sha256: "cafebabe".to_string(),
        };
        let error = require_error(
            require_matching_payload(&wheel, &planted),
            "substituted binary must fail",
        )?;
        if !error.contains("substituted PATH binary") {
            return Err(format!("unexpected digest error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn pip_and_uv_must_install_the_same_payload() -> Result<(), String> {
        let pip = InstalledPayload {
            path: "/pip/bin/ripr".to_string(),
            sha256: "abc".to_string(),
        };
        let uv = InstalledPayload {
            path: "/uv/bin/ripr".to_string(),
            sha256: "def".to_string(),
        };
        let error = require_error(
            require_matching_consumer_payloads(&pip, &uv),
            "cross-client digest drift must fail",
        )?;
        if !error.contains("disagree") {
            return Err(format!("unexpected drift error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn version_only_receipt_cannot_close_the_journey() -> Result<(), String> {
        let error = require_error(
            version_only_evidence_is_insufficient(true, false),
            "version-only must be insufficient",
        )?;
        if !error.contains("`--version` alone is insufficient") {
            return Err(format!("unexpected version-only error: {error}"));
        }
        version_only_evidence_is_insufficient(true, true)?;
        Ok(())
    }

    #[test]
    fn journey_distribution_matches_policy_contract() -> Result<(), String> {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../policy/distribution.toml");
        if !path.is_file() {
            return Err(format!(
                "workspace distribution contract missing at {}",
                path.display()
            ));
        }
        let contract = std::fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if !contract.contains("distribution = \"ripr-rs\"") {
            return Err(
                "consumer journey distribution must stay aligned with policy/distribution.toml"
                    .to_string(),
            );
        }
        if !contract.contains("executable = \"ripr\"") {
            return Err("installed executable must stay product name ripr".to_string());
        }
        Ok(())
    }
}

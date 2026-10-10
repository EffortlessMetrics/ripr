//! Native observations and retained startup directories for complete execution.
//!
//! This module does not grant a line-guard exception or publish evidence.
//! Serialized stage/profile DATA is insufficient: startup repeats the actual
//! bounded native-limit probe and independently opens and rechecks every role.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::complete_contract::CompleteVerificationLimits;
use crate::analysis::committed_source::staged::{
    DirectoryIdentity, RetainedDirectory, SourceAnchor, SourceBudget,
};

const STARTUP_PATH_BYTES_MAX: u64 = 4096;

/// Closed incoming DATA. The parent maps its nonce field explicitly without
/// normalization; this stage nonce is separate from the generation nonce.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct StageRootBinding {
    pub(super) stage_nonce: String,
    pub(super) stage: DirectoryIdentity,
    pub(super) source: DirectoryIdentity,
    pub(super) spool: DirectoryIdentity,
    pub(super) artifacts: DirectoryIdentity,
}

fn lower_hex(value: &str, size: usize) -> bool {
    value.len() == size
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn verify_held_deadline(duration_ms: u64, deadline: Instant) -> Result<(), String> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or("complete startup held deadline expired")?;
    if duration_ms == 0 || remaining > Duration::from_millis(duration_ms) {
        return Err("complete startup held deadline exceeds the admitted duration".into());
    }
    Ok(())
}

impl StageRootBinding {
    pub(super) fn validate(&self, path_limit: u64) -> Result<(), String> {
        if !lower_hex(&self.stage_nonce, 128) {
            return Err(
                "complete startup stage nonce is not 128 lowercase hexadecimal bytes".into(),
            );
        }
        for identity in [&self.stage, &self.source, &self.spool, &self.artifacts] {
            let path = identity.path.as_str();
            if path.len() as u64 > path_limit
                || !path.starts_with('/')
                || path.as_bytes().contains(&0)
                || path == "/"
                || path[1..]
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
            {
                return Err(
                    "complete startup directory path is not bounded canonical absolute UTF-8"
                        .into(),
                );
            }
        }
        for (name, identity) in [
            ("source", &self.source),
            ("spool", &self.spool),
            ("artifacts", &self.artifacts),
        ] {
            let path = std::path::Path::new(&identity.path);
            if path.parent() != Some(std::path::Path::new(&self.stage.path))
                || path.file_name() != Some(std::ffi::OsStr::new(name))
            {
                return Err("complete startup role is not the exact direct stage child".into());
            }
        }
        let identities = [&self.stage, &self.source, &self.spool, &self.artifacts];
        for (index, left) in identities.iter().enumerate() {
            for right in identities.iter().skip(index + 1) {
                if (left.dev, left.ino) == (right.dev, right.ino) {
                    return Err("complete startup directories are not distinct objects".into());
                }
            }
        }
        Ok(())
    }
}

/// Actual native-limit observation, held deadline and descriptor custody only.
/// No Clone/Default/Serde implementation, grant constructor or saved-data path.
pub(super) struct NativeStartup {
    profile: CompleteVerificationLimits,
    deadline: Instant,
    binding: StageRootBinding,
    generation_nonce: String,
    stage: RetainedDirectory,
    source: Arc<SourceAnchor>,
    spool: RetainedDirectory,
    artifacts: RetainedDirectory,
}

impl NativeStartup {
    pub(super) fn authenticate(
        profile: &CompleteVerificationLimits,
        worker_deadline: Instant,
        binding: &StageRootBinding,
        generation_nonce: &str,
    ) -> Result<Self, String> {
        // Root calls this SAME probe from tiny scalar argv before parsing
        // startup JSON; repeating it here prevents a DATA-only constructor.
        super::complete_execution::verify_limits(
            profile.address_space_bytes,
            profile.file_size_bytes,
        )?;
        profile.validate()?;
        verify_held_deadline(profile.deadline_ms, worker_deadline)?;
        if !lower_hex(generation_nonce, 32) {
            return Err(
                "complete startup generation nonce is not 32 lowercase hexadecimal bytes".into(),
            );
        }
        let path_limit = STARTUP_PATH_BYTES_MAX.min(profile.max_retained_path_bytes);
        binding.validate(path_limit)?;
        let source_budget = Self::source_budget(profile)?;
        // Opening a DTO identity is deliberately not an inherited-FD claim.
        let stage = RetainedDirectory::open_absolute(&binding.stage, path_limit, worker_deadline)?;
        stage.require_role_entries(worker_deadline)?;
        let source_directory = stage.open_child("source", &binding.source, worker_deadline)?;
        let spool = stage.open_child("spool", &binding.spool, worker_deadline)?;
        let artifacts = stage.open_child("artifacts", &binding.artifacts, worker_deadline)?;
        source_directory.require_empty(worker_deadline)?;
        spool.require_empty(worker_deadline)?;
        artifacts.require_empty(worker_deadline)?;
        let source = Arc::new(SourceAnchor::new(
            source_directory,
            source_budget,
            worker_deadline,
        )?);
        stage.verify_current(worker_deadline)?;
        source.verify_roots()?;
        spool.verify_current(worker_deadline)?;
        artifacts.verify_current(worker_deadline)?;
        Ok(Self {
            profile: profile.clone(),
            deadline: worker_deadline,
            binding: binding.clone(),
            generation_nonce: generation_nonce.to_string(),
            stage,
            source,
            spool,
            artifacts,
        })
    }

    /// I/O budget DATA, never a native observation or analyzer grant.
    pub(super) fn source_budget(
        profile: &CompleteVerificationLimits,
    ) -> Result<SourceBudget, String> {
        SourceBudget::new(
            profile.max_source_bytes,
            profile.max_inventory_entries,
            profile.max_retained_path_bytes,
            profile.file_size_bytes.min(256 * 1024 * 1024),
        )
    }

    pub(super) fn profile(&self) -> &CompleteVerificationLimits {
        &self.profile
    }

    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn binding(&self) -> &StageRootBinding {
        &self.binding
    }

    pub(super) fn generation_nonce(&self) -> &str {
        &self.generation_nonce
    }

    pub(super) fn source(&self) -> &Arc<SourceAnchor> {
        &self.source
    }



    pub(super) fn verify_stage_current(&self) -> Result<(), String> {
        self.stage.verify_current(self.deadline)?;
        self.stage.require_role_entries(self.deadline)?;
        self.source.verify_roots()?;
        self.spool.verify_current(self.deadline)?;
        self.artifacts.verify_current(self.deadline)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_binding() -> StageRootBinding {
        let identity = |path: &str, ino| DirectoryIdentity {
            path: path.to_string(),
            dev: 1,
            ino,
        };
        StageRootBinding {
            stage_nonce: "a".repeat(128),
            stage: identity("/fixture/stage", 1),
            source: identity("/fixture/stage/source", 2),
            spool: identity("/fixture/stage/spool", 3),
            artifacts: identity("/fixture/stage/artifacts", 4),
        }
    }

    fn error<T>(result: Result<T, String>, category: &str) -> Result<String, String> {
        match result {
            Err(error) if error.contains(category) => Ok(error),
            Err(error) => Err(format!(
                "wrong error category: {error}; expected {category}"
            )),
            Ok(_) => Err(format!("unexpected success; expected {category}")),
        }
    }

    #[test]
    fn stage_data_refuses_unknown_nonce_topology_alias_and_unbounded_paths() -> Result<(), String> {
        let original = fixture_binding();
        original.validate(4096)?;
        for nonce in ["A".repeat(128), "a".repeat(127), "g".repeat(128)] {
            let mut binding = original.clone();
            binding.stage_nonce = nonce;
            error(binding.validate(4096), "128 lowercase hexadecimal")?;
        }
        let mut binding = original.clone();
        binding.source.path = "/fixture/stage/nested/source".into();
        error(binding.validate(4096), "exact direct stage child")?;
        let mut binding = original.clone();
        binding.spool.ino = binding.source.ino;
        error(binding.validate(4096), "distinct objects")?;
        for path in [
            "/fixture//stage",
            "/fixture/../stage",
            "/fixture/stage/",
            "relative/stage",
        ] {
            let mut binding = original.clone();
            binding.stage.path = path.into();
            error(binding.validate(4096), "canonical absolute")?;
        }
        error(original.validate(4), "bounded canonical")?;
        let mut json = serde_json::to_value(&original).map_err(|error| error.to_string())?;
        json["unexpected"] = true.into();
        match serde_json::from_value::<StageRootBinding>(json) {
            Err(error) => assert!(error.to_string().contains("unknown field")),
            Ok(_) => return Err("unknown startup DATA field unexpectedly accepted".into()),
        }
        Ok(())
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn source_budget_keeps_existing_source_and_per_file_caps() -> Result<(), String> {
        let mut profile = super::super::complete_contract::tests::fixture_binding()?.profile;
        NativeStartup::source_budget(&profile)?;
        profile.max_source_bytes = 512 * 1024 * 1024 + 1;
        error(
            NativeStartup::source_budget(&profile),
            "invalid staged source budget",
        )?;
        error(
            SourceBudget::new(1, 0, 1, 1),
            "invalid staged source budget",
        )?;
        error(
            SourceBudget::new(1, 1, 1, 256 * 1024 * 1024 + 1),
            "invalid staged source budget",
        )?;
        Ok(())
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn serialized_stage_cannot_replace_the_actual_native_limit_probe() -> Result<(), String> {
        let profile = super::super::complete_contract::tests::fixture_binding()?.profile;
        let binding = fixture_binding();
        // Deliberately invalid native scalar profile fails before any fixture
        // path I/O, rather than fabricating a positive resource-limit receipt.
        let mut invalid = profile.clone();
        invalid.address_space_bytes = 0;
        error(
            NativeStartup::authenticate(
                &invalid,
                Instant::now() + Duration::from_millis(profile.deadline_ms),
                &binding,
                &"b".repeat(32),
            ),
            "invalid finite resource profile",
        )?;
        Ok(())
    }

    #[test]
    fn held_deadline_is_not_restarted_or_extended() -> Result<(), String> {
        error(
            verify_held_deadline(1000, Instant::now()),
            "held deadline expired",
        )?;
        error(
            verify_held_deadline(1, Instant::now() + Duration::from_mins(1)),
            "exceeds the admitted duration",
        )?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_proc_limit_probe_refuses_a_mismatched_finite_profile() -> Result<(), String> {
        // Two distinct exact profiles cannot both match the actual soft/hard
        // rows. Observe the real shared probe; create no NativeStartup fixture.
        let first = super::super::complete_execution::verify_limits(1, 1);
        let second = super::super::complete_execution::verify_limits(2, 2);
        let is_native_refusal = |error: &str| {
            error.contains("is not the requested finite soft/hard limit")
                || error.starts_with("experimental worker nonfinite Max address space: ")
                || error.starts_with("experimental worker nonfinite Max file size: ")
        };
        let mismatch = match (first, second) {
            (Err(error), _) if is_native_refusal(&error) => error,
            (_, Err(error)) if is_native_refusal(&error) => error,
            (left, right) => {
                return Err(format!(
                    "real finite-profile mismatch not observed: {left:?}; {right:?}"
                ));
            }
        };
        assert!(mismatch.starts_with("experimental worker "));
        Ok(())
    }
}

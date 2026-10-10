//! Inactive finite admission DATA for the closed whole-head-v1 request.
//!
//! These bounds preserve the existing native and compact-consumer ceilings.
//! They neither establish native limits nor promise that simultaneous analyzer,
//! source, relation and output allocations fit. The consuming caller must admit
//! every live representation and actual parent/stage resources before work.

use super::complete_contract::CompleteVerificationLimits;
use crate::analysis::CompleteRustPolicySnapshot;

const MIB: u64 = 1024 * 1024;
pub(super) const PROFILE_NAME: &str = "whole-head-v1";

pub(super) fn profile_for_request(
    name: &str,
    observed: &CompleteRustPolicySnapshot,
) -> Result<CompleteVerificationLimits, String> {
    if name != PROFILE_NAME {
        return Err("unsupported complete admission profile".into());
    }
    let file_limit = u64::try_from(observed.diff_index_file_limit())
        .map_err(|error| format!("complete profile index conversion: {error}"))?;
    let profile = CompleteVerificationLimits {
        address_space_bytes: 2 * 1024 * MIB,
        file_size_bytes: 256 * MIB,
        deadline_ms: 300_000,
        max_manifest_bytes: 16 * MIB,
        max_binding_bytes: 8 * MIB,
        max_artifact_bytes: [
            256 * MIB,
            256 * MIB,
            256 * MIB,
            256 * MIB,
            64 * MIB,
            2 * MIB,
            128 * 1024,
            32 * MIB,
            32 * MIB,
        ],
        max_total_artifact_bytes: 512 * MIB,
        max_buffered_bytes: 1024 * MIB,
        max_relation_bytes: 16 * MIB,
        max_inventory_entries: 65_536,
        max_inventory_bytes: 16 * MIB,
        max_source_bytes: 512 * MIB,
        file_limit,
        max_raw_records: 1_048_576,
        max_retained_path_bytes: 16 * MIB,
        max_raw_projection_bytes: 128 * MIB,
    };
    profile.validate()?;
    artifact_storage_reservation(&profile)?;
    Ok(profile)
}

/// Exact name-to-DATA correspondence; a valid-shaped alternative is refused.
pub(super) fn require_named_profile(
    name: &str,
    observed: &CompleteRustPolicySnapshot,
    offered: &CompleteVerificationLimits,
) -> Result<(), String> {
    if offered != &profile_for_request(name, observed)? {
        return Err("complete profile differs from the current named admission".into());
    }
    Ok(())
}

fn checked_sum(values: &[u64]) -> Result<u64, String> {
    values.iter().try_fold(0_u64, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| "complete profile composite reservation overflow".into())
    })
}

/// Logical artifact storage DATA. The existing aggregate counts nine payloads;
/// the manifest is separate. Allocated blocks/metadata and every other stage
/// role need their own actual parent admission, rather than an FS-limit claim.
pub(super) fn artifact_storage_reservation(
    profile: &CompleteVerificationLimits,
) -> Result<u64, String> {
    profile.validate()?;
    checked_sum(&[profile.max_total_artifact_bytes, profile.max_manifest_bytes])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::capture_complete_rust_policy;

    fn refuses<T>(result: Result<T, String>, category: &str) -> Result<(), String> {
        match result {
            Err(error) if error.contains(category) => Ok(()),
            Err(error) => Err(format!(
                "wrong refusal category: {error}; expected {category}"
            )),
            Ok(_) => Err(format!("unexpected success; expected {category}")),
        }
    }

    #[test]
    fn named_profile_preserves_existing_ceilings_and_actual_index_policy() -> Result<(), String> {
        let observed = capture_complete_rust_policy()?;
        let profile = profile_for_request(PROFILE_NAME, &observed)?;
        assert_eq!(profile.address_space_bytes, 2 * 1024 * MIB);
        assert_eq!(profile.file_size_bytes, 256 * MIB);
        assert_eq!(profile.deadline_ms, 300_000);
        assert_eq!(profile.max_manifest_bytes, 16 * MIB);
        assert_eq!(profile.max_binding_bytes, 8 * MIB);
        assert_eq!(
            profile.max_artifact_bytes,
            [
                256 * MIB,
                256 * MIB,
                256 * MIB,
                256 * MIB,
                64 * MIB,
                2 * MIB,
                128 * 1024,
                32 * MIB,
                32 * MIB,
            ],
        );
        assert_eq!(profile.max_total_artifact_bytes, 512 * MIB);
        assert_eq!(profile.max_buffered_bytes, 1024 * MIB);
        assert_eq!(profile.max_relation_bytes, 16 * MIB);
        assert_eq!(profile.max_inventory_entries, 65_536);
        assert_eq!(profile.max_inventory_bytes, 16 * MIB);
        assert_eq!(profile.max_source_bytes, 512 * MIB);
        assert_eq!(
            profile.file_limit,
            u64::try_from(observed.diff_index_file_limit()).map_err(|error| error.to_string())?,
        );
        assert_eq!(profile.max_raw_records, 1_048_576);
        assert_eq!(profile.max_retained_path_bytes, 16 * MIB);
        assert_eq!(profile.max_raw_projection_bytes, 128 * MIB);
        require_named_profile(PROFILE_NAME, &observed, &profile)
    }

    #[test]
    fn unknown_profile_has_no_environment_or_ordinary_fallback() -> Result<(), String> {
        let observed = capture_complete_rust_policy()?;
        for name in ["", "whole-head-v2", "whole-head-v1 ", "ordinary"] {
            refuses(
                profile_for_request(name, &observed),
                "unsupported complete admission profile",
            )?;
        }
        Ok(())
    }

    #[test]
    fn individually_valid_larger_deadline_or_storage_cannot_forge_named_profile()
    -> Result<(), String> {
        let observed = capture_complete_rust_policy()?;
        let mut offered = profile_for_request(PROFILE_NAME, &observed)?;
        offered.deadline_ms += 1;
        offered.validate()?;
        refuses(
            require_named_profile(PROFILE_NAME, &observed, &offered),
            "differs from the current named admission",
        )?;
        offered = profile_for_request(PROFILE_NAME, &observed)?;
        offered.max_total_artifact_bytes += 1;
        offered.validate()?;
        refuses(
            require_named_profile(PROFILE_NAME, &observed, &offered),
            "differs from the current named admission",
        )?;
        Ok(())
    }

    #[test]
    fn manifest_is_separately_reserved_and_composite_overflow_refuses() -> Result<(), String> {
        let observed = capture_complete_rust_policy()?;
        let profile = profile_for_request(PROFILE_NAME, &observed)?;
        assert_eq!(artifact_storage_reservation(&profile)?, 528 * MIB);
        assert_eq!(checked_sum(&[0, 1, u64::MAX - 1])?, u64::MAX);
        refuses(
            checked_sum(&[u64::MAX, 1]),
            "composite reservation overflow",
        )
    }
}

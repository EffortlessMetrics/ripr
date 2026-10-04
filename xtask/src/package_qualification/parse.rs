use serde_json::Value;

use super::identity::{check_git_sha, check_sha256_digest, reject_secret_tokens, require_nonempty};
use super::model::{
    CLAIM_BOUNDARY, Channel, PackageQualificationReceipt, RECEIPT_KIND, RECEIPT_SCHEMA_VERSION,
    RowStatus, STANDING_NON_CLAIMS,
};

pub(crate) fn parse_receipt(text: &str) -> Result<PackageQualificationReceipt, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|error| format!("package qualification receipt is not JSON: {error}"))?;
    reject_secret_tokens(&value, "receipt")?;
    let receipt: PackageQualificationReceipt = serde_json::from_value(value).map_err(|error| {
        format!("package qualification receipt does not match the typed contract: {error}")
    })?;
    validate_receipt(&receipt)?;
    Ok(receipt)
}

fn validate_receipt(receipt: &PackageQualificationReceipt) -> Result<(), String> {
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "schema_version must be {RECEIPT_SCHEMA_VERSION}, got {}",
            receipt.schema_version
        ));
    }
    if receipt.kind != RECEIPT_KIND {
        return Err(format!("kind must be {RECEIPT_KIND}, got {}", receipt.kind));
    }
    check_git_sha("source.commit", &receipt.source.commit)?;
    check_git_sha("source.tree", &receipt.source.tree)?;
    require_nonempty(
        "release_identity.product",
        &receipt.release_identity.product,
    )?;
    require_nonempty(
        "release_identity.version",
        &receipt.release_identity.version,
    )?;
    if receipt.release_identity.product != "ripr" {
        return Err("release_identity.product must be ripr".to_string());
    }
    validate_package_name(receipt.package.channel, &receipt.package.name)?;
    check_sha256_digest("package.hash", &receipt.package.hash)?;
    require_nonempty("payload.target", &receipt.payload.target)?;
    check_sha256_digest("payload.hash", &receipt.payload.hash)?;
    require_nonempty("tools.runtime", &receipt.tools.runtime)?;
    require_nonempty("tools.package_manager", &receipt.tools.package_manager)?;
    validate_subject_counts(&receipt.subjects)?;
    validate_required_rows(&receipt.required_rows)?;
    for (index, row) in receipt.rows.iter().enumerate() {
        validate_row(receipt, index, row)?;
    }
    if receipt.claim_boundary != CLAIM_BOUNDARY {
        return Err(
            "claim_boundary must use the standing package-qualification boundary".to_string(),
        );
    }
    for required in STANDING_NON_CLAIMS {
        if !receipt.non_claims.iter().any(|item| item == required) {
            return Err(format!("non_claims must include `{required}`"));
        }
    }
    Ok(())
}

fn validate_package_name(channel: Channel, name: &str) -> Result<(), String> {
    let expected = match channel {
        Channel::Pypi => "ripr-rs",
        Channel::Npm => "@effortlessmetrics/ripr",
    };
    if name == expected {
        Ok(())
    } else {
        Err(format!(
            "package.name for {} must be {expected}, got {name}",
            channel.as_str()
        ))
    }
}

fn validate_subject_counts(subjects: &super::model::SubjectCounts) -> Result<(), String> {
    if subjects.selected < subjects.executed {
        return Err("subjects.selected must be >= subjects.executed".to_string());
    }
    if subjects.executed < subjects.failed {
        return Err("subjects.executed must be >= subjects.failed".to_string());
    }
    Ok(())
}

fn validate_required_rows(rows: &[super::model::RowKey]) -> Result<(), String> {
    if rows.is_empty() {
        return Err("required_rows must name the exact selected channel/target set".to_string());
    }
    let mut seen = std::collections::BTreeSet::new();
    for key in rows {
        require_nonempty("required_rows.target", &key.target)?;
        let packed = format!("{}/{}", key.channel.as_str(), key.target);
        if !seen.insert(packed) {
            return Err(format!(
                "required_rows contains duplicate {}/{}",
                key.channel.as_str(),
                key.target
            ));
        }
    }
    Ok(())
}

fn validate_row(
    receipt: &PackageQualificationReceipt,
    index: usize,
    row: &super::model::QualificationRow,
) -> Result<(), String> {
    let prefix = format!("rows[{index}]");
    require_nonempty(&format!("{prefix}.target"), &row.target)?;
    check_sha256_digest(&format!("{prefix}.package_hash"), &row.package_hash)?;
    check_sha256_digest(&format!("{prefix}.payload_hash"), &row.payload_hash)?;
    if row.status == RowStatus::Passed {
        if row.subject_count == 0 {
            return Err(format!(
                "{prefix} cannot be passed with a zero-subject count"
            ));
        }
        let Some(executed) = row.executed_payload_hash.as_deref() else {
            return Err(format!(
                "{prefix}.executed_payload_hash is required when status is passed"
            ));
        };
        check_sha256_digest(&format!("{prefix}.executed_payload_hash"), executed)?;
        if executed != receipt.payload.hash {
            return Err(format!(
                "{prefix}.executed_payload_hash does not match payload.hash; a planted or substituted executable cannot pass"
            ));
        }
        if row.package_hash != receipt.package.hash {
            return Err(format!("{prefix}.package_hash does not match package.hash"));
        }
        if row.payload_hash != receipt.payload.hash {
            return Err(format!("{prefix}.payload_hash does not match payload.hash"));
        }
        if row.steps.is_empty() {
            return Err(format!(
                "{prefix}.steps must be nonempty when status is passed"
            ));
        }
    }
    if let Some(executed) = row.executed_payload_hash.as_deref() {
        check_sha256_digest(&format!("{prefix}.executed_payload_hash"), executed)?;
    }
    Ok(())
}

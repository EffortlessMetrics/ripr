//! Fail-closed identity-registry invariants (#4804 controls).

use super::kinds::{IdentityKind, is_forbidden_portable_input};
use super::record::{AdjacentField, FieldRole, FieldVisibility, IdentityRecord};

pub(crate) fn registry_violations(
    records: &[IdentityRecord],
    adjacent: &[AdjacentField],
) -> Vec<String> {
    let mut violations = Vec::new();
    require_taxonomy(records, &mut violations);
    require_sibling_kinds(records, &mut violations);
    require_unique_kinds(records, &mut violations);
    require_unique_field_dispositions(records, adjacent, &mut violations);
    require_parent_child_consistency(records, &mut violations);
    require_repair_attempt_separation(records, &mut violations);
    require_snapshot_id_is_not_completed_snapshot(records, &mut violations);
    require_portable_inputs(records, &mut violations);
    require_alias_generations(records, &mut violations);
    violations.sort();
    violations.dedup();
    violations
}

fn require_taxonomy(records: &[IdentityRecord], violations: &mut Vec<String>) {
    for kind in IdentityKind::REQUIRED_TAXONOMY {
        if !records.iter().any(|record| record.kind == *kind) {
            violations.push(format!(
                "required taxonomy identity `{}` is missing from the registry",
                kind.as_str()
            ));
        }
    }
}

fn require_sibling_kinds(records: &[IdentityRecord], violations: &mut Vec<String>) {
    for kind in IdentityKind::ALL {
        if IdentityKind::REQUIRED_TAXONOMY.contains(kind) {
            continue;
        }
        if !records.iter().any(|record| record.kind == *kind) {
            violations.push(format!(
                "serialized sibling identity `{}` is missing from the registry",
                kind.as_str()
            ));
        }
    }
}

fn require_unique_kinds(records: &[IdentityRecord], violations: &mut Vec<String>) {
    for (index, record) in records.iter().enumerate() {
        if records
            .iter()
            .enumerate()
            .any(|(other, candidate)| other != index && candidate.kind == record.kind)
        {
            violations.push(format!(
                "identity `{}` has contradictory duplicate owners",
                record.kind.as_str()
            ));
        }
    }
}

fn require_unique_field_dispositions(
    records: &[IdentityRecord],
    adjacent: &[AdjacentField],
    violations: &mut Vec<String>,
) {
    let mut claims = Vec::new();
    for record in records {
        for field in record.serialization {
            claims.push(FieldClaim {
                name: field.name,
                surface: field.surface,
                owner: record.kind.as_str(),
            });
        }
    }
    for entry in adjacent {
        for surface in entry.surfaces {
            claims.push(FieldClaim {
                name: entry.name,
                surface,
                owner: "adjacent",
            });
        }
    }
    for (index, claim) in claims.iter().enumerate() {
        for other in claims.iter().skip(index + 1) {
            if claim.name != other.name || claim.surface != other.surface {
                continue;
            }
            violations.push(duplicate_field_message(claim, other));
        }
    }
}

struct FieldClaim {
    name: &'static str,
    surface: &'static str,
    owner: &'static str,
}

fn duplicate_field_message(left: &FieldClaim, right: &FieldClaim) -> String {
    if left.owner == "adjacent" || right.owner == "adjacent" {
        return format!(
            "adjacent field `{}` on `{}` cannot also be a registered identity field",
            left.name, left.surface
        );
    }
    let (first, second) = if left.owner <= right.owner {
        (left.owner, right.owner)
    } else {
        (right.owner, left.owner)
    };
    format!(
        "field `{}` on `{}` has contradictory owners `{first}` and `{second}`",
        left.name, left.surface
    )
}

fn require_repair_attempt_separation(records: &[IdentityRecord], violations: &mut Vec<String>) {
    let Some(repair) = records
        .iter()
        .find(|record| record.kind == IdentityKind::RepairAttemptId)
    else {
        return;
    };
    for forbidden in [
        IdentityKind::AnalysisAttemptId,
        IdentityKind::CompletedAnalysisSnapshotId,
    ] {
        if repair.parents.contains(&forbidden) || repair.children.contains(&forbidden) {
            violations.push(format!(
                "RepairAttemptId cannot be registered as a parent or child alias of `{}`",
                forbidden.as_str()
            ));
        }
        for field in repair.serialization {
            if records.iter().any(|record| {
                record.kind == forbidden
                    && record
                        .serialization
                        .iter()
                        .any(|other| other.name == field.name && other.surface == field.surface)
            }) {
                violations.push(format!(
                    "RepairAttemptId field `{}` on `{}` collides with `{}` and cannot be an analysis-attempt or snapshot alias",
                    field.name,
                    field.surface,
                    forbidden.as_str()
                ));
            }
        }
    }
}

fn require_snapshot_id_is_not_completed_snapshot(
    records: &[IdentityRecord],
    violations: &mut Vec<String>,
) {
    for record in records {
        if record.kind != IdentityKind::CompletedAnalysisSnapshotId {
            continue;
        }
        for field in record.serialization {
            if field.name == "snapshot_id" {
                violations.push(
                    "CompletedAnalysisSnapshotId cannot own wire field `snapshot_id` while that field is the refresh-generation AnalysisAttemptId compatibility echo".to_string(),
                );
            }
        }
    }
}

fn require_portable_inputs(records: &[IdentityRecord], violations: &mut Vec<String>) {
    for record in records {
        if !record.portability.is_portable() {
            continue;
        }
        for input in record.semantic_inputs {
            if is_forbidden_portable_input(input) {
                violations.push(format!(
                    "portable identity `{}` lists forbidden semantic input `{input}`",
                    record.kind.as_str()
                ));
            }
        }
    }
}

fn require_alias_generations(records: &[IdentityRecord], violations: &mut Vec<String>) {
    for record in records {
        for field in record.serialization {
            if field.role == FieldRole::CompatibilityAlias && field.removal_generation.is_none() {
                violations.push(format!(
                    "compatibility alias `{}` on `{}` for `{}` is missing a removal generation",
                    field.name,
                    field.surface,
                    record.kind.as_str()
                ));
            }
        }
    }
}

pub(crate) fn require_catalog_surfaces_are_governed(
    records: &[IdentityRecord],
    adjacent: &[AdjacentField],
    governed: &[&str],
    violations: &mut Vec<String>,
) {
    for record in records {
        for field in record.serialization {
            if field.visibility == FieldVisibility::Private {
                continue;
            }
            if !governed.contains(&field.surface) {
                violations.push(format!(
                    "identity `{}` serializes `{name}` on ungoverned surface `{surface}`",
                    record.kind.as_str(),
                    name = field.name,
                    surface = field.surface
                ));
            }
        }
    }
    for entry in adjacent {
        for surface in entry.surfaces {
            if !governed.contains(surface) {
                violations.push(format!(
                    "adjacent field `{}` on ungoverned surface `{surface}`",
                    entry.name
                ));
            }
        }
    }
}

fn require_parent_child_consistency(records: &[IdentityRecord], violations: &mut Vec<String>) {
    for record in records {
        for child in record.children {
            match records.iter().find(|candidate| candidate.kind == *child) {
                None => violations.push(format!(
                    "identity `{}` lists missing child `{}`",
                    record.kind.as_str(),
                    child.as_str()
                )),
                Some(child_record) if !child_record.parents.contains(&record.kind) => {
                    violations.push(format!(
                        "identity `{}` lists `{}` as a child, but `{}` does not list `{}` as a parent",
                        record.kind.as_str(),
                        child.as_str(),
                        child.as_str(),
                        record.kind.as_str()
                    ));
                }
                Some(_) => {}
            }
        }
        for parent in record.parents {
            match records.iter().find(|candidate| candidate.kind == *parent) {
                None => violations.push(format!(
                    "identity `{}` lists missing parent `{}`",
                    record.kind.as_str(),
                    parent.as_str()
                )),
                Some(parent_record) if !parent_record.children.contains(&record.kind) => {
                    violations.push(format!(
                        "identity `{}` lists `{}` as a parent, but `{}` does not list `{}` as a child",
                        record.kind.as_str(),
                        parent.as_str(),
                        parent.as_str(),
                        record.kind.as_str()
                    ));
                }
                Some(_) => {}
            }
        }
    }
}

pub(crate) fn sort_records(records: &mut [IdentityRecord]) {
    records.sort_by_key(|record| record.kind.as_str());
}

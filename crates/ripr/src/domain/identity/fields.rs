//! Field-name lookup used by compile-time consumers and the xtask scanner.

use super::catalog::{ADJACENT_FIELDS, PRODUCTION_RECORDS};
use super::record::IdentityRecord;

pub use super::catalog::GOVERNED_IDENTITY_SURFACES;

/// Disposition of one serialized identity-shaped field on a governed surface.
///
/// Returns `None` when the field has no registry row — the fail-closed unknown
/// field case (#4804 control 7).
pub fn identity_field_disposition(surface: &str, field: &str) -> Option<&'static str> {
    production_disposition(PRODUCTION_RECORDS, ADJACENT_FIELDS, surface, field)
}

pub(crate) fn production_disposition(
    records: &[IdentityRecord],
    adjacent: &[super::record::AdjacentField],
    surface: &str,
    field: &str,
) -> Option<&'static str> {
    for record in records {
        for serialized in record.serialization {
            if serialized.name == field && surface_matches(serialized.surface, surface) {
                return Some(record.kind.as_str());
            }
        }
    }
    for entry in adjacent {
        if entry.name == field
            && entry
                .surfaces
                .iter()
                .any(|candidate| surface_matches(candidate, surface))
        {
            return Some("adjacent");
        }
    }
    None
}

fn surface_matches(registered: &str, observed: &str) -> bool {
    if registered == observed {
        return true;
    }
    // Observed may be an absolute or checkout-prefixed path. A truncated
    // suffix must not inherit a disposition (`schema.json` is not
    // `check.schema.json`).
    observed
        .strip_suffix(registered)
        .is_some_and(|prefix| prefix.ends_with('/') || prefix.ends_with('\\'))
}

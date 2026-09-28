//! Ordered tuple-arm evidence dispatch.
//!
//! Keep the already-qualified direct-parameter contract isolated from the
//! bounded derived-local closure contract. The direct witness remains first;
//! the derived witness may refine only evidence that is still unverified and
//! has passed its stricter owner-role and executed-oracle admission.

use crate::analysis::classify::ProbeContext;
use crate::domain::StageEvidence;

#[path = "../tuple_match_derived.rs"]
mod derived;
mod derived_admission;
mod direct;

pub(super) fn discrimination(
    context: &ProbeContext<'_>,
    observe: &StageEvidence,
    current: &StageEvidence,
) -> Option<StageEvidence> {
    direct::discrimination(context, observe, current).or_else(|| {
        if derived_admission::admits(context) {
            derived::discrimination(context, observe, current)
        } else {
            None
        }
    })
}

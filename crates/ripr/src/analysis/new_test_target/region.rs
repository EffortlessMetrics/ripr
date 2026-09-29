//! Exact governed inline test-module region identity (#4783-shaped sidecar).
//!
//! Portable identity excludes checkout spelling. Concrete containment uses
//! byte offsets into the current source digest, not a whole-file allowlist.

use super::NewTestProposalBlocker;
use crate::analysis::syntax::{GovernedCfgTestModule, governed_cfg_test_modules};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Exact governed inline test-module region carried with an InlineUnit proposal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct InlineTestRegionAuthority {
    pub(crate) file: PathBuf,
    pub(crate) module_name: String,
    pub(crate) parent_modules: Vec<String>,
    pub(crate) body_start: usize,
    pub(crate) close_brace_start: usize,
    pub(crate) source_digest: String,
}

/// Validate an external edit against a previously admitted inline region.
/// Production text outside the module body must be unchanged.
///
/// Tests own the current callers. The #4783 cage consumer is the production
/// path that will fail-close packet edits against this same function.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "producer-owned region validator is called from tests now; #4783 is the cage consumer"
    )
)]
pub(crate) fn validate_inline_region_edit(
    before: &str,
    after: &str,
    region: &InlineTestRegionAuthority,
) -> Result<(), NewTestProposalBlocker> {
    fn unique_matching_inline<'a>(
        modules: &'a [GovernedCfgTestModule],
        region: &InlineTestRegionAuthority,
    ) -> Result<&'a GovernedCfgTestModule, NewTestProposalBlocker> {
        let matches = modules
            .iter()
            .filter(|module| {
                module.is_inline
                    && module.name == region.module_name
                    && module.parent_modules == region.parent_modules
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(NewTestProposalBlocker::AmbiguousModule);
        }
        Ok(matches[0])
    }

    if source_digest(before) != region.source_digest {
        return Err(NewTestProposalBlocker::StaleSource);
    }
    let before_modules =
        governed_cfg_test_modules(before).ok_or(NewTestProposalBlocker::LexicalFallback)?;
    let after_modules =
        governed_cfg_test_modules(after).ok_or(NewTestProposalBlocker::LexicalFallback)?;
    let before_module = unique_matching_inline(&before_modules, region)?;
    let after_module = unique_matching_inline(&after_modules, region)?;
    let before_start = before_module
        .body_start
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    let after_start = after_module
        .body_start
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    let before_close = before_module
        .close_brace_start
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    let after_close = after_module
        .close_brace_start
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    let before_prefix = before
        .get(..before_start)
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    let after_prefix = after
        .get(..after_start)
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    let before_suffix = before
        .get(before_close..)
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    let after_suffix = after
        .get(after_close..)
        .ok_or(NewTestProposalBlocker::ProductionEdit)?;
    if before_prefix != after_prefix || before_suffix != after_suffix {
        return Err(NewTestProposalBlocker::ProductionEdit);
    }
    Ok(())
}

pub(super) fn source_digest(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

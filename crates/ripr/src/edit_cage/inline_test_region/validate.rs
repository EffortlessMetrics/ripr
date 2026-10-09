//! Before/after validation against one captured inline test-module region.

use std::ops::Range;

use super::{
    InlineTestRegionAuthority, InlineTestRegionRejectReason, InlineTestRegionVerdict, digest_bytes,
    observe,
};

/// Validate that `after` is a pure insertion of test-role items into the
/// exact region named by `authority`, relative to `before`.
pub(crate) fn validate_inline_test_region_edit(
    before: &str,
    authority: &InlineTestRegionAuthority,
    after: &str,
) -> InlineTestRegionVerdict {
    if digest_bytes(before.as_bytes()) != authority.source_digest {
        return InlineTestRegionVerdict::rejected(
            InlineTestRegionRejectReason::StaleSourceDigest,
            None,
        );
    }
    let body = authority.body_range.clone();
    if body.end > before.len() || body.start > body.end {
        return InlineTestRegionVerdict::rejected(
            InlineTestRegionRejectReason::StaleModuleAnchor,
            Some(body),
        );
    }
    if before.get(body.end..body.end.saturating_add(1)) != Some("}") {
        return InlineTestRegionVerdict::rejected(
            InlineTestRegionRejectReason::StaleModuleAnchor,
            Some(body),
        );
    }

    let Some(module_name) = authority
        .portable
        .module_path
        .rsplit("::")
        .next()
        .filter(|name| !name.is_empty())
    else {
        return InlineTestRegionVerdict::rejected(
            InlineTestRegionRejectReason::MissingRegion,
            None,
        );
    };
    let after_region = match observe::observe_inline_test_region(after, module_name) {
        Ok(region) => region,
        Err(error) => {
            let range = first_changed_range(before, after);
            let reason = error
                .reason()
                .unwrap_or(InlineTestRegionRejectReason::Unparseable);
            return InlineTestRegionVerdict::rejected(reason, range);
        }
    };
    if after_region.module_path != authority.portable.module_path {
        return InlineTestRegionVerdict::rejected(InlineTestRegionRejectReason::WrongModule, None);
    }

    let Some(before_prefix) = before.get(..body.start) else {
        return InlineTestRegionVerdict::rejected(
            InlineTestRegionRejectReason::StaleModuleAnchor,
            Some(body),
        );
    };
    let after_prefix = after.get(..after_region.body_range.start).unwrap_or("");
    if after_prefix != before_prefix {
        let range = first_changed_range(before, after);
        let reason = match range.as_ref() {
            Some(changed) if changed.start < authority.header_range.start => {
                InlineTestRegionRejectReason::ProductionEdit
            }
            _ => InlineTestRegionRejectReason::ModuleDeclarationChanged,
        };
        return InlineTestRegionVerdict::rejected(reason, range);
    }
    let before_suffix = before.get(body.end..).unwrap_or("");
    let after_suffix = after.get(after_region.body_range.end..).unwrap_or("");
    if after_suffix != before_suffix {
        let range = first_changed_range(before, after);
        return InlineTestRegionVerdict::rejected(
            production_or_declaration_reason(before, after, &body),
            range,
        );
    }
    if after_region.cfg_basis_digest != authority.portable.cfg_basis_digest {
        return InlineTestRegionVerdict::rejected(
            InlineTestRegionRejectReason::CfgBasisChanged,
            None,
        );
    }
    if after_region.header_anchor_digest != authority.portable.header_anchor_digest {
        return InlineTestRegionVerdict::rejected(
            InlineTestRegionRejectReason::StaleModuleAnchor,
            None,
        );
    }
    let before_items = match observe::named_module_items(before, &authority.portable.module_path) {
        Ok(items) => items,
        Err(error) => {
            return InlineTestRegionVerdict::rejected(
                error
                    .reason()
                    .unwrap_or(InlineTestRegionRejectReason::MissingRegion),
                None,
            );
        }
    };
    let after_items = match observe::named_module_items(after, &authority.portable.module_path) {
        Ok(items) => items,
        Err(error) => {
            return InlineTestRegionVerdict::rejected(
                error
                    .reason()
                    .unwrap_or(InlineTestRegionRejectReason::Unparseable),
                None,
            );
        }
    };

    match classify_item_delta(&before_items, &after_items) {
        ItemDelta::Admitted => InlineTestRegionVerdict::admitted(),
        ItemDelta::NotARepair => {
            InlineTestRegionVerdict::not_a_repair(InlineTestRegionRejectReason::NonTestSubject)
        }
        ItemDelta::Rejected(reason) => {
            InlineTestRegionVerdict::rejected(reason, first_changed_range(before, after))
        }
    }
}

enum ItemDelta {
    Admitted,
    NotARepair,
    Rejected(InlineTestRegionRejectReason),
}

fn classify_item_delta(
    before_items: &[observe::ObservedItem],
    after_items: &[observe::ObservedItem],
) -> ItemDelta {
    // Existing items must appear in order. New `fn` items (and optional `use`
    // companions) may be inserted at more than one site inside the body.
    let mut before_index = 0usize;
    let mut added_fn = false;
    for item in after_items {
        if before_items
            .get(before_index)
            .is_some_and(|existing| existing.text == item.text)
        {
            before_index += 1;
            continue;
        }
        match item.kind {
            observe::ObservedItemKind::Fn => added_fn = true,
            observe::ObservedItemKind::Use => {}
            observe::ObservedItemKind::NestedModule => {
                return ItemDelta::Rejected(InlineTestRegionRejectReason::UnsupportedModuleKind);
            }
            observe::ObservedItemKind::Other => {
                return ItemDelta::Rejected(InlineTestRegionRejectReason::NonTestSubject);
            }
        }
    }
    if before_index != before_items.len() {
        return ItemDelta::Rejected(InlineTestRegionRejectReason::ExistingAuthorityRewritten);
    }
    if added_fn {
        ItemDelta::Admitted
    } else {
        ItemDelta::NotARepair
    }
}

fn production_or_declaration_reason(
    before: &str,
    after: &str,
    body: &Range<usize>,
) -> InlineTestRegionRejectReason {
    let Some(range) = first_changed_range(before, after) else {
        return InlineTestRegionRejectReason::NotPureInsertion;
    };
    if range.start < body.start {
        return InlineTestRegionRejectReason::ProductionEdit;
    }
    if range.start >= body.end.saturating_add(1) {
        return InlineTestRegionRejectReason::ProductionEdit;
    }
    InlineTestRegionRejectReason::ModuleDeclarationChanged
}

fn first_changed_range(before: &str, after: &str) -> Option<Range<usize>> {
    let prefix = common_prefix_len(before.as_bytes(), after.as_bytes());
    if prefix == before.len() && prefix == after.len() {
        return None;
    }
    let before_tail = before.as_bytes().get(prefix..).unwrap_or(&[]);
    let after_tail = after.as_bytes().get(prefix..).unwrap_or(&[]);
    let suffix = common_suffix_len(before_tail, after_tail);
    Some(prefix..after.len().saturating_sub(suffix).max(prefix + 1))
}

fn common_prefix_len(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_suffix_len(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
        .min(left.len())
        .min(right.len())
}

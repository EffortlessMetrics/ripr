//! Parser-backed observation of one existing inline cfg-test module.
//!
//! Cfg recognition is the shared [`crate::analysis::cfg_predicates`]
//! authority. Unsupported forms (out-of-line, unparseable, ambiguous,
//! nested-without-unique-name) fail closed.

use std::ops::Range;

use ra_ap_syntax::ast::{self, HasAttrs, HasModuleItem, HasName};
use ra_ap_syntax::{AstNode, SyntaxNode};

use super::{InlineTestRegionError, InlineTestRegionRejectReason, digest_bytes};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedInlineTestRegion {
    pub(crate) module_path: String,
    pub(crate) cfg_basis_digest: String,
    pub(crate) header_anchor_digest: String,
    pub(crate) header_range: Range<usize>,
    pub(crate) body_range: Range<usize>,
}

pub(crate) fn observe_inline_test_region(
    source: &str,
    module_name: &str,
) -> Result<ObservedInlineTestRegion, InlineTestRegionError> {
    let Some(parse) = crate::analysis::parse_clean_source_file(source) else {
        return Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::Unparseable,
        });
    };
    let mut matches = Vec::new();
    for module in parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Module::cast)
        .filter(has_module_item_ancestry)
    {
        let Some(observed) = observe_module(source, &module) else {
            continue;
        };
        if observed.module_path.rsplit("::").next() == Some(module_name) {
            matches.push(observed);
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::MissingRegion,
        }),
        _ => Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::AmbiguousRegion,
        }),
    }
}

fn observe_module(source: &str, module: &ast::Module) -> Option<ObservedInlineTestRegion> {
    let list = module.item_list()?;
    module.name()?;
    let (body_start, body_end) = item_list_body_offsets(&list)?;
    if body_end < body_start || body_end > source.len() {
        return None;
    }
    let mut attributes: Vec<String> = module
        .attrs()
        .map(|attr| attr.syntax().text().to_string())
        .collect();
    attributes.extend(item_list_inner_attribute_texts(&list));
    if !crate::analysis::cfg_predicates::attributes_require_test(attributes.iter()) {
        return None;
    }
    let header_start = usize::from(module.syntax().text_range().start());
    let header_end = body_start;
    let header = source.get(header_start..header_end)?;
    Some(ObservedInlineTestRegion {
        module_path: module_path(module.syntax()),
        cfg_basis_digest: digest_bytes(attributes.join("\n").as_bytes()),
        header_anchor_digest: digest_bytes(header.as_bytes()),
        header_range: header_start..header_end,
        body_range: body_start..body_end,
    })
}

fn item_list_inner_attribute_texts(list: &ast::ItemList) -> Vec<String> {
    list.syntax()
        .children()
        .filter_map(ast::Attr::cast)
        .map(|attr| attr.syntax().text().to_string())
        .filter(|text| text.trim_start().starts_with("#!["))
        .collect()
}

/// File-level or nested-module items only. A `mod` inside a function, const,
/// or other statement list is not a rustc test-harness subject.
fn has_module_item_ancestry(module: &ast::Module) -> bool {
    let mut current = module.syntax().parent();
    while let Some(node) = current {
        if ast::SourceFile::cast(node.clone()).is_some() {
            return true;
        }
        if ast::Module::cast(node.clone()).is_some() || ast::ItemList::cast(node.clone()).is_some()
        {
            current = node.parent();
            continue;
        }
        return false;
    }
    false
}

fn item_list_body_offsets(list: &ast::ItemList) -> Option<(usize, usize)> {
    let mut open_end = None;
    let mut close_start = None;
    for element in list.syntax().children_with_tokens() {
        match element.kind() {
            ra_ap_syntax::T!['{'] => {
                open_end = Some(usize::from(element.text_range().end()));
            }
            ra_ap_syntax::T!['}'] => {
                close_start = Some(usize::from(element.text_range().start()));
            }
            _ => {}
        }
    }
    Some((open_end?, close_start?))
}

fn module_path(node: &SyntaxNode) -> String {
    let mut names = Vec::new();
    let mut current = Some(node.clone());
    while let Some(node) = current {
        if let Some(module) = ast::Module::cast(node.clone())
            && let Some(name) = module.name()
        {
            names.push(name.text().to_string());
        }
        current = node.parent();
    }
    names.reverse();
    if names.is_empty() {
        "unknown".to_string()
    } else {
        names.join("::")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ObservedItemKind {
    Fn,
    Use,
    NestedModule,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedItem {
    pub(crate) kind: ObservedItemKind,
    pub(crate) text: String,
}

pub(crate) fn named_module_items(
    source: &str,
    expected_module_path: &str,
) -> Result<Vec<ObservedItem>, InlineTestRegionError> {
    let Some(parse) = crate::analysis::parse_clean_source_file(source) else {
        return Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::Unparseable,
        });
    };
    let mut matches = Vec::new();
    for module in parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Module::cast)
        .filter(has_module_item_ancestry)
    {
        if module_path(module.syntax()) != expected_module_path {
            continue;
        }
        let Some(list) = module.item_list() else {
            continue;
        };
        matches.push(
            list.items()
                .map(|item| ObservedItem {
                    kind: item_kind(&item),
                    text: item.syntax().text().to_string(),
                })
                .collect::<Vec<_>>(),
        );
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::MissingRegion,
        }),
        _ => Err(InlineTestRegionError::Unsupported {
            reason: InlineTestRegionRejectReason::AmbiguousRegion,
        }),
    }
}

fn item_kind(item: &ast::Item) -> ObservedItemKind {
    let node = item.syntax();
    if ast::Fn::cast(node.clone()).is_some() {
        ObservedItemKind::Fn
    } else if ast::Use::cast(node.clone()).is_some() {
        ObservedItemKind::Use
    } else if ast::Module::cast(node.clone()).is_some() {
        ObservedItemKind::NestedModule
    } else {
        ObservedItemKind::Other
    }
}

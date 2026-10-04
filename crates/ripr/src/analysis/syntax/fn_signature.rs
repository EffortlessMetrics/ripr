//! Parser-owned signature facts for the function that owns one seam offset.
//!
//! The Rust test-stub producer needs the owner's exact call shape (receiver,
//! parameters, return type, enclosing impl) to write a call that compiles.
//! Line-text heuristics cannot recover that, so this reads it from the same
//! clean `ra_ap_syntax` parse the rest of the analyzer trusts and returns
//! `None` when the file is not parser-valid.

use super::parse_clean_source_file;
use super::ra::module_attributes_require_test;
use crate::analysis::facts::cfg_predicates::attribute_test_build_availability;
use ra_ap_syntax::{
    AstNode, SyntaxKind, TextSize,
    ast::{self, HasGenericParams, HasName},
};

/// How the owner takes `self`, when it is a method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerReceiver {
    Value,
    Ref,
    RefMut,
    /// `self: Box<Self>`, `self: Rc<Self>` and other explicitly typed
    /// receivers: the subject is not a plain value of the impl type.
    Typed,
}

/// One non-`self` parameter as written in the signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerParam {
    /// The binding name when the pattern is a plain identifier.
    pub(crate) name: Option<String>,
    pub(crate) ty: String,
}

/// Where the owner sits relative to `impl` and `trait` items.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OwnerContainer {
    /// A module-level `fn`.
    Free,
    /// An inherent `impl` whose self type is a plain path, with lifetime
    /// arguments at most (`Parser<'a>`), spelled as written.
    Inherent { self_type: String },
    /// A trait `impl` whose self type is a plain path with lifetime
    /// arguments at most. `trait_path` is the trait as written in the impl
    /// header, or `None` when it is not a plain path the test can name.
    TraitImpl {
        self_type: String,
        trait_path: Option<String>,
    },
    /// An `impl` that declares type or const generics, or whose self type
    /// has non-lifetime generic arguments: the stub cannot choose them.
    GenericImpl(&'static str),
    /// Anything else the stub cannot call by name: a trait body, a non-path
    /// impl, or a function nested inside another function body.
    Unsupported(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerSignature {
    pub(crate) name: String,
    pub(crate) container: OwnerContainer,
    pub(crate) receiver: Option<OwnerReceiver>,
    pub(crate) params: Vec<OwnerParam>,
    /// `None` for `()`-returning functions.
    pub(crate) return_type: Option<String>,
    pub(crate) is_async: bool,
    pub(crate) is_unsafe: bool,
    /// Type or const generic parameters (lifetimes do not count).
    pub(crate) has_type_generics: bool,
    /// Production (non-`cfg(test)`) modules enclosing the owner, outermost
    /// first, matching `production_owner_module_path`.
    pub(crate) parent_modules: Vec<String>,
}

/// Signature of the innermost `fn` whose text range contains `byte_offset`.
pub(crate) fn owner_signature_at(source: &str, byte_offset: usize) -> Option<OwnerSignature> {
    let parse = parse_clean_source_file(source)?;
    let offset = TextSize::try_from(byte_offset).ok()?;
    let function = parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter(|function| function.syntax().text_range().contains_inclusive(offset))
        .max_by_key(|function| function.syntax().text_range().start())?;
    Some(signature_of(&function))
}

/// Traits a struct or enum named `name` in `source` is known to implement,
/// from its `#[derive(..)]` list and from `impl Trait for Name` blocks. Only
/// the last path segment of each trait is kept (`fmt::Debug` → `Debug`).
/// `None` when the file does not parse or does not define exactly one type
/// with that name, so callers cannot mistake an unknown type for one without
/// the trait.
pub(crate) fn local_type_traits(source: &str, name: &str) -> Option<Vec<String>> {
    let parse = parse_clean_source_file(source)?;
    let tree = parse.tree();
    // A test-only definition does not stand in for the production type.
    let in_test_module = |node: &ra_ap_syntax::SyntaxNode| {
        node.ancestors()
            .filter_map(ast::Module::cast)
            .any(|module| module_attributes_require_test(&module))
    };
    let mut definitions = tree
        .syntax()
        .descendants()
        .filter_map(ast::Adt::cast)
        .filter(|adt| adt.name().is_some_and(|ident| ident.text() == name))
        .filter(|adt| !in_test_module(adt.syntax()));
    let adt = definitions.next()?;
    if definitions.next().is_some() {
        return None;
    }
    let last_segment = |path: &str| {
        path.rsplit("::")
            .next()
            .map(|segment| segment.trim().to_string())
            .unwrap_or_default()
    };
    let mut traits = Vec::new();
    for attr in ast::HasAttrs::attrs(&adt) {
        let text = attr.syntax().text().to_string();
        let Some(inner) = text
            .trim()
            .strip_prefix("#[")
            .and_then(|rest| rest.trim_start().strip_prefix("derive"))
            .and_then(|rest| rest.trim_start().strip_prefix('('))
            .and_then(|rest| rest.trim_end().strip_suffix("]"))
            .and_then(|rest| rest.trim_end().strip_suffix(')'))
        else {
            continue;
        };
        traits.extend(
            inner
                .split(',')
                .map(last_segment)
                .filter(|name| !name.is_empty()),
        );
    }
    for item in tree.syntax().descendants().filter_map(ast::Impl::cast) {
        // A `cfg`-gated impl (or one in a gated module) may be compiled
        // out, so its trait is not counted.
        let gated = item.syntax().ancestors().any(|node| {
            ast::AnyHasAttrs::cast(node).is_some_and(|owner| {
                ast::HasAttrs::attrs(&owner).any(|attr| {
                    // Outer `#[cfg(..)]` or inner `#![cfg(..)]`, spaced or not.
                    let text = attr
                        .syntax()
                        .text()
                        .to_string()
                        .split_whitespace()
                        .collect::<String>();
                    let text = text
                        .strip_prefix("#![")
                        .or_else(|| text.strip_prefix("#["))
                        .unwrap_or(&text);
                    text.starts_with("cfg(") || text.starts_with("cfg_attr(")
                })
            })
        });
        if gated {
            continue;
        }
        let self_is_name = item
            .self_ty()
            .is_some_and(|ty| ty.syntax().text().to_string().trim() == name);
        if let (true, Some(trait_ty)) = (self_is_name, item.trait_()) {
            // `impl PartialEq<u8> for Name` compares with another type, so
            // only a bare trait or one parameterized by `Self`/`Name` counts
            // as the trait; any other argument keeps its generics and so
            // never matches a bare-name lookup.
            let text = trait_ty.syntax().text().to_string();
            let (path, argument) = match text.split_once('<') {
                Some((path, rest)) => (path, rest.trim_end().trim_end_matches('>').trim()),
                None => (text.as_str(), ""),
            };
            if argument.is_empty() || argument == "Self" || argument == name {
                traits.push(last_segment(path));
            } else {
                traits.push(format!("{}<{argument}>", last_segment(path)));
            }
        }
    }
    Some(traits)
}

/// Whether `source` binds the type name `name` outside test-only modules,
/// which shadows any std type of that name: a struct, enum, union or type
/// alias, a non-std `use` that ends in `name`, any `use` that renames to
/// `name`, or a non-std glob import that may bring it in. That includes
/// `self::`/`super::` globs, which can reach another file's module. An
/// unparsed file counts as binding it, so callers fail closed.
pub(crate) fn shadows_type_name(source: &str, name: &str) -> bool {
    let Some(parse) = parse_clean_source_file(source) else {
        return true;
    };
    parse.tree().syntax().descendants().any(|node| {
        let named = ast::Adt::cast(node.clone())
            .and_then(|adt| adt.name())
            .or_else(|| ast::TypeAlias::cast(node.clone()).and_then(|alias| alias.name()))
            .is_some_and(|ident| ident.text() == name)
            || ast::UseTree::cast(node.clone()).is_some_and(|tree| use_tree_binds(&tree, name));
        named
            && !node
                .ancestors()
                .filter_map(ast::Module::cast)
                .any(|module| module_attributes_require_test(&module))
    })
}

/// Whether one leaf of a `use` binds `name` from outside std.
fn use_tree_binds(tree: &ast::UseTree, name: &str) -> bool {
    if tree.use_tree_list().is_some() {
        return false;
    }
    // The full path: this leaf's segments behind every enclosing prefix.
    let mut segments = Vec::new();
    let mut current = Some(tree.clone());
    while let Some(tree) = current {
        if let Some(path) = tree.path() {
            let text = path.syntax().text().to_string();
            segments.splice(0..0, text.split("::").map(|s| s.trim().to_string()));
        }
        current = tree
            .syntax()
            .parent()
            .and_then(ast::UseTreeList::cast)
            .and_then(|list| list.syntax().parent())
            .and_then(ast::UseTree::cast);
    }
    let root = segments
        .iter()
        .find(|segment| !segment.is_empty())
        .map(|segment| segment.trim_start_matches("::"));
    let from_std = matches!(root, Some("std" | "core" | "alloc"));
    if tree.star_token().is_some() {
        return !from_std;
    }
    match tree.rename() {
        // `use std::sync::Mutex as Vec` shadows `Vec` as surely as a local
        // import does.
        Some(rename) => rename.name().is_some_and(|ident| ident.text() == name),
        // A std path keeps its own name; the same-named std types ripr
        // assumes (`Result` aside, which needs both arguments) compare.
        // `use a::Name::{self}` binds the segment before `self`.
        None => {
            let leaf = match segments.as_slice() {
                [.., parent, last] if last == "self" => Some(parent),
                [.., last] => Some(last),
                [] => None,
            };
            !from_std && leaf.is_some_and(|leaf| leaf == name)
        }
    }
}

/// Whether `source` parses without errors under the analyzer's own parse.
pub(crate) fn rust_source_parses_cleanly(source: &str) -> bool {
    parse_clean_source_file(source).is_some()
}

/// 1-based inclusive line span of the innermost `fn` containing `line`.
pub(crate) fn owner_fn_line_span(source: &str, line: usize) -> Option<(usize, usize)> {
    let parse = parse_clean_source_file(source)?;
    let line_of = |at: TextSize| {
        source
            .get(..usize::from(at))
            .map_or(1, |prefix| prefix.matches('\n').count() + 1)
    };
    parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter(|function| {
            let range = function.syntax().text_range();
            line_of(range.start()) <= line && line <= line_of(range.end())
        })
        .max_by_key(|function| function.syntax().text_range().start())
        .map(|function| {
            let range = function.syntax().text_range();
            (line_of(range.start()), line_of(range.end()))
        })
}

/// Whether the innermost struct-literal field at `byte_offset` belongs to the
/// value its owning `fn` returns directly: the struct literal is that
/// function's tail expression (through parentheses and nested block tails)
/// or the operand of a `return` outside any closure. A field nested inside
/// another literal, a call argument, a branch, or a `let` is not. `None`
/// when the file does not parse or no struct-literal field covers the
/// offset.
pub(crate) fn field_init_is_returned(source: &str, byte_offset: usize) -> Option<bool> {
    let parse = parse_clean_source_file(source)?;
    let offset = TextSize::try_from(byte_offset).ok()?;
    let tree = parse.tree();
    let field = tree
        .syntax()
        .descendants()
        .filter_map(ast::RecordExprField::cast)
        .filter(|field| field.syntax().text_range().contains_inclusive(offset))
        .max_by_key(|field| field.syntax().text_range().start())?;
    let record = field.syntax().ancestors().find_map(ast::RecordExpr::cast)?;
    let owner = field.syntax().ancestors().find_map(ast::Fn::cast)?;
    let mut node = record.syntax().clone();
    loop {
        let Some(parent) = node.parent() else {
            return Some(false);
        };
        if ast::ParenExpr::can_cast(parent.kind()) {
            node = parent;
            continue;
        }
        if ast::ReturnExpr::can_cast(parent.kind()) {
            let in_closure = parent
                .ancestors()
                .take_while(|ancestor| ancestor != owner.syntax())
                .any(|ancestor| {
                    // A closure, `async` block or `const` block scopes its
                    // own `return`.
                    ast::ClosureExpr::can_cast(ancestor.kind())
                        || ast::BlockExpr::cast(ancestor).is_some_and(|block| {
                            block.async_token().is_some() || block.const_token().is_some()
                        })
                });
            return Some(!in_closure);
        }
        let Some(list) = ast::StmtList::cast(parent) else {
            return Some(false);
        };
        if list.tail_expr().map(|tail| tail.syntax().clone()) != Some(node) {
            return Some(false);
        }
        // The tail of a plain (or `unsafe`) block is that block's value; an
        // `async` block yields a future and a labelled block can `break`
        // with another value, so neither passes the literal through.
        let Some(block) = list.syntax().parent().and_then(ast::BlockExpr::cast) else {
            return Some(false);
        };
        if block.async_token().is_some() || block.label().is_some() {
            return Some(false);
        }
        let Some(outer) = block.syntax().parent() else {
            return Some(false);
        };
        if ast::Fn::cast(outer.clone()).is_some_and(|function| function == owner) {
            return Some(true);
        }
        node = block.syntax().clone();
    }
}

fn signature_of(function: &ast::Fn) -> OwnerSignature {
    let name = function
        .name()
        .map(|name| name.text().to_string())
        .unwrap_or_default();
    let param_list = function.param_list();
    let receiver = param_list
        .as_ref()
        .and_then(|list| list.self_param())
        .map(|self_param| {
            if self_param.ty().is_some() {
                OwnerReceiver::Typed
            } else if self_param.amp_token().is_none() {
                OwnerReceiver::Value
            } else if self_param.mut_token().is_some() {
                OwnerReceiver::RefMut
            } else {
                OwnerReceiver::Ref
            }
        });
    let params = param_list
        .map(|list| {
            list.params()
                .map(|param| OwnerParam {
                    name: param.pat().and_then(|pat| match pat {
                        ast::Pat::IdentPat(ident) if ident.pat().is_none() => {
                            ident.name().map(|name| name.text().to_string())
                        }
                        _ => None,
                    }),
                    ty: param
                        .ty()
                        .map(|ty| ty.syntax().text().to_string())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let return_type = function
        .ret_type()
        .and_then(|ret| ret.ty())
        .map(|ty| ty.syntax().text().to_string())
        .filter(|ty| ty.trim() != "()");
    let has_type_generics = function.generic_param_list().is_some_and(|list| {
        list.generic_params()
            .any(|param| !matches!(param, ast::GenericParam::LifetimeParam(_)))
    });
    let mut parent_modules = function
        .syntax()
        .ancestors()
        .skip(1)
        .filter_map(ast::Module::cast)
        .filter(|module| !module_attributes_require_test(module))
        .filter_map(|module| module.name().map(|name| name.text().to_string()))
        .collect::<Vec<_>>();
    parent_modules.reverse();
    OwnerSignature {
        name,
        container: container_of(function),
        receiver,
        params,
        return_type,
        is_async: function.async_token().is_some(),
        is_unsafe: function.unsafe_token().is_some(),
        has_type_generics,
        parent_modules,
    }
}

fn container_of(function: &ast::Fn) -> OwnerContainer {
    // A cfg on the owner, an enclosing module or the file (outer or inner
    // attribute) that a plain `cargo test` build may not enable would leave
    // the stub compiled out, so the printed run builds zero tests.
    let gated = function.syntax().ancestors().any(|node| {
        node.children().filter_map(ast::Attr::cast).any(|attr| {
            attribute_test_build_availability(&attr.syntax().text().to_string()) != Some(true)
        })
    });
    if gated {
        return OwnerContainer::Unsupported(
            "owner sits behind a cfg a plain test build may not enable",
        );
    }
    for ancestor in function.syntax().ancestors().skip(1) {
        if ast::Fn::can_cast(ancestor.kind()) {
            return OwnerContainer::Unsupported("owner is nested inside another function body");
        }
        if ast::Trait::can_cast(ancestor.kind()) {
            return OwnerContainer::Unsupported("owner is a trait default method");
        }
        if let Some(item) = ast::Impl::cast(ancestor.clone()) {
            // An impl inside a fn body or a `const _` block sees names a
            // test module cannot reach.
            let at_module_level = item.syntax().parent().is_some_and(|parent| {
                ast::SourceFile::can_cast(parent.kind())
                    || parent
                        .parent()
                        .is_some_and(|grand| ast::Module::can_cast(grand.kind()))
            });
            if !at_module_level {
                return OwnerContainer::Unsupported("owner impl is local to a block");
            }
            let has_generics = item.generic_param_list().is_some_and(|list| {
                list.generic_params()
                    .any(|param| !matches!(param, ast::GenericParam::LifetimeParam(_)))
            });
            if has_generics {
                return OwnerContainer::GenericImpl("owner impl declares type or const generics");
            }
            let Some(ast::Type::PathType(path)) = item.self_ty() else {
                return OwnerContainer::Unsupported("owner impl self type is not a plain path");
            };
            // Lifetime arguments (`Parser<'a>`, `Parser<'_>`) are inferred at
            // the call, so only type or const arguments block the stub.
            let non_lifetime_argument = path
                .syntax()
                .descendants()
                .filter_map(ast::GenericArgList::cast)
                .flat_map(|list| list.generic_args())
                .any(|arg| !matches!(arg, ast::GenericArg::LifetimeArg(_)));
            if non_lifetime_argument {
                return OwnerContainer::GenericImpl(
                    "owner impl self type has type or const generic arguments",
                );
            }
            let self_type = path.syntax().text().to_string();
            return match item.trait_() {
                Some(trait_ty) => OwnerContainer::TraitImpl {
                    self_type,
                    trait_path: match trait_ty {
                        ast::Type::PathType(trait_path) => {
                            Some(trait_path.syntax().text().to_string())
                        }
                        _ => None,
                    },
                },
                None => OwnerContainer::Inherent { self_type },
            };
        }
        if ast::Module::can_cast(ancestor.kind()) || ast::SourceFile::can_cast(ancestor.kind()) {
            return OwnerContainer::Free;
        }
    }
    OwnerContainer::Free
}

/// A top-level comparison `lhs op rhs` in a seam expression, as written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ComparisonFact {
    pub(crate) lhs: String,
    pub(crate) op: String,
    pub(crate) rhs: String,
}

/// Parse `expression` and return its comparison when the whole expression
/// is exactly one `<`, `<=`, `>`, `>=`, `==`, or `!=` (an optional leading
/// `if` is ignored). Compound conditions return `None`.
pub(crate) fn single_comparison(expression: &str) -> Option<ComparisonFact> {
    let trimmed = expression.trim();
    let condition = trimmed.strip_prefix("if ").unwrap_or(trimmed).trim();
    let wrapped = format!("fn ripr_probe() {{ let _ = {condition}; }}");
    let parse = parse_clean_source_file(&wrapped)?;
    let let_stmt = parse
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::LetStmt::cast)?;
    let ast::Expr::BinExpr(binary) = let_stmt.initializer()? else {
        return None;
    };
    let op = binary.op_token()?.text().to_string();
    if !matches!(op.as_str(), "<" | "<=" | ">" | ">=" | "==" | "!=") {
        return None;
    }
    Some(ComparisonFact {
        lhs: binary.lhs()?.syntax().text().to_string(),
        op,
        rhs: binary.rhs()?.syntax().text().to_string(),
    })
}

/// Start offsets of identifier tokens spelled `name`, read from the clean
/// parse so comments and string literals never count. Empty when the file
/// does not parse cleanly.
pub(crate) fn identifier_offsets(source: &str, name: &str) -> Vec<usize> {
    let Some(parse) = parse_clean_source_file(source) else {
        return Vec::new();
    };
    parse
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::IDENT && token.text() == name)
        .map(|token| usize::from(token.text_range().start()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_function_signature_reads_params_and_return() -> Result<(), String> {
        let source = "pub fn price(amount: u32, threshold: u32) -> u32 {\n    if amount >= threshold { amount - 10 } else { amount }\n}\n";
        let offset = source.find(">=").ok_or("fixture has a predicate")?;
        let signature = owner_signature_at(source, offset).ok_or("signature parses")?;
        assert_eq!(signature.name, "price");
        assert_eq!(signature.container, OwnerContainer::Free);
        assert_eq!(signature.receiver, None);
        assert_eq!(
            signature.params,
            vec![
                OwnerParam {
                    name: Some("amount".to_string()),
                    ty: "u32".to_string()
                },
                OwnerParam {
                    name: Some("threshold".to_string()),
                    ty: "u32".to_string()
                },
            ]
        );
        assert_eq!(signature.return_type.as_deref(), Some("u32"));
        assert!(signature.parent_modules.is_empty());
        Ok(())
    }

    #[test]
    fn method_signature_reads_receiver_and_inherent_self_type() -> Result<(), String> {
        let source = "mod units {\n    pub struct ByteSize(u64);\n    impl ByteSize {\n        pub fn as_whole_units(&self, unit: u64) -> u64 {\n            if unit == 0 { 0 } else { self.0 / unit }\n        }\n    }\n}\n";
        let offset = source.find("unit == 0").ok_or("fixture has a predicate")?;
        let signature = owner_signature_at(source, offset).ok_or("signature parses")?;
        assert_eq!(signature.name, "as_whole_units");
        assert_eq!(
            signature.container,
            OwnerContainer::Inherent {
                self_type: "ByteSize".to_string()
            }
        );
        assert_eq!(signature.receiver, Some(OwnerReceiver::Ref));
        assert_eq!(signature.parent_modules, vec!["units".to_string()]);
        Ok(())
    }

    #[test]
    fn trait_default_and_generic_owners_are_unsupported() -> Result<(), String> {
        let trait_source =
            "trait T {\n    fn f(&self, x: u8) -> u8 { if x > 3 { 1 } else { 0 } }\n}\n";
        let offset = trait_source.find("x > 3").ok_or("predicate")?;
        let signature = owner_signature_at(trait_source, offset).ok_or("parses")?;
        assert!(matches!(
            signature.container,
            OwnerContainer::Unsupported(_)
        ));

        let generic_source =
            "fn g<T: Into<u8>>(x: T) -> u8 { let v = x.into(); if v > 3 { 1 } else { 0 } }\n";
        let offset = generic_source.find("v > 3").ok_or("predicate")?;
        let signature = owner_signature_at(generic_source, offset).ok_or("parses")?;
        assert!(signature.has_type_generics);
        Ok(())
    }

    #[test]
    fn lifetime_only_impls_are_plain_and_type_generic_impls_are_generic() -> Result<(), String> {
        let container = |source: &str| -> Result<OwnerContainer, String> {
            let offset = source.find("n > 1").ok_or("predicate")?;
            Ok(owner_signature_at(source, offset)
                .ok_or("parses")?
                .container)
        };
        let method = "fn f(&mut self, n: u8) -> u8 { if n > 1 { 1 } else { 0 } }";
        assert_eq!(
            container(&format!("impl<'a> Parser<'a> {{ {method} }}"))?,
            OwnerContainer::Inherent {
                self_type: "Parser<'a>".to_string()
            }
        );
        assert_eq!(
            container(&format!("impl Parser<'_> {{ {method} }}"))?,
            OwnerContainer::Inherent {
                self_type: "Parser<'_>".to_string()
            }
        );
        assert_eq!(
            container(&format!(
                "impl<'a> std::fmt::Display for P<'a> {{ {method} }}"
            ))?,
            OwnerContainer::TraitImpl {
                self_type: "P<'a>".to_string(),
                trait_path: Some("std::fmt::Display".to_string()),
            }
        );
        for generic in ["impl<T> W<T>", "impl W<u8>", "impl<const N: usize> A<N>"] {
            assert!(
                matches!(
                    container(&format!("{generic} {{ {method} }}"))?,
                    OwnerContainer::GenericImpl(_)
                ),
                "{generic}"
            );
        }
        Ok(())
    }

    #[test]
    fn field_init_is_returned_only_for_the_returned_literal() {
        let at = |source: &str, needle: &str| {
            source
                .find(needle)
                .and_then(|offset| field_init_is_returned(source, offset))
        };
        let tail = "fn f(n: u8) -> S { S { a: n + 1, b: 0 } }";
        assert_eq!(at(tail, "a: n + 1"), Some(true));
        let returned =
            "fn f(n: u8) -> S { if n > 9 { return (S { a: n, b: 1 }); } S { a: 0, b: 0 } }";
        assert_eq!(at(returned, "a: n, b: 1"), Some(true));
        let nested_block = "fn f(n: u8) -> S { unsafe { S { a: n, b: 2 } } }";
        assert_eq!(at(nested_block, "a: n, b: 2"), Some(true));
        let bound = "fn f(n: u8) -> u8 { let s = S { a: n, b: 3 }; s.a }";
        assert_eq!(at(bound, "a: n, b: 3"), Some(false));
        let wrapped = "fn f(n: u8) -> Option<S> { Some(S { a: n, b: 4 }) }";
        assert_eq!(at(wrapped, "a: n, b: 4"), Some(false));
        let inner = "fn f(n: u8) -> O { O { s: S { a: n, b: 5 } } }";
        assert_eq!(at(inner, "a: n, b: 5"), Some(false));
        let closure = "fn f(n: u8) -> u8 { let g = || { return S { a: n, b: 6 }; }; 0 }";
        assert_eq!(at(closure, "a: n, b: 6"), Some(false));
        let async_block = "fn f(n: u8) -> u8 { let g = async { return S { a: n, b: 8 }; }; 0 }";
        assert_eq!(at(async_block, "a: n, b: 8"), Some(false));
        let branch = "fn f(n: u8) -> S { if n > 1 { S { a: n, b: 7 } } else { S { a: 0, b: 0 } } }";
        assert_eq!(at(branch, "a: n, b: 7"), Some(false));
        assert_eq!(at(tail, "fn f"), None);
    }

    #[test]
    fn owner_fn_line_span_finds_the_innermost_function() {
        let source =
            "fn a() {\n    1;\n}\n\nfn b(x: u8) -> u8 {\n    if x > 1 { 2 } else { 3 }\n}\n";
        assert_eq!(owner_fn_line_span(source, 6), Some((5, 7)));
        assert_eq!(owner_fn_line_span(source, 2), Some((1, 3)));
        assert_eq!(owner_fn_line_span(source, 4), None);
    }

    #[test]
    fn single_comparison_accepts_one_operator_and_refuses_compounds() {
        assert_eq!(
            single_comparison("amount >= threshold"),
            Some(ComparisonFact {
                lhs: "amount".to_string(),
                op: ">=".to_string(),
                rhs: "threshold".to_string(),
            })
        );
        assert_eq!(
            single_comparison("if x < 10").map(|fact| fact.rhs),
            Some("10".to_string())
        );
        assert_eq!(single_comparison("a > 1 && b < 2"), None);
        assert_eq!(single_comparison("is_ready(x)"), None);
    }

    #[test]
    fn local_type_traits_count_partial_eq_only_against_the_type_itself() {
        let source = "#[derive(Debug, Clone)]
pub struct Seen(u8);
impl PartialEq<u8> for Seen {
    fn eq(&self, other: &u8) -> bool { self.0 == *other }
}
#[derive(Debug)]
pub struct Own(u8);
impl std::cmp::PartialEq<Self> for Own {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
";
        let seen = local_type_traits(source, "Seen").unwrap_or_default();
        assert!(seen.iter().any(|t| t == "Debug"), "{seen:?}");
        assert!(seen.iter().any(|t| t == "PartialEq<u8>"), "{seen:?}");
        assert!(!seen.iter().any(|t| t == "PartialEq"), "{seen:?}");
        let own = local_type_traits(source, "Own").unwrap_or_default();
        assert!(own.iter().any(|t| t == "PartialEq"), "{own:?}");
        assert_eq!(local_type_traits(source, "Missing"), None);
        let gated = "#[derive(Debug)]
pub struct Out(u8);
#[cfg(feature = \"cmp\")]
impl PartialEq for Out {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
";
        let out = local_type_traits(gated, "Out").unwrap_or_default();
        assert!(!out.iter().any(|t| t == "PartialEq"), "{out:?}");
        let inner_gated = "#[derive(Debug)]
pub struct Out(u8);
mod cmp {
    #![cfg(feature = \"cmp\")]
    use super::Out;
    impl PartialEq for Out {
        fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
    }
}
";
        let out = local_type_traits(inner_gated, "Out").unwrap_or_default();
        assert!(!out.iter().any(|t| t == "PartialEq"), "{out:?}");
    }

    #[test]
    fn imports_with_std_names_shadow_them() {
        let shadows = |source: &str, name: &str| shadows_type_name(source, name);
        assert!(shadows("use crate::time::Duration;", "Duration"));
        assert!(shadows("use crate::time::{Instant, Duration};", "Duration"));
        assert!(shadows("use crate::Opaque as String;", "String"));
        assert!(shadows("use crate::prelude::*;", "String"));
        assert!(shadows("pub struct Duration(u64);", "Duration"));
        assert!(shadows("use std::sync::Mutex as Vec;", "Vec"));
        assert!(!shadows("use std::time::Duration;", "Duration"));
        assert!(!shadows("use std::collections::*;", "HashMap"));
        assert!(!shadows("use core::{cmp::Ordering, fmt};", "Ordering"));
        assert!(shadows("use super::*;", "String"));
        assert!(shadows("use self::types::*;", "String"));
        assert!(shadows("use crate::t::Duration::{self};", "Duration"));
        assert!(!shadows("use crate::Opaque as _;", "Opaque"));
        assert!(!shadows("use crate::time::Instant;", "Duration"));
        assert!(!shadows(
            "#[cfg(test)]\nmod tests {\n    use crate::time::Duration;\n}",
            "Duration"
        ));
    }
}

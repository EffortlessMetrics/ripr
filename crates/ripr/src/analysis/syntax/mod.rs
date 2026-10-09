mod adapter;
mod error_return;
pub(crate) use error_return::guarded_opaque_error_transition;
pub(crate) mod fn_signature;
pub(crate) mod lexical;
mod module_tree;
mod nesting;
mod owner_pin;
pub(crate) use owner_pin::{
    OwnerPinAssertions, empty_macro_binding_ambiguities, local_empty_macro_names,
    macro_binding_scan, owner_pin_assertions, trusted_macro_binding_ambiguities,
};
pub(crate) mod ra;

pub(crate) use adapter::ChangedOwnerSpan;
pub use adapter::{LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RustSyntaxAdapter, TextRange};
#[cfg(test)]
pub use adapter::SyntaxNodeFact;
pub(crate) use module_tree::{RustModuleTreeEdge, RustModuleTreeScan, rust_module_tree_scan};
pub(crate) use nesting::{non_code_token_end, parse_clean_source_file, rust_nesting_refusal};
pub(crate) use ra::parser_oracles_for_function;
#[cfg(test)]
pub(crate) use ra::production_owner_module_path;
pub(crate) use ra::rust_include_directives;
pub(crate) use ra::{
    GovernedCfgTestModule, ModuleItemScopes, fn_name_binding_count, governed_cfg_test_modules,
    inline_unit_module_layout, module_item_scopes,
};

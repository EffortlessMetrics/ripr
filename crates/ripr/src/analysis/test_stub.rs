//! Rust test stubs: from a static gap to a test the developer can run.
//!
//! A stub is a compiling `#[test]` placed where the owner can be called: the
//! owner file's governed inline `#[cfg(test)]` module, a new inline module at
//! the end of a top-level owner file that has none, or the producer-admitted
//! integration file. It calls the owner with its real receiver and argument
//! shape, fills a boundary input when the seam's own comparison names one,
//! and leaves every value ripr cannot know as a labelled `todo!()`.
//!
//! The expected value is never invented. A value-seam stub always stops at
//! `todo!("ripr: …")` after exercising the call, so it fails until the
//! developer writes the value the behavior should produce. Static evidence
//! does not decide that value; the stub only makes writing it one step.
//!
//! This is suggestion text for a person or agent to apply. It is not a
//! repair-cage authorization; edit admission stays with `edit_cage`.

use super::ClassifiedSeam;
use super::new_test_target::{NewTestKind, NewTestTargetProposal};
use super::repair_route::{
    RepairTargetSelection, cross_language_test_target_unresolved, repair_packet_eligibility,
};
use super::seams::{RepoSeam, RequiredDiscriminator, SeamKind};
use super::syntax::fn_signature::{
    OwnerContainer, OwnerParam, OwnerReceiver, OwnerSignature, owner_signature_at,
    single_comparison,
};
use super::syntax::{GovernedCfgTestModule, governed_cfg_test_modules};
use std::path::{Path, PathBuf};

/// Where the stub text goes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TestStubPlacement {
    /// Insert `text` at `offset` (just before the closing brace) of the
    /// existing inline `#[cfg(test)] mod module_name` in `file`.
    ExistingInlineModule {
        file: PathBuf,
        module_name: String,
        offset: usize,
    },
    /// Insert `text` (a whole `#[cfg(test)] mod` item) at `offset`, the end
    /// of `file`.
    NewInlineModule { file: PathBuf, offset: usize },
    /// Create `file` with `text` as its whole content.
    NewIntegrationFile { file: PathBuf },
}

impl TestStubPlacement {
    pub(crate) fn file(&self) -> &Path {
        match self {
            Self::ExistingInlineModule { file, .. }
            | Self::NewInlineModule { file, .. }
            | Self::NewIntegrationFile { file } => file,
        }
    }

    pub(crate) fn kind_str(&self) -> &'static str {
        match self {
            Self::ExistingInlineModule { .. } => "existing_inline_module",
            Self::NewInlineModule { .. } => "new_inline_module",
            Self::NewIntegrationFile { .. } => "new_integration_file",
        }
    }

    pub(crate) fn offset(&self) -> Option<usize> {
        match self {
            Self::ExistingInlineModule { offset, .. } | Self::NewInlineModule { offset, .. } => {
                Some(*offset)
            }
            Self::NewIntegrationFile { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RustTestStub {
    pub(crate) placement: TestStubPlacement,
    pub(crate) test_name: String,
    /// Exact text to insert at the placement offset, or the whole new file.
    pub(crate) text: String,
    /// Each `todo!()` the developer still fills, in source order.
    pub(crate) fill_ins: Vec<String>,
    /// Inputs ripr wrote from the seam's own comparison, as `name = value`.
    pub(crate) derived_inputs: Vec<String>,
}

/// Typed reason no stub was produced. Every refusal names what blocked it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TestStubRefusal {
    NotAGap,
    CrossLanguage,
    NotRustSource,
    ObserverRequired,
    FieldTypeUnresolved,
    SourceUnparsed,
    OwnerUnsupported,
    OwnerAsync,
    OwnerUnsafe,
    OwnerGeneric,
    ParameterUnsupported,
    NoReturnValue,
    OpaqueReturn,
    OutOfLineTestModule,
    AmbiguousTestModule,
    OwnerInNestedModule,
    OwnerTraitMethod,
}

impl TestStubRefusal {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotAGap => "not_a_gap",
            Self::CrossLanguage => "cross_language_target",
            Self::NotRustSource => "not_rust_source",
            Self::ObserverRequired => "observer_required",
            Self::FieldTypeUnresolved => "field_type_unresolved",
            Self::SourceUnparsed => "source_unparsed",
            Self::OwnerUnsupported => "owner_unsupported",
            Self::OwnerAsync => "owner_async",
            Self::OwnerUnsafe => "owner_unsafe",
            Self::OwnerGeneric => "owner_generic",
            Self::ParameterUnsupported => "parameter_unsupported",
            Self::NoReturnValue => "no_return_value",
            Self::OpaqueReturn => "opaque_return",
            Self::OutOfLineTestModule => "out_of_line_test_module",
            Self::AmbiguousTestModule => "ambiguous_test_module",
            Self::OwnerInNestedModule => "owner_in_nested_module",
            Self::OwnerTraitMethod => "owner_trait_method",
        }
    }

    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::NotAGap => "the seam is not a reported gap",
            Self::CrossLanguage => "the test target is in another language",
            Self::NotRustSource => "the owner is not a Rust source file",
            Self::ObserverRequired => {
                "the change shows up as a side effect or call, which needs an observer or mock ripr cannot write"
            }
            Self::FieldTypeUnresolved => {
                "the changed field's type is not in the owner signature, so an expected value cannot be typed"
            }
            Self::SourceUnparsed => "the owner file did not parse cleanly",
            Self::OwnerUnsupported => {
                "the owner is a trait default method, nested function, generic impl member, or takes a typed `self` receiver"
            }
            Self::OwnerAsync => "the owner is async and needs a runtime the stub cannot pick",
            Self::OwnerUnsafe => "the owner is unsafe; its preconditions need a person",
            Self::OwnerGeneric => "the owner has type parameters the stub cannot choose",
            Self::ParameterUnsupported => {
                "a parameter or the return type uses `impl Trait`, an unnamed pattern, or a module-relative path the test cannot reach"
            }
            Self::NoReturnValue => "the owner returns no value to assert on",
            Self::OpaqueReturn => "the owner returns `impl Trait`, which cannot be compared",
            Self::OutOfLineTestModule => {
                "the owner's tests live in an out-of-line `mod tests;` file"
            }
            Self::AmbiguousTestModule => "the owner file has more than one inline test module",
            Self::OwnerInNestedModule => {
                "the owner is in a nested module with no inline test module of its own"
            }
            Self::OwnerTraitMethod => {
                "the owner implements a trait method; calling it needs the trait in scope, which the stub does not resolve"
            }
        }
    }
}

/// Stub for one classified seam given the current text of its file.
pub(crate) fn rust_test_stub_for_classified_seam(
    entry: &ClassifiedSeam,
    source: &str,
) -> Result<RustTestStub, TestStubRefusal> {
    if !entry.class.is_headline_eligible() {
        return Err(TestStubRefusal::NotAGap);
    }
    if cross_language_test_target_unresolved(entry) {
        return Err(TestStubRefusal::CrossLanguage);
    }
    // Only the producer's target selection is read here. A stub is suggestion
    // text, so it does not wait on the repair-packet flip; that stays with
    // the packet surfaces.
    let integration = match repair_packet_eligibility(entry).readiness.target_selection {
        RepairTargetSelection::Proposed(proposal) if proposal.kind == NewTestKind::Integration => {
            Some(proposal)
        }
        _ => None,
    };
    rust_test_stub(&entry.seam, integration.as_ref(), source)
}

/// Stub for one seam. `integration` is a producer-admitted integration
/// proposal; when present the stub goes into that new file.
pub(crate) fn rust_test_stub(
    seam: &RepoSeam,
    integration: Option<&NewTestTargetProposal>,
    source: &str,
) -> Result<RustTestStub, TestStubRefusal> {
    if seam.file().extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return Err(TestStubRefusal::NotRustSource);
    }
    match seam.kind() {
        SeamKind::SideEffect | SeamKind::CallPresence => {
            return Err(TestStubRefusal::ObserverRequired);
        }
        SeamKind::FieldConstruction => return Err(TestStubRefusal::FieldTypeUnresolved),
        SeamKind::PredicateBoundary
        | SeamKind::ReturnValue
        | SeamKind::MatchArm
        | SeamKind::ErrorVariant => {}
    }
    let signature =
        owner_signature_at(source, seam.byte_offset()).ok_or(TestStubRefusal::SourceUnparsed)?;
    check_signature(&signature)?;

    let integration_crate = integration
        .and_then(|proposal| proposal.owner.split("::").next())
        .filter(|name| !name.is_empty());
    let (placement, indent, existing_source) = match (integration, integration_crate) {
        (Some(proposal), Some(_)) if matches!(signature.container, OwnerContainer::Free) => (
            TestStubPlacement::NewIntegrationFile {
                file: proposal.file.clone(),
            },
            String::new(),
            None,
        ),
        _ => inline_placement(seam.file(), source, &signature)?,
    };
    let scope_import = match &placement {
        TestStubPlacement::NewIntegrationFile { .. } => {
            format!("use {}::*;", integration_crate.unwrap_or("crate"))
        }
        _ => "use super::*;".to_string(),
    };

    let base_name = format!(
        "{}_{}",
        snake_case(&signature.name),
        name_suffix(seam.kind())
    );
    let test_name = unique_test_name(&base_name, existing_source.unwrap_or(source));
    let path_scope = match &placement {
        TestStubPlacement::NewIntegrationFile { .. } => {
            PathScope::Integration(integration_crate.unwrap_or("crate"))
        }
        _ => PathScope::ChildModule,
    };
    let body = stub_body(seam, &signature, &scope_import, source, path_scope)?;
    let test_fn = render_test_fn(seam, &signature, &test_name, &body, &indent);

    let text = match &placement {
        TestStubPlacement::ExistingInlineModule { .. } => format!("\n{test_fn}"),
        TestStubPlacement::NewInlineModule { .. } => {
            let separator = if source.ends_with('\n') { "\n" } else { "\n\n" };
            format!("{separator}#[cfg(test)]\nmod ripr_tests {{\n{test_fn}}}\n")
        }
        TestStubPlacement::NewIntegrationFile { .. } => test_fn,
    };
    Ok(RustTestStub {
        placement,
        test_name,
        text,
        fill_ins: body.fill_ins,
        derived_inputs: body.derived_inputs,
    })
}

fn check_signature(signature: &OwnerSignature) -> Result<(), TestStubRefusal> {
    if matches!(signature.container, OwnerContainer::Unsupported(_))
        || signature.receiver == Some(OwnerReceiver::Typed)
    {
        return Err(TestStubRefusal::OwnerUnsupported);
    }
    if matches!(signature.container, OwnerContainer::TraitImpl { .. }) {
        return Err(TestStubRefusal::OwnerTraitMethod);
    }
    if signature.is_async {
        return Err(TestStubRefusal::OwnerAsync);
    }
    if signature.is_unsafe {
        return Err(TestStubRefusal::OwnerUnsafe);
    }
    if signature.has_type_generics {
        return Err(TestStubRefusal::OwnerGeneric);
    }
    if signature
        .params
        .iter()
        .any(|param| param.ty.contains("impl ") || param.ty.trim().is_empty())
    {
        return Err(TestStubRefusal::ParameterUnsupported);
    }
    Ok(())
}

/// Inline placement in the owner file: the unique governed inline test
/// module that is a child of the owner's module, or a new module at the end
/// of a top-level owner file that has no test module at all.
fn inline_placement<'a>(
    file: &Path,
    source: &'a str,
    signature: &OwnerSignature,
) -> Result<(TestStubPlacement, String, Option<&'a str>), TestStubRefusal> {
    let modules = governed_cfg_test_modules(source).ok_or(TestStubRefusal::SourceUnparsed)?;
    let siblings = modules
        .iter()
        .filter(|module| module.parent_modules == signature.parent_modules)
        .collect::<Vec<_>>();
    let inline = siblings
        .iter()
        .filter(|module| module.is_inline)
        .copied()
        .collect::<Vec<&GovernedCfgTestModule>>();
    if siblings.iter().any(|module| !module.is_inline) {
        return Err(TestStubRefusal::OutOfLineTestModule);
    }
    match inline.as_slice() {
        [module] => {
            let offset = module
                .close_brace_start
                .ok_or(TestStubRefusal::AmbiguousTestModule)?;
            let module_indent = line_indent(source, module.item_start);
            let module_body = module
                .body_start
                .and_then(|start| source.get(start..offset));
            Ok((
                TestStubPlacement::ExistingInlineModule {
                    file: file.to_path_buf(),
                    module_name: module.name.clone(),
                    offset,
                },
                format!("{module_indent}    "),
                module_body,
            ))
        }
        [] if signature.parent_modules.is_empty() => Ok((
            TestStubPlacement::NewInlineModule {
                file: file.to_path_buf(),
                offset: source.len(),
            },
            "    ".to_string(),
            Some(""),
        )),
        [] => Err(TestStubRefusal::OwnerInNestedModule),
        _ => Err(TestStubRefusal::AmbiguousTestModule),
    }
}

struct StubBody {
    lines: Vec<String>,
    fill_ins: Vec<String>,
    derived_inputs: Vec<String>,
}

/// Where the test sits relative to the owner's module, which decides how
/// module-relative paths in the signature are spelled from the test.
#[derive(Clone, Copy)]
enum PathScope<'a> {
    /// An inline test module one level below the owner's module.
    ChildModule,
    /// A `tests/` file that reaches the crate by this name.
    Integration(&'a str),
}

fn stub_body(
    seam: &RepoSeam,
    signature: &OwnerSignature,
    scope_import: &str,
    source: &str,
    path_scope: PathScope<'_>,
) -> Result<StubBody, TestStubRefusal> {
    // `Self` is substituted with the impl type as the owner spells it, and
    // each completed type is respelled for the test once; respelling the
    // impl type first would rebase it twice (`self::S` -> `super::super::S`).
    let owner_self = match &signature.container {
        OwnerContainer::Inherent { self_type } | OwnerContainer::TraitImpl { self_type } => {
            Some(self_type.as_str())
        }
        OwnerContainer::Free | OwnerContainer::Unsupported(_) => None,
    };
    let self_type = owner_self
        .map(|ty| rebase_paths(ty, path_scope).ok_or(TestStubRefusal::ParameterUnsupported))
        .transpose()?;
    let self_type = self_type.as_deref();
    let return_type = signature
        .return_type
        .as_deref()
        .map(|ty| concrete_type(ty, owner_self))
        .ok_or(TestStubRefusal::NoReturnValue)?;
    if return_type.contains("impl ") {
        return Err(TestStubRefusal::OpaqueReturn);
    }
    let return_type =
        rebase_paths(&return_type, path_scope).ok_or(TestStubRefusal::ParameterUnsupported)?;
    // A parameter named `subject` would shadow the receiver binding.
    let subject = if signature
        .params
        .iter()
        .any(|param| param.name.as_deref() == Some("subject"))
    {
        "ripr_subject"
    } else {
        "subject"
    };

    let mut lines = vec![scope_import.to_string()];
    let mut fill_ins = Vec::new();
    let mut derived_inputs = Vec::new();
    let mut boundary = boundary_inputs(seam, signature);
    if matches!(path_scope, PathScope::Integration(_)) {
        // A `tests/` file sees only the crate's public items; a named
        // constant may be private, so it stays a fill-in there.
        boundary.retain(|(_, value)| {
            value
                .trim_start_matches('-')
                .starts_with(|c: char| c.is_ascii_digit())
        });
    }

    let receiver = match (signature.receiver, self_type) {
        (Some(receiver), Some(self_type)) => {
            let label = format!(
                "ripr: build the `{self_type}` that `{}` runs on",
                signature.name
            );
            let keyword = if receiver == OwnerReceiver::RefMut {
                "let mut"
            } else {
                "let"
            };
            lines.push(format!(
                "{keyword} {subject}: {self_type} = todo!(\"{label}\");"
            ));
            fill_ins.push(label);
            Some(subject)
        }
        (Some(_), None) => return Err(TestStubRefusal::OwnerUnsupported),
        (None, _) => None,
    };

    let mut arguments = Vec::new();
    for (index, param) in signature.params.iter().enumerate() {
        let binding = param
            .name
            .clone()
            .unwrap_or_else(|| format!("arg{}", index + 1));
        let ty = rebase_paths(&concrete_type(&param.ty, owner_self), path_scope)
            .ok_or(TestStubRefusal::ParameterUnsupported)?;
        let (binding_ty, argument, mutable) = match strip_mut_reference(&ty) {
            Some(inner) => (inner.trim().to_string(), format!("&mut {binding}"), true),
            None => (ty.clone(), binding.clone(), false),
        };
        let keyword = if mutable { "let mut" } else { "let" };
        match boundary.iter().find(|(name, _)| name == &binding) {
            Some((_, value)) => {
                lines.push(format!("{keyword} {binding}: {binding_ty} = {value};"));
                derived_inputs.push(format!("{binding} = {value}"));
            }
            None => {
                let label = format!("ripr: choose `{binding}`{}", input_hint(seam, &binding));
                lines.push(format!(
                    "{keyword} {binding}: {binding_ty} = todo!(\"{}\");",
                    escape(&label)
                ));
                fill_ins.push(label);
            }
        }
        arguments.push(argument);
    }

    let call = match (receiver, self_type) {
        (Some(subject), _) => format!("{subject}.{}({})", signature.name, arguments.join(", ")),
        (None, Some(self_type)) => {
            format!("{self_type}::{}({})", signature.name, arguments.join(", "))
        }
        (None, None) => format!("{}({})", signature.name, arguments.join(", ")),
    };
    lines.push(format!("let actual = {call};"));

    let variant = match seam.required_discriminator() {
        RequiredDiscriminator::ErrorVariant { variant }
            if seam.kind() == SeamKind::ErrorVariant =>
        {
            variant_pattern(variant, owner_self, path_scope)
                .filter(|_| is_result_type(&return_type))
        }
        _ => None,
    };
    match variant {
        Some(pattern) => {
            lines.push(format!(
                "assert!(matches!(actual, Err({pattern})), \"expected Err({})\");",
                escape(&pattern)
            ));
        }
        // A value seam, or an error seam whose variant is not a nameable
        // path: assert the whole value the developer writes.
        None => {
            let hint = if derived_inputs.is_empty() {
                discriminator_hint(seam)
            } else {
                format!(" for {}", derived_inputs.join(", "))
            };
            let traits = value_traits(&return_type, source);
            let label = match traits {
                ValueTraits::Unknown => format!(
                    "ripr: assert that `actual` is the value `{}` should return{hint}",
                    signature.name
                ),
                ValueTraits::PartialEqAndDebug => format!(
                    "ripr: write the value `{}` should return{hint}",
                    signature.name
                ),
            };
            match traits {
                ValueTraits::PartialEqAndDebug => {
                    lines.push(format!(
                        "let expected: {return_type} = todo!(\"{}\");",
                        escape(&label)
                    ));
                    lines.push("assert_eq!(actual, expected);".to_string());
                }
                // `PartialEq` or `Debug` is not visible for the return
                // type, so no generated comparison is sure to compile (a
                // Debug-text comparison also needs `format!`, which no_std
                // crates lack): the assertion itself is the fill-in.
                ValueTraits::Unknown => {
                    lines.push("let _ = &actual;".to_string());
                    lines.push(format!("todo!(\"{}\");", escape(&label)));
                }
            }
            fill_ins.push(label);
        }
    }
    Ok(StubBody {
        lines,
        fill_ins,
        derived_inputs,
    })
}

fn render_test_fn(
    seam: &RepoSeam,
    signature: &OwnerSignature,
    test_name: &str,
    body: &StubBody,
    indent: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{indent}// ripr: discriminate `{}` in `{}` ({}:{}).\n",
        one_line(seam.expression()),
        signature.name,
        seam.file().display().to_string().replace('\\', "/"),
        seam.display_line()
    ));
    out.push_str(&format!("{indent}#[test]\n"));
    if !body.fill_ins.is_empty() {
        out.push_str(&format!(
            "{indent}#[allow(unreachable_code)] // delete with the last todo!()\n"
        ));
    }
    out.push_str(&format!("{indent}fn {test_name}() {{\n"));
    for line in &body.lines {
        out.push_str(&format!("{indent}    {line}\n"));
    }
    out.push_str(&format!("{indent}}}\n"));
    out
}

/// Concrete boundary inputs from a single comparison over integer
/// parameters. `x OP 10` sets `x = 10`; `a OP b` sets both to `100`; `x OP
/// LIMIT` sets `x = LIMIT`. Anything else stays a `todo!()`.
fn boundary_inputs(seam: &RepoSeam, signature: &OwnerSignature) -> Vec<(String, String)> {
    if seam.kind() != SeamKind::PredicateBoundary {
        return Vec::new();
    }
    let Some(comparison) = single_comparison(seam.expression()) else {
        return Vec::new();
    };
    let integer_param = |operand: &str| -> Option<&OwnerParam> {
        signature.params.iter().find(|param| {
            param.name.as_deref() == Some(operand.trim()) && is_integer_type(&param.ty)
        })
    };
    let lhs = comparison.lhs.trim();
    let rhs = comparison.rhs.trim();
    match (integer_param(lhs), integer_param(rhs)) {
        (Some(left), Some(right)) if left.ty.trim() == right.ty.trim() => vec![
            (lhs.to_string(), "100".to_string()),
            (rhs.to_string(), "100".to_string()),
        ],
        (Some(param), None) => boundary_value(rhs, &param.ty)
            .map(|value| vec![(lhs.to_string(), value)])
            .unwrap_or_default(),
        (None, Some(param)) => boundary_value(lhs, &param.ty)
            .map(|value| vec![(rhs.to_string(), value)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn boundary_value(operand: &str, ty: &str) -> Option<String> {
    let signed = ty.trim().starts_with('i');
    let (negative, digits) = match operand.strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, operand),
    };
    if negative && !signed {
        return None;
    }
    if digits.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        let literal = strip_integer_suffix(digits)?;
        if !literal.chars().all(|c| c.is_ascii_digit() || c == '_') {
            return None;
        }
        return Some(if negative {
            format!("-{literal}")
        } else {
            literal
        });
    }
    let is_const = !negative
        && digits
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
        && digits
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    is_const.then(|| digits.to_string())
}

fn strip_integer_suffix(literal: &str) -> Option<String> {
    const SUFFIXES: [&str; 12] = [
        "usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
    ];
    let trimmed = SUFFIXES
        .iter()
        .find_map(|suffix| literal.strip_suffix(suffix))
        .unwrap_or(literal)
        .trim_end_matches('_');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// What the stub can assume about a return type when comparing values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValueTraits {
    /// `assert_eq!` on the value compiles.
    PartialEqAndDebug,
    Unknown,
}

/// Whether every named type in `ty` is known to have `PartialEq` and `Debug`:
/// std types ripr knows implement `PartialEq` and `Debug` when their
/// arguments do, and types defined in the owner file whose derives or
/// impls name the traits. Anything else is `Unknown`, so no generated
/// comparison depends on a trait ripr cannot see.
fn value_traits(ty: &str, source: &str) -> ValueTraits {
    const STD: [&str; 33] = [
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        "f32", "f64", "bool", "char", "str", "String", "Option", "Result", "Vec", "VecDeque",
        "Box", "Rc", "Arc", "Cow", "Ordering", "Duration", "PathBuf", "HashMap", "HashSet",
        "BTreeMap", "BTreeSet",
    ];
    // A qualified path (`io::Error`, `crate::Out`) may name a type other
    // than the same-named one ripr can see, so it is never assumed.
    if ty.contains("::") {
        return ValueTraits::Unknown;
    }
    let chars = ty.char_indices().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let (start, c) = chars[index];
        if !(c.is_alphanumeric() || c == '_') {
            index += 1;
            continue;
        }
        let mut end_index = index;
        while end_index < chars.len()
            && (chars[end_index].1.is_alphanumeric() || chars[end_index].1 == '_')
        {
            end_index += 1;
        }
        let end = chars.get(end_index).map_or(ty.len(), |(at, _)| *at);
        let word = &ty[start..end];
        index = end_index;
        if ty[..start].ends_with('\'')
            || word == "_"
            || word == "mut"
            || word.starts_with(|c: char| c.is_ascii_digit())
        {
            continue;
        }
        if STD.contains(&word) {
            // A local item or import with a std name (`type Result<T> = ..`,
            // `struct Duration`, `use crate::time::Duration`) shadows it; an
            // alias can hide a non-comparable error type. `Result` must also show both type arguments.
            if super::syntax::fn_signature::shadows_type_name(source, word)
                || (word == "Result" && top_level_type_arguments(&ty[end..]) != Some(2))
            {
                return ValueTraits::Unknown;
            }
            continue;
        }
        let traits =
            super::syntax::fn_signature::local_type_traits(source, word).unwrap_or_default();
        let has = |name: &str| traits.iter().any(|item| item == name);
        if !(has("PartialEq") && has("Debug")) {
            return ValueTraits::Unknown;
        }
    }
    ValueTraits::PartialEqAndDebug
}

/// The number of top-level type arguments in the `<..>` list that starts
/// `rest`, or `None` when `rest` does not start with one.
fn top_level_type_arguments(rest: &str) -> Option<usize> {
    let inner = rest.trim_start().strip_prefix('<')?;
    let mut depth = 0usize;
    let mut arguments = 1;
    for c in inner.chars() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' if depth == 0 => return Some(arguments),
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => arguments += 1,
            _ => {}
        }
    }
    None
}

/// Whether `ty` is a `Result` (`Result<..>`, `io::Result<..>`, ...), so an
/// `Err(..)` pattern can match it.
fn is_result_type(ty: &str) -> bool {
    let head = ty.split('<').next().unwrap_or_default().trim();
    head.rsplit("::").next() == Some("Result") && ty.contains('<')
}

/// Respell module-relative paths in `ty` for where the test sits. In a
/// child test module `self::` becomes `super::` and a leading `super::`
/// gains one more; a `tests/` file reaches `crate::` by the crate name and
/// cannot reach `self::` or `super::` at all (`None`).
fn rebase_paths(ty: &str, scope: PathScope<'_>) -> Option<String> {
    let mut out = String::new();
    let mut index = 0;
    while index < ty.len() {
        let rest = &ty[index..];
        let starts_path = ty[..index]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == ':'));
        if starts_path {
            let rewritten = match (scope, rest) {
                (PathScope::ChildModule, _) if rest.starts_with("self::") => {
                    Some(("self::".len(), "super::".to_string()))
                }
                (PathScope::ChildModule, _) if rest.starts_with("super::") => {
                    Some(("super::".len(), "super::super::".to_string()))
                }
                (PathScope::Integration(_), _)
                    if rest.starts_with("self::") || rest.starts_with("super::") =>
                {
                    return None;
                }
                (PathScope::Integration(name), _) if rest.starts_with("crate::") => {
                    Some(("crate::".len(), format!("{name}::")))
                }
                _ => None,
            };
            if let Some((consumed, replacement)) = rewritten {
                out.push_str(&replacement);
                index += consumed;
                continue;
            }
        }
        let c = rest.chars().next()?;
        out.push(c);
        index += c.len_utf8();
    }
    Some(out)
}

fn is_integer_type(ty: &str) -> bool {
    matches!(
        ty.trim(),
        "u8" | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
    )
}

/// Replace `Self` with the impl type and named lifetimes with `'_`, so the
/// type can annotate a `let` inside a test.
fn concrete_type(ty: &str, self_type: Option<&str>) -> String {
    let mut out = String::new();
    let mut chars = ty.trim().chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            let mut name = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    name.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push_str(if name == "static" { "'static" } else { "'_" });
            continue;
        }
        out.push(c);
    }
    match self_type {
        Some(self_type) => replace_word(&out, "Self", self_type),
        None => out,
    }
}

fn replace_word(text: &str, word: &str, replacement: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(index) = rest.find(word) {
        let before = rest[..index].chars().next_back();
        let after = rest[index + word.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        out.push_str(&rest[..index]);
        if boundary(before) && boundary(after) {
            out.push_str(replacement);
        } else {
            out.push_str(word);
        }
        rest = &rest[index + word.len()..];
    }
    out.push_str(rest);
    out
}

/// The referent of a `&mut T`, `&'_ mut T` or `&'a mut T` parameter type.
fn strip_mut_reference(ty: &str) -> Option<&str> {
    let rest = ty.strip_prefix('&')?.trim_start();
    let rest = match rest.strip_prefix('\'') {
        Some(lifetime) => lifetime
            .trim_start_matches(|c: char| c.is_alphanumeric() || c == '_')
            .trim_start(),
        None => rest,
    };
    rest.strip_prefix("mut ").map(str::trim)
}

/// `Error::Variant` (optionally written as `Err(Error::Variant(..))`) as a
/// `matches!` pattern spelled from the test's scope. Anything that is not a
/// path ending in an upper-case segment is not a nameable variant, and a
/// `Self::` path needs the impl type to respell it.
fn variant_pattern(variant: &str, self_type: Option<&str>, scope: PathScope<'_>) -> Option<String> {
    let text = variant.trim();
    let text = text.strip_prefix("Err(").unwrap_or(text);
    let path = text.split(['(', '{', ' ', ')']).next().unwrap_or_default();
    let segments = path.split("::").collect::<Vec<_>>();
    let valid = segments.iter().all(|segment| {
        !segment.is_empty() && segment.chars().all(|c| c.is_alphanumeric() || c == '_')
    }) && segments
        .last()
        .is_some_and(|last| last.starts_with(|c: char| c.is_ascii_uppercase()));
    if !valid {
        return None;
    }
    let path = match (segments.first(), self_type) {
        (Some(&"Self"), Some(self_type)) => replace_word(path, "Self", self_type),
        (Some(&"Self"), None) => return None,
        _ => path.to_string(),
    };
    let path = rebase_paths(&path, scope)?;
    Some(format!("{path} {{ .. }}"))
}

fn input_hint(seam: &RepoSeam, binding: &str) -> String {
    match seam.required_discriminator() {
        RequiredDiscriminator::BoundaryValue { description }
            if description.contains(binding) && !description.trim().is_empty() =>
        {
            format!(" so that {}", one_line(description))
        }
        RequiredDiscriminator::ErrorVariant { variant } if !variant.trim().is_empty() => {
            format!(" so the call returns {}", one_line(variant))
        }
        RequiredDiscriminator::MatchArmTaken { arm } if !arm.trim().is_empty() => {
            format!(" so the `{}` arm runs", one_line(arm))
        }
        _ => String::new(),
    }
}

fn discriminator_hint(seam: &RepoSeam) -> String {
    match seam.required_discriminator() {
        RequiredDiscriminator::BoundaryValue { description }
        | RequiredDiscriminator::ReturnValue { description }
            if !description.trim().is_empty() =>
        {
            format!(" when {}", one_line(description))
        }
        RequiredDiscriminator::MatchArmTaken { arm } if !arm.trim().is_empty() => {
            format!(" when the `{}` arm runs", one_line(arm))
        }
        _ => format!(" for these inputs (`{}`)", one_line(seam.expression())),
    }
}

fn name_suffix(kind: SeamKind) -> &'static str {
    match kind {
        SeamKind::PredicateBoundary => "boundary_discriminator",
        SeamKind::ErrorVariant => "exact_error_variant",
        SeamKind::ReturnValue => "return_value_discriminator",
        SeamKind::FieldConstruction => "field_discriminator",
        SeamKind::SideEffect => "side_effect_observer",
        SeamKind::MatchArm => "match_arm_discriminator",
        SeamKind::CallPresence => "call_presence_observer",
    }
}

fn unique_test_name(base: &str, scope_source: &str) -> String {
    let taken = |name: &str| {
        scope_source
            .match_indices(&format!("fn {name}"))
            .any(|(index, matched)| {
                scope_source[index + matched.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c == '(' || c == '<' || c.is_whitespace())
            })
    };
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|name| !taken(name))
        .unwrap_or_else(|| base.to_string())
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (index, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        }
    }
    out
}

fn line_indent(source: &str, offset: usize) -> String {
    let line_start = source
        .get(..offset)
        .and_then(|prefix| prefix.rfind('\n').map(|index| index + 1))
        .unwrap_or(0);
    source
        .get(line_start..)
        .unwrap_or_default()
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('{', "{{")
        .replace('}', "}}")
}

#[cfg(test)]
mod tests;

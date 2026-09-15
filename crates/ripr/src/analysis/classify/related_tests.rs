use super::super::rust_index::{
    FunctionSummary, RustIndex, TestSummary, extract_identifier_tokens,
};
use crate::analysis::extract::{
    ShadowAuthority, fact_body_defines_callee_fn, fact_body_let_shadow_line,
    mask_comments_and_strings, test_body_defines_callee_fn, test_body_let_shadow_line,
};
use crate::analysis::seam_cache::PathDependencySection;
use crate::analysis::workspace::{PathDependencyAdjacency, PathDependencyGraphStatus};
use crate::domain::{Probe, RelationReason};
use std::collections::BTreeSet;
use std::path::Path;

/// Per-pass workspace dependency context for the #2972 cross-crate admit.
/// Built once per classification pass in the Rust adapter and shared by
/// every probe; `None` (no workspace root, narrower diff passes) disables
/// the admit entirely.
pub(in crate::analysis) struct DependencyEdgeContext<'a> {
    /// Adjacency over the captured path-dependency provenance.
    pub(in crate::analysis) adjacency: &'a PathDependencyAdjacency,
    /// Discovered manifest directory prefixes (`/`-separated, trailing `/`,
    /// including the empty root prefix when the scan root has its own
    /// manifest) — the same inventory authority the #3616 diff-scope
    /// expansion threads through, so both sides of a candidate relation
    /// attribute to their nearest owning manifest instead of a synthesized
    /// package prefix.
    pub(in crate::analysis) manifest_dir_prefixes: &'a [String],
    /// The classified index, for the full source of the test's file: `use`
    /// statements live at module level, outside the test function body the
    /// test summary carries (#3619 review).
    pub(in crate::analysis) index: &'a RustIndex,
}

/// Minimum token length for the `assertions_reference_owner` signal.
/// Tokens shorter than this threshold are too common to safely assert ownership.
const ASSERTION_TOKEN_MIN_LEN: usize = 5;

/// #3714: the converted callee of a wrapper error seam
/// (`callee(..).map_err(..)` whose changed expression carries no parseable
/// variant), for error-shaped probes. A test whose captured calls include
/// this callee reaches the changed behavior through the wrapper even when
/// its name carries no owner or probe-token affinity — the name-anchored
/// weak-signal gate would otherwise leave the seam with no related tests at
/// all. A parseable variant (`Err(Type::Variant)`) means the changed
/// expression IS the variant identity; the pre-existing variant-bound paths
/// own attribution there, so this returns `None`.
fn wrapper_seam_callee(probe: &Probe) -> Option<String> {
    if !matches!(
        probe.family,
        crate::domain::ProbeFamily::ErrorPath | crate::domain::ProbeFamily::ReturnValue
    ) {
        return None;
    }
    if super::text::exact_error_variant(&probe.expression).is_some() {
        return None;
    }
    let trimmed = probe.expression.trim();
    let trimmed = trimmed
        .strip_prefix("return ")
        .unwrap_or(trimmed)
        .trim_start();
    // The conversion applied to the seam's result is the LAST top-level
    // `.map_err`: an argument may carry its own nested conversion whose
    // receiver is not the seam callee (#3714 round-1 review, devin ik).
    // Harmless outer grouping is stripped by the shared scanner, so the
    // conversion index refers to the same string the chain is cut from
    // (#3714 round-2 review, devin hGdAZ).
    let unwrapped = super::reveal::without_harmless_outer_groups(trimmed);
    let conversion = super::reveal::last_top_level_map_err_dot(unwrapped)?;
    // The chain itself may be parenthesized (`(try_x(raw)).map_err(..)`);
    // strip its own outer grouping before the head extraction
    // (#3714 round-2 review, devin hGdAZ).
    let chain = super::reveal::without_harmless_outer_groups(unwrapped[..conversion].trim_end());
    // The converted callee is the identifier of the LAST top-level call in
    // the chain: an identifier at bracket depth zero immediately followed by
    // `(`. Chains (`self.client().try_x(..)`) resolve to the innermost hop,
    // statement labels and braced blocks before the call are skipped, and a
    // bare value receiver (`value.map_err(..)`, no call) yields no candidate
    // and fails closed.
    let bytes = chain.as_bytes();
    let mut depth = 0isize;
    let mut best: Option<&str> = None;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => {
                if depth == 0 {
                    // Skip balanced turbofish/generic argument lists and
                    // `::` separators ending before the paren
                    // (`parse::<View<'_>>(raw)` -> `parse`,
                    // `try_parse_summary::<'a>(raw)` -> `try_parse_summary`).
                    let mut name_end = index;
                    loop {
                        if name_end > 0 && bytes[name_end - 1] == b'>' {
                            let mut angle = 0isize;
                            let mut scan = name_end;
                            let mut matched = false;
                            while scan > 0 {
                                scan -= 1;
                                match bytes[scan] {
                                    b'>' => angle += 1,
                                    b'<' => {
                                        angle -= 1;
                                        if angle == 0 {
                                            name_end = scan;
                                            matched = true;
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if matched {
                                continue;
                            }
                        }
                        if name_end >= 2 && &bytes[name_end - 2..name_end] == b"::" {
                            name_end -= 2;
                            continue;
                        }
                        break;
                    }
                    if let Some(name_start) = chain[..name_end]
                        .char_indices()
                        .rev()
                        .take_while(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '_')
                        .map(|(position, _)| position)
                        .last()
                    {
                        let name = &chain[name_start..name_end];
                        if name
                            .chars()
                            .next()
                            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
                        {
                            best = Some(name.rsplit("::").next().unwrap_or(name));
                        }
                    }
                }
                depth += 1;
            }
            b')' => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    best.map(str::to_string)
}

pub(in crate::analysis) fn find_related_tests<'a>(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    index: &'a RustIndex,
    workspace_complete: bool,
    helper_chain: Option<&super::helper_transfer::HelperChain>,
    dependency_edges: Option<&DependencyEdgeContext<'_>>,
) -> Vec<(&'a TestSummary, RelationReason)> {
    let mut related: Vec<(&TestSummary, RelationReason)> = Vec::new();
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    // #3714: the converted callee of a wrapper error seam, if any. A test
    // whose captured calls name it relates weakly (SeamCalleeCall) even when
    // no name-affinity signal fires.
    let seam_callee = wrapper_seam_callee(probe);
    let file_name = normalized_file_stem(&probe.location.file);
    let owner_package_prefix = owner_fn.and_then(|owner| package_prefix(&owner.file));

    // #2971 ambiguity rule: a call to `owner_name` can bypass the
    // package-prefix guard only when (a) the owner name is unique in the
    // workspace AND (b) the index spans the whole workspace. When multiple
    // crates define functions with the same name, a bare call in crate_b
    // calling `score()` cannot be attributed to crate_a::score — that is the
    // token-coincidence false-exposed family.
    //
    // (b) is not a property of the analysis mode. The diff path indexes only
    // the changed files whenever `include_unchanged_tests` is false — Deep and
    // Ready included — and only the changed packages under Draft/Fast, so a
    // same-named function in an unindexed file is absent from the count and
    // the name would falsely appear unique. The caller therefore derives
    // `workspace_complete` from the file selection that actually built the
    // index, and anything narrower fails closed. This follows the
    // owner_shape.rs precedent of scanning index.functions for same-name
    // collision.
    //
    // #2972: the same scan also records the nearest-manifest identity of
    // every same-named definition, so the dependency-edge admit below can
    // refuse when a same-named definition shadows the owner in (or near)
    // the calling test's own package. Files that no discovered manifest
    // directory covers (`None`) are tracked separately: an unplaceable
    // same-named definition can never be refuted as the call target. The
    // whole scan is skipped when the workspace index is incomplete — the
    // uniqueness bypass stays off and the admit requires completeness, so
    // the counts would be unused.
    let mut same_name_function_count = 0usize;
    let mut same_name_definition_manifests: BTreeSet<String> = BTreeSet::new();
    let mut same_name_unattributed_definition = false;
    if workspace_complete && !owner_name.is_empty() {
        for function in &index.functions {
            if function.name == owner_name {
                same_name_function_count += 1;
                match dependency_edges.and_then(|context| {
                    nearest_manifest_identity(context.manifest_dir_prefixes, &function.file)
                }) {
                    Some(manifest) => {
                        same_name_definition_manifests.insert(manifest);
                    }
                    None => same_name_unattributed_definition = true,
                }
            }
        }
    }
    let owner_name_is_unique = workspace_complete && same_name_function_count == 1;

    // For module-level struct/field probes (owner_fn is None) derive the package
    // prefix from the probe's source file so cross-crate spurious matches are
    // still filtered out.
    let struct_package_prefix = if owner_fn.is_none() {
        package_prefix(&probe.location.file)
    } else {
        None
    };

    // #3235: an absolute owner/probe path without a `crates/` segment carries
    // no derivable package scope, and the old behavior treated that as "no
    // scoping needed", silently disabling the package guard. When the
    // candidate test's path is *also* absolute, a weak match cannot be scoped
    // at all, so fail closed: only the strong uniquely-owned call path (#2971)
    // may admit. A *relative* test file keeps current behavior — the
    // mixed-form companion flow (absolute probe, relative test files) is
    // production-pinned. A future producer that supplies absolute test paths
    // must carry an explicit relativization authority instead of relying on
    // ambient prefix stripping.
    let owner_scope_unscopable = owner_fn
        .is_some_and(|owner| owner_package_prefix.is_none() && path_is_absolute_form(&owner.file));
    let struct_scope_unscopable = owner_fn.is_none()
        && struct_package_prefix.is_none()
        && path_is_absolute_form(&probe.location.file);

    // Probe tokens that are long enough to assert ownership via assertion text.
    let long_probe_tokens: Vec<&str> = probe_tokens
        .iter()
        .filter(|t| t.len() >= ASSERTION_TOKEN_MIN_LEN)
        .map(String::as_str)
        .collect();

    for test in &index.tests {
        // Compute calls_owner BEFORE the package-prefix guard so a cross-crate
        // test that genuinely calls a uniquely-named owner is not filtered out
        // before the strong signal can save it.
        let calls_owner = !owner_name.is_empty()
            && (test.calls.iter().any(|call| call.name == owner_name)
                || body_contains_owner_call(&test.body, owner_name));
        // #3714: captured `calls` facts only — the same authority as
        // `calls_owner`. Never overrides owner attribution. A same-named
        // local definition or binding in the test body defeats the admit
        // (round-1 review, devin hC). #3714 round-2 (devin hIL0i): a `let`
        // binding only shadows uses at or after its own line, so the defeat
        // is positional for `let` shadows; `fn <callee>` items are hoisted
        // and defeat every call in the body. #3728 round-3 (coderabbit,
        // devin): comments and string contents are masked before the scans
        // so mentioned shapes never defeat real calls, and the shadow scans
        // run only after a callee-named captured call exists.
        // #3727 Slice A: the shadow AUTHORITY follows the file's producer
        // flag — parser-backed files decide from the test's parser-produced
        // fact fields; fallback files (or tests whose file is absent from
        // the index) run the byte-level lexical scanners byte-identically.
        // THE FLAG — not set emptiness — is the discriminator: a
        // parser-backed file can legitimately contain zero bindings, and
        // empty fact sets on parser-backed files are real "no shadow"
        // results.
        let test_file_is_parser_backed = index
            .files
            .get(&test.file)
            .is_some_and(|file_facts| !file_facts.used_lexical_fallback);
        let shadow_authority = if test_file_is_parser_backed {
            ShadowAuthority::ParserBodyFacts {
                nested_fn_names: &test.nested_fn_names,
                let_bindings: &test.let_bindings,
            }
        } else {
            ShadowAuthority::LexicalMaskedBody
        };
        let calls_seam_callee = !calls_owner
            && seam_callee.as_deref().is_some_and(|callee| {
                // Lazy gate (#3728 round-3 review, gemini): the shadow scans
                // run only after a callee-named captured call exists.
                if !test.calls.iter().any(|call| call.name == callee) {
                    return false;
                }
                // Scan the comment-and-string-masked body via the shared
                // `mask_comments_and_strings` (char literals included) so a
                // same-named definition inside a comment, string, or char
                // literal can neither phantom-shadow a real call nor be
                // erased by a stray quote byte (#3728 round-3 review,
                // coderabbit + devin round-4). Masking preserves byte
                // layout, so body-relative shadow lines still align with
                // `test.start_line`-relative call lines. The parser-backed
                // authority ignores the masked body: its facts came from
                // real syntax, so comments and strings never became facts.
                let body = mask_comments_and_strings(&test.body);
                let (fn_shadows, let_shadow_line) = match shadow_authority {
                    ShadowAuthority::ParserBodyFacts {
                        nested_fn_names,
                        let_bindings,
                    } => (
                        fact_body_defines_callee_fn(nested_fn_names, callee),
                        fact_body_let_shadow_line(let_bindings, callee),
                    ),
                    ShadowAuthority::LexicalMaskedBody => (
                        test_body_defines_callee_fn(&body, callee),
                        test_body_let_shadow_line(&body, callee),
                    ),
                };
                test.calls.iter().any(|call| {
                    call.name == callee
                        // Defeat only when a shadow PRECEDES the call: fn
                        // items hoist (whole body), `let` bindings are
                        // positional (shadow at or before the call line;
                        // `CallFact` carries no column, so a same-line
                        // initializer stays conservatively defeated).
                        && !(fn_shadows
                            || let_shadow_line.is_some_and(|shadow_line| {
                                test.start_line + shadow_line <= call.line
                            }))
                })
            });
        // #3296 review: the test-to-entry edge must also be a direct
        // free-function call site — a method or qualified call sharing
        // the entry's terminal name is not callee identity.
        let calls_helper_entry = helper_chain.is_some_and(|chain| {
            test.calls.iter().any(|call| {
                chain.hops.iter().any(|hop| {
                    hop.caller.name == call.name
                        && super::helper_transfer::is_direct_call_site(&call.text, &hop.caller.name)
                })
            })
        });

        // #2971: Only apply the package-prefix guard to weak signals — tests
        // that do not directly call the owner, OR tests that call a bare name
        // that is ambiguous across crates. A cross-crate test that calls a
        // uniquely-named owner is genuinely related and must not be suppressed.
        //
        // #2972: the remaining way an ambiguous-name cross-crate call stays
        // related is GRAPH evidence — one captured, callable forward
        // path-dependency edge from the calling test's package to the owner's
        // package (nearest-manifest attributed on both sides), with no local
        // binding or shadowing definition competing for the bare call. The
        // `calls_owner` signal stays mandatory: an edge never admits on name
        // similarity alone, and no other weak signal can ride through it.
        if let Some(prefix) = &owner_package_prefix
            && !normalize_path(&test.file).starts_with(prefix)
            && !(calls_owner && owner_name_is_unique)
            && !calls_helper_entry
            && !(calls_owner
                && workspace_complete
                && owner_fn.is_some_and(|owner| {
                    dependency_edges.is_some_and(|context| {
                        dependency_edge_admits_owner_call(
                            context,
                            owner_name,
                            &owner.file,
                            test,
                            &same_name_definition_manifests,
                            same_name_unattributed_definition,
                        )
                    })
                }))
        {
            continue;
        }
        // Apply the same package-prefix guard for the owner_fn=None path.
        if let Some(prefix) = &struct_package_prefix
            && !normalize_path(&test.file).starts_with(prefix)
        {
            continue;
        }
        // #3235: unscopable-absolute owner against an absolute test file —
        // no package comparison is possible, so weak matches fail closed.
        if owner_scope_unscopable
            && path_is_absolute_form(&test.file)
            && !(calls_owner && owner_name_is_unique)
        {
            continue;
        }
        if struct_scope_unscopable && path_is_absolute_form(&test.file) {
            continue;
        }

        // Signal: a test's assertion observed_tokens intersect the probe's
        // long-enough identifier tokens. Only fires when the probe has no named
        // function owner (i.e. it is a module-level struct/field), so it cannot
        // override the `calls_owner` path for function-bodied probes.
        let assertions_reference_owner = owner_fn.is_none()
            && !long_probe_tokens.is_empty()
            && test.assertions.iter().any(|oracle| {
                oracle.observed_tokens.iter().any(|obs_tok| {
                    obs_tok.len() >= ASSERTION_TOKEN_MIN_LEN
                        && long_probe_tokens.contains(&obs_tok.as_str())
                })
            });

        let test_name = test.name.to_ascii_lowercase();
        let owner_name_lc = owner_name.to_ascii_lowercase();
        let same_test_file = same_test_file(&probe.location.file, &test.file);
        // Keep the broad path-token association available to callers, but do
        // not publish it as file identity.  A short source stem such as
        // `config` can occur in unrelated test paths (for example
        // `reconfigure.rs`).
        let file_path_token_matches =
            !file_name.is_empty() && normalize_path(&test.file).contains(&file_name);
        let owner_name_in_test = !owner_name_lc.is_empty() && test_name.contains(&owner_name_lc);
        let token_in_test_name = probe_tokens
            .iter()
            .any(|token| token.len() > 2 && test_name.contains(&token.to_ascii_lowercase()));
        let same_file_or_named =
            same_test_file || file_path_token_matches || owner_name_in_test || token_in_test_name;

        // #3296 helper transfer: a test that calls a resolved hop's
        // caller directly reaches the owner through the bounded chain
        // (same authority the activation rows consume). The chain is
        // resolved once per probe above the test loop (#3296 review
        // M1 — the per-test re-resolution was O(tests x functions)).
        let helper_chain_reaches = !calls_owner
            && !assertions_reference_owner
            && !same_file_or_named
            && calls_helper_entry;

        if !calls_owner
            && !assertions_reference_owner
            && !same_file_or_named
            && !helper_chain_reaches
            && !calls_seam_callee
        {
            continue;
        }

        // Determine the single highest-priority reason for the match so the
        // emitted `RelatedTest` can carry `relation_reason` /
        // `relation_confidence` tags for consumer filtering.
        let reason = if calls_owner {
            // The test directly calls or mentions the changed owner function.
            RelationReason::DirectOwnerCall
        } else if helper_chain_reaches {
            // The test calls a function that reaches the owner through
            // the bounded helper-transfer chain (#3296).
            RelationReason::HelperOwnerCall
        } else if assertions_reference_owner {
            // Struct/field probe whose tokens appear in assertion observed_tokens.
            RelationReason::AssertionTargetAffinity
        } else if owner_name_in_test {
            // Test name contains the owner function name.
            RelationReason::OwnerNamedTest
        } else if same_test_file {
            // Test file uses the probe's source stem or one of the canonical
            // `_test`/`_tests` companion conventions.
            RelationReason::SameTestFile
        } else if calls_seam_callee {
            // #3714: the test calls the wrapper seam's converted callee —
            // a direct captured call fact, but the attribution to THIS
            // seam is weaker than every name-anchored signal, so it ranks
            // last: only when no name affinity fired (the generic-name
            // witness the weak-signal gate used to skip) does the reason
            // surface, still above the bare WeakTokenSubstring fallback in
            // the priority order.
            RelationReason::SeamCalleeCall
        } else {
            // A path or test-name token substring is the broadest, least
            // precise match branch. `same_file_or_named` guarantees that one
            // of those weak signals is present here.
            RelationReason::WeakTokenSubstring
        };

        related.push((test, reason));
    }

    related.sort_by(|(a, _), (b, _)| a.name.cmp(&b.name).then_with(|| a.file.cmp(&b.file)));
    related.dedup_by(|(a, _), (b, _)| a.name == b.name && a.file == b.file);
    related
}

/// #2972: whether one captured, callable path-dependency declaration lets
/// an ambiguous-name cross-crate call be attributed to the owner's
/// package. Under-emit is the contract for relation credit, so every
/// boundary fails closed:
///
/// - a `let` binding of the owner name in the test body (a closure or
///   shadowing local, invisible to `index.functions`) defeats the admit
///   (#2972 review).
/// - the graph must be `Complete`. A `limited` capture is a partial edge
///   inventory and this surface has no disclosure channel, so it fails
///   closed exactly the way the #2971 rule fails closed on an incomplete
///   index; `unavailable` means no adjacency exists and admits nothing.
/// - both the test file and the owner file must attribute to a discovered
///   manifest directory (the #3616 nearest-manifest rule); an
///   unattributable side admits nothing.
/// - a same-named definition inside the calling test's own package is the
///   most plausible call target (a local shadow), a second same-named
///   definition the test's package can also call through a captured
///   callable edge is an equally plausible competing candidate, and an
///   unattributable same-named definition cannot be refuted at all; each
///   defeats the admit.
/// - one direct forward declaration in a callable section only: the
///   test's package declares a normal or dev path dependency on the
///   owner's package. A bare call binds names the declaring crate's own
///   dependency declarations provide, so reverse edges (the owner's crate
///   depends on the test crate) and build-only edges admit nothing
///   (#2972 review).
/// - call identity comes from the parser-backed captured `calls` facts
///   only, never from raw body text: a `score(` occurrence inside a
///   comment or string literal fabricates no call (#2972 review round
///   3). Calls the extractor could not capture stay unadmitted.
/// - the captured call must be attributable to THIS dependency: either a
///   qualified call `dependency_name::owner(`, or a bare call whose
///   stripped body imports the owner from the declared dependency name
///   (`use dependency_name::...owner;`, including nested paths and brace
///   lists). Aliased (`as`) and glob (`*`) imports prove nothing and are
///   refused; another dependency supplying the same name defeats the
///   admit (#2972 review round 3).
fn dependency_edge_admits_owner_call(
    context: &DependencyEdgeContext<'_>,
    owner_name: &str,
    owner_file: &Path,
    test: &TestSummary,
    same_name_definition_manifests: &BTreeSet<String>,
    same_name_unattributed_definition: bool,
) -> bool {
    if body_binds_owner_name(&test.body, owner_name) {
        return false;
    }
    if same_name_unattributed_definition {
        return false;
    }
    if context.adjacency.status() != PathDependencyGraphStatus::Complete {
        return false;
    }
    let Some(test_manifest) = nearest_manifest_identity(context.manifest_dir_prefixes, &test.file)
    else {
        return false;
    };
    let Some(owner_manifest) = nearest_manifest_identity(context.manifest_dir_prefixes, owner_file)
    else {
        return false;
    };
    if same_name_definition_manifests.contains(&test_manifest) {
        // Local shadow: the calling test's own package defines the name.
        return false;
    }
    if same_name_definition_manifests.iter().any(|manifest| {
        manifest != &owner_manifest
            && has_callable_forward_dependency(context.adjacency, &test_manifest, manifest)
    }) {
        // A second same-named definition the test's package can also call
        // through a captured callable edge: the bare call is ambiguous
        // between them.
        return false;
    }
    let Some(declarations) = context
        .adjacency
        .forward_dependency_declarations(&test_manifest, &owner_manifest)
    else {
        return false;
    };
    let callable_dependency_names: BTreeSet<&str> = declarations
        .iter()
        .filter(|(section, _)| {
            matches!(
                section,
                PathDependencySection::Dependencies | PathDependencySection::DevDependencies
            )
        })
        .map(|(_, name)| name.as_str())
        .collect();
    if callable_dependency_names.is_empty() {
        return false;
    }
    // Call identity is parser-backed (a bare direct call of the owner name
    // among the captured calls) except for one spelling read from the
    // comment-and-string-stripped source: a qualified call through the
    // dependency's declared name, which is unambiguous by construction.
    let bare_call_captured = test.calls.iter().any(|call| {
        call.name == owner_name
            && super::helper_transfer::is_direct_call_site(&call.text, owner_name)
    });
    let stripped_body = strip_comments_and_strings(&test.body);
    // Imports are module-scoped and visible file-wide: scan the test's
    // stripped file source, not the function body, so a file-level `use` is
    // seen (#3619 review). Only file-level imports count — a `use` inside a
    // nested module is invisible to tests outside it, so it must not
    // credit them (#3619 review, second round).
    let stripped_file = context
        .index
        .files
        .get(&test.file)
        .map(|facts| strip_comments_and_strings(&facts.source))
        .unwrap_or_else(|| stripped_body.clone());
    let file_level_imports = file_level_use_text(&stripped_file);
    callable_dependency_names.iter().any(|dependency_name| {
        let qualified = format!("{dependency_name}::{owner_name}");
        let qualified_call = test
            .calls
            .iter()
            .any(|call| is_qualified_owner_call(&call.text, &qualified))
            || is_qualified_owner_call(&stripped_body, &qualified);
        if qualified_call {
            // The call is spelled through this dependency's declared name:
            // unambiguous without an import.
            return true;
        }
        bare_call_captured
            && imports_owner_from_dependency(&file_level_imports, dependency_name, owner_name)
    })
}

/// Whether `text` invokes the owner through the dependency's declared name,
/// spelled exactly `dependency_name::owner_name(` at a token boundary not
/// preceded by a longer path or a receiver.
fn is_qualified_owner_call(text: &str, qualified_name: &str) -> bool {
    super::helper_transfer::is_direct_call_site(text, qualified_name)
}

/// Whether the test's package declares a callable path dependency on the
/// other manifest: a direct forward edge whose captured section is normal
/// or dev dependencies (consumed by the competing-candidate guard).
/// Normal dependencies are callable from every target of the declaring
/// package; dev dependencies are additionally callable from its test
/// targets, which is the surface being classified. Build dependencies are
/// visible only to the build script, and the reverse direction provides
/// no route from the test crate to the owner — neither is callable here.
fn has_callable_forward_dependency(
    adjacency: &PathDependencyAdjacency,
    test_manifest: &str,
    dependency_manifest: &str,
) -> bool {
    adjacency
        .forward_dependency_declarations(test_manifest, dependency_manifest)
        .is_some_and(|declarations| {
            declarations.iter().any(|(section, _)| {
                matches!(
                    section,
                    PathDependencySection::Dependencies | PathDependencySection::DevDependencies
                )
            })
        })
}

/// Whether the stripped body imports the owner name from the declaring
/// dependency: `use dependency_name::...owner_name;` in simple or nested
/// path form, or a brace list `use dependency_name::path::{a, b};` naming
/// the owner as a whole item. Aliased (`as`) and glob (`*`) imports are
/// refused — neither establishes that a bare call binds the owner's name
/// from this dependency — and any brace shape this conservative parser
/// cannot read is skipped (fail closed).
/// Extract the depth-zero `use` statements of a comment-and-string-stripped
/// source: file-level items only. A `use` inside a nested module is
/// invisible to tests outside that module, so nested text is dropped
/// rather than credited (#3619 review). Brace lists inside a `use`
/// statement are kept: their braces do not change the statement's depth.
pub(in crate::analysis) fn file_level_use_text(stripped: &str) -> String {
    let chars: Vec<char> = stripped.chars().collect();
    let mut depth_at = vec![0usize; chars.len() + 1];
    let mut depth = 0usize;
    for (index, ch) in chars.iter().enumerate() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        depth_at[index + 1] = depth;
    }
    let is_ident_char = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    let mut out = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        let use_starts = depth_at[index] == 0
            && chars[index] == 'u'
            && (index == 0 || !is_ident_char(chars[index - 1]))
            && chars.get(index + 1).is_some_and(|ch| *ch == 's')
            && chars.get(index + 2).is_some_and(|ch| *ch == 'e')
            && chars.get(index + 3).is_some_and(|ch| !is_ident_char(*ch));
        if use_starts {
            let mut end = index;
            while end < chars.len() {
                if chars[end] == ';' && depth_at[end] == 0 {
                    break;
                }
                end += 1;
            }
            if end < chars.len() {
                out.extend(&chars[index..=end]);
                out.push('\n');
                index = end + 1;
                continue;
            }
        }
        index += 1;
    }
    out
}

fn imports_owner_from_dependency(stripped: &str, dependency_name: &str, owner_name: &str) -> bool {
    let use_prefix = format!("use {dependency_name}::");
    let mut search_from = 0usize;
    while let Some(rel) = stripped[search_from..].find(&use_prefix) {
        let at = search_from + rel;
        let keyword_delimited = at == 0
            || !stripped
                .as_bytes()
                .get(at - 1)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        if keyword_delimited && use_path_names_owner(&stripped[at + use_prefix.len()..], owner_name)
        {
            return true;
        }
        search_from = at + use_prefix.len();
    }
    false
}

/// Whether the `use` path beginning after `use dependency_name::` names the
/// owner as an imported item: the path before `;` must equal the owner, end
/// on a `::owner` segment, or — for a brace list — carry the owner as a
/// whole top-level item.
fn use_path_names_owner(rest: &str, owner_name: &str) -> bool {
    let Some(semicolon) = rest.find(';') else {
        return false;
    };
    let path = rest[..semicolon].trim();
    if path.contains(" as ") || path.contains('*') {
        return false;
    }
    match path.find('{') {
        None => path == owner_name || path.ends_with(&format!("::{owner_name}")),
        Some(brace) => {
            let items_text = &path[brace + 1..];
            let Some(items_text) = items_text.strip_suffix('}') else {
                return false;
            };
            if items_text.contains('{') || items_text.contains('}') {
                // Nested brace groups: the conservative parser refuses to
                // guess which level the owner would import from.
                return false;
            }
            items_text.split(',').any(|item| {
                let item = item.trim();
                !item.is_empty()
                    && (item == owner_name || item.ends_with(&format!("::{owner_name}")))
            })
        }
    }
}

/// Nearest discovered manifest directory owning `file` (longest prefix, the
/// same #3616 attribution rule the diff-scope expansion uses), as an
/// adjacency manifest identity. The scan root's own manifest is the empty
/// prefix. `None` when no discovered manifest directory covers the file:
/// the identity is unattributable and the admit fails closed.
fn nearest_manifest_identity(manifest_dir_prefixes: &[String], file: &Path) -> Option<String> {
    let normalized = normalize_path(file);
    manifest_dir_prefixes
        .iter()
        .filter(|prefix| normalized.starts_with(prefix.as_str()))
        .max_by_key(|prefix| prefix.len())
        .map(|prefix| format!("{prefix}Cargo.toml"))
}

/// Whether `body` introduces a local binding of `owner_name` through a
/// `let` declaration (including `let mut`). Such a binding — a closure, a
/// shadowing local — is invisible to `index.functions` and is the most
/// plausible resolution target for the bare call, so the dependency-edge
/// admit must not fire. Comment text is not distinguished: a spurious
/// match only ever suppresses the admit (under-emit).
fn body_binds_owner_name(body: &str, owner_name: &str) -> bool {
    if owner_name.is_empty() {
        return false;
    }
    let mut search_from = 0usize;
    while let Some(rel) = body[search_from..].find("let") {
        let at = search_from + rel;
        let keyword_delimited = at == 0
            || !body
                .as_bytes()
                .get(at - 1)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        let after = body[at + 3..].trim_start();
        let after = match after.strip_prefix("mut") {
            Some(tail) if tail.starts_with(char::is_whitespace) => tail.trim_start(),
            _ => after,
        };
        let binds = after.starts_with(owner_name)
            && after[owner_name.len()..]
                .chars()
                .next()
                .is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'));
        if keyword_delimited && binds {
            return true;
        }
        search_from = at + 3;
    }
    false
}

/// Comment- and string-stripped form of a test body: code content only.
/// Mirrors the `value_resolution::strip_comments_and_strings` semantics
/// (line comments, escaped string literals) and adds nested block comments
/// and raw string literals with hash delimiters, carried across lines the
/// way `owner_shape::strip_comments_and_strings` carries them, so import
/// or call evidence can never be fabricated from comment or string text
/// (#2972 review). Every ambiguity strips MORE: a lost evidence site only
/// suppresses the admit.
fn strip_comments_and_strings(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut block_depth = 0usize;
    let mut raw_hashes: Option<usize> = None;
    while let Some(ch) = chars.next() {
        if let Some(hashes) = raw_hashes {
            if ch == '"' {
                let mut seen = 0usize;
                while seen < hashes && chars.peek().is_some_and(|next| *next == '#') {
                    let _ = chars.next();
                    seen += 1;
                }
                if seen == hashes {
                    raw_hashes = None;
                }
            }
            if ch == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if block_depth > 0 {
            match (ch, chars.peek().copied()) {
                ('*', Some('/')) => {
                    let _ = chars.next();
                    block_depth -= 1;
                }
                ('/', Some('*')) => {
                    let _ = chars.next();
                    block_depth += 1;
                }
                _ => {}
            }
            if ch == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                out.push('"');
                continue;
            }
            if ch == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        match ch {
            '/' if chars.peek() == Some(&'/') => {
                for skipped in chars.by_ref() {
                    if skipped == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                let _ = chars.next();
                block_depth += 1;
                out.push(' ');
            }
            '"' => {
                in_string = true;
                out.push('"');
            }
            'r' => {
                let mut hashes = 0usize;
                while chars.peek().is_some_and(|next| *next == '#') {
                    let _ = chars.next();
                    hashes += 1;
                }
                if chars.peek().is_some_and(|next| *next == '"') {
                    let _ = chars.next();
                    raw_hashes = Some(hashes);
                    out.push(' ');
                } else {
                    out.push('r');
                    for _ in 0..hashes {
                        out.push('#');
                    }
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Return whether `test_file` is the canonical companion test file for
/// `probe_file`.  This intentionally mirrors the stronger test-grip
/// authority: only the source stem itself and its `_test`/`_tests` variants
/// count as file identity.  Paths are compared by stem, so host-specific
/// separators do not affect the result.
fn same_test_file(probe_file: &Path, test_file: &Path) -> bool {
    // Merge resolution (#3545 x PR-3547): keep the PR's `cross_host_stem`
    // authority — it normalizes both separator flavors (main's #3545 intent)
    // AND fails closed on non-UTF-8 paths, where lossy conversion could
    // collapse distinct names into one stem (#3545 review).  Keep main's
    // empty-stem guard so degenerate stems never count as file identity.
    let Some(probe_stem) = cross_host_stem(probe_file) else {
        return false;
    };
    if probe_stem.is_empty() {
        return false;
    }
    let Some(test_stem) = cross_host_stem(test_file) else {
        return false;
    };
    if test_stem.is_empty() {
        return false;
    }
    test_stem == probe_stem
        || test_stem == format!("{probe_stem}_test")
        || test_stem == format!("{probe_stem}_tests")
}

/// Extract the file stem on any host: split on both separator flavors
/// (a probe path recorded with `\` must resolve on Unix and vice versa),
/// then strip the extension the way [`std::path::Path::file_stem`] does.
/// Non-UTF-8 paths fail closed — lossy conversion could collapse distinct
/// names into one stem and fake a canonical-companion relation (#3545
/// review).
fn cross_host_stem(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let unified = text.replace('\\', "/");
    let name = unified.rsplit('/').next()?;
    let stem = match name.rfind('.') {
        Some(0) | None => name,
        Some(extension) => &name[..extension],
    };
    Some(stem.to_string())
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

/// Extract a source-file stem after normalizing separators from either host.
/// Analysis inputs can contain paths produced on a different platform than the
/// host running the association pass. Non-UTF-8 paths fail closed: lossy
/// replacement characters can collapse distinct leaf names onto one stem and
/// fabricate file-name affinity (#3599; same guard as the grip-side copy
/// added in #3598).
fn normalized_file_stem(path: &Path) -> String {
    let Some(text) = path.to_str() else {
        return String::new();
    };
    let unified = text.replace('\\', "/").trim_start_matches("./").to_string();
    let file = unified.rsplit('/').next().unwrap_or_default();
    Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

/// #3235: one authority for recognizing an absolute path form on any host —
/// Unix-rooted, Windows drive-prefixed, or host-absolute. Shared by
/// `package_prefix` (which cannot derive a package scope from such a path
/// without a `crates/` segment) and the package guards in
/// `find_related_tests`, which fail closed when both sides of a candidate
/// relation are unscopable absolute paths.
fn path_is_absolute_form(path: &Path) -> bool {
    let normalized = normalize_path(path);
    let has_drive_prefix = normalized.as_bytes().get(1).copied() == Some(b':');
    path.is_absolute() || normalized.starts_with('/') || has_drive_prefix
}

/// The workspace-package scope of a source path (`crates/foo/src/lib.rs`
/// -> `crates/foo/`), or `None` when the path carries no package scope:
/// single-crate relative paths (`src/lib.rs`, `tests/basic.rs`) and
/// absolute paths are unscopable (#3235). Shared with the classifier's
/// evidence stage, whose cross-package same-name defeat gate (#3731 review
/// G1) compares the changed owner's package against each related test's.
pub(in crate::analysis) fn package_prefix(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    let crate_relative = normalized
        .strip_prefix("crates/")
        .or_else(|| normalized.rsplit_once("/crates/").map(|(_, rest)| rest));
    if let Some(crate_relative) = crate_relative
        && let Some((crate_name, package_relative)) = crate_relative.split_once('/')
        && (package_relative.starts_with("src/") || package_relative.starts_with("tests/"))
    {
        return Some(format!("crates/{crate_name}/"));
    }
    if path_is_absolute_form(path) {
        return None;
    }
    for marker in ["/src/", "/tests/"] {
        if let Some(idx) = normalized.rfind(marker) {
            let prefix = &normalized[..idx];
            if prefix.is_empty() {
                return None;
            }
            return Some(format!("{prefix}/"));
        }
    }
    None
}

/// True when `body` mentions `owner_name` immediately followed by `(`.
///
/// A fallback for tests whose `calls` facts did not capture the call, e.g. a
/// path-qualified `crate_under_test::internal::inner(10, 3)`.
///
/// The boundary check rejects a longer identifier that merely ends in
/// `owner_name` (`compute_score(`), but deliberately does **not** reject a
/// preceding `.`. A receiver is not evidence of a different function: the
/// owner is frequently an impl method, and `ledger.apply(5)` is the only way a
/// test can call `Ledger::apply` at all. #3047 tried rejecting `.` to suppress
/// cross-crate `other.compute_hash(...)` matches and downgraded every
/// method-owner fixture from `direct_owner_call` to `owner_named_test`.
///
/// That suppression is also unnecessary. `CallFact` carries no receiver or
/// path information (`analysis/extract/calls.rs` keeps only the bare trailing
/// identifier), so receiver identity cannot be recovered here — but the
/// cross-crate bypass is already gated on the owner name being unique across
/// `index.functions`, and `syntax/ra.rs` indexes impl methods alongside free
/// functions. A same-named method on another type is therefore itself in the
/// index, the name is not unique, and the bypass never fires. The uniqueness
/// gate subsumes the receiver concern.
pub(in crate::analysis) fn body_contains_owner_call(body: &str, owner_name: &str) -> bool {
    if owner_name.is_empty() {
        return false;
    }
    body.match_indices(owner_name).any(|(start, _)| {
        let end = start.saturating_add(owner_name.len());
        let before_ok = start == 0
            || !body
                .as_bytes()
                .get(start - 1)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        let after_call = body
            .get(end..)
            .map(|tail| tail.trim_start().starts_with('('))
            .unwrap_or(false);
        before_ok && after_call
    })
}

/// Whether `test` invokes `owner_name` through a call site that spells the
/// owner itself — a bare `owner(..)` call, never a receiver-qualified
/// `other.owner(..)` or path-qualified `Type::owner(..)` spelling.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::{FileFacts, FunctionSourceRole};
    use crate::analysis::rust_index::{CallFact, OracleFact, extract_identifier_tokens};
    use crate::domain::{
        DeltaKind, OracleKind, OracleStrength, ProbeFamily, ProbeId, SourceLocation, SymbolId,
    };
    use std::path::PathBuf;

    #[test]
    fn given_owner_function_when_tests_share_name_across_packages_then_filters_to_package() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![
                test(
                    "crates/crate_b/tests/score.rs",
                    "crate_b_score_test",
                    "score(2)",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0.name, "crate_a_score_test");
    }

    /// #2971 / #3033 positive control: a cross-crate test that calls a
    /// **uniquely-named** owner is retained via `DirectOwnerCall`. The owner
    /// name `compute_hash` appears once in the workspace, so the bare call in
    /// a separate crate's integration test is unambiguously calling the owner.
    #[test]
    fn given_unique_owner_when_cross_crate_test_calls_owner_then_retained() {
        let owner = function("crates/digest/src/lib.rs", "compute_hash");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![test_with_call(
                "crates/digest-tests/tests/integration.rs",
                "hash_integration_test",
                "let result = compute_hash(b\"input\");",
                "compute_hash",
            )],
            ..RustIndex::default()
        };
        let probe = probe("crates/digest/src/lib.rs", "compute_hash(input)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(
            related.len(),
            1,
            "cross-crate test calling unique owner should be retained"
        );
        assert_eq!(related[0].0.name, "hash_integration_test");
        assert_eq!(related[0].1, RelationReason::DirectOwnerCall);
    }

    /// An impl-method owner is reachable only through a receiver, so a `.`
    /// before the name must not disqualify the call. The `propagate_*` goldens
    /// are exactly this shape — owner `Ledger::apply`, test body
    /// `ledger.apply(5);` — and a `.`-rejecting matcher silently downgraded
    /// them from `direct_owner_call` to `owner_named_test`, weakening the
    /// relation on every method owner in the corpus.
    #[test]
    fn given_method_owner_when_test_calls_through_receiver_then_direct_owner_call() {
        let owner = function("src/lib.rs", "apply");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![TestSummary {
                name: "changes_balance".to_string(),
                file: PathBuf::from("tests/ledger_tests.rs"),
                start_line: 1,
                end_line: 4,
                body: "let mut ledger = Ledger::new(100);\nledger.apply(5);".to_string(),
                // Body-only: the receiver form is what must be credited.
                calls: Vec::new(),
                assertions: Vec::new(),
                literals: Vec::new(),
                attrs: Vec::new(),
                nested_fn_names: Vec::new(),
                let_bindings: Vec::new(),
            }],
            ..RustIndex::default()
        };
        let probe = probe("src/lib.rs", "self.persist(amount * 9)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1, "method owner must keep its calling test");
        assert_eq!(related[0].1, RelationReason::DirectOwnerCall);
    }

    /// #2971 scope control: the same workspace as the positive control above,
    /// reached through a partial index. The diff path indexes only the changed
    /// files whenever `include_unchanged_tests` is false, and only the changed
    /// packages under Draft/Fast, so a name seen once in that index is not
    /// evidence that it is unique in the workspace. The bypass must not fire:
    /// a sibling crate's unindexed same-named function would otherwise be
    /// credited as the owner — the token-coincidence false-`exposed` family.
    #[test]
    fn given_incomplete_index_when_cross_crate_test_calls_owner_then_filtered() {
        let owner = function("crates/digest/src/lib.rs", "compute_hash");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![test_with_call(
                "crates/digest-tests/tests/integration.rs",
                "hash_integration_test",
                "let result = compute_hash(b\"input\");",
                "compute_hash",
            )],
            ..RustIndex::default()
        };
        let probe = probe("crates/digest/src/lib.rs", "compute_hash(input)");

        // The only difference from the positive control.
        let related = find_related_tests(&probe, Some(&owner), &index, false, None, None);

        assert!(
            related.is_empty(),
            "a partial index cannot establish name uniqueness, so the \
             package-prefix guard must stay unconditional"
        );
    }

    /// #2971 / #3033 negative control: a cross-crate test calling an
    /// **ambiguously-named** function is filtered by the package-prefix guard.
    /// Both crate_a and crate_b define `score`, so a bare `score()` call in
    /// crate_b cannot be attributed to crate_a::score.
    #[test]
    fn given_ambiguous_owner_when_cross_crate_test_calls_same_name_then_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let crate_b_fn = function("crates/crate_b/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![owner.clone(), crate_b_fn],
            tests: vec![
                test_with_call(
                    "crates/crate_b/tests/score.rs",
                    "crate_b_calls_score",
                    "score(42)",
                    "score",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        // The crate_a test is in-package and passes; the crate_b test is
        // cross-crate and filtered despite calling `score()` because the name
        // is ambiguous across crates.
        assert_eq!(
            related.len(),
            1,
            "ambiguous-name cross-crate test must be filtered"
        );
        assert_eq!(related[0].0.name, "crate_a_score_test");
    }

    // --- #2972: dependency-edge admit for ambiguous cross-crate owner calls ---
    // (review round 3: parser-backed call identity only - comments and
    // strings never fabricate calls - and owner-crate import evidence via
    // the captured dependency name)

    /// A path-dependency adjacency over synthetic provenance — pure, no
    /// filesystem, the same fixture style the graph module's own tests use.
    /// Each edge names the declaring manifest, the resolved dependency
    /// directory, the Cargo section it was declared in, and the declared
    /// dependency name; `limitations` degrades the capture to a partial
    /// inventory (`limited` status).
    fn adjacency_with(
        package_graph_status: &str,
        edges: &[(&str, &str, PathDependencySection, &str)],
        limitations: bool,
    ) -> PathDependencyAdjacency {
        use crate::analysis::seam_cache::{
            PathDependencyEdge, PathDependencyLimitation, PathDependencyLimitationKind,
            PathDependencyResolution, PathDependencySource, WorkspaceGraphProvenance,
        };
        let provenance = WorkspaceGraphProvenance {
            package_graph_status: package_graph_status.to_string(),
            path_dependency_edges: edges
                .iter()
                .map(|(from, to, section, dependency_name)| PathDependencyEdge {
                    from_manifest: (*from).to_string(),
                    section: *section,
                    target: None,
                    dependency_name: (*dependency_name).to_string(),
                    declared_path: Some("../dep".to_string()),
                    resolved_path: Some((*to).to_string()),
                    resolution: PathDependencyResolution::Resolved,
                    source: PathDependencySource::Package,
                })
                .collect(),
            path_dependency_limitations: if limitations {
                vec![PathDependencyLimitation {
                    manifest: "crates/crate_c/Cargo.toml".to_string(),
                    kind: PathDependencyLimitationKind::UnfollowedWorkspaceRedirect,
                    detail: "redirect not followed".to_string(),
                }]
            } else {
                Vec::new()
            },
            ..WorkspaceGraphProvenance::default()
        };
        PathDependencyAdjacency::build(&provenance)
    }

    fn owned_prefixes(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    /// The discovered manifest-dir inventory for the crate_a/b/c workspace
    /// used by most #2972 fixtures: the scan root's own manifest (the empty
    /// prefix) plus one prefix per package.
    fn crate_abc_prefixes() -> Vec<String> {
        owned_prefixes(&["", "crates/crate_a/", "crates/crate_b/", "crates/crate_c/"])
    }

    fn edge_context<'a>(
        adjacency: &'a PathDependencyAdjacency,
        manifest_dir_prefixes: &'a [String],
        index: &'a RustIndex,
    ) -> DependencyEdgeContext<'a> {
        DependencyEdgeContext {
            adjacency,
            manifest_dir_prefixes,
            index,
        }
    }

    /// A complete adjacency joining the calling test's crate to the owner's
    /// crate through one normal forward dependency, plus the standard
    /// prefix inventory.
    fn crate_a_dependency_context() -> (PathDependencyAdjacency, Vec<String>) {
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/crate_a",
                PathDependencySection::Dependencies,
                "crate_a",
            )],
            false,
        );
        (adjacency, crate_abc_prefixes())
    }

    /// #2972 positive control: the owner name `score` is ambiguous (crate_a
    /// and crate_b both define it), so the #2971 uniqueness bypass cannot
    /// fire. A captured, callable forward path-dependency edge — the calling
    /// test's crate declares a normal path dependency named `crate_a` —
    /// plus the `use crate_a::score;` import in the stripped test body is
    /// the evidence that attributes the bare call.
    #[test]
    fn given_ambiguous_owner_when_cross_crate_dependent_test_calls_owner_then_edge_admits() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![
                test(
                    "crates/crate_c/tests/score.rs",
                    "crate_c_score_test",
                    "use crate_a::score; score(7)",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                ),
            ],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            2,
            "the dependent crate's calling test must be admitted: {related:?}"
        );
        let admitted: Vec<_> = related
            .iter()
            .filter(|(test, _)| test.name == "crate_c_score_test")
            .collect();
        assert_eq!(
            admitted.len(),
            1,
            "crate_c test missing or duplicated: {related:?}"
        );
        assert_eq!(admitted[0].1, RelationReason::DirectOwnerCall);
    }

    /// #2972 review: dev dependencies are callable from the declaring
    /// package's test targets — exactly the surface being classified — so a
    /// captured dev-dependency edge admits the same way a normal one does
    /// when the body imports the owner through the declared name.
    #[test]
    fn given_forward_dev_dependency_edge_when_cross_crate_test_calls_owner_then_admits() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/crate_a",
                PathDependencySection::DevDependencies,
                "crate_a",
            )],
            false,
        );
        let prefixes = crate_abc_prefixes();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a dev-dependency edge is callable from the test target: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_c_score_test");
    }

    /// #2972 review: imports are module-scoped. The `use crate_a::score;`
    /// lives at file level, outside the test function body the summary
    /// carries — the import scan reads the whole stripped file source, so a
    /// genuine dependent test is not lost.
    #[test]
    fn given_module_level_import_when_cross_crate_test_calls_owner_then_admits() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let test_file = PathBuf::from("crates/crate_c/tests/score.rs");
        let source = "use crate_a::score;

#[test]
fn crate_c_score_test() {
    score(7);
}
";
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "score(7)",
            )],
            files: std::collections::BTreeMap::from([(
                test_file.clone(),
                FileFacts {
                    path: test_file.clone(),
                    source: source.to_string(),
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/crate_a",
                PathDependencySection::Dependencies,
                "crate_a",
            )],
            false,
        );
        let prefixes = crate_abc_prefixes();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a module-level import must be seen by the whole-file scan: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_c_score_test");
    }

    /// #3619 review, second round: an import inside a nested module is
    /// invisible to tests outside it. A file-level import is required; the
    /// nested-module form must not credit an unrelated bare call.
    #[test]
    fn given_nested_module_import_when_bare_call_has_no_file_level_import_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let test_file = PathBuf::from("crates/crate_c/tests/score.rs");
        let source = "mod helpers {
    use crate_a::score;
}

#[test]
fn crate_c_score_test() {
    score(7);
}
";
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "score(7)",
            )],
            files: std::collections::BTreeMap::from([(
                test_file.clone(),
                FileFacts {
                    path: test_file.clone(),
                    source: source.to_string(),
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/crate_a",
                PathDependencySection::Dependencies,
                "crate_a",
            )],
            false,
        );
        let prefixes = crate_abc_prefixes();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a nested-module import must not credit a test outside it: {related:?}"
        );
    }

    /// #2972 review round 3: a nested `use` path ending on the owner name is
    /// import evidence too — the dependency supplies the module path that
    /// binds the owner.
    #[test]
    fn given_nested_use_path_import_when_cross_crate_test_calls_owner_then_admits() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::math::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a nested use path from the declared dependency must admit: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_c_score_test");
    }

    /// #2972 review round 3: a brace-list import naming the owner as a
    /// whole item is import evidence; items the conservative parser cannot
    /// read stay failed closed (no test here admits through them).
    #[test]
    fn given_brace_list_use_import_when_cross_crate_test_calls_owner_then_admits() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::math::{score, other}; score(7)",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a brace-list import naming the owner must admit: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_c_score_test");
    }

    /// #2972 review round 3: a call qualified through the dependency's
    /// declared name binds the owner without any import — the second
    /// evidence form.
    #[test]
    fn given_qualified_call_through_dependency_name_when_no_import_exists_then_admits() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "let ok = crate_a::score(7);",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a call qualified through the declared dependency name must admit: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_c_score_test");
    }

    /// #2972 discrimination: the same workspace shape, but the probe's owner
    /// lives in crate_b. The only edge connects crate_c to crate_a — and the
    /// test even imports crate_a's `score` — so the bare call stays
    /// unattributed for crate_b::score: the captured edge to the owner's
    /// package, not the call or the name, is what admits.
    #[test]
    fn given_ambiguous_owner_when_test_crate_has_no_edge_to_owner_crate_then_stays_filtered() {
        let owner = function("crates/crate_b/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![
                test(
                    "crates/crate_c/tests/score.rs",
                    "crate_c_score_test",
                    "use crate_a::score; score(7)",
                ),
                test(
                    "crates/crate_b/tests/score.rs",
                    "crate_b_score_test",
                    "score(2)",
                ),
            ],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_b/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a crate the edge does not reach must stay filtered: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_b_score_test");
    }

    /// #2972 review: the reverse direction admits nothing. The owner's crate
    /// declares the path dependency on the calling test's crate, which gives
    /// the owner crate a route to the test crate — not the other way round —
    /// so even an import of the owner name through the (nonexistent)
    /// forward declaration cannot attribute the call.
    #[test]
    fn given_reverse_dependency_edge_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![
                test(
                    "crates/crate_c/tests/score.rs",
                    "crate_c_score_test",
                    "use crate_a::score; score(7)",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                ),
            ],
            ..RustIndex::default()
        };
        // crate_a declares a path dependency on crate_c: the reverse direction.
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_a/Cargo.toml",
                "crates/crate_c",
                PathDependencySection::Dependencies,
                "crate_c",
            )],
            false,
        );
        let prefixes = crate_abc_prefixes();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a reverse edge provides no route from the test crate to the owner: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_a_score_test");
    }

    /// #2972 review: a build-dependency edge is not callable from test code
    /// (build dependencies are visible only to the build script), so it
    /// admits nothing even though it joins the two packages.
    #[test]
    fn given_build_only_dependency_edge_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/crate_a",
                PathDependencySection::BuildDependencies,
                "crate_a",
            )],
            false,
        );
        let prefixes = crate_abc_prefixes();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a build-only edge must not attribute test calls: {related:?}"
        );
    }

    /// #2972 fail-closed: an ambiguous owner with no usable graph (the
    /// adjacency reports an unavailable manifest scan) admits nothing; the
    /// #2971 package-prefix filter stands.
    #[test]
    fn given_unavailable_graph_when_cross_crate_test_calls_ambiguous_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![
                test(
                    "crates/crate_d/tests/score.rs",
                    "crate_d_score_test",
                    "score(9)",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                ),
            ],
            ..RustIndex::default()
        };
        let adjacency = adjacency_with("unavailable", &[], false);
        assert_eq!(adjacency.status(), PathDependencyGraphStatus::Unavailable);
        let prefixes =
            owned_prefixes(&["", "crates/crate_a/", "crates/crate_b/", "crates/crate_d/"]);
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "an unavailable graph must not produce edge admits: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_a_score_test");
    }

    /// #2972 graph-status honesty: a `limited` capture is a partial edge
    /// inventory and this relation surface has no disclosure channel, so the
    /// admit requires a complete graph — the same fail-closed posture the
    /// #2971 rule takes toward an incomplete index.
    #[test]
    fn given_limited_graph_when_cross_crate_test_calls_ambiguous_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/crate_a",
                PathDependencySection::Dependencies,
                "crate_a",
            )],
            true,
        );
        assert_eq!(adjacency.status(), PathDependencyGraphStatus::Limited);
        let prefixes = crate_abc_prefixes();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a partial edge inventory must not produce edge admits: {related:?}"
        );
    }

    /// #2972 identity guard: the calling test's own package also defines
    /// `score`, so the bare call most plausibly names the local definition.
    /// The dependency edge cannot establish that the call crosses packages
    /// and the match stays filtered (the token-coincidence family).
    #[test]
    fn given_local_same_name_definition_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
                function("crates/crate_c/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a local same-named definition must defeat the edge admit: {related:?}"
        );
    }

    /// #2972 competing-candidate guard: the calling crate depends on both
    /// crate_a and crate_b through callable sections, and both define
    /// `score`. A bare `score()` call is ambiguous between two
    /// edge-connected candidates, so neither owner is credited through the
    /// edge.
    #[test]
    fn given_competing_edge_connected_candidate_when_cross_crate_test_calls_owner_then_stays_filtered()
     {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "score(7)",
            )],
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[
                (
                    "crates/crate_c/Cargo.toml",
                    "crates/crate_a",
                    PathDependencySection::Dependencies,
                    "crate_a",
                ),
                (
                    "crates/crate_c/Cargo.toml",
                    "crates/crate_b",
                    PathDependencySection::Dependencies,
                    "crate_b",
                ),
            ],
            false,
        );
        let prefixes = crate_abc_prefixes();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "two edge-connected same-named candidates must fail closed: {related:?}"
        );
    }

    /// #2972 review round 3: the test imports `score` from a DIFFERENT
    /// dependency than the one whose edge reaches the owner's package. The
    /// bare call plausibly binds the other dependency's name, so the
    /// crate_a edge admits nothing.
    #[test]
    fn given_import_from_another_dependency_when_cross_crate_test_calls_owner_then_stays_filtered()
    {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use other_dep::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "an import naming another dependency must defeat the edge admit: {related:?}"
        );
    }

    /// #2972 review round 3: an aliased import renames the item, so a bare
    /// `score(` call cannot bind through it; the alias defeats the admit.
    #[test]
    fn given_aliased_import_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score as compute; score(7)",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "an aliased import must not read as owner-import evidence: {related:?}"
        );
    }

    /// #2972 review round 3 (fabricated-call guard): the only `score(`
    /// occurrence lives in a comment. The parser-backed calls carry no bare
    /// owner call, so the comment fabricates nothing and the import alone
    /// admits nothing.
    #[test]
    fn given_comment_only_call_evidence_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test_with_call(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; // score(9)",
                "helper",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a commented-out call must not ride through the edge admit: {related:?}"
        );
    }

    /// #2972 review round 3 (fabricated-call guard): the only `score(`
    /// occurrence lives inside a string literal. Same refusal as the
    /// comment case — captured calls, not body text, carry call identity.
    #[test]
    fn given_string_literal_call_evidence_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test_with_call(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; let s = \"score(9)\";",
                "helper",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a string-literal call must not ride through the edge admit: {related:?}"
        );
    }

    /// #2972 unattributable-definition guard: an indexed same-named
    /// definition whose file no discovered manifest directory covers can
    /// never be refuted as the call target, so it defeats the edge admit.
    /// The prefix inventory here lacks the scan-root manifest, which is the
    /// shape that leaves root-level files unattributable.
    #[test]
    fn given_unattributed_same_name_definition_when_cross_crate_test_calls_owner_then_stays_filtered()
     {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
                function("src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "score(7)",
            )],
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/crate_a",
                PathDependencySection::Dependencies,
                "crate_a",
            )],
            false,
        );
        let prefixes = owned_prefixes(&["crates/crate_a/", "crates/crate_b/", "crates/crate_c/"]);
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "an unattributable same-named definition must defeat the edge admit: {related:?}"
        );
    }

    /// #2972 review (nested-package identity): a `package_prefix` collapse
    /// would read `crates/outer/inner/src/lib.rs` as `crates/outer/`, but
    /// the nearest-manifest attribution resolves the owner to
    /// `crates/outer/inner/Cargo.toml`. A test crate that depends only on
    /// `outer` has no edge to `inner`, so the call stays filtered — even
    /// though the test imports `outer`'s own `score`.
    #[test]
    fn given_nested_package_owner_when_test_depends_only_on_outer_crate_then_stays_filtered() {
        let owner = function("crates/outer/inner/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/outer/inner/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use outer::score; score(7)",
            )],
            ..RustIndex::default()
        };
        // crate_c declares a normal path dependency on outer — not on the
        // nested inner package that owns the changed function.
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/outer",
                PathDependencySection::Dependencies,
                "outer",
            )],
            false,
        );
        let prefixes = owned_prefixes(&[
            "",
            "crates/outer/",
            "crates/outer/inner/",
            "crates/crate_b/",
            "crates/crate_c/",
        ]);
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/outer/inner/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "an outer-crate edge must not attribute a nested-package owner: {related:?}"
        );
    }

    /// Positive companion to the nested-package guard: an edge that reaches
    /// the nested owning package itself admits, proving the guard fires on
    /// the resolved manifest identity and not on the nested path shape.
    #[test]
    fn given_nested_package_owner_when_test_depends_on_the_nested_crate_then_admits() {
        let owner = function("crates/outer/inner/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/outer/inner/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use inner::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let adjacency = adjacency_with(
            "complete",
            &[(
                "crates/crate_c/Cargo.toml",
                "crates/outer/inner",
                PathDependencySection::Dependencies,
                "inner",
            )],
            false,
        );
        let prefixes = owned_prefixes(&[
            "",
            "crates/outer/",
            "crates/outer/inner/",
            "crates/crate_b/",
            "crates/crate_c/",
        ]);
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/outer/inner/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "an edge to the nested owning package must admit: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_c_score_test");
    }

    /// #2972 review (local binding): a `let score = |x| x;` closure in the
    /// test body is invisible to `index.functions` and is the most plausible
    /// resolution target for the bare call, so the edge-connected crate's
    /// owner is not credited even with an import present.
    #[test]
    fn given_local_let_binding_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; let score = |x| x; assert_eq!(score(7), 7);",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a local binding of the owner name must defeat the edge admit: {related:?}"
        );
    }

    /// #2972 review (call form): a method call resolves on the receiver's
    /// type, not the calling crate's scope, so a method-shaped `score` call
    /// admits nothing even across a captured callable edge.
    #[test]
    fn given_method_call_form_when_cross_crate_test_calls_owner_then_stays_filtered() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/tests/score.rs",
                "crate_c_score_test",
                "use crate_a::score; let report = harness.score(7);",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert!(
            related.is_empty(),
            "a method-call form must not ride through the edge admit: {related:?}"
        );
    }

    /// #2972 review (identity authority): a custom Cargo test target path
    /// outside `src/`/`tests/` carries no `package_prefix`, but the
    /// nearest-manifest attribution still resolves it to the owning
    /// package's manifest, so a genuine dependent-crate test keeps working.
    #[test]
    fn given_custom_cargo_test_target_path_when_edge_connects_then_admits() {
        let owner = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![
                function("crates/crate_a/src/lib.rs", "score"),
                function("crates/crate_b/src/lib.rs", "score"),
            ],
            tests: vec![test(
                "crates/crate_c/custom/suite.rs",
                "crate_c_score_suite",
                "use crate_a::score; score(7)",
            )],
            ..RustIndex::default()
        };
        let (adjacency, prefixes) = crate_a_dependency_context();
        let context = edge_context(&adjacency, &prefixes, &index);
        let probe = probe("crates/crate_a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, Some(&context));

        assert_eq!(
            related.len(),
            1,
            "a custom-target test in an edge-connected crate must be attributed: {related:?}"
        );
        assert_eq!(related[0].0.name, "crate_c_score_suite");
    }

    #[test]
    fn given_same_named_tests_when_finding_related_then_orders_by_file_path() {
        let owner = function("src/lib.rs", "score");
        let index = RustIndex {
            tests: vec![
                test("tests/z_case.rs", "score_shared", "score(3)"),
                test("tests/a_case.rs", "score_shared", "score(1)"),
            ],
            ..RustIndex::default()
        };
        let probe = probe("src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 2);
        assert_eq!(related[0].0.file, PathBuf::from("tests/a_case.rs"));
        assert_eq!(related[1].0.file, PathBuf::from("tests/z_case.rs"));
    }

    #[test]
    fn path_substring_match_is_weak_not_same_test_file() {
        let owner = function("crates/core/src/config.rs", "load_config");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![test(
                "crates/core/tests/reconfigure.rs",
                "checks_value",
                "assert_eq!(value, 3);",
            )],
            ..RustIndex::default()
        };
        let probe = probe("crates/core/src/config.rs", "marker");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::WeakTokenSubstring);
    }

    #[test]
    fn canonical_companion_stems_remain_same_test_file() {
        let owner = function("crates/core/src/config.rs", "load_config");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![
                test(
                    "crates/core/tests/config.rs",
                    "config_file",
                    "assert!(true);",
                ),
                test(
                    "crates/core/tests/config_test.rs",
                    "config_test_file",
                    "assert!(true);",
                ),
                test(
                    "crates/core/tests/config_tests.rs",
                    "config_tests_file",
                    "assert!(true);",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = probe("crates/core/src/config.rs", "marker");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 3);
        assert!(
            related
                .iter()
                .all(|(_, reason)| *reason == RelationReason::SameTestFile)
        );
    }

    #[test]
    fn foreign_separator_probe_still_resolves_the_canonical_companion() {
        // The probe path was recorded with Windows separators; the stem
        // extraction must split on both flavors so `config_tests` stays a
        // canonical companion (SameTestFile, medium confidence) instead of
        // degrading to WeakTokenSubstring on a Unix host.
        let owner = function("crates/core/src/config.rs", "load_config");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![test(
                "crates/core/tests/config_tests.rs",
                "config_tests_file",
                "assert!(true);",
            )],
            ..RustIndex::default()
        };
        let probe = probe("crates\\core\\src\\config.rs", "marker");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SameTestFile);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_stems_fail_closed_instead_of_lossily_collapsing() {
        use std::os::unix::ffi::OsStringExt;
        // Two DISTINCT names whose lossy renderings would both collapse to
        // the same `foo<FFFD>` stem: the companion check must not fire.
        let probe_file = std::path::PathBuf::from(std::ffi::OsString::from_vec(
            b"crates/core/src/foo\x80.rs".to_vec(),
        ));
        let test_file = std::path::PathBuf::from(std::ffi::OsString::from_vec(
            b"crates/core/tests/foo\x81_tests.rs".to_vec(),
        ));
        assert!(!super::same_test_file(&probe_file, &test_file));
    }

    #[test]
    fn given_probe_token_in_test_name_when_owner_is_not_called_then_test_is_related() {
        let owner = function("src/lib.rs", "tax_total");
        let index = RustIndex {
            tests: vec![test(
                "tests/tax.rs",
                "vat_boundary_is_checked_by_macro",
                "assert_eq!(macro_tax_case!(100), 120);",
            )],
            ..RustIndex::default()
        };
        let probe = probe("src/lib.rs", "vat >= threshold");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0.name, "vat_boundary_is_checked_by_macro");
    }

    #[test]
    fn given_macro_name_contains_owner_when_no_owner_call_then_test_is_not_directly_related() {
        let owner = function("src/internal.rs", "inner");
        let macro_test = TestSummary {
            name: "macro_wrapper_boundary".to_string(),
            file: PathBuf::from("tests/macro_boundary.rs"),
            start_line: 1,
            end_line: 5,
            body: "let result = call_inner!(10, 3); assert_eq!(result, 7);".to_string(),
            calls: vec![CallFact {
                line: 1,
                name: "call_inner".to_string(),
                text: "call_inner!(10, 3)".to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let index = RustIndex {
            tests: vec![macro_test],
            ..RustIndex::default()
        };
        let probe = probe("src/internal.rs", "if a >= b");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "macro wrapper name containing the owner must not become a direct owner call"
        );
    }

    #[test]
    fn given_body_contains_qualified_owner_call_then_fallback_is_directly_related() {
        let owner = function("src/internal.rs", "inner");
        let call_test = TestSummary {
            name: "public_api_calls_inner".to_string(),
            file: PathBuf::from("tests/public_api.rs"),
            start_line: 1,
            end_line: 5,
            body: "let result = crate_under_test::internal::inner(10, 3);".to_string(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let index = RustIndex {
            tests: vec![call_test],
            ..RustIndex::default()
        };
        let probe = probe("src/internal.rs", "if a >= b");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::DirectOwnerCall);
    }

    #[test]
    fn given_workspace_paths_when_extracting_package_prefix_then_handles_nested_markers() {
        assert_eq!(
            package_prefix(Path::new("crates/foo/src/support/src/lib.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/foo/tests/support/tests/cases.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("vendor/foo/src/support/src/lib.rs")).as_deref(),
            Some("vendor/foo/src/support/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/ripr/examples/sample/src/lib.rs")).as_deref(),
            Some("crates/ripr/examples/sample/")
        );
    }

    #[test]
    fn given_non_workspace_paths_when_extracting_package_prefix_then_returns_none() {
        assert_eq!(package_prefix(Path::new("src/lib.rs")), None);
        assert_eq!(package_prefix(Path::new("tests/basic.rs")), None);
        assert_eq!(package_prefix(Path::new("README.md")), None);
    }

    #[test]
    fn given_mixed_separator_path_when_normalizing_then_uses_workspace_relative_form() {
        let normalized = normalize_path(Path::new("./crates\\ripr\\src\\lib.rs"));
        assert_eq!(normalized, "crates/ripr/src/lib.rs");
    }

    fn function(file: &str, name: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 3,
            body: String::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn test(file: &str, name: &str, body: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body: body.to_string(),
            calls: vec![CallFact {
                line: 1,
                name: "score".to_string(),
                text: body.to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    /// Like `test` but with a configurable call name — needed for cross-crate
    /// tests where the owner name is not hardcoded to `"score"`.
    /// #3714: an ErrorPath probe on a wrapper seam, for SeamCalleeCall tests.
    /// #3714 round-2 (devin hIL0i): the captured call sits AFTER the shadow
    /// binding in the body (binding on body line 1, call on body line 2), so
    /// the positional defeat applies.
    fn shadowing_test_with_call_after_binding() -> TestSummary {
        let mut summary = test_with_call(
            "tests/utils.rs",
            "misc_edge_case",
            "fn build() -> usize { 0 }
let try_parse_summary = build();
let r = try_parse_summary;",
            "try_parse_summary",
        );
        summary.calls[0].line = 3;
        summary
    }

    /// #3714 round-2 (devin hIL0i): the captured call sits AFTER the
    /// `let mut` shadow binding (binding body line 0, call body line 1).
    fn let_mut_shadowing_test_with_call_after_binding() -> TestSummary {
        let mut summary = test_with_call(
            "tests/utils.rs",
            "misc_edge_case",
            "let mut try_parse_summary = || Ok(3);
let r = try_parse_summary();",
            "try_parse_summary",
        );
        summary.calls[0].line = 2;
        summary
    }

    fn wrapper_error_probe(file: &str, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:wrapper".to_string()),
            location: SourceLocation::new(file, 5, 1),
            owner: Some(SymbolId(format!("{file}::owner"))),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Value,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    #[test]
    fn given_generic_named_test_calling_seam_callee_when_wrapper_probe_then_related_seam_callee_call()
     {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![
                function("src/lib.rs", "parse_summary"),
                function("src/lib.rs", "try_parse_summary"),
            ],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "let result = try_parse_summary(\"@bad;\");",
                "try_parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
    }

    #[test]
    fn given_generic_named_test_calling_nothing_when_wrapper_probe_then_not_related() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "other_helper();",
                "other_helper",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "a test that calls nothing relevant must not relate"
        );
    }

    #[test]
    fn given_test_calling_both_owner_and_callee_when_wrapper_probe_then_direct_owner_call_wins() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![
                function("src/lib.rs", "parse_summary"),
                function("src/lib.rs", "try_parse_summary"),
            ],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "let a = parse_summary(\"x\"); let b = try_parse_summary(\"y\");",
                "parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::DirectOwnerCall);
    }

    #[test]
    fn given_test_defining_shadow_fn_when_wrapper_probe_then_no_seam_callee_call() {
        // Round-1 review (devin hC): a same-named fn defined in the test
        // body impersonates the seam callee; the admit is defeated.
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "fn try_parse_summary(raw: &str) -> usize { raw.len() }
let r = try_parse_summary(\"x\");",
                "try_parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "a same-named local fn must not establish SeamCalleeCall"
        );
    }

    #[test]
    fn given_test_shadowing_callee_with_let_when_wrapper_probe_then_no_seam_callee_call() {
        // Round-1 review (devin hC): a local binding named like the callee
        // also defeats the name-level admit (#2972 defeat discipline).
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![shadowing_test_with_call_after_binding()],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "a local binding shadowing the callee must not establish SeamCalleeCall"
        );
    }

    // Round-2 review (devin hDROE): `let mut <callee>` binds a mutable
    // local; the pattern-region scan covers the binding modifier.
    #[test]
    fn given_test_shadowing_callee_with_let_mut_when_wrapper_probe_then_no_seam_callee_call() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![let_mut_shadowing_test_with_call_after_binding()],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "a mutable local binding shadowing the callee must not establish SeamCalleeCall"
        );
    }

    // --- #3727 Slice A: parser-backed shadow authority (flag law) ---
    //
    // The relation consumes the test's parser-produced fact fields on
    // parser-backed files and the byte-level lexical scanners under
    // fallback. Each test below summarizes a REAL test file through the
    // parser producer and then varies only the fallback condition on the
    // file's `FileFacts` (or drops the entry entirely), so the two
    // authorities face byte-identical inputs.

    /// Wrapper-seam scenario over one real test-file source: the index is
    /// built from the parser producer, then `adapt` varies the fallback
    /// condition on the file's `FileFacts`.
    fn shadow_flag_law_index(
        source: &str,
        adapt: impl FnOnce(&mut FileFacts),
    ) -> Result<(RustIndex, Probe), String> {
        let mut file_facts = crate::analysis::syntax::ra::summarize_file_with_parser(
            std::path::Path::new("tests/utils.rs"),
            source,
        )
        .map_err(|error| error.to_string())?;
        adapt(&mut file_facts);
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![owner],
            tests: file_facts.tests.clone(),
            files: std::iter::once((PathBuf::from("tests/utils.rs"), file_facts)).collect(),
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");
        Ok((index, probe))
    }

    // #3739 review (coderabbit h6ce-): the shadowing binding's use must be
    // a CAPTURED CALL (`try_parse_summary()`), not a bare read —
    // `extract_call_facts` records only call expressions, so a bare read
    // never reaches the exact-name call gate and the negative tests below
    // would pass vacuously, before either shadow authority runs. The
    // binding stays initialized and names the callee, so the gate is
    // reached and BOTH authorities must then defeat it.
    const SHADOWING_TEST_SOURCE: &str = "fn build() -> usize { 0 }\n\n#[test]\nfn misc_edge_case() {\n    let try_parse_summary = || build();\n    let r = try_parse_summary();\n}\n";

    #[test]
    fn given_parser_backed_file_when_facts_shadow_callee_then_no_seam_callee_call()
    -> Result<(), String> {
        let (index, probe) = shadow_flag_law_index(SHADOWING_TEST_SOURCE, |_facts| {})?;
        let owner = index.functions[0].clone();

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "parser-backed facts must defeat the shadowed seam-callee admit: {related:?}"
        );
        Ok(())
    }

    /// Removal control (#3727 acceptance): with the parser-path facts
    /// DELETED — the file absent from the index — the consumer falls back
    /// to the lexical scanners and the #3728 pin still holds (defeat, no
    /// over-credit).
    #[test]
    fn given_parser_facts_deleted_when_wrapper_probe_then_lexical_fallback_still_defeats()
    -> Result<(), String> {
        let (mut index, probe) = shadow_flag_law_index(SHADOWING_TEST_SOURCE, |_facts| {})?;
        assert!(
            index
                .files
                .remove(&PathBuf::from("tests/utils.rs"))
                .is_some(),
            "the removal control deletes the parser-backed file facts"
        );
        let owner = index.functions[0].clone();

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "with facts deleted the lexical fallback must still defeat the shadowed admit"
        );
        Ok(())
    }

    /// Removal control variant: the file falls back explicitly
    /// (`used_lexical_fallback = true`, fact fields empty, exactly what the
    /// lexical producer emits) — the lexical scanners decide and the pin
    /// still holds.
    #[test]
    fn given_lexical_fallback_file_when_wrapper_probe_then_lexical_scanners_still_defeats()
    -> Result<(), String> {
        let (index, probe) = shadow_flag_law_index(SHADOWING_TEST_SOURCE, |facts| {
            facts.used_lexical_fallback = true;
            for test in &mut facts.tests {
                test.nested_fn_names = Vec::new();
                test.let_bindings = Vec::new();
            }
        })?;
        let owner = index.functions[0].clone();

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "a fallback file must route through the lexical scanners, which still defeat"
        );
        Ok(())
    }

    /// Empty fact sets on a parser-backed file are REAL "no shadow"
    /// results: a clean wrapper-callee call stays related through the
    /// fact authority (the flag — not set emptiness — is the
    /// discriminator).
    #[test]
    fn given_parser_backed_file_with_no_shadow_facts_then_seam_callee_call_stays_related()
    -> Result<(), String> {
        let clean_source = "#[test]\nfn misc_edge_case() {\n    let result = try_parse_summary(\"@bad;\");\n    assert!(result.is_err());\n}\n";
        let (index, probe) = shadow_flag_law_index(clean_source, |_facts| {})?;
        let owner = index.functions[0].clone();

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1, "a clean callee call must stay related");
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
        Ok(())
    }

    // #3714 round-2 review (coderabbit hGkkh, critical): a `let ` occurrence
    // inside a string literal or an initializer-less binding must not hang
    // the scan. Pre-fix, the cursor never advanced past the occurrence and
    // the loop spun forever; the suite would time out on this input.
    #[test]
    fn shadow_scan_terminates_on_string_and_initializer_less_let_occurrences() {
        // `let ` inside a string literal near the end of the body.
        let string_occurrence = "assert_eq!(message, \"let x\");";
        assert!(
            test_body_let_shadow_line(string_occurrence, "try_parse_summary").is_none(),
            "a `let ` inside a string literal is not a shadow"
        );
        // An initializer-less binding (`let flag;`) has no depth-zero `=`.
        let initializer_less = "let flag;
if flag { }";
        assert!(
            test_body_let_shadow_line(initializer_less, "try_parse_summary").is_none(),
            "an initializer-less binding is not a shadow"
        );
    }

    // #3714 round-2 review (devin hIL0i), positive pin: a `let` binding
    // shadows only uses at or after its own line, so a captured call that
    // PRECEDES the shadow binding is a real seam-callee call and the
    // `SeamCalleeCall` relation survives. The whole-body defeat this
    // refinement replaces dropped exactly this relation.
    #[test]
    fn given_captured_call_before_shadow_binding_when_wrapper_probe_then_relation_survives() {
        let owner = function("src/lib.rs", "parse_summary");
        // Same body as `shadowing_test_with_call_after_binding`, but the
        // captured call sits on file line 1 — before the binding on file
        // line 2 (test `start_line` is 1, binding body line 1).
        let mut summary = test_with_call(
            "tests/utils.rs",
            "misc_edge_case",
            "fn build() -> usize { 0 }
let try_parse_summary = build();
let r = try_parse_summary;",
            "try_parse_summary",
        );
        summary.calls[0].line = 1;
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![summary],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
    }

    // #3728 round-3 review (coderabbit, devin): shadow-shaped text in a
    // comment must not defeat a real call — the scans run on a masked body.
    #[test]
    fn given_shadow_fn_shape_in_comment_when_wrapper_probe_then_relation_survives() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "// fn try_parse_summary(raw: &str) -> usize { 0 }
let r = try_parse_summary(\"x\");",
                "try_parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
    }

    // #3728 round-3 review (devin): a `let <callee> = ..` shape mentioned
    // inside a string literal opens before the scanner's region and used to
    // defeat a real call; masking erases string contents first.
    #[test]
    fn given_shadow_let_shape_in_string_when_wrapper_probe_then_relation_survives() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "let note = \"let try_parse_summary = earlier\";
let r = try_parse_summary(\"x\");",
                "try_parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
    }

    // #3728 round-4 review (devin): a quote character literal before the
    // shadow definition must not blind the mask — the shared
    // mask_comments_and_strings masks char literals wholesale, so the
    // `fn <callee>` item still defeats the captured call. A stripper that
    // treats the literal's quote byte as a string opener would erase the
    // definition and fabricate the relation.
    #[test]
    fn given_quote_char_literal_before_shadow_fn_when_wrapper_probe_then_no_relation() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "let quote = '\"';
fn try_parse_summary(raw: &str) -> usize { raw.len() }
let r = try_parse_summary(\"x\");",
                "try_parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "a real fn shadow behind a char literal must still defeat the admit"
        );
    }

    // #3728 round-5 review (devin): an initializer-less declaration must
    // not borrow a later binding's `=` — the pattern scan stops at the
    // declaration's own depth-zero `;`, so a captured call between the two
    // declarations keeps its relation. Pre-fix, `let flag;` reported the
    // LATER `let <callee> = ..` shadow line as its own and defeated the
    // call in between.
    #[test]
    fn given_initializer_less_declaration_between_calls_when_wrapper_probe_then_relation_survives()
    {
        let owner = function("src/lib.rs", "parse_summary");
        let mut summary = test_with_call(
            "tests/utils.rs",
            "misc_edge_case",
            "try_parse_summary(\"setup\");
let flag;
try_parse_summary(\"real\");
let try_parse_summary = build();",
            "try_parse_summary",
        );
        // The captured call sits between the initializer-less declaration
        // (body line 1) and the real same-named binding (body line 3).
        summary.calls[0].line = 3;
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![summary],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
    }

    // #3728 round-5 review (devin): ordinary strings may span lines; the
    // shared masker keeps `Str` state across the newline, so shadow-shaped
    // string text on a continuation line cannot impersonate a local fn.
    #[test]
    fn given_shadow_shape_in_multiline_string_when_wrapper_probe_then_relation_survives() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "let note = \"first
fn try_parse_summary()\";
let r = try_parse_summary(\"x\");",
                "try_parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
    }

    // #3714 round-2 review (coderabbit hGkkm): a binding whose name merely
    // BEGINS with the callee is a different binding — the unbounded prefix
    // check would falsely defeat the admit and drop the test's relation.
    #[test]
    fn given_near_name_binding_when_wrapper_probe_then_relation_survives() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![
                function("src/lib.rs", "parse_summary"),
                function("src/lib.rs", "try_parse_summary"),
            ],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "let try_parse_summary_result = try_parse_summary(\"x\");",
                "try_parse_summary",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].1, RelationReason::SeamCalleeCall);
    }

    // #3714 round-2 review (devin hGdAZ): harmless outer grouping must not
    // hide the wrapper conversion — the parenthesized form still attributes
    // the callee and (via the shared scanner) still carries the typed
    // limitation gate.
    #[test]
    fn given_parenthesized_wrapper_when_expecting_callee_then_extracts_callee() {
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "(try_parse_summary(raw)).map_err(Into::into)"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "return (try_parse_summary(raw).map_err(Into::into));"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "{ try_parse_summary(raw).map_err(Into::into) }"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        // Grouping that does not span the whole expression stays fail-closed.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "(a) + (try_parse_summary(raw).map_err(Into::into))"
            ))
            .as_deref(),
            None
        );
    }

    #[test]
    fn given_unrelated_function_sharing_token_when_wrapper_probe_then_no_seam_callee_call() {
        let owner = function("src/lib.rs", "parse_summary");
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "parse_summary")],
            tests: vec![test_with_call(
                "tests/utils.rs",
                "misc_edge_case",
                "try_parse_summary_impl(\"@bad;\");",
                "try_parse_summary_impl",
            )],
            ..RustIndex::default()
        };
        let probe = wrapper_error_probe("src/lib.rs", "try_parse_summary(raw).map_err(Into::into)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "a near-name callee must not establish SeamCalleeCall: captured call names match exactly"
        );
    }

    #[test]
    fn given_parseable_variant_seam_when_probe_then_no_seam_callee_attribution() {
        // A parseable `Err(V)` construction IS the variant identity; the
        // pre-existing variant-bound paths own attribution there.
        assert!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "return Err(ParseSummaryError::MalformedSource);"
            ))
            .is_none()
        );
        // The wrapper seam itself yields the converted callee.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "try_parse_summary(raw).map_err(Into::into)"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        // Chained receivers resolve to the last segment; non-call shapes fail
        // closed; `return ` prefixes are stripped.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "self.client().try_parse_summary(raw) .map_err (Into::into)"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        assert!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "value.map_err(Into::into)"
            ))
            .is_none()
        );
        // #3714 round-1 review (devin f2 / coderabbit glUM-f0): the
        // return-form display recovers the callee.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "return try_parse_summary(raw).map_err(Into::into)"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        // #3714 round-1 review (devin ik): an argument's own nested
        // conversion must not be mistaken for the seam conversion — the LAST
        // top-level conversion's receiver is the callee.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "validate(raw.map_err(wrap)?, other).map_err(Into::into)"
            ))
            .as_deref(),
            Some("validate")
        );
        // A conversion mentioned inside a string literal is not top-level.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "report(\"use .map_err(x) here\").map_err(Into::into)"
            ))
            .as_deref(),
            Some("report")
        );
        // #3714 round-2 review (devin hDRNH): lifetimes and labels are not
        // character literals — the scanner must not swallow the conversion
        // behind them.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "parse::<View<'_>>(raw).map_err(Into::into)"
            ))
            .as_deref(),
            Some("parse")
        );
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "try_parse_summary::<'a>(raw).map_err(Into::into)"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "'outer: loop { break 'outer; }
try_parse_summary(raw).map_err(Into::into)"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
        // An escaped-quote character literal is still a char literal.
        assert_eq!(
            wrapper_seam_callee(&wrapper_error_probe(
                "src/lib.rs",
                "skip('\'');
try_parse_summary(raw).map_err(Into::into)"
            ))
            .as_deref(),
            Some("try_parse_summary")
        );
    }

    fn test_with_call(file: &str, name: &str, body: &str, call_name: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body: body.to_string(),
            calls: vec![CallFact {
                line: 1,
                name: call_name.to_string(),
                text: body.to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn probe(file: &str, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new(file, 2, 1),
            owner: Some(SymbolId(format!("{file}::owner"))),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    /// A probe with no function owner — models a module-level struct field.
    fn struct_field_probe(file: &str, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:struct-field".to_string()),
            location: SourceLocation::new(file, 46, 1),
            owner: None,
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Value,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn oracle_fact(assertion: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: assertion.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(assertion),
            ok_value_observed: None,
        }
    }

    fn test_with_assertions(
        file: &str,
        name: &str,
        body: &str,
        assertions: Vec<OracleFact>,
    ) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 10,
            body: body.to_string(),
            calls: Vec::new(),
            assertions,
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    // --- #1052 repro ---
    // A module-level struct-field probe (owner_fn=None) whose changed expression
    // token (`open_in`) appears in the assertion observed_tokens of an exact-value
    // oracle test.  The oracle test MUST be selected as related.
    #[test]
    fn given_struct_field_probe_when_assertion_references_field_token_then_oracle_test_is_related()
    {
        let assertion_text = "assert_eq!(lane.open_in, OpenIn::Browser);";
        let oracle_test = test_with_assertions(
            "crates/ripr/tests/repo_lane.rs",
            "repo_lane_deserializes_fields_correctly",
            &format!("let lane = RepoLane {{ open_in: OpenIn::Browser, .. }}; {assertion_text}"),
            vec![oracle_fact(
                assertion_text,
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let index = RustIndex {
            tests: vec![oracle_test],
            ..RustIndex::default()
        };
        // Struct field probe: expression contains `open_in` (7 chars, >= 5)
        let probe = struct_field_probe("crates/ripr/src/config.rs", "open_in");

        let related = find_related_tests(&probe, None, &index, true, None, None);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0.name, "repo_lane_deserializes_fields_correctly");
    }

    #[test]
    fn given_foreign_separator_file_stem_when_associated_then_assertion_text_is_preserved() {
        let assertion_text = "assert_eq!(value, 3);";
        let oracle_test = test_with_assertions(
            "crates/ripr/tests/config.rs",
            "checks_value",
            assertion_text,
            vec![oracle_fact(
                assertion_text,
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let index = RustIndex {
            tests: vec![oracle_test],
            ..RustIndex::default()
        };
        let forward_probe = struct_field_probe("crates/ripr/src/config.rs", "marker");
        let windows_probe = struct_field_probe("crates\\ripr\\src\\config.rs", "marker");

        let forward_related = find_related_tests(&forward_probe, None, &index, true, None, None);
        let windows_related = find_related_tests(&windows_probe, None, &index, true, None, None);

        assert_eq!(forward_related.len(), 1);
        assert_eq!(windows_related.len(), 1);
        assert_eq!(forward_related[0].0.name, windows_related[0].0.name);
        assert_eq!(forward_related[0].1, RelationReason::SameTestFile);
        assert_eq!(windows_related[0].1, RelationReason::SameTestFile);
        assert_eq!(
            forward_related[0].0.assertions[0].text, assertion_text,
            "path normalization must not rewrite assertion text"
        );
        assert_eq!(
            windows_related[0].0.assertions[0].text, assertion_text,
            "path normalization must not rewrite assertion text"
        );
    }

    #[test]
    fn given_dotfile_probe_when_associated_then_only_matching_companion_is_related() {
        let matching_test = test_with_assertions(
            "crates/ripr/tests/.config.rs",
            "checks_config_value",
            "assert_eq!(value, 3);",
            Vec::new(),
        );
        let unrelated_test = test_with_assertions(
            "crates/ripr/tests/config.rs",
            "checks_value",
            "assert_eq!(value, 3);",
            Vec::new(),
        );
        let index = RustIndex {
            tests: vec![matching_test, unrelated_test],
            ..RustIndex::default()
        };
        let forward_probe = struct_field_probe("crates/ripr/src/.config", "marker");
        let windows_probe = struct_field_probe("crates\\ripr\\src\\.config", "marker");

        for probe in [forward_probe, windows_probe] {
            let related = find_related_tests(&probe, None, &index, true, None, None);

            assert_eq!(related.len(), 1);
            assert_eq!(related[0].0.file, Path::new("crates/ripr/tests/.config.rs"));
            assert_eq!(related[0].1, RelationReason::SameTestFile);
        }
    }

    #[test]
    #[cfg(unix)]
    fn given_non_utf8_stems_when_classify_extracts_then_lossy_collision_fails_closed() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        // Two distinct non-UTF-8 leaf names share one lossy rendering;
        // deriving stems through lossy conversion would fabricate
        // file-name affinity between unrelated entities, so extraction
        // must fail closed instead (#3599).
        let first = Path::new(OsStr::from_bytes(b"crates/app/src/pricing_\xff.rs"));
        let second = Path::new(OsStr::from_bytes(b"crates/app/src/pricing_\xfe.rs"));
        assert_eq!(normalized_file_stem(first), "");
        assert_eq!(normalized_file_stem(second), "");
        // Valid-UTF-8 stems are unchanged, including dotfiles and
        // `./`-prefixed forms that `normalize_path` trims.
        assert_eq!(normalized_file_stem(Path::new("src/config.rs")), "config");
        assert_eq!(normalized_file_stem(Path::new("./src/config.rs")), "config");
    }

    // --- #1054 repro ---
    // Two sibling struct-field probes (`open_in` and `open_cap`) are both
    // asserted by the SAME exact-value oracle test.  Both MUST select that test
    // consistently (no asymmetry).
    #[test]
    fn given_sibling_struct_field_probes_when_both_asserted_then_both_select_same_oracle_test() {
        let assertion_open_in = "assert_eq!(lane.open_in, OpenIn::Browser);";
        let assertion_open_cap = "assert_eq!(lane.open_cap, 8);";
        let body =
            format!("let lane = RepoLane::default(); {assertion_open_in} {assertion_open_cap}");
        let oracle_test = test_with_assertions(
            "crates/ripr/tests/repo_lane.rs",
            "repo_lane_fields_have_expected_defaults",
            &body,
            vec![
                oracle_fact(
                    assertion_open_in,
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                ),
                oracle_fact(
                    assertion_open_cap,
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                ),
            ],
        );
        let index = RustIndex {
            tests: vec![oracle_test],
            ..RustIndex::default()
        };

        let probe_open_in = struct_field_probe("crates/ripr/src/config.rs", "open_in");
        let probe_open_cap = struct_field_probe("crates/ripr/src/config.rs", "open_cap");

        let related_in = find_related_tests(&probe_open_in, None, &index, true, None, None);
        let related_cap = find_related_tests(&probe_open_cap, None, &index, true, None, None);

        assert_eq!(
            related_in.len(),
            1,
            "open_in probe must find the oracle test"
        );
        assert_eq!(
            related_cap.len(),
            1,
            "open_cap probe must find the oracle test"
        );
        assert_eq!(
            related_in[0].0.name, related_cap[0].0.name,
            "sibling fields must select the same oracle test"
        );
    }

    // --- Anti-over-association: short 4-char token rejected ---
    // A struct-field probe whose only shared token with a test assertion is a
    // common 4-char name (`port`) must NOT select that test via the
    // assertions_reference_owner signal — the >= 5 threshold rejects it.
    // The test and probe live in different-stem files and the test name does
    // not contain the probe token, so same_file_or_named also does not fire.
    #[test]
    fn given_struct_field_probe_when_shared_token_is_4_chars_then_assertions_signal_does_not_fire()
    {
        // Test file stem "rendering" != probe file stem "scheduler".
        // Test name "rendering_is_initialized" does not contain "port".
        let assertion_text = "assert_eq!(srv.port, 8080);";
        let unrelated_test = test_with_assertions(
            "crates/ripr/tests/rendering.rs",
            "rendering_is_initialized",
            assertion_text,
            vec![oracle_fact(
                assertion_text,
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let index = RustIndex {
            tests: vec![unrelated_test],
            ..RustIndex::default()
        };
        // Probe expression is `port` (4 chars) — below the >= 5 threshold.
        let probe = struct_field_probe("crates/ripr/src/scheduler.rs", "port");

        let related = find_related_tests(&probe, None, &index, true, None, None);

        assert!(
            related.is_empty(),
            "a 4-char shared token must not trigger assertions_reference_owner"
        );
    }

    // --- Anti-over-association: owner_fn=Some blocks the signal ---
    // When owner_fn is Some, assertions_reference_owner must NOT fire even if
    // probe tokens appear in assertion observed_tokens.  The calls_owner / name
    // proximity gates govern function-bodied probes.
    #[test]
    fn given_function_owner_probe_when_assertion_references_probe_token_then_signal_does_not_fire()
    {
        let owner = function("src/lib.rs", "discounted_total");
        let assertion_text =
            "assert_eq!(token_label(\"discount_threshold\"), \"token:discount_threshold\");";
        let unrelated_test = test_with_assertions(
            "tests/tokens.rs",
            "token_label_includes_token_text",
            assertion_text,
            vec![oracle_fact(
                assertion_text,
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let index = RustIndex {
            tests: vec![unrelated_test],
            ..RustIndex::default()
        };
        // Probe expression contains `discount_threshold` (>= 5 chars) which is
        // also in the assertion observed_tokens — but owner_fn is Some, so the
        // assertions_reference_owner signal must NOT fire.
        let probe = probe("src/lib.rs", "amount >= discount_threshold");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "assertions_reference_owner must not fire when owner_fn is Some"
        );
    }

    #[test]
    fn absolute_root_probe_matches_relative_companion_test_file() -> Result<(), String> {
        let index = RustIndex {
            tests: vec![test(
                "src/tests/gate_watchdog_tests.rs",
                "terminal_states_are_exact",
                "classify_gate_watchdog(&input);",
            )],
            ..RustIndex::default()
        };
        let probe = struct_field_probe(
            "/repo/ub-review/src/gate_watchdog.rs",
            "pub(crate) state: GateWatchdogState",
        );

        let related = find_related_tests(&probe, None, &index, true, None, None);
        let Some((test, reason)) = related.first() else {
            return Err(
                "absolute root probe did not match its relative companion test".to_string(),
            );
        };
        if test.file != Path::new("src/tests/gate_watchdog_tests.rs")
            || *reason != RelationReason::SameTestFile
        {
            return Err(format!("unexpected companion-test relation: {related:?}"));
        }
        Ok(())
    }

    #[test]
    fn absolute_workspace_probe_keeps_relative_cross_crate_guard() -> Result<(), String> {
        let index = RustIndex {
            tests: vec![test(
                "crates/other/tests/state_tests.rs",
                "state_is_exact",
                "state();",
            )],
            ..RustIndex::default()
        };
        let probe = struct_field_probe(
            "/repo/ripr/crates/core/src/state.rs",
            "pub(crate) state: State",
        );

        let related = find_related_tests(&probe, None, &index, true, None, None);
        if !related.is_empty() {
            return Err(format!("cross-crate test must stay unrelated: {related:?}"));
        }
        Ok(())
    }

    /// #3235: two non-`crates/` packages addressed by absolute paths carry no
    /// derivable package scope. Before the fail-closed guard the prefix came
    /// back `None` and the package scoping was silently disabled, admitting
    /// wrong-package name/token coincidences. The fixture is a pure weak
    /// match — the test never calls the owner, only its name contains it.
    #[test]
    fn given_absolute_owner_and_absolute_wrong_package_test_when_weak_match_then_filtered() {
        let owner = function("/ws/pkg-a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![test_with_call(
                "/ws/pkg-b/tests/coincidence.rs",
                "pkg_b_score_named_test",
                "observe(&input);",
                "observe",
            )],
            ..RustIndex::default()
        };
        let probe = probe("/ws/pkg-a/src/lib.rs", "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "wrong-package weak match across unscopable absolute paths must be filtered: {related:?}"
        );
    }

    /// #3235 positive control: the #2971 strong path still admits a
    /// cross-package test that calls a uniquely-named owner even when both
    /// paths are absolute — failing closed must not break the deliberate
    /// unique-owner bypass.
    #[test]
    fn given_absolute_paths_when_unique_owner_is_called_cross_package_then_retained() {
        let owner = function("/ws/pkg-a/src/lib.rs", "compute_hash");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![test_with_call(
                "/ws/pkg-b/tests/integration.rs",
                "hash_integration_test",
                "let result = compute_hash(b\"input\");",
                "compute_hash",
            )],
            ..RustIndex::default()
        };
        let probe = probe("/ws/pkg-a/src/lib.rs", "compute_hash(input)");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert_eq!(
            related.len(),
            1,
            "uniquely-owned call must stay admitted across absolute paths"
        );
        assert_eq!(related[0].1, RelationReason::DirectOwnerCall);
    }

    /// #3235: the Windows drive-prefixed form must fail closed through the
    /// same authority as the Unix-rooted form on every host. Pure weak match:
    /// the test never calls the owner. The tokens are built at runtime so no
    /// literal drive-letter path sits in the source tree.
    #[test]
    fn given_drive_prefixed_owner_and_absolute_wrong_package_test_when_weak_match_then_filtered() {
        let owner_file = format!("F:{}ws/pkg-a/src/lib.rs", '/');
        let wrong_package_test = format!("F:{}ws/pkg-b/tests/coincidence.rs", '/');
        let owner = function(&owner_file, "score");
        let index = RustIndex {
            functions: vec![owner.clone()],
            tests: vec![test_with_call(
                &wrong_package_test,
                "pkg_b_score_named_test",
                "observe(&input);",
                "observe",
            )],
            ..RustIndex::default()
        };
        let probe = probe(&owner_file, "score + 1");

        let related = find_related_tests(&probe, Some(&owner), &index, true, None, None);

        assert!(
            related.is_empty(),
            "drive-prefixed wrong-package weak match must be filtered: {related:?}"
        );
    }

    /// #3235 struct-probe symmetry: an unscopable-absolute module-level probe
    /// against an absolute wrong-package test fails closed too.
    #[test]
    fn given_absolute_struct_probe_and_absolute_wrong_package_test_when_weak_match_then_filtered()
    -> Result<(), String> {
        let index = RustIndex {
            tests: vec![test(
                "/ws/pkg-b/tests/gate_watchdog_tests.rs",
                "gate_watchdog_is_exact",
                "observe(&input);",
            )],
            ..RustIndex::default()
        };
        let probe = struct_field_probe("/ws/pkg-a/src/gate_watchdog.rs", "pub(crate) state: State");

        let related = find_related_tests(&probe, None, &index, true, None, None);
        if !related.is_empty() {
            return Err(format!(
                "struct-probe wrong-package weak match must be filtered: {related:?}"
            ));
        }
        Ok(())
    }

    /// #3717: string masking must preserve balanced quotes so string-aware
    /// downstream scanners over masked source do not treat trailing code
    /// as an unclosed string.
    #[test]
    fn given_source_with_string_literals_when_stripped_then_quotes_remain_balanced() {
        let source = "let x = \"hello world\";\nlet y = \"escaped \\\" quote\";\nuse crate::bar;";
        let stripped = strip_comments_and_strings(source);
        let quote_count = stripped.chars().filter(|&c| c == '"').count();
        assert_eq!(
            quote_count % 2,
            0,
            "quotes in stripped output must be balanced, got {quote_count}: {stripped:?}"
        );
        assert!(
            stripped.contains("use crate::bar;"),
            "post-string statement must remain visible outside quotes: {stripped:?}"
        );
        // Delimiters preserved, content blanked
        assert!(
            stripped.starts_with("let x = \"           \";"),
            "string contents replaced with spaces: {stripped:?}"
        );
    }
}

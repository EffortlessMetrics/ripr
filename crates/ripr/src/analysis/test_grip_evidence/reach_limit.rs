//! Unknown, not a gap, for seam reach (#5411).
//!
//! A seam with no related test reads `ungripped` only when ripr established
//! that no test path reaches its owner. Two analyzer limits leave that
//! negative unestablished, the same limits `ripr check` already names on
//! `no_static_path` findings:
//!
//! - a bounded transitive or macro path from a test toward the owner exists
//!   but is unresolved (RIPR-SPEC-0114, RIPR-SPEC-0118);
//! - the owner is a trait-impl method, which operators, formatting macros
//!   and generic calls run without naming it, and test-reached code names
//!   the impl's self type.
//!
//! Either one makes the reach stage `opaque` with the witness named, so the
//! seam classifies `opaque` (an unknown with a limit) instead of a gap. The
//! witness is a candidate path, never a related test: it adds no reach,
//! activation or oracle credit.

use super::related_tests::{CompactGripContext, strip_comments_and_strings};
use crate::analysis::classify::{
    MAX_TRANSITIVE_DEPTH, MacroReachWitness, TransitiveWitness, impl_self_type_name,
    is_trait_impl_method, macro_reach_limit_kind, transitive_reach_limit_kind,
};
use crate::analysis::rust_index::{FunctionSummary, TestSummary};
use crate::domain::{Confidence, StageEvidence, StageState};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Where an unresolved trait-dispatch path starts: a test that names the
/// self type, or a test-reached production function that does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TypeMention {
    Test {
        name: String,
        file: PathBuf,
        line: usize,
    },
    Function {
        name: String,
        file: PathBuf,
        line: usize,
        /// Reached only through another trait method, not a test's calls.
        via_dispatch: bool,
    },
}

/// A trait-impl method whose self type test-reached code names. Trait
/// dispatch may run it, and the functions it calls, without any call naming
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DispatchRoot {
    method: String,
    self_type: String,
    file: PathBuf,
    line: usize,
}

/// First mention of each type-shaped identifier by a test or by code a test
/// may run, and the functions reached only through trait dispatch. Built
/// once per run on first use.
#[derive(Default)]
pub(in crate::analysis::test_grip_evidence) struct TypeMentionIndex {
    /// Best mention per identifier and its rank (lower wins): a test that
    /// builds or calls through the type, then a test that only names it
    /// (`assert_send_sync::<T>()`), then test-reached production code.
    mentions: BTreeMap<String, (TypeMention, u8)>,
    /// Owner id to the trait method it may run from.
    dispatch_reached: BTreeMap<String, DispatchRoot>,
}

impl TypeMentionIndex {
    pub(in crate::analysis::test_grip_evidence) fn build(context: &CompactGripContext<'_>) -> Self {
        let mut index = Self::default();
        let mut tests: Vec<_> = context.tests.iter().map(|indexed| indexed.test).collect();
        tests.sort_by(|a, b| {
            (&a.file, a.start_line, &a.name).cmp(&(&b.file, b.start_line, &b.name))
        });
        for test in tests {
            index.add_test_mentions(test);
        }
        let reach = &context.transitive_reach;
        let mut reached: BTreeSet<&str> = BTreeSet::new();
        let mut functions = reach.test_reached_functions();
        sort_functions(&mut functions);
        for function in functions {
            reached.insert(function.id.0.as_str());
            index.add_function_mentions(function, false);
        }

        // A dispatch root's callees may name further types whose trait
        // methods are roots in turn. Each round is one more hop, bounded
        // like the transitive walk.
        let mut trait_methods: Vec<&FunctionSummary> = reach
            .production_functions()
            .filter(|function| is_trait_impl_method(function))
            .collect();
        sort_functions(&mut trait_methods);
        for _ in 0..MAX_TRANSITIVE_DEPTH {
            let mut grew = false;
            for &method in &trait_methods {
                if reached.contains(method.id.0.as_str()) {
                    continue;
                }
                let Some(self_type) = impl_self_type_name(&method.id.0) else {
                    continue;
                };
                if !index.mentions.contains_key(&self_type) {
                    continue;
                }
                let root = DispatchRoot {
                    method: method.name.clone(),
                    self_type,
                    file: method.file.clone(),
                    line: method.start_line,
                };
                reached.insert(method.id.0.as_str());
                index.add_function_mentions(method, true);
                let mut callees = reach.functions_reached_from(&method.calls, MAX_TRANSITIVE_DEPTH);
                sort_functions(&mut callees);
                for callee in callees {
                    if reached.insert(callee.id.0.as_str()) {
                        index
                            .dispatch_reached
                            .insert(callee.id.0.clone(), root.clone());
                        index.add_function_mentions(callee, true);
                    }
                }
                grew = true;
            }
            if !grew {
                break;
            }
        }
        index
    }

    fn add_function_mentions(&mut self, function: &FunctionSummary, via_dispatch: bool) {
        for (token, _) in identifiers(&function.body) {
            self.mentions.entry(token).or_insert_with(|| {
                let mention = TypeMention::Function {
                    name: function.name.clone(),
                    file: function.file.clone(),
                    line: function.start_line,
                    via_dispatch,
                };
                (mention, 2)
            });
        }
    }

    fn add_test_mentions(&mut self, test: &TestSummary) {
        for (token, used_as_value) in identifiers(&test.body) {
            let rank = if used_as_value { 0 } else { 1 };
            if self
                .mentions
                .get(&token)
                .is_some_and(|(_, existing)| *existing <= rank)
            {
                continue;
            }
            let mention = TypeMention::Test {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.start_line,
            };
            self.mentions.insert(token, (mention, rank));
        }
    }

    fn mention(&self, type_name: &str) -> Option<&TypeMention> {
        self.mentions.get(type_name).map(|(mention, _)| mention)
    }

    fn dispatch_root(&self, owner_id: &str) -> Option<&DispatchRoot> {
        self.dispatch_reached.get(owner_id)
    }
}

fn sort_functions(functions: &mut [&FunctionSummary]) {
    functions
        .sort_by(|a, b| (&a.file, a.start_line, &a.id.0).cmp(&(&b.file, b.start_line, &b.id.0)));
}

/// Type-shaped identifiers (capitalized) outside comments and strings, each
/// with whether any occurrence builds or calls through it (`T::`, `T(`,
/// `T {`) rather than only naming it in a type position.
fn identifiers(body: &str) -> Vec<(String, bool)> {
    let code = body
        .lines()
        .map(strip_comments_and_strings)
        .collect::<Vec<_>>()
        .join("\n");
    let mut out: BTreeMap<String, bool> = BTreeMap::new();
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut start = None;
    for (index, ch) in code.char_indices().chain([(code.len(), ' ')]) {
        if is_ident(ch) {
            start.get_or_insert(index);
            continue;
        }
        let Some(begin) = start.take() else {
            continue;
        };
        let token = &code[begin..index];
        if !token.chars().next().is_some_and(char::is_uppercase) {
            continue;
        }
        let rest = code[index..].trim_start();
        let used_as_value =
            rest.starts_with("::") || rest.starts_with('(') || rest.starts_with('{');
        let entry = out.entry(token.to_string()).or_insert(false);
        *entry |= used_as_value;
    }
    out.into_iter().collect()
}

/// The reach stage for a seam with no related test: `opaque` with the named
/// limit when a candidate path is unresolved, else the established `no`.
pub(in crate::analysis::test_grip_evidence) fn reach_without_related_tests(
    owner: &str,
    owner_fn: Option<&FunctionSummary>,
    context: &CompactGripContext<'_>,
) -> StageEvidence {
    if let Some(owner_fn) = owner_fn
        && let Some(summary) = unresolved_reach_summary(owner_fn, context)
    {
        return StageEvidence::new(StageState::Opaque, Confidence::Low, summary);
    }
    StageEvidence::new(
        StageState::No,
        Confidence::Medium,
        format!("No static test path found for seam owner `{owner}`"),
    )
}

fn unresolved_reach_summary(
    owner_fn: &FunctionSummary,
    context: &CompactGripContext<'_>,
) -> Option<String> {
    let owner_name = owner_fn.name.as_str();
    if owner_name.is_empty() {
        return None;
    }
    // The witness walks match by name, so every owner sharing a name shares
    // their answer; only the trait-dispatch checks need the owner id.
    if let Some(summary) = cached(context, format!("name:{owner_name}"), || {
        witness_summary(owner_name, context)
    }) {
        return Some(summary);
    }
    cached(context, format!("id:{}", owner_fn.id.0), || {
        trait_dispatch_reach_summary(owner_fn, context)
    })
}

fn cached(
    context: &CompactGripContext<'_>,
    key: String,
    compute: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if let Some(cached) = context.unresolved_reach_cached(&key) {
        return cached;
    }
    let summary = compute();
    context.cache_unresolved_reach(key, summary.clone());
    summary
}

fn witness_summary(owner_name: &str, context: &CompactGripContext<'_>) -> Option<String> {
    if let Some(witness) = context.transitive_reach.transitive_witness(owner_name) {
        return Some(transitive_summary(owner_name, &witness));
    }
    let witness = context.transitive_reach.macro_reach_witness(owner_name)?;
    Some(macro_summary(owner_name, &witness))
}

fn trait_dispatch_reach_summary(
    owner_fn: &FunctionSummary,
    context: &CompactGripContext<'_>,
) -> Option<String> {
    let owner_name = owner_fn.name.as_str();
    let mentions = context.type_mentions();
    if is_trait_impl_method(owner_fn)
        && let Some(self_type) = impl_self_type_name(&owner_fn.id.0)
        && let Some(mention) = mentions.mention(&self_type)
    {
        return Some(trait_dispatch_summary(owner_name, &self_type, mention));
    }
    // A trait method whose own type nothing names may still run from another
    // trait method that delegates to it (`self.inner.fmt(f)`).
    let root = mentions.dispatch_root(&owner_fn.id.0)?;
    let mention = mentions.mention(&root.self_type)?;
    Some(dispatch_reached_summary(owner_name, root, mention))
}

fn location(file: &std::path::Path, line: usize) -> String {
    format!("{}:{line}", file.display().to_string().replace('\\', "/"))
}

fn transitive_summary(owner_name: &str, witness: &TransitiveWitness) -> String {
    format!(
        "Reach unresolved ({}): `{}` ({}) calls `{}`, which may lead to `{owner_name}` through a call path ripr does not fully trace; no gap is reported",
        transitive_reach_limit_kind(&witness.test_file).as_str(),
        witness.test_name,
        location(&witness.test_file, witness.test_line),
        witness.entry_symbol,
    )
}

fn macro_summary(owner_name: &str, witness: &MacroReachWitness) -> String {
    format!(
        "Reach unresolved ({}): `{}` ({}) calls `{}`, which reaches macro `{}!` ({}) that names `{owner_name}`; ripr does not expand macros, so no gap is reported",
        macro_reach_limit_kind(&witness.macro_host).as_str(),
        witness.test_name,
        location(&witness.test_file, witness.test_line),
        witness.entry_symbol,
        witness.macro_name,
        location(&witness.macro_file, witness.macro_line),
    )
}

fn mention_text(self_type: &str, mention: &TypeMention) -> String {
    match mention {
        TypeMention::Test { name, file, line } => {
            format!(
                "test `{name}` ({}) uses `{self_type}`",
                location(file, *line)
            )
        }
        TypeMention::Function {
            name,
            file,
            line,
            via_dispatch,
        } => format!(
            "`{name}` ({}), which {}, uses `{self_type}`",
            location(file, *line),
            if *via_dispatch {
                "may itself run through trait dispatch"
            } else {
                "tests may call"
            }
        ),
    }
}

fn trait_dispatch_summary(owner_name: &str, self_type: &str, mention: &TypeMention) -> String {
    format!(
        "Reach unresolved (trait dispatch): `{owner_name}` is a trait method of `{self_type}`, which operators, formatting and generic calls run without naming it; {}. ripr does not trace trait dispatch, so no gap is reported",
        mention_text(self_type, mention)
    )
}

fn dispatch_reached_summary(
    owner_name: &str,
    root: &DispatchRoot,
    mention: &TypeMention,
) -> String {
    format!(
        "Reach unresolved (trait dispatch): `{owner_name}` may run from `{}` ({}), a trait method of `{}` that operators, formatting and generic calls run without naming it; {}. ripr does not trace trait dispatch, so no gap is reported",
        root.method,
        location(&root.file, root.line),
        root.self_type,
        mention_text(&root.self_type, mention)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_keep_type_shaped_tokens_outside_comments_and_strings() {
        let tokens = identifiers(
            "let v = Version::parse(\"Prerelease\"); // BuildMetadata\nassert_send_sync::<Op>();",
        );
        assert_eq!(
            tokens,
            vec![("Op".to_string(), false), ("Version".to_string(), true)]
        );
    }
}

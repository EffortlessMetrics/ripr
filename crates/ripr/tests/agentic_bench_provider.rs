//! B5 provider-contract corpus benchmark.
//!
//! Runs the JSON corpus in `benchmarks/agentic/provider-corpus/` through
//! the public `ripr::provider_contract` validators. Every invalid case is
//! exactly one mutation away from its valid baseline (enforced below),
//! so a failure names the single rejected field. See the corpus README
//! for the oracle table.

use std::path::PathBuf;

use ripr::provider_contract::{
    RiprAnalysisReceiptV1, RiprAnalysisRequestV1, RiprProviderCapabilitySetV1,
    RiprProviderContractErrorCodeV1,
};

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/agentic/provider-corpus")
}

fn read_json(path: &std::path::Path) -> Result<serde_json::Value, String> {
    let bytes =
        std::fs::read(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|err| format!("parse {} failed: {err}", path.display()))
}

/// Applies one corpus mutation: a JSON-pointer set, remove, or `/-` append.
fn apply_mutation(
    document: &mut serde_json::Value,
    mutation: &serde_json::Value,
) -> Result<(), String> {
    let pointer = mutation
        .get("pointer")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("mutation lacks a string pointer: {mutation}"))?;
    if mutation
        .get("remove")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return remove_pointer(document, pointer);
    }
    let value = mutation
        .get("value")
        .cloned()
        .ok_or_else(|| format!("mutation lacks a value: {mutation}"))?;
    if let Some(parent_pointer) = pointer.strip_suffix("/-") {
        let parent = if parent_pointer.is_empty() {
            document
        } else {
            document
                .pointer_mut(parent_pointer)
                .ok_or_else(|| format!("mutation parent {parent_pointer} does not resolve"))?
        };
        let items = parent
            .as_array_mut()
            .ok_or_else(|| format!("mutation parent {parent_pointer} is not an array"))?;
        items.push(value);
        return Ok(());
    }
    let slot = document
        .pointer_mut(pointer)
        .ok_or_else(|| format!("mutation pointer {pointer} does not resolve"))?;
    *slot = value;
    Ok(())
}

/// Removes the value at `pointer` (object key or array index).
fn remove_pointer(document: &mut serde_json::Value, pointer: &str) -> Result<(), String> {
    let (parent_pointer, token) = pointer
        .rsplit_once('/')
        .ok_or_else(|| format!("mutation pointer {pointer} has no parent"))?;
    let token = token.replace("~1", "/").replace("~0", "~");
    let parent = if parent_pointer.is_empty() {
        document
    } else {
        document
            .pointer_mut(parent_pointer)
            .ok_or_else(|| format!("mutation parent {parent_pointer} does not resolve"))?
    };
    if let Some(object) = parent.as_object_mut() {
        object
            .remove(&token)
            .ok_or_else(|| format!("mutation key {pointer} is absent"))?;
        return Ok(());
    }
    if let Some(items) = parent.as_array_mut() {
        let index: usize = token
            .parse()
            .map_err(|_index| format!("mutation index {pointer} is not a number"))?;
        if index >= items.len() {
            return Err(format!("mutation index {pointer} is out of bounds"));
        }
        items.remove(index);
        return Ok(());
    }
    Err(format!(
        "mutation parent {parent_pointer} is not a container"
    ))
}

fn expected_outcome(
    case: &serde_json::Value,
) -> Result<Option<RiprProviderContractErrorCodeV1>, String> {
    let name = case
        .get("expect")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("case lacks an expect code: {case}"))?;
    match name {
        "Ok" => Ok(None),
        "MissingField" => Ok(Some(RiprProviderContractErrorCodeV1::MissingField)),
        "UnsupportedSchema" => Ok(Some(RiprProviderContractErrorCodeV1::UnsupportedSchema)),
        "DuplicateCapability" => Ok(Some(RiprProviderContractErrorCodeV1::DuplicateCapability)),
        "AuthorityViolation" => Ok(Some(RiprProviderContractErrorCodeV1::AuthorityViolation)),
        "UnsafeOutputRoot" => Ok(Some(RiprProviderContractErrorCodeV1::UnsafeOutputRoot)),
        "IdentityMismatch" => Ok(Some(RiprProviderContractErrorCodeV1::IdentityMismatch)),
        "CompletenessConflict" => Ok(Some(RiprProviderContractErrorCodeV1::CompletenessConflict)),
        "MalformedIdentity" => Ok(Some(RiprProviderContractErrorCodeV1::MalformedIdentity)),
        other => Err(format!("unknown expect code: {other}")),
    }
}

#[test]
fn provider_contract_corpus_accepts_valid_and_rejects_one_mutation_invalid() -> Result<(), String> {
    let root = corpus_root();
    let corpus = read_json(&root.join("corpus.json"))?;
    if corpus
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some("agentic_provider_corpus.v1")
    {
        return Err(format!(
            "corpus schema drifted: {}",
            corpus
                .get("schema_version")
                .unwrap_or(&serde_json::Value::Null)
        ));
    }
    let bases = corpus
        .get("bases")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "corpus lacks a bases map".to_string())?;
    let cases = corpus
        .get("cases")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "corpus lacks a cases array".to_string())?;
    if cases.is_empty() {
        return Err("corpus has no cases".to_string());
    }
    let mut failures: Vec<String> = Vec::new();
    for case in cases {
        let id = case
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<missing id>");
        let outcome = run_case(&root, bases, case);
        if let Err(err) = outcome {
            failures.push(format!("{id}: {err}"));
        }
    }
    if failures.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} of {} corpus cases failed:\n  {}",
        failures.len(),
        cases.len(),
        failures.join("\n  ")
    ))
}

fn run_case(
    root: &std::path::Path,
    bases: &serde_json::Map<String, serde_json::Value>,
    case: &serde_json::Value,
) -> Result<(), String> {
    let base = case
        .get("base")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("case lacks a base: {case}"))?;
    let file = bases
        .get(base)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("unknown base: {base}"))?;
    let mutations = case
        .get("mutations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("case lacks a mutations array: {case}"))?;
    let expected = expected_outcome(case)?;
    // The corpus contract: an invalid case is exactly one mutation away
    // from valid. `valid-identity` variants (a paired id+digest) are the
    // only multi-mutation cases, and they must validate.
    let variant = case.get("variant").and_then(serde_json::Value::as_str);
    if expected.is_some() {
        if mutations.len() != 1 {
            return Err(format!(
                "invalid case must carry exactly one mutation, got {}",
                mutations.len()
            ));
        }
    } else if variant != Some("valid-identity") && !mutations.is_empty() {
        return Err("valid case must carry no mutations without a variant".to_string());
    }
    let mut document = read_json(&root.join(file))?;
    for mutation in mutations {
        apply_mutation(&mut document, mutation)?;
    }
    let actual = match base {
        "request" => {
            let typed: RiprAnalysisRequestV1 = serde_json::from_value(document)
                .map_err(|err| format!("untyped request: {err}"))?;
            typed.validate().err().map(|err| err.code)
        }
        "receipt" => {
            let typed: RiprAnalysisReceiptV1 = serde_json::from_value(document)
                .map_err(|err| format!("untyped receipt: {err}"))?;
            typed.validate().err().map(|err| err.code)
        }
        "capabilities" => {
            let typed: RiprProviderCapabilitySetV1 = serde_json::from_value(document)
                .map_err(|err| format!("untyped capabilities: {err}"))?;
            typed.validate().err().map(|err| err.code)
        }
        other => return Err(format!("unknown base: {other}")),
    };
    if actual != expected {
        return Err(format!(
            "expected {expected:?}, validator returned {actual:?}"
        ));
    }
    Ok(())
}

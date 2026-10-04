use super::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) document: Value,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    pub(crate) fn new() -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-live-head-contract-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // Shared CARGO_TARGET_DIR can leave the repo-local configured TMPDIR
        // absent. Create only its parent, then require an exclusively new root.
        if let Some(parent) = root.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create fixture parent: {error}"))?;
        }
        std::fs::create_dir(&root).map_err(|error| format!("create owned fixture: {error}"))?;
        let mut fixture = Self {
            root,
            document: json!({}),
        };
        let sha = "a".repeat(40);
        let evidence = |name: &str, owner, bytes: &[u8]| -> Result<Value, String> {
            std::fs::write(fixture.root.join(name), bytes)
                .map_err(|error| format!("write fixture evidence: {error}"))?;
            Ok(json!({"owner_issue": owner, "path": name, "sha256": digest(bytes)}))
        };
        let selection = evidence("selection.json", 2766, b"{\"synthetic\":\"selection\"}")?;
        let denominator = evidence("denominator.json", 2768, b"{\"synthetic\":\"denominator\"}")?;
        let audit = evidence("audit.json", 3807, b"{\"synthetic\":\"audit\"}")?;
        let readback = evidence("pin-remote.sha", 1609, format!("{sha}\n").as_bytes())?;
        let ruleset_bytes = serde_json::to_vec(&json!({
            "name": "release-transaction-pins", "target": "tag", "enforcement": "active",
            "conditions": {"ref_name": {"include": ["refs/tags/ripr-release-*"], "exclude": []}},
            "rules": [{"type": "update"}, {"type": "deletion"}], "bypass_actors": []
        }))
        .map_err(|error| error.to_string())?;
        let ruleset = evidence("ruleset.json", 1609, &ruleset_bytes)?;
        let harness = evidence("harness.json", 4510, b"{\"synthetic\":\"harness\"}")?;
        let contract = evidence(
            "blind-contract.json",
            4603,
            b"{\"synthetic\":\"contract0200\"}",
        )?;
        let accepted = |packet: Value| {
            json!({
                "acceptance": {"status": "accepted", "candidate_sha": sha, "candidate_tree": "b".repeat(40),
                    "reviewed_packet_sha256": packet["sha256"],
                    "decision_ref": format!("https://github.com/EffortlessMetrics/ripr-swarm/issues/{}#issuecomment-1", packet["owner_issue"])},
                "packet": packet
            })
        };
        fixture.document = json!({
            "schema_version": SCHEMA, "kind": KIND, "release_line": "0.11.0",
            "authority_issue": 2379, "candidate_owner_issue": 1609,
            "status": "pinned_exact_head",
            "candidate": {"repository": REPOSITORY, "sha": sha, "tree": "b".repeat(40),
                "ref": format!("refs/tags/ripr-release-0.11.0-{sha}"),
                "package": {"name": "ripr", "version": "0.11.0",
                    "workspace_manifest_sha256": digest(b"workspace"),
                    "package_manifest_sha256": digest(b"package"), "lock_sha256": digest(b"lock")}},
            "range": {"last_integrated_swarm_parent": LAST_INTEGRATED,
                "all_reachable_count": 3, "first_parent_count": 2,
                "all_reachable_sha256": "c".repeat(64), "first_parent_sha256": "d".repeat(64),
                "record_set_sha256": "e".repeat(64)},
            "prerequisites": {"selected_claims": accepted(selection), "denominator": accepted(denominator), "audit": accepted(audit)},
            "pin": {"remote_ref_readback": readback, "ruleset": ruleset},
            "qualification": {"state": "required_not_run", "required_execution_owners": [2769, 4508, 4604],
                "proof_inputs": [harness, contract]},
            "source_parent": null, "non_claims": ["synthetic fixture, not release acceptance"]
        });
        Ok(fixture)
    }

    pub(crate) fn write(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec_pretty(&self.document).map_err(|error| error.to_string())?;
        std::fs::write(self.root.join("manifest.json"), &bytes)
            .map_err(|error| error.to_string())?;
        Ok(digest(&bytes))
    }

    fn admit(&self, approved: &str) -> Result<LiveHeadSnapshot, String> {
        LiveHeadSnapshot::admit(&self.root, "0.11.0", Path::new("manifest.json"), approved)
    }
}

fn refusal(result: Result<LiveHeadSnapshot, String>, expected: &str) -> Result<(), String> {
    match result {
        Err(error) if error.contains(expected) => Ok(()),
        Err(error) => Err(format!("wrong refusal: {error}; expected {expected}")),
        Ok(_) => Err(format!("unexpected admission; expected {expected}")),
    }
}

#[test]
fn direct_manifest_custody_does_not_require_historical_registry() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let approved = fixture.write()?;
    let admitted = fixture.admit(&approved)?;
    if fixture.root.join("policy/release-targets.toml").exists()
        || fixture
            .root
            .join("docs/release-candidates/index.json")
            .exists()
        || admitted.candidate_sha()? != "a".repeat(40)
        || admitted.candidate_tree()? != "b".repeat(40)
    {
        return Err("direct subject was not independently admitted".to_string());
    }
    admitted.revalidate()?;
    Ok(())
}

#[test]
fn producer_output_without_independently_accepted_digest_is_not_authority() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let _generated_digest = fixture.write()?;
    refusal(fixture.admit(""), "independently accepted manifest digest")?;
    refusal(
        fixture.admit(&"0".repeat(64)),
        "independently accepted raw digest",
    )
}

#[test]
fn direct_manifest_refuses_templates_missing_bindings_and_predicted_source() -> Result<(), String> {
    for (pointer, replacement, reason) in [
        (
            "/status",
            json!("active_selection_template"),
            "not pinned_exact_head",
        ),
        ("/schema_version", json!("1.0"), "unsupported"),
        ("/candidate", Value::Null, "candidate binding missing"),
        ("/candidate/tree", json!(""), "candidate tree"),
        (
            "/candidate/repository",
            json!("EffortlessMetrics/ripr"),
            "repository/ref/package",
        ),
        (
            "/candidate/ref",
            json!("refs/heads/main"),
            "repository/ref/package",
        ),
        (
            "/candidate/package/version",
            json!("0.11.1"),
            "repository/ref/package",
        ),
        (
            "/prerequisites",
            Value::Null,
            "prerequisite bindings missing",
        ),
        (
            "/prerequisites/audit/packet/owner_issue",
            json!(1609),
            "evidence owner/path",
        ),
        (
            "/prerequisites/denominator/packet/sha256",
            json!(""),
            "evidence digest",
        ),
        (
            "/range/first_parent_count",
            json!(0),
            "denominator is empty",
        ),
        ("/pin", Value::Null, "pin bindings missing"),
        (
            "/source_parent",
            json!("f".repeat(40)),
            "must not predict SOURCE_PARENT",
        ),
        ("/qualification/state", json!("passed"), "required_not_run"),
        (
            "/qualification/required_execution_owners",
            json!([]),
            "required_not_run",
        ),
    ] {
        let mut fixture = Fixture::new()?;
        *fixture
            .document
            .pointer_mut(pointer)
            .ok_or_else(|| format!("fixture pointer missing: {pointer}"))? = replacement;
        let approved = fixture.write()?;
        refusal(fixture.admit(&approved), reason)?;
    }
    Ok(())
}

#[test]
fn direct_manifest_revalidation_refuses_raw_packet_or_manifest_substitution() -> Result<(), String>
{
    for relative in [
        "manifest.json",
        "selection.json",
        "denominator.json",
        "audit.json",
        "harness.json",
    ] {
        let fixture = Fixture::new()?;
        let approved = fixture.write()?;
        let admitted = fixture.admit(&approved)?;
        let path = fixture.root.join(relative);
        let mut bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
        bytes.push(b' ');
        std::fs::write(path, bytes).map_err(|error| error.to_string())?;
        if admitted.revalidate().is_ok() {
            return Err(format!("substituted {relative} remained admitted"));
        }
    }
    Ok(())
}

#[test]
fn direct_manifest_refuses_wrong_readback_and_unprotected_pin() -> Result<(), String> {
    for mutate_ruleset in [false, true] {
        let mut fixture = Fixture::new()?;
        let (path, bytes, pointer, expected) = if mutate_ruleset {
            (
                "ruleset.json",
                b"{}".to_vec(),
                "/pin/ruleset/sha256",
                "does not protect",
            )
        } else {
            (
                "pin-remote.sha",
                format!("{}\n", "f".repeat(40)).into_bytes(),
                "/pin/remote_ref_readback/sha256",
                "another candidate",
            )
        };
        std::fs::write(fixture.root.join(path), &bytes).map_err(|error| error.to_string())?;
        *fixture
            .document
            .pointer_mut(pointer)
            .ok_or_else(|| "missing pin digest".to_string())? = json!(digest(&bytes));
        let approved = fixture.write()?;
        refusal(fixture.admit(&approved), expected)?;
    }
    Ok(())
}

#[test]
fn direct_manifest_binds_actual_committed_package_inputs() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let approved = fixture.write()?;
    let admitted = fixture.admit(&approved)?;
    let mut blobs = BTreeMap::from([
        ("Cargo.toml".to_string(), b"workspace".to_vec()),
        ("crates/ripr/Cargo.toml".to_string(), b"package".to_vec()),
        ("Cargo.lock".to_string(), b"lock".to_vec()),
    ]);
    admitted.verify_package_inputs("ripr", &blobs)?;
    blobs.insert(
        "Cargo.lock".to_string(),
        b"another same-version graph".to_vec(),
    );
    if admitted.verify_package_inputs("ripr", &blobs).is_ok() {
        return Err("same-version foreign lock accepted".to_string());
    }
    Ok(())
}

#[test]
fn direct_manifest_rejects_unknown_fields_and_future_self_issued_grants() -> Result<(), String> {
    let mut fixture = Fixture::new()?;
    fixture.document["self_issued_acceptance"] = json!("approved because the producer wrote this");
    let approved = fixture.write()?;
    refusal(fixture.admit(&approved), "unknown field")
}

#[test]
fn direct_manifest_hash_match_cannot_supply_missing_or_stale_owner_acceptance() -> Result<(), String>
{
    for (pointer, replacement) in [
        (
            "/prerequisites/audit/acceptance/status",
            json!("not_established"),
        ),
        (
            "/prerequisites/selected_claims/acceptance/candidate_sha",
            json!("f".repeat(40)),
        ),
        (
            "/prerequisites/denominator/acceptance/candidate_tree",
            json!("f".repeat(40)),
        ),
        (
            "/prerequisites/audit/acceptance/reviewed_packet_sha256",
            json!("f".repeat(64)),
        ),
        (
            "/prerequisites/audit/acceptance/decision_ref",
            json!("local-sidecar.json"),
        ),
    ] {
        let mut fixture = Fixture::new()?;
        *fixture
            .document
            .pointer_mut(pointer)
            .ok_or_else(|| format!("missing pointer {pointer}"))? = replacement;
        // Matching raw identity is deliberately insufficient without the exact accepted envelope.
        let approved = fixture.write()?;
        refusal(fixture.admit(&approved), "not_established")?;
    }
    let mut fixture = Fixture::new()?;
    fixture.document["prerequisites"]["audit"]
        .as_object_mut()
        .ok_or_else(|| "missing audit object".to_string())?
        .remove("acceptance");
    let approved = fixture.write()?;
    refusal(fixture.admit(&approved), "missing field")
}

#[path = "source_tests.rs"]
mod source_tests;

#[path = "budget_tests.rs"]
mod budget_tests;

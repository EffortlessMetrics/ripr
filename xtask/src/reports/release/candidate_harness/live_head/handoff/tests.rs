use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

struct Fixture {
    manifest: super::super::tests::Fixture,
    input: HandoffInput,
    native: BTreeMap<String, Value>,
    selection: Value,
    qualification: Value,
    bundle: Value,
}

#[test]
fn native_handoff_parses_crlf_without_changing_raw_decision_identity() -> Result<(), String> {
    let payload = json!({"fixture": "raw-body-identity"});
    let raw = body(&payload)?.replace("\n", "\r\n");
    let decision = native::decode_native(
        &reference(1609),
        1609,
        &serde_json::to_vec(&response(1609, raw.clone())).map_err(|error| error.to_string())?,
    )?;
    if decision.payload::<Value>()? != payload || decision.body_sha256 != digest(raw.as_bytes()) {
        return Err("CRLF parsing changed the payload or raw decision digest".to_string());
    }
    let retained = serde_json::to_value(decision).map_err(|error| error.to_string())?;
    if retained["body"] != json!(raw) {
        return Err("CRLF parsing changed the retained native body".to_string());
    }
    Ok(())
}

fn reference(owner: u64) -> String {
    format!("https://github.com/EffortlessMetrics/ripr-swarm/issues/{owner}#issuecomment-{owner}")
}

fn response(owner: u64, body: String) -> Value {
    json!({"id": owner, "html_url": reference(owner),
        "issue_url": format!("https://api.github.com/repos/EffortlessMetrics/ripr-swarm/issues/{owner}"),
        "body": body, "user": {"login": "release-controller"}, "author_association": "COLLABORATOR"})
}

fn body(payload: &Value) -> Result<String, String> {
    Ok(format!(
        "Reviewed native owner decision.\n```ripr-release-acceptance\n{}\n```\n",
        serde_json::to_string_pretty(payload).map_err(|error| error.to_string())?
    ))
}

impl Fixture {
    fn new() -> Result<Self, String> {
        let mut manifest = super::super::tests::Fixture::new()?;
        manifest.document["prerequisites"]["selected_claims"]["acceptance"]["decision_ref"] =
            json!(reference(2766));
        let approved = manifest.write()?;
        let subject = json!({"candidate_sha": manifest.document["candidate"]["sha"],
            "candidate_tree": manifest.document["candidate"]["tree"],
            "candidate_ref": manifest.document["candidate"]["ref"], "manifest_sha256": approved});
        let rows = json!([
            {"id": "complete-matrix", "owner_issue": 2769},
            {"id": "selected-installed-journey", "owner_issue": 4508},
            {"id": "blind-execution", "owner_issue": 4604}
        ]);
        let claims_body = "Accepted exact terminal selection packet after review.";
        let selection = json!({"schema_version": 1, "kind": "ripr_native_selection_acceptance",
            "status": "accepted", "subject": subject,
            "selected_claims": manifest.document["prerequisites"]["selected_claims"],
            "selected_claims_decision_sha256": digest(claims_body.as_bytes()),
            "excluded_subjects": [], "required_execution_owners": manifest.document["qualification"]["required_execution_owners"],
            "proof_inputs": manifest.document["qualification"]["proof_inputs"], "required_qualification_rows": rows});
        let selection_digest = digest(body(&selection)?.as_bytes());
        let mut result_rows = Vec::new();
        for (owner, id) in [
            (2769, "complete-matrix"),
            (4508, "selected-installed-journey"),
            (4604, "blind-execution"),
        ] {
            let path = format!("result-{owner}.json");
            let bytes = format!("synthetic reviewed results for {owner}");
            std::fs::write(manifest.root.join(&path), &bytes).map_err(|error| error.to_string())?;
            result_rows.push(json!({"id": id, "owner_issue": owner, "status": "passed",
                "selected": 3, "executed": 3, "failed": 0, "skipped": 0,
                "packet": {"owner_issue": owner, "path": path, "sha256": digest(bytes.as_bytes())}}));
        }
        let bundle = json!({"schema_version": 1, "kind": "ripr_complete_qualification_bundle",
            "status": "qualified", "subject": subject, "selection_decision": reference(1609),
            "selection_decision_sha256": selection_digest, "excluded_subjects": [], "rows": result_rows});
        let qualification = json!({"schema_version": 1, "kind": "ripr_native_qualification_acceptance",
            "status": "qualified", "subject": subject, "selection_decision": reference(1609),
            "selection_decision_sha256": selection_digest, "qualification_bundle_sha256": "",
            "required_qualification_rows": rows});
        let input = HandoffInput {
            controller_root: manifest.root.clone(),
            manifest: "manifest.json".into(),
            selection_decision: reference(1609),
            qualification_bundle: "qualification.json".into(),
            qualification_decision: reference(2769),
        };
        let mut fixture = Self {
            manifest,
            input,
            selection,
            qualification,
            bundle,
            native: BTreeMap::from([(reference(2766), response(2766, claims_body.to_string()))]),
        };
        fixture.publish_selection()?;
        fixture.publish_bundle()?;
        Ok(fixture)
    }

    fn publish_selection(&mut self) -> Result<(), String> {
        self.native
            .insert(reference(1609), response(1609, body(&self.selection)?));
        Ok(())
    }

    fn publish_qualification(&mut self) -> Result<(), String> {
        self.native
            .insert(reference(2769), response(2769, body(&self.qualification)?));
        Ok(())
    }

    fn publish_bundle(&mut self) -> Result<(), String> {
        let bytes = serde_json::to_vec(&self.bundle).map_err(|error| error.to_string())?;
        std::fs::write(self.manifest.root.join("qualification.json"), &bytes)
            .map_err(|error| error.to_string())?;
        self.qualification["qualification_bundle_sha256"] = json!(digest(&bytes));
        self.publish_qualification()
    }

    fn admit(&self) -> Result<AdmittedHandoff, String> {
        admit_with(&self.input, "0.11.0", |reference, owner| {
            let value = self
                .native
                .get(reference)
                .ok_or_else(|| "native decision unavailable".to_string())?;
            let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
            native::decode_native(reference, owner, &bytes)
        })
    }

    fn refuses(&self, reason: &str) -> Result<(), String> {
        match self.admit() {
            Err(error) if error.contains(reason) => Ok(()),
            Err(error) => Err(format!("wrong refusal {error}; expected {reason}")),
            Ok(_) => Err(format!("handoff admitted the negative control: {reason}")),
        }
    }
}

#[test]
fn native_handoff_accepts_independently_selected_applicable_subset() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let admitted = fixture.admit()?;
    if admitted.receipt.rows.len() != 3
        || admitted.receipt.required_execution_owners != [2769, 4508, 4604]
        || admitted.manifest.manifest.qualification.state != "required_not_run"
    {
        return Err("applicable subset or immutable freeze-time state changed".to_string());
    }
    Ok(())
}

#[test]
fn native_handoff_retains_original_raw_inputs_for_downstream_replay() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let admitted = fixture.admit()?;
    let receipt = &admitted.receipt;
    assert_eq!(receipt.schema, "ripr.source_handoff_acceptance.v2");
    let wire = serde_json::to_value(receipt).map_err(|error| error.to_string())?;
    for (field, bytes) in [
        ("manifest_bytes", &receipt.manifest_bytes),
        (
            "qualification_bundle_bytes",
            &receipt.qualification_bundle_bytes,
        ),
    ] {
        let encoded = wire[field]
            .as_str()
            .ok_or_else(|| format!("{field} is not a hex string"))?;
        assert_eq!(encoded.len(), 2 * bytes.len());
        assert_eq!(
            encoded,
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
    }
    assert_eq!(
        receipt.manifest_bytes,
        std::fs::read(fixture.manifest.root.join("manifest.json"))
            .map_err(|error| error.to_string())?
    );
    assert_eq!(
        digest(&receipt.manifest_bytes),
        receipt.subject.manifest_sha256
    );
    assert_eq!(
        receipt.qualification_bundle_bytes,
        std::fs::read(fixture.manifest.root.join("qualification.json"))
            .map_err(|error| error.to_string())?
    );
    assert_eq!(
        digest(&receipt.qualification_bundle_bytes),
        receipt.qualification_bundle_sha256
    );
    let bundle: Value = serde_json::from_slice(&receipt.qualification_bundle_bytes)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        bundle["rows"],
        serde_json::to_value(&receipt.rows).map_err(|error| error.to_string())?
    );
    let expected_paths = admitted
        .manifest
        .raw
        .keys()
        .filter(|path| path.as_str() != "manifest.json")
        .cloned()
        .chain(receipt.rows.iter().map(|row| row.packet.path.clone()))
        .collect::<BTreeSet<_>>();
    assert!(!expected_paths.is_empty());
    assert_eq!(
        receipt
            .packet_inputs
            .iter()
            .map(|packet| packet.path.clone())
            .collect::<BTreeSet<_>>(),
        expected_paths
    );
    for packet in &receipt.packet_inputs {
        assert_eq!(
            packet.bytes,
            std::fs::read(fixture.manifest.root.join(&packet.path))
                .map_err(|error| error.to_string())?
        );
        assert_eq!(digest(&packet.bytes), packet.sha256);
    }
    for packet in wire["packet_inputs"]
        .as_array()
        .ok_or_else(|| "wire packets missing".to_string())?
    {
        assert!(packet["bytes"].is_string());
    }
    // Removing one retained packet cannot satisfy the original packet denominator.
    let mut omitted = receipt.packet_inputs.clone();
    let _removed = omitted
        .pop()
        .ok_or_else(|| "no retained inputs exercised".to_string())?;
    assert_ne!(
        omitted
            .iter()
            .map(|packet| packet.path.clone())
            .collect::<BTreeSet<_>>(),
        expected_paths
    );
    let mut changed = receipt.packet_inputs.clone();
    let packet = changed
        .first_mut()
        .ok_or_else(|| "no retained packet exercised".to_string())?;
    packet.bytes.push(b'!');
    assert_ne!(digest(&packet.bytes), packet.sha256);
    let retained_size = receipt.manifest_bytes.len()
        + receipt.qualification_bundle_bytes.len()
        + receipt
            .packet_inputs
            .iter()
            .map(|packet| packet.bytes.len())
            .sum::<usize>();
    assert!(retained_size <= MAX_BUNDLE_BYTES as usize);
    Ok(())
}

#[test]
fn native_handoff_revalidation_refuses_changed_retained_row_bytes() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let admitted = fixture.admit()?;
    std::fs::write(
        fixture.manifest.root.join("result-4508.json"),
        "changed after admission",
    )
    .map_err(|error| error.to_string())?;
    let result = admitted.revalidate_with("0.11.0", |reference, owner| {
        let response = fixture
            .native
            .get(reference)
            .ok_or_else(|| "missing fixture response".to_string())?;
        native::decode_native(
            reference,
            owner,
            &serde_json::to_vec(response).map_err(|error| error.to_string())?,
        )
    });
    assert!(result.is_err_and(|error| error.contains("packet changed")));
    Ok(())
}

#[test]
fn native_handoff_refuses_absent_decisions_and_generic_successful_ci() -> Result<(), String> {
    for owner in [1609, 2766, 2769] {
        let mut fixture = Fixture::new()?;
        fixture.native.remove(&reference(owner));
        fixture.refuses("unavailable")?;
    }
    let mut fixture = Fixture::new()?;
    fixture.native.insert(
        reference(2769),
        response(
            2769,
            body(&json!({"status":"completed", "conclusion":"success", "headSha":"a".repeat(40)}))?,
        ),
    );
    fixture.refuses("native acceptance payload")
}

#[test]
fn native_handoff_refuses_identity_roster_proof_and_decision_drift() -> Result<(), String> {
    for (pointer, value, reason) in [
        (
            "/subject/candidate_sha",
            json!("f".repeat(40)),
            "another candidate",
        ),
        (
            "/subject/candidate_tree",
            json!("f".repeat(40)),
            "another candidate",
        ),
        (
            "/subject/candidate_ref",
            json!("refs/heads/main"),
            "another candidate",
        ),
        (
            "/subject/manifest_sha256",
            json!("f".repeat(64)),
            "accepted raw digest",
        ),
        ("/required_execution_owners", json!([999]), "roster"),
        (
            "/proof_inputs/0/sha256",
            json!("f".repeat(64)),
            "proof-input",
        ),
        (
            "/selected_claims/packet/sha256",
            json!("f".repeat(64)),
            "#2766 packet",
        ),
        (
            "/selected_claims_decision_sha256",
            json!("f".repeat(64)),
            "#2766 decision differs",
        ),
        ("/required_qualification_rows", json!([]), "row denominator"),
        ("/status", json!("pending"), "not accepted"),
    ] {
        let mut fixture = Fixture::new()?;
        *fixture
            .selection
            .pointer_mut(pointer)
            .ok_or_else(|| format!("bad test pointer {pointer}"))? = value;
        fixture.publish_selection()?;
        fixture.refuses(reason)?;
    }
    Ok(())
}

#[test]
fn native_handoff_refuses_omitted_failed_skipped_and_zero_qualification_rows() -> Result<(), String>
{
    for (pointer, value, reason) in [
        ("/rows", json!([]), "row denominator"),
        ("/rows/0/owner_issue", json!(999), "omits or changes"),
        ("/rows/0/status", json!("not_run"), "non-positive"),
        ("/rows/0/selected", json!(0), "non-positive"),
        ("/rows/0/executed", json!(2), "non-positive"),
        ("/rows/0/failed", json!(1), "non-positive"),
        ("/rows/0/skipped", json!(1), "non-positive"),
        ("/status", json!("failed"), "nonterminal"),
        (
            "/subject/manifest_sha256",
            json!("f".repeat(64)),
            "another selection",
        ),
        (
            "/selection_decision_sha256",
            json!("f".repeat(64)),
            "another selection",
        ),
    ] {
        let mut fixture = Fixture::new()?;
        *fixture
            .bundle
            .pointer_mut(pointer)
            .ok_or_else(|| format!("bad test pointer {pointer}"))? = value;
        // Even a matching native bundle digest cannot turn incomplete results into pass.
        fixture.publish_bundle()?;
        fixture.refuses(reason)?;
    }
    Ok(())
}

#[test]
fn native_handoff_refuses_tampered_manifest_bundle_and_packets() -> Result<(), String> {
    for (path, reason) in [
        ("manifest.json", "accepted raw digest"),
        ("qualification.json", "accepted raw digest"),
        ("result-4508.json", "packet changed"),
    ] {
        let fixture = Fixture::new()?;
        std::fs::write(fixture.manifest.root.join(path), "tampered")
            .map_err(|error| error.to_string())?;
        fixture.refuses(reason)?;
    }
    Ok(())
}

#[test]
fn native_handoff_refuses_stale_incomplete_or_unknown_qualification_acceptance()
-> Result<(), String> {
    for (pointer, value) in [
        ("/subject/manifest_sha256", json!("f".repeat(64))),
        ("/selection_decision", json!(reference(1608))),
        ("/selection_decision_sha256", json!("f".repeat(64))),
        ("/status", json!("pending")),
        ("/required_qualification_rows/0/id", json!("different-row")),
    ] {
        let mut fixture = Fixture::new()?;
        *fixture
            .qualification
            .pointer_mut(pointer)
            .ok_or_else(|| format!("bad test pointer {pointer}"))? = value;
        fixture.publish_qualification()?;
        fixture.refuses("complete-bundle acceptance")?;
    }
    for selection in [true, false] {
        let mut fixture = Fixture::new()?;
        if selection {
            fixture.selection["unknown"] = json!(true);
            fixture.publish_selection()?;
        } else {
            fixture.qualification["unknown"] = json!(true);
            fixture.publish_qualification()?;
        }
        fixture.refuses("unknown field")?;
    }
    Ok(())
}

#[test]
fn native_handoff_refuses_wrong_issue_comment_host_and_untrusted_issuer() -> Result<(), String> {
    for owner in [1609, 2766, 2769] {
        for (key, value) in [
            ("id", json!(1)),
            ("html_url", json!(reference(999))),
            (
                "issue_url",
                json!("https://api.github.com/repos/other/repo/issues/1609"),
            ),
            ("author_association", json!("NONE")),
        ] {
            let mut fixture = Fixture::new()?;
            let comment = fixture
                .native
                .get_mut(&reference(owner))
                .ok_or_else(|| "fixture native comment missing".to_string())?;
            comment[key] = value;
            fixture.refuses("identity or trusted issuer")?;
        }
    }
    for reference in [
        "https://example.com/issues/1609#issuecomment-1",
        "https://github.com/EffortlessMetrics/ripr-swarm/issues/2769#issuecomment-1",
    ] {
        if native::decode_native(reference, 1609, b"{}").is_ok() {
            return Err("native selection accepted a wrong owner or host".to_string());
        }
    }
    Ok(())
}

#[test]
fn native_handoff_preserves_explicitly_unselected_configured_subjects() -> Result<(), String> {
    let mut fixture = Fixture::new()?;
    let excluded = json!([{"id": "configured-exclusions", "owner_issue": 2769, "count": 2,
        "disposition": "excluded", "reason": "Two configured non-selected controls, explicitly accepted by #1609"}]);
    fixture.selection["excluded_subjects"] = excluded.clone();
    fixture.publish_selection()?;
    let selection_digest = digest(body(&fixture.selection)?.as_bytes());
    fixture.bundle["selection_decision_sha256"] = json!(selection_digest);
    fixture.qualification["selection_decision_sha256"] = json!(selection_digest);
    fixture.bundle["rows"][0]["selected"] = json!(10968);
    fixture.bundle["rows"][0]["executed"] = json!(10968);
    fixture.bundle["excluded_subjects"] = excluded;
    fixture.publish_bundle()?;
    let _accepted = fixture.admit()?;
    fixture.bundle["excluded_subjects"][0]["count"] = json!(3);
    fixture.publish_bundle()?;
    fixture.refuses("another selection")
}

#[test]
fn native_handoff_rereads_decisions_before_receipt_and_binds_source_identity() -> Result<(), String>
{
    let mut fixture = Fixture::new()?;
    let admitted = fixture.admit()?;
    let error = admitted
        .verify_source(Path::new("missing"), &"f".repeat(40), "refs/heads/main")
        .err()
        .ok_or_else(|| "wrong source handoff identity admitted".to_string())?;
    if !error.contains("candidate/ref differs") {
        return Err(error);
    }
    let response = fixture
        .native
        .get_mut(&reference(2769))
        .ok_or_else(|| "fixture native comment missing".to_string())?;
    let changed = format!(
        "{}\nDecision edited after initial observation.",
        response["body"]
            .as_str()
            .ok_or_else(|| "fixture body missing".to_string())?
    );
    response["body"] = json!(changed);
    let error = admitted
        .revalidate_with("0.11.0", |reference, owner| {
            let value = fixture
                .native
                .get(reference)
                .ok_or_else(|| "native decision unavailable".to_string())?;
            native::decode_native(
                reference,
                owner,
                &serde_json::to_vec(value).map_err(|error| error.to_string())?,
            )
        })
        .err()
        .ok_or_else(|| "changed native decision admitted at output boundary".to_string())?;
    if !error.contains("acceptance changed during preflight") {
        return Err(error);
    }
    Ok(())
}

use super::*;
use serde_json::json;

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn reference(owner: u64) -> String {
    format!("https://github.com/EffortlessMetrics/ripr-swarm/issues/{owner}#issuecomment-{owner}")
}
fn decision(owner: u64, body: &str) -> Value {
    json!({"reference": reference(owner), "body_sha256": digest(body.as_bytes()),
        "author": "fixture-controller", "author_association": "COLLABORATOR", "body": body})
}
fn block(value: &Value) -> Result<String, String> {
    Ok(format!(
        "```ripr-release-acceptance\n{}\n```\n",
        serde_json::to_string(value).map_err(|error| error.to_string())?
    ))
}
pub(crate) fn fixture() -> Result<Value, String> {
    let sha = "a".repeat(40);
    fixture_for_candidate(
        &sha,
        &"b".repeat(40),
        &format!("refs/tags/ripr-release-0.11.0-{sha}"),
    )
}
pub(crate) fn fixture_for_candidate(sha: &str, tree: &str, git_ref: &str) -> Result<Value, String> {
    let mut packet_inputs = Vec::new();
    let mut packet = |path: &str, owner, bytes: &[u8]| {
        let sha256 = digest(bytes);
        packet_inputs.push(json!({"path":path,"sha256":sha256,"bytes":hex_bytes(bytes)}));
        json!({"owner_issue":owner,"path":path,"sha256":sha256})
    };
    let selection_packet = packet("selection.json", 2766, b"local fixture selection packet");
    let denominator = packet(
        "denominator.json",
        2768,
        b"local fixture denominator packet",
    );
    let audit = packet("audit.json", 3807, b"local fixture audit packet");
    let readback = packet("pin-remote.sha", 1609, format!("{sha}\n").as_bytes());
    let ruleset_bytes = serde_json::to_vec(
        &json!({"name":"release-transaction-pins","target":"tag","enforcement":"active",
        "conditions":{"ref_name":{"include":["refs/tags/ripr-release-*"],"exclude":[]}},
        "rules":[{"type":"update"},{"type":"deletion"}],"bypass_actors":[]}),
    )
    .map_err(|error| error.to_string())?;
    let ruleset = packet("ruleset.json", 1609, &ruleset_bytes);
    let proof_inputs = json!([packet(
        "harness.json",
        4510,
        b"local installed custody fixture"
    )]);
    let result_packet = packet(
        "qualification/result.json",
        2769,
        b"local terminal execution fixture",
    );
    let accepted = |packet: Value| {
        json!({"acceptance":{"status":"accepted","candidate_sha":sha,"candidate_tree":tree,
        "reviewed_packet_sha256":packet["sha256"],"decision_ref":reference(packet["owner_issue"].as_u64().unwrap_or_default())},"packet":packet})
    };
    let selected_claims = accepted(selection_packet);
    let manifest = json!({"schema_version":"1.1","kind":"ripr_swarm_live_head_release_authority","release_line":"0.11.0",
        "authority_issue":2379,"candidate_owner_issue":1609,"status":"pinned_exact_head",
        "candidate":{"repository":"EffortlessMetrics/ripr-swarm","sha":sha,"tree":tree,"ref":git_ref,
            "package":{"name":"ripr","version":"0.11.0","workspace_manifest_sha256":digest(b"workspace"),
                "package_manifest_sha256":digest(b"package"),"lock_sha256":digest(b"lock")}},
        "range":{"last_integrated_swarm_parent":"45b56c0957ad7e7360114edceca4b844c85f846e","all_reachable_count":3,
            "first_parent_count":2,"all_reachable_sha256":"c".repeat(64),"first_parent_sha256":"d".repeat(64),"record_set_sha256":"e".repeat(64)},
        "prerequisites":{"selected_claims":selected_claims,"denominator":accepted(denominator),"audit":accepted(audit)},
        "pin":{"remote_ref_readback":readback,"ruleset":ruleset},
        "qualification":{"state":"required_not_run","required_execution_owners":[2769],"proof_inputs":proof_inputs},
        "source_parent":null,"non_claims":["local deterministic fixture, never native acceptance"]});
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    let subject = json!({"candidate_sha":sha,"candidate_tree":tree,"candidate_ref":git_ref,"manifest_sha256":digest(&manifest_bytes)});
    let claims = "Local fixture selected-claims decision; never posted.";
    let required = json!([{"id":"complete", "owner_issue":2769}]);
    let selected = json!({"schema_version":1, "kind":"ripr_native_selection_acceptance", "status":"accepted",
        "subject":subject, "selected_claims":selected_claims,
        "selected_claims_decision_sha256":digest(claims.as_bytes()), "required_execution_owners":[2769],
        "excluded_subjects":[], "proof_inputs":proof_inputs, "required_qualification_rows":required});
    let selection_body = block(&selected)?;
    let rows = json!([{"id":"complete","owner_issue":2769,"status":"passed","selected":3,"executed":3,
        "failed":0,"skipped":0,"packet":result_packet}]);
    let bundle = json!({"schema_version":1,"kind":"ripr_complete_qualification_bundle","status":"qualified",
        "subject":subject,"selection_decision":reference(1609),"selection_decision_sha256":digest(selection_body.as_bytes()),
        "excluded_subjects":[],"rows":rows});
    let qualification_bundle_bytes =
        serde_json::to_vec(&bundle).map_err(|error| error.to_string())?;
    let qualified = json!({"schema_version":1,"kind":"ripr_native_qualification_acceptance","status":"qualified",
        "subject":subject,"selection_decision":reference(1609),"selection_decision_sha256":digest(selection_body.as_bytes()),
        "qualification_bundle_sha256":digest(&qualification_bundle_bytes),"required_qualification_rows":required});
    Ok(
        json!({"schema":"ripr.source_promotion_preflight.v2", "swarm_parent":sha,"swarm_ref_sha":sha,
        "swarm_ref":git_ref,"version_state":{"requested_version":"0.11.0"},
        "acceptance":{"schema":"ripr.source_handoff_acceptance.v2","subject":subject,
            "selection":decision(1609,&selection_body),"selected_claims":decision(2766,claims),
            "qualification":decision(2769,&block(&qualified)?),"qualification_bundle_sha256":digest(&qualification_bundle_bytes),
            "required_execution_owners":[2769],"excluded_subjects":[],"proof_inputs":proof_inputs,"rows":rows,
            "manifest_bytes":hex_bytes(&manifest_bytes),"qualification_bundle_bytes":hex_bytes(&qualification_bundle_bytes),"packet_inputs":packet_inputs,
            "claim":"Local deterministic fixture only"}}),
    )
}
pub(crate) fn responses(preflight: &Value, reference: &str, owner: u64) -> Result<Vec<u8>, String> {
    let receipt = admitted(preflight)?;
    let retained = match owner {
        1609 => receipt.selection,
        2766 => receipt.selected_claims,
        2769 => receipt.qualification,
        _ => return Err("unexpected owner".into()),
    };
    let value = serde_json::to_value(json!({"id":owner,"html_url":reference,
        "issue_url":format!("https://api.github.com/repos/EffortlessMetrics/ripr-swarm/issues/{owner}"),
        "user":{"login":"fixture-controller"},"author_association":"COLLABORATOR",
        "body":preflight["acceptance"][match owner {1609=>"selection",2766=>"selected_claims",_=>"qualification"}]["body"]}))
        .map_err(|error| error.to_string())?;
    if retained.reference != reference {
        return Err("wrong fixture reference".into());
    }
    serde_json::to_vec(&value).map_err(|error| error.to_string())
}
#[test]
fn complete_v2_replays_and_refetches_all_three_native_owners() -> Result<(), String> {
    let fixture = fixture()?;
    validate(&fixture)?;
    let mut seen = Vec::new();
    revalidate_with_reader(&fixture, |reference, owner| {
        seen.push(owner);
        responses(&fixture, reference, owner)
    })?;
    assert_eq!(seen, [1609, 2766, 2769]);
    Ok(())
}
#[test]
fn historical_missing_mismatched_and_incomplete_acceptance_is_rejected() -> Result<(), String> {
    for (pointer, value) in [
        ("/schema", json!("ripr.source_promotion_preflight.v1")),
        ("/acceptance", Value::Null),
        ("/swarm_parent", json!("f".repeat(40))),
        ("/acceptance/subject/manifest_sha256", json!("d".repeat(64))),
        ("/acceptance/subject/candidate_tree", json!("d".repeat(40))),
        ("/acceptance/required_execution_owners", json!([999])),
        ("/acceptance/rows", json!([])),
        ("/acceptance/rows/0/skipped", json!(1)),
        ("/acceptance/rows/0/executed", json!(2)),
        ("/acceptance/rows/0/selected", json!(0)),
        ("/acceptance/rows/0/packet/owner_issue", json!(999)),
        ("/acceptance/rows/0/packet/path", json!("../outside")),
        ("/acceptance/selection/author_association", json!("NONE")),
        ("/acceptance/selection/body_sha256", json!("e".repeat(64))),
        (
            "/acceptance/qualification_bundle_sha256",
            json!("d".repeat(64)),
        ),
    ] {
        let mut fixture = fixture()?;
        *fixture
            .pointer_mut(pointer)
            .ok_or_else(|| format!("fixture pointer {pointer}"))? = value;
        assert!(
            validate(&fixture).is_err(),
            "accepted counterexample {pointer}"
        );
    }
    Ok(())
}
#[test]
fn retained_raw_inputs_discriminate_complete_projection_tampering() -> Result<(), String> {
    for (pointer, value) in [
        (
            "/acceptance/schema",
            json!("ripr.source_handoff_acceptance.v1"),
        ),
        ("/acceptance/manifest_bytes", json!("")),
        ("/acceptance/manifest_bytes", json!("20")),
        ("/acceptance/qualification_bundle_bytes", json!("20")),
        ("/acceptance/packet_inputs", json!([])),
        ("/acceptance/packet_inputs/0/bytes", json!("41")),
        ("/acceptance/packet_inputs/0/sha256", json!("a".repeat(64))),
        ("/acceptance/rows/0/packet/sha256", json!("a".repeat(64))),
    ] {
        let mut fixture = fixture()?;
        *fixture
            .pointer_mut(pointer)
            .ok_or_else(|| format!("fixture pointer {pointer}"))? = value;
        assert!(
            validate(&fixture).is_err(),
            "accepted raw-input tampering {pointer}"
        );
    }
    let mut complete_but_changed = fixture()?;
    complete_but_changed["acceptance"]["rows"][0]["selected"] = json!(4);
    complete_but_changed["acceptance"]["rows"][0]["executed"] = json!(4);
    let error = validate(&complete_but_changed)
        .err()
        .ok_or_else(|| "accepted changed complete rows".to_string())?;
    assert!(
        error.contains("rows differ"),
        "wrong complete-row oracle: {error}"
    );
    for duplicate in [false, true] {
        let mut fixture = fixture()?;
        let packets = fixture["acceptance"]["packet_inputs"]
            .as_array_mut()
            .ok_or_else(|| "fixture packets missing".to_string())?;
        let mut packet = packets[0].clone();
        if !duplicate {
            packet["path"] = json!("unreferenced.json");
        }
        packets.push(packet);
        assert!(
            validate(&fixture).is_err(),
            "accepted repeated/unreferenced packet"
        );
    }
    let mut removed = fixture()?;
    let _ = removed["acceptance"]["packet_inputs"]
        .as_array_mut()
        .ok_or_else(|| "fixture packets missing".to_string())?
        .pop();
    assert!(
        validate(&removed).is_err(),
        "accepted missing required row packet"
    );
    Ok(())
}

#[test]
fn manifest_original_whitespace_is_digest_bound() -> Result<(), String> {
    let mut fixture = fixture()?;
    let encoded = fixture["acceptance"]["manifest_bytes"]
        .as_str()
        .ok_or_else(|| "manifest bytes missing".to_string())?;
    fixture["acceptance"]["manifest_bytes"] = json!(format!("{encoded}0a"));
    assert!(
        validate(&fixture).is_err(),
        "accepted changed original whitespace"
    );
    Ok(())
}

#[test]
fn retained_bytes_wire_codec_requires_lowercase_even_hex_strings() -> Result<(), String> {
    for value in [
        json!([123, 125]),
        json!("a"),
        json!("AA"),
        json!("gg"),
        json!("00 0a"),
        json!(null),
    ] {
        let mut fixture = fixture()?;
        fixture["acceptance"]["manifest_bytes"] = value;
        assert!(
            validate(&fixture).is_err(),
            "accepted invalid raw-byte wire encoding"
        );
    }
    let fixture = fixture()?;
    let receipt = admitted(&fixture)?;
    assert_eq!(
        hex_bytes(&receipt.manifest_bytes),
        fixture["acceptance"]["manifest_bytes"]
    );
    assert_eq!(
        hex_bytes(&receipt.qualification_bundle_bytes),
        fixture["acceptance"]["qualification_bundle_bytes"]
    );
    for packet in &receipt.packet_inputs {
        assert_eq!(digest(&packet.bytes), packet.sha256);
    }
    Ok(())
}
#[test]
fn fresh_native_response_cannot_substitute_body_issuer_location_or_author() -> Result<(), String> {
    for (field, value) in [
        ("body", json!("changed")),
        ("author_association", json!("NONE")),
        (
            "issue_url",
            json!("https://api.github.com/repos/EffortlessMetrics/ripr-swarm/issues/999"),
        ),
        ("html_url", json!(reference(999))),
        ("user", json!({"login":"different-author"})),
    ] {
        let fixture = fixture()?;
        let result = revalidate_with_reader(&fixture, |reference, owner| {
            let mut response: Value =
                serde_json::from_slice(&responses(&fixture, reference, owner)?)
                    .map_err(|error| error.to_string())?;
            response[field] = value.clone();
            serde_json::to_vec(&response).map_err(|error| error.to_string())
        });
        assert!(result.is_err(), "accepted native drift {field}");
    }
    Ok(())
}
#[test]
fn crlf_parsing_preserves_raw_digest_and_refuses_duplicate_blocks() -> Result<(), String> {
    let reference = reference(1609);
    let raw = block(&json!({"fixture":true}))?.replace('\n', "\r\n");
    let retained: NativeDecision =
        serde_json::from_value(decision(1609, &raw)).map_err(|error| error.to_string())?;
    retained.validate(1609)?;
    assert_eq!(retained.payload::<Value>()?, json!({"fixture":true}));
    assert_eq!(retained.body_sha256, digest(raw.as_bytes()));
    let repeated: NativeDecision = serde_json::from_value(decision(1609, &format!("{raw}{raw}")))
        .map_err(|error| error.to_string())?;
    if repeated.payload::<Value>().is_ok() {
        return Err("duplicate native acceptance block accepted".into());
    }
    if native::decode(&reference, 1609, &vec![b' '; 1024 * 1024 + 1]).is_ok() {
        return Err("oversized native acceptance response accepted".into());
    }
    Ok(())
}

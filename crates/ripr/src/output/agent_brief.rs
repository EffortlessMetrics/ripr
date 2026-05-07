use crate::app::Mode;
use crate::app::agent_brief::{
    AgentBriefResolvedWorkingSet, AgentBriefSelectedSeam, AgentBriefSelection,
};
use crate::config::RiprConfig;
use crate::output::agent_seam_packets;
use serde_json::{Value, json};
use std::path::Path;

pub(crate) const AGENT_BRIEF_SCHEMA_VERSION: &str = "0.1";

pub(crate) fn render_agent_brief_json(
    root: &Path,
    mode: &Mode,
    config: &RiprConfig,
    working_set: &AgentBriefResolvedWorkingSet,
    selection: &AgentBriefSelection<'_>,
) -> Result<String, String> {
    let value = json!({
        "schema_version": AGENT_BRIEF_SCHEMA_VERSION,
        "tool": "ripr",
        "scope": "working_set",
        "root": display_path(root),
        "mode": mode.as_str(),
        "config": config_json(config),
        "working_set": working_set_json(working_set),
        "limits": {
            "requested": selection.requested,
            "returned": selection.returned,
            "default": selection.default,
            "hard_cap": selection.hard_cap,
        },
        "top_seams": selection
            .top_seams
            .iter()
            .map(|entry| top_seam_json(entry, root, mode, config))
            .collect::<Vec<_>>(),
        "next": {
            "inspect_packet": format!(
                "ripr check --root {} --mode {} --format agent-seam-packets-json > target/ripr/workflow/agent-seam-packets.json",
                display_path(root),
                mode.as_str()
            ),
            "verify_after_edit": format!(
                "ripr agent verify --root {} --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json",
                display_path(root)
            ),
        },
        "warnings": &selection.warnings,
    });
    serde_json::to_string_pretty(&value).map_err(|err| err.to_string())
}

fn config_json(config: &RiprConfig) -> Value {
    json!({
        "state": if config.source_path().is_some() { "loaded" } else { "missing" },
        "path": config.source_path().map(display_path),
        "fingerprint": config.source_text().map(config_fingerprint),
    })
}

fn working_set_json(working_set: &AgentBriefResolvedWorkingSet) -> Value {
    json!({
        "source": working_set.source.as_str(),
        "files": working_set.files.iter().map(|path| display_path(path)).collect::<Vec<_>>(),
        "changed_lines": working_set.changed_lines.iter().map(|line| {
            json!({
                "file": display_path(&line.file),
                "line": line.line,
            })
        }).collect::<Vec<_>>(),
        "base": working_set.base.as_deref(),
        "diff": working_set.diff.as_ref().map(|path| display_path(path)),
        "seam_id": working_set.seam_id.as_deref(),
    })
}

fn top_seam_json(
    selected: &AgentBriefSelectedSeam<'_>,
    root: &Path,
    mode: &Mode,
    config: &RiprConfig,
) -> Value {
    let entry = selected.seam;
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let missing = agent_seam_packets::missing_discriminator_records_for(entry);
    let recommended = agent_seam_packets::recommended_test_for(entry);
    let nearest = agent_seam_packets::nearest_strong_test_to_imitate(evidence);
    let candidate_values = agent_seam_packets::candidate_values_for(entry, &missing);
    let assertion_shape =
        agent_seam_packets::assertion_shape_for(seam.kind(), seam.owner(), evidence);

    json!({
        "seam_id": seam.id().as_str(),
        "owner": seam.owner(),
        "seam_kind": seam.kind().as_str(),
        "file": display_path(seam.file()),
        "line": seam.display_line(),
        "expression": seam.expression(),
        "grip_class": entry.class.as_str(),
        "severity": config.severity().for_seam(entry.class).as_str(),
        "headline_eligible": entry.class.is_headline_eligible(),
        "why_now": {
            "reason": selected.why_now.reason.as_str(),
            "confidence": selected.why_now.confidence.as_str(),
            "evidence": selected.why_now.evidence.as_str(),
        },
        "evidence": {
            "reach": evidence.reach.state.as_str(),
            "activate": evidence.activate.state.as_str(),
            "propagate": evidence.propagate.state.as_str(),
            "observe": evidence.observe.state.as_str(),
            "discriminate": evidence.discriminate.state.as_str(),
        },
        "recommended_test": {
            "name": recommended.name,
            "file": display_text_path(&recommended.file),
            "reason": recommended.reason,
        },
        "nearest_strong_test_to_imitate": nearest.map(|test| json!({
            "name": test.test_name.as_str(),
            "file": display_path(&test.file),
            "line": test.line,
            "oracle_kind": test.oracle_kind.as_str(),
            "oracle_strength": test.oracle_strength.as_str(),
            "relation_reason": test.relation_reason.as_str(),
            "relation_confidence": test.relation_confidence.as_str(),
        })),
        "candidate_values": candidate_values.iter().map(|record| json!({
            "value": record.value.as_str(),
            "reason": record.reason.as_str(),
        })).collect::<Vec<_>>(),
        "missing_discriminators": missing.iter().map(|record| json!({
            "value": record.value.as_str(),
            "reason": record.reason.as_str(),
        })).collect::<Vec<_>>(),
        "assertion_shape": {
            "kind": assertion_shape.kind,
            "example": assertion_shape.example,
        },
        "packet_ref": {
            "format": "agent-seam-packets-json",
            "seam_id": seam.id().as_str(),
        },
        "verification": verification_json(root, mode, &recommended.name),
    })
}

fn verification_json(root: &Path, mode: &Mode, recommended_name: &str) -> Value {
    let root = display_path(root);
    json!({
        "before_snapshot_command": format!(
            "ripr check --root {root} --mode {} --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json",
            mode.as_str()
        ),
        "after_snapshot_command": format!(
            "ripr check --root {root} --mode {} --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
            mode.as_str()
        ),
        "verify_command": format!("ripr agent verify --root {root} --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json"),
        "suggested_test_command": format!("cargo test {recommended_name}"),
    })
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn display_text_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn config_fingerprint(source_text: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in source_text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ClassifiedSeam;
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::app::agent_brief::{
        AGENT_BRIEF_HARD_MAX_SEAMS, AgentBriefLine, AgentBriefPolicy, AgentBriefResolvedWorkingSet,
        DEFAULT_AGENT_BRIEF_MAX_SEAMS, select_agent_brief_seams,
    };
    use crate::config::{CONFIG_FILE_NAME, load_for_root};
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueFact,
    };
    use serde_json::Value;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn classified() -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            880,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            class: SeamGripClass::WeaklyGripped,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: vec![RelatedTestGrip {
                    test_name: "below_threshold_has_no_discount".to_string(),
                    file: PathBuf::from("tests/pricing.rs"),
                    line: 12,
                    oracle_kind: OracleKind::ExactValue,
                    oracle_strength: OracleStrength::Strong,
                    evidence_summary: "exact returned value assertion".to_string(),
                    relation_reason: RelationReason::DirectOwnerCall,
                    relation_confidence: RelationConfidence::High,
                }],
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Yes),
                observe: stage(StageState::Yes),
                discriminate: stage(StageState::Weak),
                observed_values: Vec::<ValueFact>::new(),
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "discount_threshold (equality boundary)".to_string(),
                    reason: "observed values do not include the equality-boundary case".to_string(),
                    flow_sink: None,
                }],
            },
        }
    }

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("time moved backwards: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-agent-brief-render-{label}-{stamp}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("create temp root {}: {err}", root.display()))?;
        Ok(root)
    }

    #[test]
    fn agent_brief_json_renders_ranked_seam_summary() -> Result<(), String> {
        let seams = vec![classified()];
        let working_set = AgentBriefResolvedWorkingSet::diff(
            "change.diff",
            vec![AgentBriefLine::new("src/pricing.rs", 88)],
        );
        let config = RiprConfig::default();
        let selection = select_agent_brief_seams(
            &seams,
            &working_set,
            3,
            AgentBriefPolicy::from_config(&config),
        );

        let json = render_agent_brief_json(
            Path::new("."),
            &Mode::Draft,
            &config,
            &working_set,
            &selection,
        )?;
        let value: Value = serde_json::from_str(&json).map_err(|err| err.to_string())?;

        assert_eq!(value["schema_version"], AGENT_BRIEF_SCHEMA_VERSION);
        assert_eq!(value["scope"], "working_set");
        assert_eq!(value["working_set"]["source"], "diff");
        assert_eq!(value["limits"]["returned"], 1);
        assert_eq!(
            value["top_seams"][0]["why_now"]["reason"],
            "changed_line_intersects_seam"
        );
        assert_eq!(
            value["top_seams"][0]["packet_ref"]["format"],
            "agent-seam-packets-json"
        );
        assert_eq!(
            value["top_seams"][0]["nearest_strong_test_to_imitate"]["name"],
            "below_threshold_has_no_discount"
        );
        assert_eq!(
            value["top_seams"][0]["candidate_values"][0]["value"],
            "discount_threshold (equality boundary)"
        );
        assert!(
            value["top_seams"][0]["verification"]["after_snapshot_command"]
                .as_str()
                .is_some_and(|command| command.contains("--format repo-exposure-json"))
        );
        Ok(())
    }

    #[test]
    fn agent_brief_json_renders_loaded_config_and_empty_file_scope() -> Result<(), String> {
        let root = temp_root("loaded-config")?;
        std::fs::write(root.join(CONFIG_FILE_NAME), "[analysis]\nmode = \"fast\"\n")
            .map_err(|err| format!("write ripr.toml: {err}"))?;
        let config = load_for_root(&root)?;
        let working_set =
            AgentBriefResolvedWorkingSet::files(vec![PathBuf::from(".\\src\\pricing.rs")]);
        let selection = AgentBriefSelection {
            requested: 0,
            returned: 0,
            default: DEFAULT_AGENT_BRIEF_MAX_SEAMS,
            hard_cap: AGENT_BRIEF_HARD_MAX_SEAMS,
            top_seams: Vec::new(),
            warnings: vec!["configured-off seam omitted".to_string()],
        };

        let json = render_agent_brief_json(&root, &Mode::Fast, &config, &working_set, &selection)?;
        let value: Value = serde_json::from_str(&json).map_err(|err| err.to_string())?;
        std::fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {}: {err}", root.display()))?;

        assert_eq!(value["config"]["state"], "loaded");
        assert!(
            value["config"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(CONFIG_FILE_NAME))
        );
        assert!(
            value["config"]["fingerprint"]
                .as_str()
                .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
        );
        assert_eq!(value["working_set"]["source"], "files");
        assert_eq!(value["working_set"]["files"][0], "./src/pricing.rs");
        assert_eq!(value["limits"]["requested"], 0);
        assert_eq!(value["warnings"][0], "configured-off seam omitted");
        assert_eq!(
            value["next"]["inspect_packet"],
            format!(
                "ripr check --root {} --mode fast --format agent-seam-packets-json > target/ripr/workflow/agent-seam-packets.json",
                root.to_string_lossy().replace('\\', "/")
            )
        );
        Ok(())
    }

    #[test]
    fn agent_brief_config_fingerprint_changes_with_source_text() {
        let left = config_fingerprint("[analysis]\nmode = \"fast\"\n");
        let right = config_fingerprint("[analysis]\nmode = \"deep\"\n");

        assert!(left.starts_with("fnv1a64:"));
        assert_eq!(left.len(), "fnv1a64:0000000000000000".len());
        assert_ne!(left, right);
    }
}

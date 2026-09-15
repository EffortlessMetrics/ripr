//! Evidence-promotion-honesty cluster: the evidence-promotion-honesty gate
//! (honesty options parsing, corpus validation, typed semantic assertions,
//! pinned external checkout machinery, external run report, corpus summary
//! report), plus the report-local `evidence_promotion_*` JSON/markdown and
//! human-projection helpers that sit physically inside this region.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` and re-exported from `main.rs` so
//! existing call sites (`dispatch.rs` and `tests.rs`) compile unchanged.

use super::*;

pub(crate) const EVIDENCE_PROMOTION_HONESTY_CORPUS: &str =
    "fixtures/evidence-promotion-honesty-corpus/corpus.json";

const EVIDENCE_PROMOTION_CHECKOUT_ROOT: &str = "target/ripr/evidence-promotion-honesty/checkouts";
pub(crate) const EVIDENCE_PROMOTION_EXTERNAL_JSON: &str = "evidence-promotion-pinned-external.json";
pub(crate) const EVIDENCE_PROMOTION_EXTERNAL_MD: &str = "evidence-promotion-pinned-external.md";
const CORPUS_SUMMARY_JSON: &str = "corpus-summary.json";
const CORPUS_SUMMARY_MD: &str = "corpus-summary.md";

#[derive(Clone)]
struct EvidencePromotionHonestyOptions {
    run_pinned_external: bool,
    clone: bool,
    checkout_root: PathBuf,
    only_case: Option<String>,
    timeout: Duration,
}

impl Default for EvidencePromotionHonestyOptions {
    fn default() -> Self {
        Self {
            run_pinned_external: false,
            clone: false,
            checkout_root: PathBuf::from(EVIDENCE_PROMOTION_CHECKOUT_ROOT),
            only_case: None,
            timeout: Duration::from_mins(2),
        }
    }
}

fn parse_evidence_promotion_honesty_args(
    args: &[String],
) -> Result<EvidencePromotionHonestyOptions, String> {
    let mut options = EvidencePromotionHonestyOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--pinned-external" => options.run_pinned_external = true,
            "--clone" => options.clone = true,
            "--case" => {
                index += 1;
                let case = args.get(index).cloned().ok_or_else(|| {
                    "check-evidence-promotion-honesty --case requires a value".to_string()
                })?;
                if case.trim().is_empty() {
                    return Err(
                        "check-evidence-promotion-honesty --case requires a non-empty value"
                            .to_string(),
                    );
                }
                options.only_case = Some(case);
            }
            "--checkout-root" => {
                index += 1;
                let root = args.get(index).cloned().ok_or_else(|| {
                    "check-evidence-promotion-honesty --checkout-root requires a value".to_string()
                })?;
                if root.trim().is_empty() {
                    return Err(
                        "check-evidence-promotion-honesty --checkout-root requires a non-empty value"
                            .to_string(),
                    );
                }
                options.checkout_root = PathBuf::from(root);
            }
            "--timeout-secs" => {
                index += 1;
                let raw = args.get(index).cloned().ok_or_else(|| {
                    "check-evidence-promotion-honesty --timeout-secs requires a value".to_string()
                })?;
                let seconds = raw.parse::<u64>().map_err(|err| {
                    format!(
                        "check-evidence-promotion-honesty --timeout-secs expects an integer, got `{raw}`: {err}"
                    )
                })?;
                if seconds == 0 {
                    return Err(
                        "check-evidence-promotion-honesty --timeout-secs must be positive"
                            .to_string(),
                    );
                }
                options.timeout = Duration::from_secs(seconds);
            }
            other => {
                return Err(format!(
                    "unknown check-evidence-promotion-honesty argument `{other}`"
                ));
            }
        }
        index += 1;
    }
    if options.clone && !options.run_pinned_external {
        return Err(
            "check-evidence-promotion-honesty --clone requires --pinned-external".to_string(),
        );
    }
    if options.only_case.is_some() && !options.run_pinned_external {
        return Err(
            "check-evidence-promotion-honesty --case requires --pinned-external".to_string(),
        );
    }
    Ok(options)
}

#[derive(Clone)]
pub(crate) struct EvidencePromotionExternalCase {
    pub(crate) id: String,
    pub(crate) language: String,
    pub(crate) external_repo: String,
    pub(crate) external_commit: String,
    pub(crate) external_patch: PathBuf,
    pub(crate) external_command: String,
    pub(crate) runtime_budget_seconds: u64,
    pub(crate) artifact_budget_bytes: u64,
    pub(crate) assertions: Vec<EvidencePromotionSemanticAssertion>,
}

#[derive(Clone)]
pub(crate) struct EvidencePromotionExternalLaunch {
    pub(crate) repo: String,
    pub(crate) commit: String,
    pub(crate) patch: String,
    pub(crate) command: String,
    pub(crate) runtime_budget_seconds: u64,
    pub(crate) artifact_budget_bytes: u64,
}

impl EvidencePromotionExternalLaunch {
    fn from_case(case: &EvidencePromotionExternalCase) -> Self {
        Self {
            repo: case.external_repo.clone(),
            commit: case.external_commit.clone(),
            patch: normalize_path(&case.external_patch),
            command: case.external_command.clone(),
            runtime_budget_seconds: case.runtime_budget_seconds,
            artifact_budget_bytes: case.artifact_budget_bytes,
        }
    }

    fn from_case_json(case: &Value) -> Option<Self> {
        Some(Self {
            repo: case.get("external_repo")?.as_str()?.to_string(),
            commit: case.get("external_commit")?.as_str()?.to_string(),
            patch: case.get("external_patch")?.as_str()?.to_string(),
            command: case.get("external_command")?.as_str()?.to_string(),
            runtime_budget_seconds: case.get("runtime_budget_seconds")?.as_u64()?,
            artifact_budget_bytes: case.get("artifact_budget_bytes")?.as_u64()?,
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "repo": self.repo,
            "commit": self.commit,
            "patch": self.patch,
            "command": self.command,
            "runtime_budget_seconds": self.runtime_budget_seconds,
            "artifact_budget_bytes": self.artifact_budget_bytes,
        })
    }
}

pub(crate) struct EvidencePromotionExternalRun {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) result_kind: String,
    pub(crate) runtime_ms: u128,
    pub(crate) artifact_bytes: u64,
    pub(crate) external_case: Option<EvidencePromotionExternalLaunch>,
    pub(crate) checkout: String,
    pub(crate) violations: Vec<String>,
}

impl EvidencePromotionExternalRun {
    fn terminal(
        case: &EvidencePromotionExternalCase,
        result_kind: &str,
        violation: String,
    ) -> Self {
        Self {
            id: case.id.clone(),
            status: "fail".to_string(),
            result_kind: result_kind.to_string(),
            runtime_ms: 0,
            artifact_bytes: 0,
            external_case: Some(EvidencePromotionExternalLaunch::from_case(case)),
            checkout: String::new(),
            violations: vec![violation],
        }
    }

    fn runner_failure(result_kind: &str, violation: String) -> Self {
        Self {
            id: "__runner__".to_string(),
            status: "fail".to_string(),
            result_kind: result_kind.to_string(),
            runtime_ms: 0,
            artifact_bytes: 0,
            external_case: None,
            checkout: String::new(),
            violations: vec![violation],
        }
    }
}

#[derive(Clone)]
struct EvidencePromotionCorpusCaseMeta {
    id: String,
    language: String,
    tier: String,
    external_case: Option<EvidencePromotionExternalLaunch>,
}

struct CorpusSummaryCase {
    id: String,
    language: String,
    tier: String,
    status: String,
    result_kind: String,
    message: String,
    runtime_ms: Option<u128>,
    artifact_bytes: Option<u64>,
    external_case: Option<EvidencePromotionExternalLaunch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExpectedRepairPacketDetail {
    pub(crate) canonical_gap_id: String,
    pub(crate) source_file: String,
    pub(crate) source_line: usize,
    pub(crate) target_test: String,
    pub(crate) assertion_shape: String,
    pub(crate) authority_boundary: String,
    pub(crate) repair_kind: String,
    pub(crate) verify_command: String,
    pub(crate) receipt_command: String,
    pub(crate) allowed_edit_surface: Vec<String>,
    pub(crate) forbidden_files: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EvidencePromotionSemanticAssertion {
    MustPromote,
    MustNotPromote,
    MustReportClean,
    MustNotReportClean,
    MustDiscloseScope,
    MustDiscloseNoScope,
    MustNotDiscloseNoScope,
    MustDiscloseUnanalyzedWorkingTree,
    MustNotDiscloseUnanalyzedWorkingTree,
    MustEmitLimitation {
        expected_limit_kind: String,
    },
    MustNotEmitLimitation,
    MustHaveVerifyCommand,
    MustNotHaveVerifyCommand,
    MustHaveReceiptCommand,
    MustNotHaveReceiptCommand,
    MustEmitRepairPacket,
    MustNotEmitRepairPacket,
    MustDiscloseRepairPacketDetail,
    ExpectedRepairPacketDetail {
        detail: ExpectedRepairPacketDetail,
    },
    MustNotHaveContradictoryPacketMessaging,
    ExpectedOracle {
        kind: String,
        strength: String,
    },
    ExpectedClass {
        class: String,
    },
    MaximumClass {
        class: String,
    },
    ExpectedCompleteness {
        completeness: String,
    },
    ExpectedChangedRustFiles {
        count: u64,
    },
    ExpectedFindingCount {
        count: u64,
    },
    MustDiscloseWitness,
    MustDiscloseLimitationDetail,
    ExpectedLimitationDetail {
        last_established_edge: String,
        first_unresolved_edge: String,
        non_claim: String,
    },
    ExpectedLimitationRoute {
        route: String,
    },
    MustNotClaimNoTestsFound,
    MustSeeChangedFile {
        path: String,
    },
}

fn run_evidence_promotion_pinned_external_cases(
    corpus_path: &Path,
    options: &EvidencePromotionHonestyOptions,
) -> Result<Vec<EvidencePromotionExternalRun>, String> {
    let cases = match load_evidence_promotion_pinned_external_cases(corpus_path, options) {
        Ok(cases) => cases,
        Err(err) => {
            let runs = vec![EvidencePromotionExternalRun::runner_failure(
                "setup_failure",
                err.clone(),
            )];
            write_evidence_promotion_external_report(&runs, std::slice::from_ref(&err))?;
            return Ok(runs);
        }
    };
    if cases.is_empty() {
        let reason = if let Some(id) = &options.only_case {
            format!("no pinned_external evidence-promotion case matched `{id}`")
        } else {
            "no pinned_external evidence-promotion cases found".to_string()
        };
        let runs = vec![EvidencePromotionExternalRun::runner_failure(
            "setup_failure",
            reason.clone(),
        )];
        write_evidence_promotion_external_report(&runs, std::slice::from_ref(&reason))?;
        return Ok(runs);
    }

    if let Err(err) = build_ripr_for_evidence_promotion_external() {
        let runs = vec![EvidencePromotionExternalRun::runner_failure(
            "setup_failure",
            err.clone(),
        )];
        write_evidence_promotion_external_report(&runs, std::slice::from_ref(&err))?;
        return Ok(runs);
    }
    let binary = PathBuf::from("target")
        .join("debug")
        .join(format!("ripr{}", std::env::consts::EXE_SUFFIX));
    let mut runs = Vec::new();
    let mut all_violations = Vec::new();
    for case in &cases {
        let run = run_evidence_promotion_external_case(case, options, &binary);
        all_violations.extend(run.violations.iter().cloned());
        runs.push(run);
    }
    write_evidence_promotion_external_report(&runs, &all_violations)?;
    Ok(runs)
}

fn load_evidence_promotion_pinned_external_cases(
    corpus_path: &Path,
    options: &EvidencePromotionHonestyOptions,
) -> Result<Vec<EvidencePromotionExternalCase>, String> {
    let corpus_text = read_text_lossy(corpus_path)?;
    let corpus_value: Value = parse_json_rejecting_duplicate_keys(&corpus_text)
        .map_err(|err| format!("failed to parse {}: {err}", normalize_path(corpus_path)))?;
    let Some(cases) = corpus_value.get("cases").and_then(Value::as_array) else {
        return Err(format!(
            "{} has no `cases` array",
            normalize_path(corpus_path)
        ));
    };
    let mut parsed = Vec::new();
    for case in cases {
        if case.get("tier").and_then(Value::as_str) != Some("pinned_external") {
            continue;
        }
        let id = evidence_promotion_required_string(case, "id")?;
        if let Some(only) = &options.only_case
            && &id != only
        {
            continue;
        }
        parsed.push(EvidencePromotionExternalCase {
            id,
            language: evidence_promotion_required_string(case, "language")?,
            external_repo: evidence_promotion_required_string(case, "external_repo")?,
            external_commit: evidence_promotion_required_string(case, "external_commit")?,
            external_patch: PathBuf::from(evidence_promotion_required_string(
                case,
                "external_patch",
            )?),
            external_command: evidence_promotion_required_string(case, "external_command")?,
            runtime_budget_seconds: evidence_promotion_required_u64(
                case,
                "runtime_budget_seconds",
            )?,
            artifact_budget_bytes: evidence_promotion_required_u64(case, "artifact_budget_bytes")?,
            assertions: evidence_promotion_case_assertions(case)?,
        });
    }
    Ok(parsed)
}

fn evidence_promotion_required_string(case: &Value, field: &str) -> Result<String, String> {
    case.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            let id = case
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<missing-id>");
            format!("evidence promotion case `{id}` missing required string `{field}`")
        })
}

fn evidence_promotion_required_u64(case: &Value, field: &str) -> Result<u64, String> {
    case.get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            let id = case
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<missing-id>");
            format!("evidence promotion case `{id}` missing positive integer `{field}`")
        })
}

fn evidence_promotion_bool(case: &Value, field: &str) -> bool {
    case.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn evidence_promotion_case_assertions(
    case: &Value,
) -> Result<Vec<EvidencePromotionSemanticAssertion>, String> {
    let id = case
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("<missing-id>");
    let Some(raw_assertions) = case.get("assertions") else {
        return Ok(evidence_promotion_legacy_assertions(case));
    };
    let assertions = raw_assertions
        .as_array()
        .ok_or_else(|| format!("evidence promotion case `{id}`: `assertions` must be an array"))?;
    if assertions.is_empty() {
        return Err(format!(
            "evidence promotion case `{id}`: `assertions` must not be empty"
        ));
    }

    let mut parsed = Vec::new();
    for (index, assertion) in assertions.iter().enumerate() {
        parsed.push(evidence_promotion_parse_assertion(id, index, assertion)?);
    }
    Ok(parsed)
}

fn evidence_promotion_legacy_assertions(case: &Value) -> Vec<EvidencePromotionSemanticAssertion> {
    let mut assertions = Vec::new();
    if let Some(path) = evidence_promotion_non_empty_string_field(case, "expected_changed_file") {
        assertions.push(EvidencePromotionSemanticAssertion::MustSeeChangedFile {
            path: path.to_string(),
        });
    }
    if evidence_promotion_bool(case, "must_not_report_clean") {
        assertions.push(EvidencePromotionSemanticAssertion::MustNotReportClean);
    }
    if evidence_promotion_bool(case, "must_disclose_scope") {
        assertions.push(EvidencePromotionSemanticAssertion::MustDiscloseScope);
    }
    if evidence_promotion_bool(case, "must_emit_limitation") {
        assertions.push(EvidencePromotionSemanticAssertion::MustEmitLimitation {
            expected_limit_kind: evidence_promotion_non_empty_string_field(
                case,
                "expected_limit_kind",
            )
            .unwrap_or("")
            .to_string(),
        });
    }
    if evidence_promotion_bool(case, "must_not_emit_repair_packet") {
        assertions.push(EvidencePromotionSemanticAssertion::MustNotEmitRepairPacket);
    }
    if evidence_promotion_bool(case, "must_disclose_witness") {
        assertions.push(EvidencePromotionSemanticAssertion::MustDiscloseWitness);
    }
    if evidence_promotion_bool(case, "must_disclose_limitation_detail") {
        assertions.push(EvidencePromotionSemanticAssertion::MustDiscloseLimitationDetail);
    }
    if evidence_promotion_bool(case, "must_not_claim_no_tests_found") {
        assertions.push(EvidencePromotionSemanticAssertion::MustNotClaimNoTestsFound);
    }
    if evidence_promotion_bool(case, "must_remain_non_promoted") {
        assertions.push(EvidencePromotionSemanticAssertion::MustNotPromote);
        assertions.push(EvidencePromotionSemanticAssertion::MaximumClass {
            class: evidence_promotion_non_empty_string_field(case, "expected_max_class")
                .unwrap_or("weakly_exposed")
                .to_string(),
        });
    }
    if evidence_promotion_bool(case, "expected_promoted") {
        assertions.push(EvidencePromotionSemanticAssertion::MustPromote);
    }
    assertions
}

fn evidence_promotion_parse_assertion(
    case_id: &str,
    index: usize,
    assertion: &Value,
) -> Result<EvidencePromotionSemanticAssertion, String> {
    let kind = if let Some(kind) = assertion.as_str() {
        kind
    } else {
        assertion
            .get("type")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "evidence promotion case `{case_id}` assertion {index}: missing string `type`"
                )
            })?
    };
    match kind {
        "must_promote" => Ok(EvidencePromotionSemanticAssertion::MustPromote),
        "must_not_promote" => Ok(EvidencePromotionSemanticAssertion::MustNotPromote),
        "must_report_clean" => Ok(EvidencePromotionSemanticAssertion::MustReportClean),
        "must_not_report_clean" => Ok(EvidencePromotionSemanticAssertion::MustNotReportClean),
        "must_disclose_scope" => Ok(EvidencePromotionSemanticAssertion::MustDiscloseScope),
        "must_disclose_no_scope" => Ok(EvidencePromotionSemanticAssertion::MustDiscloseNoScope),
        "must_not_disclose_no_scope" => {
            Ok(EvidencePromotionSemanticAssertion::MustNotDiscloseNoScope)
        }
        "must_disclose_unanalyzed_working_tree" => {
            Ok(EvidencePromotionSemanticAssertion::MustDiscloseUnanalyzedWorkingTree)
        }
        "must_not_disclose_unanalyzed_working_tree" => {
            Ok(EvidencePromotionSemanticAssertion::MustNotDiscloseUnanalyzedWorkingTree)
        }
        "must_emit_limitation" => Ok(EvidencePromotionSemanticAssertion::MustEmitLimitation {
            expected_limit_kind: evidence_promotion_required_assertion_string(
                case_id,
                index,
                assertion,
                "expected_limit_kind",
            )?,
        }),
        "must_not_emit_limitation" => Ok(EvidencePromotionSemanticAssertion::MustNotEmitLimitation),
        "must_have_verify_command" => Ok(EvidencePromotionSemanticAssertion::MustHaveVerifyCommand),
        "must_not_have_verify_command" => {
            Ok(EvidencePromotionSemanticAssertion::MustNotHaveVerifyCommand)
        }
        "must_have_receipt_command" => {
            Ok(EvidencePromotionSemanticAssertion::MustHaveReceiptCommand)
        }
        "must_not_have_receipt_command" => {
            Ok(EvidencePromotionSemanticAssertion::MustNotHaveReceiptCommand)
        }
        "must_emit_repair_packet" => Ok(EvidencePromotionSemanticAssertion::MustEmitRepairPacket),
        "must_not_emit_repair_packet" => {
            Ok(EvidencePromotionSemanticAssertion::MustNotEmitRepairPacket)
        }
        "must_disclose_repair_packet_detail" => {
            Ok(EvidencePromotionSemanticAssertion::MustDiscloseRepairPacketDetail)
        }
        "expected_repair_packet_detail" => Ok(
            EvidencePromotionSemanticAssertion::ExpectedRepairPacketDetail {
                detail: ExpectedRepairPacketDetail {
                    canonical_gap_id: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "canonical_gap_id",
                    )?,
                    source_file: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "source_file",
                    )?,
                    source_line: evidence_promotion_required_assertion_usize(
                        case_id,
                        index,
                        assertion,
                        "source_line",
                    )?,
                    target_test: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "target_test",
                    )?,
                    assertion_shape: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "assertion_shape",
                    )?,
                    authority_boundary: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "authority_boundary",
                    )?,
                    repair_kind: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "repair_kind",
                    )?,
                    verify_command: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "verify_command",
                    )?,
                    receipt_command: evidence_promotion_required_assertion_string(
                        case_id,
                        index,
                        assertion,
                        "receipt_command",
                    )?,
                    allowed_edit_surface: evidence_promotion_required_assertion_string_array(
                        case_id,
                        index,
                        assertion,
                        "allowed_edit_surface",
                    )?,
                    forbidden_files: evidence_promotion_required_assertion_string_array(
                        case_id,
                        index,
                        assertion,
                        "forbidden_files",
                    )?,
                },
            },
        ),
        "must_not_have_contradictory_packet_messaging" => {
            Ok(EvidencePromotionSemanticAssertion::MustNotHaveContradictoryPacketMessaging)
        }
        "expected_oracle" => Ok(EvidencePromotionSemanticAssertion::ExpectedOracle {
            kind: evidence_promotion_required_assertion_string(case_id, index, assertion, "kind")?,
            strength: evidence_promotion_required_assertion_string(
                case_id, index, assertion, "strength",
            )?,
        }),
        "expected_class" => {
            let class =
                evidence_promotion_required_assertion_class(case_id, index, assertion, "class")?;
            Ok(EvidencePromotionSemanticAssertion::ExpectedClass { class })
        }
        "maximum_class" => {
            let class =
                evidence_promotion_required_assertion_class(case_id, index, assertion, "class")?;
            Ok(EvidencePromotionSemanticAssertion::MaximumClass { class })
        }
        "expected_completeness" => {
            let completeness = evidence_promotion_required_assertion_string(
                case_id,
                index,
                assertion,
                "completeness",
            )?;
            if !matches!(
                completeness.as_str(),
                "complete" | "limited" | "deferred" | "stale"
            ) {
                return Err(format!(
                    "evidence promotion case `{case_id}` assertion {index}: \
                     expected_completeness.completeness must be one of complete, limited, \
                     deferred, or stale"
                ));
            }
            Ok(EvidencePromotionSemanticAssertion::ExpectedCompleteness { completeness })
        }
        "expected_changed_rust_files" => Ok(
            EvidencePromotionSemanticAssertion::ExpectedChangedRustFiles {
                count: evidence_promotion_required_assertion_u64(
                    case_id, index, assertion, "count",
                )?,
            },
        ),
        "expected_finding_count" => Ok(EvidencePromotionSemanticAssertion::ExpectedFindingCount {
            count: evidence_promotion_required_assertion_u64(case_id, index, assertion, "count")?,
        }),
        "must_disclose_witness" => Ok(EvidencePromotionSemanticAssertion::MustDiscloseWitness),
        "must_disclose_limitation_detail" => {
            Ok(EvidencePromotionSemanticAssertion::MustDiscloseLimitationDetail)
        }
        "expected_limitation_detail" => {
            let last_established_edge = evidence_promotion_required_assertion_string(
                case_id,
                index,
                assertion,
                "last_established_edge",
            )?;
            let first_unresolved_edge = evidence_promotion_required_assertion_string(
                case_id,
                index,
                assertion,
                "first_unresolved_edge",
            )?;
            let non_claim = evidence_promotion_required_assertion_string(
                case_id,
                index,
                assertion,
                "non_claim",
            )?;
            Ok(
                EvidencePromotionSemanticAssertion::ExpectedLimitationDetail {
                    last_established_edge,
                    first_unresolved_edge,
                    non_claim,
                },
            )
        }
        "expected_limitation_route" => {
            let route =
                evidence_promotion_required_assertion_string(case_id, index, assertion, "route")?;
            Ok(EvidencePromotionSemanticAssertion::ExpectedLimitationRoute { route })
        }
        "must_not_claim_no_tests_found" => {
            Ok(EvidencePromotionSemanticAssertion::MustNotClaimNoTestsFound)
        }
        "must_see_changed_file" => Ok(EvidencePromotionSemanticAssertion::MustSeeChangedFile {
            path: evidence_promotion_required_assertion_string(case_id, index, assertion, "path")?,
        }),
        other => Err(format!(
            "evidence promotion case `{case_id}` assertion {index}: unknown assertion type `{other}`"
        )),
    }
}

fn evidence_promotion_required_assertion_string(
    case_id: &str,
    index: usize,
    assertion: &Value,
    field: &str,
) -> Result<String, String> {
    assertion
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "evidence promotion case `{case_id}` assertion {index}: missing non-empty string `{field}`"
            )
        })
}

fn evidence_promotion_required_assertion_usize(
    case_id: &str,
    index: usize,
    assertion: &Value,
    field: &str,
) -> Result<usize, String> {
    assertion
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            format!(
                "evidence promotion case `{case_id}` assertion {index}: missing positive integer `{field}`"
            )
        })
}

fn evidence_promotion_required_assertion_u64(
    case_id: &str,
    index: usize,
    assertion: &Value,
    field: &str,
) -> Result<u64, String> {
    assertion.get(field).and_then(Value::as_u64).ok_or_else(|| {
        format!(
            "evidence promotion case `{case_id}` assertion {index}: missing unsigned integer `{field}`"
        )
    })
}

fn evidence_promotion_required_assertion_string_array(
    case_id: &str,
    index: usize,
    assertion: &Value,
    field: &str,
) -> Result<Vec<String>, String> {
    let Some(items) = assertion.get(field).and_then(Value::as_array) else {
        return Err(format!(
            "evidence promotion case `{case_id}` assertion {index}: missing non-empty string array `{field}`"
        ));
    };
    if items.is_empty() {
        return Err(format!(
            "evidence promotion case `{case_id}` assertion {index}: missing non-empty string array `{field}`"
        ));
    }
    let mut values = Vec::with_capacity(items.len());
    for (item_index, item) in items.iter().enumerate() {
        let Some(value) = item.as_str().filter(|value| !value.trim().is_empty()) else {
            return Err(format!(
                "evidence promotion case `{case_id}` assertion {index}: `{field}` item {item_index} must be a non-empty string"
            ));
        };
        values.push(value.to_string());
    }
    Ok(values)
}

fn evidence_promotion_required_assertion_class(
    case_id: &str,
    index: usize,
    assertion: &Value,
    field: &str,
) -> Result<String, String> {
    let class = evidence_promotion_required_assertion_string(case_id, index, assertion, field)?;
    if evidence_promotion_known_class(&class) {
        Ok(class)
    } else {
        Err(format!(
            "evidence promotion case `{case_id}` assertion {index}: unknown evidence class `{class}`"
        ))
    }
}

fn build_ripr_for_evidence_promotion_external() -> Result<(), String> {
    run("cargo", &["build", "-p", "ripr", "--quiet"])
        .map(|_| ())
        .map_err(|err| {
            format!(
                "check-evidence-promotion-honesty failed to build ripr for pinned external cases: {err}"
            )
        })
}

fn evidence_promotion_existing_repo_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| format!("failed to resolve current directory: {err}"))?
            .join(path)
    };
    if absolute.exists() {
        Ok(absolute)
    } else {
        Err(format!(
            "path does not exist: {}",
            normalize_path(&absolute)
        ))
    }
}

fn run_evidence_promotion_external_case(
    case: &EvidencePromotionExternalCase,
    options: &EvidencePromotionHonestyOptions,
    binary: &Path,
) -> EvidencePromotionExternalRun {
    if case.language != "rust" {
        return EvidencePromotionExternalRun::terminal(
            case,
            "setup_failure",
            format!(
                "evidence promotion pinned external case `{}` uses unsupported language `{}`; only rust is supported in this vertical slice",
                case.id, case.language
            ),
        );
    }
    let patch = match evidence_promotion_existing_repo_path(&case.external_patch) {
        Ok(path) => path,
        Err(err) => {
            return EvidencePromotionExternalRun::terminal(
                case,
                "setup_failure",
                format!(
                    "evidence promotion pinned external case `{}`: failed to resolve patch {}: {err}",
                    case.id,
                    normalize_path(&case.external_patch)
                ),
            );
        }
    };
    let checkout = options
        .checkout_root
        .join(evidence_promotion_checkout_name(case));
    let checkout_display = normalize_path(&checkout);
    if let Err(err) = prepare_evidence_promotion_checkout(case, options, &checkout) {
        let result_kind = evidence_promotion_external_failure_kind(std::slice::from_ref(&err));
        return EvidencePromotionExternalRun::terminal(case, &result_kind, err);
    }
    let result = run_evidence_promotion_external_check(case, options, binary, &checkout, &patch);
    let cleanup_result = clean_evidence_promotion_checkout(case, &checkout);
    match (result, cleanup_result) {
        (Ok(mut run), Ok(())) => {
            run.checkout = checkout_display;
            run
        }
        (Ok(mut run), Err(err)) => {
            run.checkout = checkout_display;
            run.status = "fail".to_string();
            run.result_kind = "setup_failure".to_string();
            run.violations.push(err);
            run
        }
        (Err(err), Ok(())) => {
            let result_kind = evidence_promotion_external_failure_kind(std::slice::from_ref(&err));
            EvidencePromotionExternalRun::terminal(case, &result_kind, err)
        }
        (Err(err), Err(cleanup_err)) => EvidencePromotionExternalRun::terminal(
            case,
            "setup_failure",
            format!("{err}; {cleanup_err}"),
        ),
    }
}

fn evidence_promotion_checkout_name(case: &EvidencePromotionExternalCase) -> String {
    let prefix = case.external_commit.chars().take(12).collect::<String>();
    format!("{}-{prefix}", sanitize_path_segment(&case.id))
}

fn sanitize_path_segment(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    if out.is_empty() {
        "case".to_string()
    } else {
        out
    }
}

fn prepare_evidence_promotion_checkout(
    case: &EvidencePromotionExternalCase,
    options: &EvidencePromotionHonestyOptions,
    checkout: &Path,
) -> Result<(), String> {
    if !checkout.exists() {
        if !options.clone {
            return Err(format!(
                "evidence promotion pinned external case `{}`: checkout {} is missing; rerun with --pinned-external --clone to create the bounded cache",
                case.id,
                normalize_path(checkout)
            ));
        }
        let parent = checkout.parent().ok_or_else(|| {
            format!(
                "evidence promotion pinned external case `{}`: checkout path has no parent: {}",
                case.id,
                normalize_path(checkout)
            )
        })?;
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "evidence promotion pinned external case `{}`: failed to create checkout root {}: {err}",
                case.id,
                normalize_path(parent)
            )
        })?;
        let checkout_text = checkout.to_string_lossy().to_string();
        run_with_envs(
            "git",
            &[
                "clone",
                "--filter=blob:none",
                &case.external_repo,
                &checkout_text,
            ],
            &[],
        )
        .map_err(|err| {
            format!(
                "evidence promotion pinned external case `{}`: clone failed: {err}",
                case.id
            )
        })?;
    }

    let checkout_text = checkout.to_string_lossy().to_string();
    if options.clone {
        run_with_envs(
            "git",
            &[
                "-C",
                &checkout_text,
                "fetch",
                "--depth",
                "1",
                "origin",
                &case.external_commit,
            ],
            &[],
        )
        .map_err(|err| {
            format!(
                "evidence promotion pinned external case `{}`: fetch of exact commit failed: {err}",
                case.id
            )
        })?;
    } else if !evidence_promotion_checkout_has_commit(checkout, &case.external_commit)? {
        return Err(format!(
            "evidence promotion pinned external case `{}`: cached checkout {} does not contain exact commit {}; rerun with --pinned-external --clone to refresh the bounded cache",
            case.id,
            normalize_path(checkout),
            case.external_commit
        ));
    }
    run_with_envs(
        "git",
        &[
            "-C",
            &checkout_text,
            "checkout",
            "--detach",
            &case.external_commit,
        ],
        &[],
    )
    .map_err(|err| {
        format!(
            "evidence promotion pinned external case `{}`: checkout of exact commit failed: {err}",
            case.id
        )
    })?;
    clean_evidence_promotion_checkout(case, checkout)?;
    Ok(())
}

fn evidence_promotion_checkout_has_commit(checkout: &Path, commit: &str) -> Result<bool, String> {
    let checkout_text = checkout.to_string_lossy().to_string();
    command_success_owned(
        "git",
        &[
            "-C".to_string(),
            checkout_text,
            "cat-file".to_string(),
            "-e".to_string(),
            format!("{commit}^{{commit}}"),
        ],
    )
}

fn clean_evidence_promotion_checkout(
    case: &EvidencePromotionExternalCase,
    checkout: &Path,
) -> Result<(), String> {
    let checkout_text = checkout.to_string_lossy().to_string();
    run_with_envs(
        "git",
        &[
            "-C",
            &checkout_text,
            "reset",
            "--hard",
            &case.external_commit,
        ],
        &[],
    )
    .map_err(|err| {
        format!(
            "evidence promotion pinned external case `{}`: reset cleanup failed: {err}",
            case.id
        )
    })?;
    run_with_envs("git", &["-C", &checkout_text, "clean", "-fdx"], &[]).map_err(|err| {
        format!(
            "evidence promotion pinned external case `{}`: clean cleanup failed: {err}",
            case.id
        )
    })?;
    Ok(())
}

fn run_evidence_promotion_external_check(
    case: &EvidencePromotionExternalCase,
    options: &EvidencePromotionHonestyOptions,
    binary: &Path,
    checkout: &Path,
    patch: &Path,
) -> Result<EvidencePromotionExternalRun, String> {
    let checkout_text = checkout.to_string_lossy().to_string();
    let patch_text = patch.to_string_lossy().to_string();
    run_with_envs(
        "git",
        &["-C", &checkout_text, "apply", "--check", &patch_text],
        &[],
    )
    .map_err(|err| {
        format!(
            "evidence promotion pinned external case `{}`: patch did not apply cleanly: {err}",
            case.id
        )
    })?;
    run_with_envs("git", &["-C", &checkout_text, "apply", &patch_text], &[]).map_err(|err| {
        format!(
            "evidence promotion pinned external case `{}`: patch apply failed: {err}",
            case.id
        )
    })?;

    let program = binary.to_string_lossy().to_string();
    let command_args = vec![
        "check".to_string(),
        "--root".to_string(),
        checkout_text.clone(),
        "--diff".to_string(),
        patch_text,
        "--mode".to_string(),
        "fast".to_string(),
        "--json".to_string(),
    ];
    let rendered_command =
        "ripr check --root {checkout} --diff {external_patch} --mode fast --json";
    let mut violations = Vec::new();
    if case.external_command != rendered_command {
        violations.push(format!(
            "evidence promotion pinned external case `{}`: external_command `{}` does not match supported command template `{rendered_command}`",
            case.id, case.external_command
        ));
    }
    let output = capture_output_with_timeout(
        &program,
        &command_args,
        &[],
        options.timeout,
        "check-evidence-promotion-honesty pinned external ripr check",
    )
    .map_err(|err| {
        format!(
            "evidence promotion pinned external case `{}`: ripr check failed to start: {err}",
            case.id
        )
    })?;
    let runtime_ms = output.duration.as_millis();
    if output.timed_out {
        violations.push(format!(
            "evidence promotion pinned external case `{}`: runtime budget exceeded by timeout after {}ms",
            case.id, runtime_ms
        ));
    }
    if runtime_ms > u128::from(case.runtime_budget_seconds) * 1000 {
        violations.push(format!(
            "evidence promotion pinned external case `{}`: runtime {}ms exceeded budget {}s",
            case.id, runtime_ms, case.runtime_budget_seconds
        ));
    }
    if !output.status.is_some_and(|status| status.success()) {
        violations.push(format!(
            "evidence promotion pinned external case `{}`: ripr check exited non-zero; stderr: {}",
            case.id,
            excerpt_for_report(&output.stderr, 400)
        ));
    }
    let parsed: Value = serde_json::from_str(&output.stdout).map_err(|err| {
        format!(
            "evidence promotion pinned external case `{}`: ripr stdout was not JSON: {err}; stdout: {}",
            case.id,
            excerpt_for_report(&output.stdout, 400)
        )
    })?;
    let artifact_bytes = directory_size_bytes(&checkout.join("target").join("ripr"))?;
    if artifact_bytes > case.artifact_budget_bytes {
        violations.push(format!(
            "evidence promotion pinned external case `{}`: artifact bytes {} exceeded budget {}",
            case.id, artifact_bytes, case.artifact_budget_bytes
        ));
    }
    violations.extend(evidence_promotion_external_semantic_violations(
        case, &parsed,
    ));
    Ok(EvidencePromotionExternalRun {
        id: case.id.clone(),
        status: if violations.is_empty() {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
        result_kind: if violations.is_empty() {
            "pass".to_string()
        } else {
            evidence_promotion_external_failure_kind(&violations)
        },
        runtime_ms,
        artifact_bytes,
        external_case: Some(EvidencePromotionExternalLaunch::from_case(case)),
        checkout: normalize_path(checkout),
        violations,
    })
}

pub(crate) fn evidence_promotion_external_semantic_violations(
    case: &EvidencePromotionExternalCase,
    check_json: &Value,
) -> Vec<String> {
    evidence_promotion_semantic_violations(
        &case.id,
        None,
        &case.assertions,
        check_json,
        None,
        false,
    )
}

pub(crate) fn evidence_promotion_semantic_violations(
    case_id: &str,
    source_fixture: Option<&str>,
    assertions: &[EvidencePromotionSemanticAssertion],
    check_json: &Value,
    human_text: Option<&str>,
    fixture_human_required: bool,
) -> Vec<String> {
    let mut violations = Vec::new();
    let case_label = evidence_promotion_assertion_case_label(case_id, source_fixture);
    let findings = check_json
        .get("findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for assertion in assertions {
        match assertion {
            EvidencePromotionSemanticAssertion::MustPromote => {
                let has_exposed = findings.iter().any(|finding| {
                    finding.get("classification").and_then(Value::as_str) == Some("exposed")
                });
                if !has_exposed {
                    violations.push(format!(
                        "{case_label}: `must_promote` requires at least one finding with classification `exposed`"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustNotPromote => {
                for finding in &findings {
                    let class = finding
                        .get("classification")
                        .and_then(Value::as_str)
                        .unwrap_or("static_unknown");
                    if class == "exposed" {
                        let finding_id = evidence_promotion_finding_id(finding);
                        violations.push(format!(
                            "{case_label}: finding `{finding_id}` promoted to exposed (classification `exposed`) but `must_not_promote` is asserted"
                        ));
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustReportClean => {
                if !evidence_promotion_report_reads_clean(check_json, &findings) {
                    violations.push(format!(
                        "{case_label}: `must_report_clean` requires an empty complete-looking result with no scope or limitation disclosure"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustNotReportClean => {
                if evidence_promotion_report_reads_clean(check_json, &findings) {
                    violations.push(format!(
                        "{case_label}: `must_not_report_clean` requires findings, a scope disclosure, or a named limitation; result has no non-clean signal"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustDiscloseScope => {
                let missing_scope = evidence_promotion_missing_scope_fields(check_json);
                if !missing_scope.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_disclose_scope` requires report-level scope fields schema_version/tool/mode/root/base, but missing or empty field(s): {}",
                        missing_scope.join(", ")
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustDiscloseNoScope => {
                if !evidence_promotion_discloses_no_scope(check_json) {
                    violations.push(format!(
                        "{case_label}: `must_disclose_no_scope` requires a no_scope_provided/no_scope_disclosure scope disclosure"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustNotDiscloseNoScope => {
                if evidence_promotion_discloses_no_scope(check_json) {
                    violations.push(format!(
                        "{case_label}: `must_not_disclose_no_scope` forbids no_scope_provided/no_scope_disclosure because the case asserts an explicit analysis scope"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustDiscloseUnanalyzedWorkingTree => {
                if !evidence_promotion_discloses_unanalyzed_working_tree(check_json) {
                    violations.push(format!(
                        "{case_label}: `must_disclose_unanalyzed_working_tree` requires unanalyzed_working_tree=true"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustNotDiscloseUnanalyzedWorkingTree => {
                if evidence_promotion_discloses_unanalyzed_working_tree(check_json) {
                    violations.push(format!(
                        "{case_label}: `must_not_disclose_unanalyzed_working_tree` forbids unanalyzed_working_tree=true because the case asserts the working-tree draft was in scope"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustEmitLimitation {
                expected_limit_kind,
            } => {
                if expected_limit_kind.trim().is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_emit_limitation` requires expected_limit_kind"
                    ));
                } else {
                    // Findings come from the validated report slice (pure
                    // and pinned-external cases pass different shapes);
                    // the harness projection comes from the check output.
                    let has_limit = findings.iter().any(|finding| {
                        finding.get("static_limit_kind").and_then(Value::as_str)
                            == Some(expected_limit_kind.as_str())
                    }) || evidence_promotion_emits_any_harness_limitation_kind(
                        check_json,
                        expected_limit_kind,
                    );
                    if !has_limit {
                        violations.push(format!(
                            "{case_label}: expected limitation kind `{expected_limit_kind}` was not emitted in findings' static_limit_kind or test_harnesses limitations"
                        ));
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustNotEmitLimitation => {
                let limit_paths =
                    json_non_empty_string_field_paths(check_json, "static_limit_kind");
                if !limit_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_not_emit_limitation` forbids static_limit_kind, but found it at {}",
                        limit_paths.join(", ")
                    ));
                }
                if evidence_promotion_emits_any_harness_limitation(check_json) {
                    violations.push(format!(
                        "{case_label}: `must_not_emit_limitation` forbids test_harnesses limitations, but the harness projection emitted at least one"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustHaveVerifyCommand => {
                let verify_paths = json_non_empty_string_field_paths(check_json, "verify_command");
                if verify_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_have_verify_command` requires a non-empty verify_command"
                    ));
                } else if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let verify_values =
                                json_non_empty_string_field_values(check_json, "verify_command");
                            let missing_human = evidence_promotion_missing_human_command_values(
                                human_text,
                                EvidencePromotionHumanCommandKind::Verify,
                                &verify_values,
                            );
                            if !missing_human.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `must_have_verify_command` requires fixture human output to project the same verify command, but missing {}",
                                    missing_human.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `must_have_verify_command` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustNotHaveVerifyCommand => {
                let verify_paths = json_non_empty_string_field_paths(check_json, "verify_command");
                if !verify_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_not_have_verify_command` forbids verify_command, but found it at {}",
                        verify_paths.join(", ")
                    ));
                }
                if fixture_human_required && let Some(human_text) = human_text {
                    let human_verify = evidence_promotion_human_command_projection_lines(
                        human_text,
                        EvidencePromotionHumanCommandKind::Verify,
                    );
                    if !human_verify.is_empty() {
                        violations.push(format!(
                            "{case_label}: `must_not_have_verify_command` forbids fixture human verify command projection, but found {}",
                            human_verify.join(", ")
                        ));
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustHaveReceiptCommand => {
                let receipt_paths =
                    json_non_empty_string_field_paths(check_json, "receipt_command");
                if receipt_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_have_receipt_command` requires a non-empty receipt_command"
                    ));
                } else if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let receipt_values =
                                json_non_empty_string_field_values(check_json, "receipt_command");
                            let missing_human = evidence_promotion_missing_human_command_values(
                                human_text,
                                EvidencePromotionHumanCommandKind::Receipt,
                                &receipt_values,
                            );
                            if !missing_human.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `must_have_receipt_command` requires fixture human output to project the same receipt command, but missing {}",
                                    missing_human.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `must_have_receipt_command` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustNotHaveReceiptCommand => {
                let receipt_paths =
                    json_non_empty_string_field_paths(check_json, "receipt_command");
                if !receipt_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_not_have_receipt_command` forbids receipt_command, but found it at {}",
                        receipt_paths.join(", ")
                    ));
                }
                if fixture_human_required && let Some(human_text) = human_text {
                    let human_receipt = evidence_promotion_human_command_projection_lines(
                        human_text,
                        EvidencePromotionHumanCommandKind::Receipt,
                    );
                    if !human_receipt.is_empty() {
                        violations.push(format!(
                            "{case_label}: `must_not_have_receipt_command` forbids fixture human receipt command projection, but found {}",
                            human_receipt.join(", ")
                        ));
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustEmitRepairPacket => {
                let packet_ready_paths =
                    json_bool_field_paths(check_json, "repair_packet_ready", true);
                if packet_ready_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_emit_repair_packet` requires repair_packet_ready=true"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustNotEmitRepairPacket => {
                let packet_ready_paths =
                    json_bool_field_paths(check_json, "repair_packet_ready", true);
                if !packet_ready_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_not_emit_repair_packet` forbids repair_packet_ready=true, but found it at {}",
                        packet_ready_paths.join(", ")
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustDiscloseRepairPacketDetail => {
                let packet_detail_violations =
                    evidence_promotion_repair_packet_detail_violations(check_json);
                if !packet_detail_violations.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_disclose_repair_packet_detail` requires a packet with canonical gap, source/target, edit cage, repair shape, verify/receipt commands, must-not-change constraints, and raw evidence refs, but found {}",
                        packet_detail_violations.join(", ")
                    ));
                }
                if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let missing_human =
                                evidence_promotion_missing_human_repair_packet_detail_paths(
                                    human_text,
                                );
                            if !missing_human.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `must_disclose_repair_packet_detail` requires fixture human output to surface repair-packet handoff fields, but missing {}",
                                    missing_human.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `must_disclose_repair_packet_detail` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedRepairPacketDetail { detail } => {
                let mismatches =
                    evidence_promotion_expected_repair_packet_detail_mismatches(check_json, detail);
                if !mismatches.is_empty() {
                    violations.push(format!(
                        "{case_label}: `expected_repair_packet_detail` requires an exact repair-packet handoff for canonical gap `{}` but found {}",
                        detail.canonical_gap_id,
                        mismatches.join(", ")
                    ));
                }
                if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let missing_human =
                                evidence_promotion_expected_human_repair_packet_detail_mismatches(
                                    human_text, detail,
                                );
                            if !missing_human.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `expected_repair_packet_detail` requires fixture human output to surface the same exact handoff fields, but missing {}",
                                    missing_human.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `expected_repair_packet_detail` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustNotHaveContradictoryPacketMessaging => {
                let contradictory =
                    evidence_promotion_contradictory_packet_messaging_paths(check_json);
                if !contradictory.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_not_have_contradictory_packet_messaging` forbids packet-ready findings from retaining blocked actionability messaging, but found {}",
                        contradictory.join(", ")
                    ));
                }
                if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let contradictory_human =
                                evidence_promotion_human_contradictory_packet_messaging_lines(
                                    check_json, human_text,
                                );
                            if !contradictory_human.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `must_not_have_contradictory_packet_messaging` forbids fixture human output for packet-ready findings from retaining blocked actionability messaging, but found {}",
                                    contradictory_human.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `must_not_have_contradictory_packet_messaging` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedOracle {
                kind: expected_kind,
                strength: expected_strength,
            } => {
                if findings.is_empty() {
                    violations.push(format!(
                        "{case_label}: `expected_oracle` `{expected_kind}/{expected_strength}` requires at least one finding"
                    ));
                }
                for finding in &findings {
                    let finding_id = evidence_promotion_finding_id(finding);
                    let kind = finding
                        .get("oracle_kind")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>");
                    if kind != expected_kind {
                        violations.push(format!(
                            "{case_label}: `expected_oracle` requires oracle_kind `{expected_kind}`, but finding `{finding_id}` has oracle_kind `{kind}`"
                        ));
                    }
                    let strength = finding
                        .get("oracle_strength")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>");
                    if strength != expected_strength {
                        violations.push(format!(
                            "{case_label}: `expected_oracle` requires oracle_strength `{expected_strength}`, but finding `{finding_id}` has oracle_strength `{strength}`"
                        ));
                    }
                }
                if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let missing_human =
                                evidence_promotion_missing_human_oracle_projection_paths(
                                    human_text,
                                    expected_kind,
                                    expected_strength,
                                );
                            if !missing_human.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `expected_oracle` requires fixture human output to project oracle `{expected_kind}/{expected_strength}`, but found {}",
                                    missing_human.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `expected_oracle` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedClass {
                class: expected_class,
            } => {
                if findings.is_empty() {
                    violations.push(format!(
                        "{case_label}: `expected_class` `{expected_class}` requires at least one finding"
                    ));
                }
                for finding in &findings {
                    let class = finding
                        .get("classification")
                        .and_then(Value::as_str)
                        .unwrap_or("static_unknown");
                    if class != expected_class {
                        let finding_id = evidence_promotion_finding_id(finding);
                        violations.push(format!(
                            "{case_label}: finding `{finding_id}` has classification `{class}`, expected_class `{expected_class}`"
                        ));
                    }
                }
                if fixture_human_required && !findings.is_empty() {
                    match human_text {
                        Some(human_text) => {
                            let class_count = evidence_promotion_human_class_projection_count(
                                human_text,
                                expected_class,
                            );
                            if class_count < findings.len() {
                                violations.push(format!(
                                    "{case_label}: `expected_class` requires fixture human output to project class `{expected_class}` for {} finding(s), but found {class_count} projection(s)",
                                    findings.len()
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `expected_class` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MaximumClass {
                class: expected_max_class,
            } => {
                let max_severity = evidence_class_severity(expected_max_class);
                for finding in &findings {
                    let class = finding
                        .get("classification")
                        .and_then(Value::as_str)
                        .unwrap_or("static_unknown");
                    if evidence_class_severity(class) > max_severity {
                        let finding_id = evidence_promotion_finding_id(finding);
                        violations.push(format!(
                            "{case_label}: finding `{finding_id}` class `{class}` exceeds maximum `{expected_max_class}`"
                        ));
                    }
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedCompleteness { completeness } => {
                let observed = check_json
                    .get("analysis_scope")
                    .and_then(|scope| scope.get("completeness"))
                    .and_then(Value::as_str);
                if observed != Some(completeness.as_str()) {
                    violations.push(format!(
                        "{case_label}: `expected_completeness` requires analysis_scope.completeness `{completeness}`, got `{}`",
                        observed.unwrap_or("<missing>")
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedChangedRustFiles { count } => {
                let observed = check_json
                    .get("summary")
                    .and_then(|summary| summary.get("changed_rust_files"))
                    .and_then(Value::as_u64);
                if observed != Some(*count) {
                    violations.push(format!(
                        "{case_label}: `expected_changed_rust_files` requires summary.changed_rust_files `{count}`, got `{}`",
                        observed
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "<missing>".to_string())
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedFindingCount { count } => {
                let observed = u64::try_from(findings.len()).unwrap_or(u64::MAX);
                if observed != *count {
                    violations.push(format!(
                        "{case_label}: `expected_finding_count` requires {count} finding(s), found {observed}"
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustDiscloseWitness => {
                let witness_lines = evidence_promotion_witness_lines(&findings);
                if witness_lines.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_disclose_witness` did not find evidence prefix `{EVIDENCE_PROMOTION_WITNESS_PREFIX}`"
                    ));
                }
                if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let missing_human_paths =
                                evidence_promotion_missing_human_witness_paths(
                                    human_text,
                                    &witness_lines,
                                );
                            if !missing_human_paths.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `must_disclose_witness` requires fixture human output to surface the same witness under `Where to look`, but missing {}",
                                    missing_human_paths.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `must_disclose_witness` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::MustDiscloseLimitationDetail => {
                let missing_details = evidence_promotion_missing_limitation_detail_paths(&findings);
                if !missing_details.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_disclose_limitation_detail` requires every static limitation to name the last established edge, first unresolved edge, analyzer route, and non-claim, but missing {}",
                        missing_details.join(", ")
                    ));
                }
                if fixture_human_required {
                    match human_text {
                        Some(human_text) => {
                            let expected_details =
                                evidence_promotion_limitation_detail_lines(&findings);
                            let missing_human_paths =
                                evidence_promotion_missing_human_limitation_detail_paths(
                                    human_text,
                                    &expected_details,
                                );
                            if !missing_human_paths.is_empty() {
                                violations.push(format!(
                                    "{case_label}: `must_disclose_limitation_detail` requires fixture human output to surface the same limitation detail under `Limitation detail`, but missing {}",
                                    missing_human_paths.join(", ")
                                ));
                            }
                        }
                        None => {
                            violations.push(format!(
                                "{case_label}: `must_disclose_limitation_detail` requires fixture human output at `expected/human-full.txt`, but it was missing"
                            ));
                        }
                    }
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedLimitationDetail {
                last_established_edge,
                first_unresolved_edge,
                non_claim,
            } => {
                let expected_details = [
                    (
                        "last established edge",
                        "limitation_last_established_edge: ",
                        last_established_edge.as_str(),
                    ),
                    (
                        "first unresolved edge",
                        "limitation_first_unresolved_edge: ",
                        first_unresolved_edge.as_str(),
                    ),
                    ("non-claim", "limitation_non_claim: ", non_claim.as_str()),
                ];
                let mismatches =
                    evidence_promotion_limitation_detail_mismatches(&findings, &expected_details);
                if !mismatches.is_empty() {
                    violations.push(format!(
                        "{case_label}: `expected_limitation_detail` requires every static limitation to carry the expected last edge, unresolved edge, and non-claim, but found {}",
                        mismatches.join(", ")
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::ExpectedLimitationRoute { route } => {
                let mismatches = evidence_promotion_limitation_route_mismatches(&findings, route);
                if !mismatches.is_empty() {
                    violations.push(format!(
                        "{case_label}: `expected_limitation_route` requires every static limitation to use analyzer route `{route}`, but found {}",
                        mismatches.join(", ")
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustNotClaimNoTestsFound => {
                let mut no_tests_paths = evidence_promotion_no_tests_found_claim_paths(check_json);
                if let Some(human_text) = human_text {
                    no_tests_paths
                        .extend(evidence_promotion_no_tests_found_human_paths(human_text));
                }
                if !no_tests_paths.is_empty() {
                    violations.push(format!(
                        "{case_label}: `must_not_claim_no_tests_found` forbids `No tests were found` claims when candidate test evidence is disclosed, but found them at {}",
                        no_tests_paths.join(", ")
                    ));
                }
            }
            EvidencePromotionSemanticAssertion::MustSeeChangedFile { path } => {
                let saw_file = findings.iter().any(|finding| {
                    finding
                        .get("probe")
                        .and_then(|probe| probe.get("file"))
                        .and_then(Value::as_str)
                        .is_some_and(|file| normalize_slashes(file).ends_with(path))
                });
                if !saw_file {
                    violations.push(format!(
                        "{case_label}: `must_see_changed_file` expected changed file `{path}` was not present in finding probe files"
                    ));
                }
            }
        }
    }
    violations
}

const EVIDENCE_PROMOTION_WITNESS_PREFIX: &str = "For example, the test ";
const EVIDENCE_PROMOTION_HUMAN_PROJECTION_PATH: &str = "expected/human-full.txt";
const EVIDENCE_PROMOTION_LIMITATION_DETAILS: [(&str, &str); 4] = [
    (
        "last established edge",
        "limitation_last_established_edge: ",
    ),
    (
        "first unresolved edge",
        "limitation_first_unresolved_edge: ",
    ),
    ("analyzer route", "limitation_analyzer_route: "),
    ("non-claim", "limitation_non_claim: "),
];

fn evidence_promotion_witness_lines(findings: &[Value]) -> Vec<String> {
    let mut witness_lines = Vec::new();
    for finding in findings {
        let Some(evidence) = finding.get("evidence").and_then(Value::as_array) else {
            continue;
        };
        for line in evidence.iter().filter_map(Value::as_str) {
            let witness_line = line.trim();
            if witness_line.starts_with(EVIDENCE_PROMOTION_WITNESS_PREFIX)
                && !witness_lines.iter().any(|seen| seen == witness_line)
            {
                witness_lines.push(witness_line.to_string());
            }
        }
    }
    witness_lines
}

fn evidence_promotion_missing_limitation_detail_paths(findings: &[Value]) -> Vec<String> {
    let mut missing = Vec::new();
    let mut limitation_count = 0usize;
    for (index, finding) in findings.iter().enumerate() {
        if finding
            .get("static_limit_kind")
            .and_then(Value::as_str)
            .is_none()
        {
            continue;
        }
        limitation_count += 1;
        let evidence_lines: Vec<&str> = finding
            .get("evidence")
            .and_then(Value::as_array)
            .map(|evidence| evidence.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for (label, prefix) in EVIDENCE_PROMOTION_LIMITATION_DETAILS {
            let has_detail = evidence_lines.iter().any(|line| {
                line.trim()
                    .strip_prefix(prefix)
                    .is_some_and(|value| !value.trim().is_empty())
            });
            if !has_detail {
                missing.push(format!("$.findings[{index}].evidence:missing {label}"));
            }
        }
        let structured = finding.get("static_limitation").and_then(Value::as_object);
        let kind = finding.get("static_limit_kind").and_then(Value::as_str);
        if structured
            .and_then(|object| object.get("kind"))
            .and_then(Value::as_str)
            != kind
        {
            missing.push(format!("$.findings[{index}].static_limitation.kind"));
        }
        for (label, _) in EVIDENCE_PROMOTION_LIMITATION_DETAILS {
            let key = evidence_promotion_structured_limitation_detail_key(label);
            let has_detail = structured
                .and_then(|object| object.get(key))
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty());
            if !has_detail {
                missing.push(format!("$.findings[{index}].static_limitation.{key}"));
            }
        }
    }

    if limitation_count == 0 {
        missing.push("$.findings:missing static_limit_kind".to_string());
    }

    missing
}

fn evidence_promotion_structured_limitation_detail_key(label: &str) -> &'static str {
    match label {
        "last established edge" => "last_established_edge",
        "first unresolved edge" => "first_unresolved_edge",
        "analyzer route" => "analyzer_route",
        "non-claim" => "non_claim",
        _ => "unknown",
    }
}

fn evidence_promotion_limitation_detail_lines(findings: &[Value]) -> Vec<(String, String)> {
    let mut details = Vec::new();
    for finding in findings {
        if finding
            .get("static_limit_kind")
            .and_then(Value::as_str)
            .is_none()
        {
            continue;
        }
        let Some(evidence) = finding.get("evidence").and_then(Value::as_array) else {
            continue;
        };
        for line in evidence.iter().filter_map(Value::as_str) {
            let trimmed = line.trim();
            for (label, prefix) in EVIDENCE_PROMOTION_LIMITATION_DETAILS {
                let Some(value) = trimmed.strip_prefix(prefix).map(str::trim) else {
                    continue;
                };
                if value.is_empty() {
                    continue;
                }
                push_unique_limitation_detail(&mut details, label.to_string(), value.to_string());
            }
        }
    }
    details
}

fn evidence_promotion_limitation_detail_mismatches(
    findings: &[Value],
    expected_details: &[(&str, &str, &str)],
) -> Vec<String> {
    let mut mismatches = Vec::new();
    let mut limitation_count = 0usize;
    for (index, finding) in findings.iter().enumerate() {
        if finding
            .get("static_limit_kind")
            .and_then(Value::as_str)
            .is_none()
        {
            continue;
        }
        limitation_count += 1;
        let evidence_lines: Vec<&str> = finding
            .get("evidence")
            .and_then(Value::as_array)
            .map(|evidence| evidence.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for (label, prefix, expected) in expected_details {
            let observed = evidence_lines
                .iter()
                .find_map(|line| line.trim().strip_prefix(prefix).map(str::trim));
            match observed {
                Some(value) if value == *expected => {}
                Some("") => {
                    mismatches.push(format!("$.findings[{index}].evidence:empty {label}"));
                }
                Some(value) => {
                    mismatches.push(format!("$.findings[{index}].evidence:{label}:{value}"));
                }
                None => {
                    mismatches.push(format!("$.findings[{index}].evidence:missing {label}"));
                }
            }
        }
    }

    if limitation_count == 0 {
        mismatches.push("$.findings:missing static_limit_kind".to_string());
    }

    mismatches
}

fn evidence_promotion_limitation_route_mismatches(
    findings: &[Value],
    expected_route: &str,
) -> Vec<String> {
    let mut mismatches = Vec::new();
    let mut limitation_count = 0usize;
    for (index, finding) in findings.iter().enumerate() {
        if finding
            .get("static_limit_kind")
            .and_then(Value::as_str)
            .is_none()
        {
            continue;
        }
        limitation_count += 1;
        let route = finding
            .get("evidence")
            .and_then(Value::as_array)
            .and_then(|evidence| {
                evidence.iter().filter_map(Value::as_str).find_map(|line| {
                    line.trim()
                        .strip_prefix("limitation_analyzer_route: ")
                        .map(str::trim)
                })
            });
        match route {
            Some(route) if route == expected_route => {}
            Some("") => {
                mismatches.push(format!("$.findings[{index}].evidence:empty route"));
            }
            Some(route) => {
                mismatches.push(format!("$.findings[{index}].evidence:{route}"));
            }
            None => {
                mismatches.push(format!("$.findings[{index}].evidence:missing route"));
            }
        }
    }

    if limitation_count == 0 {
        mismatches.push("$.findings:missing static_limit_kind".to_string());
    }

    mismatches
}

const EVIDENCE_PROMOTION_REPAIR_PACKET_STRING_FIELDS: [(&str, &str); 11] = [
    ("canonical gap id", "canonical_gap_id"),
    ("gap id", "gap_id"),
    ("language", "language"),
    ("language status", "language_status"),
    ("source file", "file"),
    ("target test", "target_test"),
    ("assertion shape", "assertion_shape"),
    ("authority boundary", "authority_boundary"),
    ("repair kind", "repair_kind"),
    ("verify command", "verify_command"),
    ("receipt command", "receipt_command"),
];
const EVIDENCE_PROMOTION_REPAIR_PACKET_ARRAY_FIELDS: [(&str, &str); 3] = [
    ("allowed edit surface", "allowed_edit_surface"),
    ("forbidden files", "forbidden_files"),
    ("must-not-change constraints", "must_not_change"),
];
const EVIDENCE_PROMOTION_REPAIR_PACKET_HUMAN_SNIPPETS: [(&str, &str); 10] = [
    ("packet section", "TypeScript repair packet"),
    ("canonical gap", "canonical gap:"),
    ("source", "source:"),
    ("target test", "related test:"),
    ("repair shape", "oracle:"),
    ("edit surface", "edit surface:"),
    ("verify command", "verify:"),
    ("receipt command", "receipt:"),
    ("must-not-change constraints", "must not change:"),
    ("authority", "authority:"),
];

fn evidence_promotion_repair_packet_detail_violations(check_json: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    let packets = evidence_promotion_repair_packet_objects(check_json);
    if packets.is_empty() {
        violations.push("$.findings:missing repair packet object".to_string());
    }
    for (path, packet) in packets {
        for (label, field) in EVIDENCE_PROMOTION_REPAIR_PACKET_STRING_FIELDS {
            if packet
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                violations.push(format!("{path}.{field}:missing {label}"));
            }
        }
        for (label, field) in EVIDENCE_PROMOTION_REPAIR_PACKET_ARRAY_FIELDS {
            if packet
                .get(field)
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
            {
                violations.push(format!("{path}.{field}:missing {label}"));
            }
        }
        if packet
            .get("line")
            .and_then(Value::as_u64)
            .is_none_or(|line| line == 0)
        {
            violations.push(format!("{path}.line:missing source line"));
        }
    }
    if json_non_empty_array_field_paths(check_json, "raw_evidence_refs").is_empty() {
        violations.push("$.raw_evidence_refs:missing raw evidence refs".to_string());
    }
    violations
}

fn evidence_promotion_repair_packet_objects(check_json: &Value) -> Vec<(String, &Value)> {
    fn walk<'a>(value: &'a Value, path: String, packets: &mut Vec<(String, &'a Value)>) {
        match value {
            Value::Object(map) => {
                let looks_like_packet = map
                    .get("canonical_gap_id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                    && map.get("verify_command").and_then(Value::as_str).is_some()
                    && map.get("receipt_command").and_then(Value::as_str).is_some();
                if looks_like_packet {
                    packets.push((path.clone(), value));
                }
                for (key, child) in map {
                    let child_path = if path == "$" {
                        format!("$.{key}")
                    } else {
                        format!("{path}.{key}")
                    };
                    walk(child, child_path, packets);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(child, format!("{path}[{index}]"), packets);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    let mut packets = Vec::new();
    walk(check_json, "$".to_string(), &mut packets);
    packets
}

fn evidence_promotion_missing_human_repair_packet_detail_paths(human_text: &str) -> Vec<String> {
    EVIDENCE_PROMOTION_REPAIR_PACKET_HUMAN_SNIPPETS
        .iter()
        .filter(|(_, snippet)| !human_text.contains(snippet))
        .map(|(label, snippet)| format!("expected/human-full.txt:missing {label} `{snippet}`"))
        .collect()
}

fn evidence_promotion_expected_repair_packet_detail_mismatches(
    check_json: &Value,
    expected: &ExpectedRepairPacketDetail,
) -> Vec<String> {
    let packets = evidence_promotion_repair_packet_objects(check_json);
    if packets.is_empty() {
        return vec!["$.findings:missing repair packet object".to_string()];
    }

    let mut closest = Vec::new();
    for (path, packet) in packets {
        let mismatches =
            evidence_promotion_single_repair_packet_detail_mismatches(&path, packet, expected);
        if mismatches.is_empty() {
            return Vec::new();
        }
        if closest.is_empty() || mismatches.len() < closest.len() {
            closest = mismatches;
        }
    }
    closest
}

fn evidence_promotion_single_repair_packet_detail_mismatches(
    path: &str,
    packet: &Value,
    expected: &ExpectedRepairPacketDetail,
) -> Vec<String> {
    let mut mismatches = Vec::new();
    for (field, expected_value) in [
        ("canonical_gap_id", expected.canonical_gap_id.as_str()),
        ("file", expected.source_file.as_str()),
        ("target_test", expected.target_test.as_str()),
        ("assertion_shape", expected.assertion_shape.as_str()),
        ("authority_boundary", expected.authority_boundary.as_str()),
        ("repair_kind", expected.repair_kind.as_str()),
        ("verify_command", expected.verify_command.as_str()),
        ("receipt_command", expected.receipt_command.as_str()),
    ] {
        match packet.get(field).and_then(Value::as_str) {
            Some(actual) if actual == expected_value => {}
            Some(actual) => mismatches.push(format!(
                "{path}.{field}:expected `{expected_value}` got `{actual}`"
            )),
            None => mismatches.push(format!(
                "{path}.{field}:expected `{expected_value}` got `<missing>`"
            )),
        }
    }

    match packet.get("line").and_then(Value::as_u64) {
        Some(actual) if actual == expected.source_line as u64 => {}
        Some(actual) => mismatches.push(format!(
            "{path}.line:expected `{}` got `{actual}`",
            expected.source_line
        )),
        None => mismatches.push(format!(
            "{path}.line:expected `{}` got `<missing>`",
            expected.source_line
        )),
    }

    for (field, expected_values) in [
        ("allowed_edit_surface", &expected.allowed_edit_surface),
        ("forbidden_files", &expected.forbidden_files),
    ] {
        let actual_values = json_string_array_field(packet, field);
        if actual_values != *expected_values {
            mismatches.push(format!(
                "{path}.{field}:expected [{}] got [{}]",
                expected_values.join(", "),
                actual_values.join(", ")
            ));
        }
    }

    mismatches
}

fn evidence_promotion_expected_human_repair_packet_detail_mismatches(
    human_text: &str,
    expected: &ExpectedRepairPacketDetail,
) -> Vec<String> {
    let mut missing = Vec::new();
    let Some(packet_section) = evidence_promotion_human_repair_packet_section(human_text) else {
        return vec![
            "expected/human-full.txt:missing TypeScript repair packet section".to_string(),
        ];
    };
    let source_line = expected.source_line.to_string();
    for (label, snippet) in [
        ("canonical gap", expected.canonical_gap_id.as_str()),
        ("source file", expected.source_file.as_str()),
        ("source line", source_line.as_str()),
        ("target test", expected.target_test.as_str()),
        ("assertion shape", expected.assertion_shape.as_str()),
        ("authority boundary", expected.authority_boundary.as_str()),
        ("verify command", expected.verify_command.as_str()),
        ("receipt command", expected.receipt_command.as_str()),
    ] {
        if !packet_section.contains(snippet) {
            missing.push(format!(
                "expected/human-full.txt:missing {label} `{snippet}`"
            ));
        }
    }
    for value in &expected.allowed_edit_surface {
        if !packet_section.contains(value) {
            missing.push(format!(
                "expected/human-full.txt:missing allowed edit surface `{value}`"
            ));
        }
    }
    for value in &expected.forbidden_files {
        if !packet_section.contains(value) {
            missing.push(format!(
                "expected/human-full.txt:missing forbidden file `{value}`"
            ));
        }
    }
    missing
}

fn evidence_promotion_human_repair_packet_section(human_text: &str) -> Option<&str> {
    evidence_promotion_human_repair_packet_sections(human_text)
        .into_iter()
        .next()
}

fn evidence_promotion_human_repair_packet_sections(human_text: &str) -> Vec<&str> {
    let mut sections = Vec::new();
    let mut search_start = 0;
    while let Some(relative_start) = human_text[search_start..].find("TypeScript repair packet") {
        let start = search_start + relative_start;
        let tail = &human_text[start..];
        let end = tail
            .find("\n\n")
            .map(|relative_end| start + relative_end)
            .unwrap_or(human_text.len());
        sections.push(&human_text[start..end]);
        if end == human_text.len() {
            break;
        }
        search_start = end + 2;
    }
    sections
}

fn evidence_promotion_contradictory_packet_messaging_paths(check_json: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(findings) = check_json.get("findings").and_then(Value::as_array) else {
        return violations;
    };
    for (finding_index, finding) in findings.iter().enumerate() {
        if !evidence_promotion_finding_packet_ready(finding) {
            continue;
        }
        let mut strings = Vec::new();
        collect_string_paths(
            finding,
            format!("$.findings[{finding_index}]"),
            &mut strings,
        );
        for (path, value) in strings {
            if let Some(reason) = evidence_promotion_packet_blocked_message_reason(&value) {
                violations.push(format!("{path}:{reason}"));
            }
        }
    }
    violations
}

fn evidence_promotion_finding_packet_ready(finding: &Value) -> bool {
    finding.get("repair_packet_ready").and_then(Value::as_bool) == Some(true)
        || finding
            .get("preview_actionability")
            .and_then(|actionability| actionability.get("repair_packet_ready"))
            .and_then(Value::as_bool)
            == Some(true)
}

fn evidence_promotion_packet_blocked_message_reason(value: &str) -> Option<&'static str> {
    let trimmed = value.trim();
    if trimmed == "gap_state: advisory" {
        return Some("blocked gap_state evidence");
    }
    if trimmed == "actionability_category: incomplete_repair_packet" {
        return Some("blocked actionability category evidence");
    }
    if trimmed.starts_with("why_not_actionable: ") {
        return Some("blocked why-not-actionable evidence");
    }
    if trimmed.starts_with("repair_route: ") {
        return Some("blocked repair-route evidence");
    }
    if trimmed.starts_with("missing_actionability_fields: ") {
        return Some("blocked missing-field evidence");
    }
    if trimmed.starts_with("evidence_needed_to_promote: ")
        && trimmed
            .strip_prefix("evidence_needed_to_promote: ")
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Some("blocked evidence-needed evidence");
    }
    if trimmed.contains("lacks a complete repair packet contract")
        || trimmed.contains("only after verify, receipt, evidence refs")
    {
        return Some("blocked incomplete-packet text");
    }
    None
}

fn evidence_promotion_human_contradictory_packet_messaging_lines(
    check_json: &Value,
    human_text: &str,
) -> Vec<String> {
    let ready_gap_ids = evidence_promotion_packet_ready_canonical_gap_ids(check_json);
    evidence_promotion_human_repair_packet_sections(human_text)
        .into_iter()
        .filter(|section| {
            ready_gap_ids.is_empty()
                || ready_gap_ids
                    .iter()
                    .any(|canonical_gap_id| section.contains(canonical_gap_id))
        })
        .flat_map(|section| {
            section.lines().filter_map(|line| {
                evidence_promotion_human_packet_blocked_message_reason(line)
                    .map(|reason| format!("expected/human-full.txt:{}:{reason}", line.trim()))
            })
        })
        .collect()
}

fn evidence_promotion_packet_ready_canonical_gap_ids(check_json: &Value) -> Vec<String> {
    let Some(findings) = check_json.get("findings").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut gap_ids = Vec::new();
    for finding in findings {
        if evidence_promotion_finding_packet_ready(finding) {
            collect_canonical_gap_ids(finding, &mut gap_ids);
        }
    }
    gap_ids.sort();
    gap_ids.dedup();
    gap_ids
}

fn collect_canonical_gap_ids(value: &Value, gap_ids: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(gap_id) = map.get("canonical_gap_id").and_then(Value::as_str) {
                let gap_id = gap_id.trim();
                if !gap_id.is_empty() {
                    gap_ids.push(gap_id.to_string());
                }
            }
            for child in map.values() {
                collect_canonical_gap_ids(child, gap_ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_canonical_gap_ids(item, gap_ids);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn evidence_promotion_human_packet_blocked_message_reason(value: &str) -> Option<&'static str> {
    let trimmed = value.trim();
    let normalized = trimmed.to_ascii_lowercase();
    if normalized == "status: not actionable" {
        return Some("blocked not-actionable status");
    }
    if normalized == "repair packet ready: false" {
        return Some("blocked packet-ready false status");
    }
    if normalized == "gap state: advisory" {
        return Some("blocked advisory gap state");
    }
    if normalized == "category: incomplete_repair_packet" {
        return Some("blocked incomplete actionability category");
    }
    if normalized.starts_with("why not actionable:") {
        return Some("blocked why-not-actionable line");
    }
    if normalized.starts_with("limitation:") {
        return Some("blocked limitation line");
    }
    if normalized.starts_with("repair route:") {
        return Some("blocked repair-route line");
    }
    if normalized.starts_with("missing fields:") {
        return Some("blocked missing-fields line");
    }
    if normalized.starts_with("evidence needed:") {
        return Some("blocked evidence-needed line");
    }
    if normalized.contains("lacks a complete repair packet contract")
        || normalized.contains("no actionable repair packet is emitted")
        || normalized.contains("no repair packet is emitted")
    {
        return Some("blocked incomplete-packet text");
    }
    None
}

fn evidence_promotion_missing_human_limitation_detail_paths(
    human_text: &str,
    expected_details: &[(String, String)],
) -> Vec<String> {
    let mut missing = Vec::new();
    if expected_details.is_empty() {
        return missing;
    }

    let human_details = evidence_promotion_human_limitation_details(human_text);
    if human_details.is_empty() {
        missing.push("expected/human-full.txt:missing Limitation detail".to_string());
    }
    for (label, value) in expected_details {
        if !human_details
            .iter()
            .any(|(human_label, human_value)| human_label == label && human_value == value)
        {
            missing.push(format!(
                "expected/human-full.txt:missing detail `{label}: {value}`"
            ));
        }
    }
    missing
}

fn evidence_promotion_human_limitation_details(human_text: &str) -> Vec<(String, String)> {
    let mut details = Vec::new();
    let mut in_limitation_detail = false;
    for line in human_text.lines() {
        let trimmed = line.trim();
        if trimmed == "Limitation detail" {
            in_limitation_detail = true;
            continue;
        }
        if in_limitation_detail && trimmed.is_empty() {
            in_limitation_detail = false;
            continue;
        }
        if !in_limitation_detail {
            continue;
        }

        for (label, _) in EVIDENCE_PROMOTION_LIMITATION_DETAILS {
            let human_prefix = format!("{label}: ");
            let Some(value) = trimmed.strip_prefix(&human_prefix).map(str::trim) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            push_unique_limitation_detail(&mut details, label.to_string(), value.to_string());
        }
    }
    details
}

fn push_unique_limitation_detail(
    details: &mut Vec<(String, String)>,
    label: String,
    value: String,
) {
    if !details.iter().any(|(existing_label, existing_value)| {
        existing_label == &label && existing_value == &value
    }) {
        details.push((label, value));
    }
}

fn evidence_promotion_no_tests_found_claim_paths(value: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_no_tests_found_claim_paths(value, "$".to_string(), &mut paths);
    paths
}

fn evidence_promotion_missing_human_witness_paths(
    human_text: &str,
    witness_lines: &[String],
) -> Vec<String> {
    let mut missing = Vec::new();
    let human_witness_lines = evidence_promotion_human_where_to_look_witnesses(human_text);

    if human_witness_lines.is_empty() {
        missing.push("expected/human-full.txt:missing Where to look".to_string());
    }
    for witness_line in witness_lines {
        if !human_witness_lines
            .iter()
            .any(|human_line| human_line == witness_line)
        {
            missing.push(format!(
                "expected/human-full.txt:missing witness `{witness_line}`"
            ));
        }
    }

    missing
}

fn evidence_promotion_human_where_to_look_witnesses(human_text: &str) -> Vec<String> {
    let mut witness_lines = Vec::new();
    let mut in_where_to_look = false;
    for line in human_text.lines() {
        let trimmed = line.trim();
        if trimmed == "Where to look" {
            in_where_to_look = true;
            continue;
        }
        if in_where_to_look && trimmed.is_empty() {
            in_where_to_look = false;
            continue;
        }
        if in_where_to_look
            && trimmed.starts_with(EVIDENCE_PROMOTION_WITNESS_PREFIX)
            && !witness_lines
                .iter()
                .any(|witness_line| witness_line == trimmed)
        {
            witness_lines.push(trimmed.to_string());
        }
    }
    witness_lines
}

fn evidence_promotion_no_tests_found_human_paths(human_text: &str) -> Vec<String> {
    human_text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("No tests were found"))
        .map(|(index, _)| format!("expected/human-full.txt:{}", index + 1))
        .collect()
}

fn collect_no_tests_found_claim_paths(value: &Value, path: String, paths: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if text.contains("No tests were found") {
                paths.push(path);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_no_tests_found_claim_paths(item, format!("{path}[{index}]"), paths);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                collect_no_tests_found_claim_paths(item, format!("{path}.{key}"), paths);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn evidence_promotion_missing_human_oracle_projection_paths(
    human_text: &str,
    expected_kind: &str,
    expected_strength: &str,
) -> Vec<String> {
    if human_text.lines().any(|line| {
        evidence_promotion_human_oracle_line_matches(line, expected_kind, expected_strength)
    }) {
        return Vec::new();
    }

    vec![format!(
        "expected/human-full.txt:missing oracle projection `{expected_kind}/{expected_strength}`"
    )]
}

pub(crate) fn evidence_promotion_human_oracle_line_matches(
    line: &str,
    expected_kind: &str,
    expected_strength: &str,
) -> bool {
    let expected_kind = expected_kind.to_ascii_lowercase();
    let expected_strength = expected_strength.to_ascii_lowercase();
    let normalized = line.trim().to_ascii_lowercase();

    if evidence_promotion_human_line_field_value(&normalized, "oracle_kind").as_deref()
        == Some(expected_kind.as_str())
        && evidence_promotion_human_line_field_value(&normalized, "oracle_strength").as_deref()
            == Some(expected_strength.as_str())
    {
        return true;
    }

    let Some(card) = normalized.strip_prefix("oracle:") else {
        return false;
    };
    let tokens = evidence_promotion_human_oracle_tokens(card);
    tokens.iter().any(|token| token == &expected_kind)
        && tokens.iter().any(|token| token == &expected_strength)
}

fn evidence_promotion_human_line_field_value(line: &str, field: &str) -> Option<String> {
    let mut tail = line;
    while let Some(index) = tail.find(field) {
        let after_field = &tail[index + field.len()..];
        let after_delimiter = after_field.trim_start();
        let Some(after_delimiter) = after_delimiter
            .strip_prefix('=')
            .or_else(|| after_delimiter.strip_prefix(':'))
        else {
            tail = after_field.get(1..).unwrap_or("");
            continue;
        };
        let value = after_delimiter.trim_start();
        let end = value
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
            .unwrap_or(value.len());
        if end == 0 {
            return None;
        }
        let value = value.get(..end)?;
        return Some(value.to_string());
    }
    None
}

fn evidence_promotion_human_oracle_tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn evidence_promotion_human_class_projection_count(
    human_text: &str,
    expected_class: &str,
) -> usize {
    human_text
        .lines()
        .filter(|line| evidence_promotion_human_class_line_matches(line, expected_class))
        .count()
}

pub(crate) fn evidence_promotion_human_class_line_matches(
    line: &str,
    expected_class: &str,
) -> bool {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix(expected_class) else {
        return false;
    };
    rest.is_empty()
        || rest
            .chars()
            .next()
            .is_some_and(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
}

#[derive(Clone, Copy)]
enum EvidencePromotionHumanCommandKind {
    Verify,
    Receipt,
}

impl EvidencePromotionHumanCommandKind {
    fn labels(self) -> &'static [&'static str] {
        match self {
            Self::Verify => &["verify", "verify command", "verify_command"],
            Self::Receipt => &["receipt", "receipt command", "receipt_command"],
        }
    }

    fn field_name(self) -> &'static str {
        match self {
            Self::Verify => "verify_command",
            Self::Receipt => "receipt_command",
        }
    }
}

fn evidence_promotion_missing_human_command_values(
    human_text: &str,
    kind: EvidencePromotionHumanCommandKind,
    expected_values: &[String],
) -> Vec<String> {
    let projected = evidence_promotion_human_command_projection_lines(human_text, kind);
    expected_values
        .iter()
        .filter(|expected| {
            !projected
                .iter()
                .any(|line| line.contains(expected.as_str()))
        })
        .map(|expected| {
            format!(
                "expected/human-full.txt:missing {} `{expected}`",
                kind.field_name()
            )
        })
        .collect()
}

fn evidence_promotion_human_command_projection_lines(
    human_text: &str,
    kind: EvidencePromotionHumanCommandKind,
) -> Vec<String> {
    human_text
        .lines()
        .filter_map(|line| evidence_promotion_human_command_projection_line(line, kind))
        .collect()
}

fn evidence_promotion_human_command_projection_line(
    line: &str,
    kind: EvidencePromotionHumanCommandKind,
) -> Option<String> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    for label in kind.labels() {
        let Some(after_label) = lower.strip_prefix(label) else {
            continue;
        };
        let after_label = after_label.trim_start();
        let Some(after_colon) = after_label.strip_prefix(':') else {
            continue;
        };
        let value_start = trimmed.len() - after_colon.len();
        let value = trimmed.get(value_start..)?.trim();
        if evidence_promotion_human_command_value_is_concrete(value) {
            return Some(format!("expected/human-full.txt:{trimmed}"));
        }
    }
    None
}

fn evidence_promotion_human_command_value_is_concrete(value: &str) -> bool {
    let value = value.trim().trim_matches('`').trim();
    if value.is_empty() {
        return false;
    }
    let normalized = value.to_ascii_lowercase();
    let first_token = normalized
        .split(|ch: char| ch.is_whitespace() || ch == '(' || ch == ';' || ch == ',')
        .next()
        .unwrap_or("");
    !matches!(
        first_token,
        "none"
            | "null"
            | "n/a"
            | "unknown"
            | "missing"
            | "<none>"
            | "<missing>"
            | "not_applicable"
            | "verify_command_unknown"
            | "receipt_command_unknown"
            | "unavailable_until_python_gap_ledger"
    ) && !normalized.starts_with("not available")
        && !normalized.starts_with("unavailable")
}

fn collect_string_paths(value: &Value, path: String, paths: &mut Vec<(String, String)>) {
    match value {
        Value::String(text) => {
            paths.push((path, text.clone()));
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_string_paths(item, format!("{path}[{index}]"), paths);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                collect_string_paths(item, format!("{path}.{key}"), paths);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn evidence_promotion_assertion_case_label(case_id: &str, source_fixture: Option<&str>) -> String {
    match source_fixture {
        Some(fixture) => {
            format!("evidence promotion honesty case `{case_id}` (fixture `{fixture}`)")
        }
        None => format!("evidence promotion pinned external case `{case_id}`"),
    }
}

fn evidence_promotion_finding_id(finding: &Value) -> &str {
    finding
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("<no-id>")
}

fn evidence_promotion_missing_scope_fields(check_json: &Value) -> Vec<&'static str> {
    ["schema_version", "tool", "mode", "root", "base"]
        .iter()
        .filter_map(|field| {
            let present = check_json
                .get(*field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty());
            (!present).then_some(*field)
        })
        .collect()
}

fn evidence_promotion_report_reads_clean(check_json: &Value, findings: &[Value]) -> bool {
    let summary_findings = check_json
        .get("summary")
        .and_then(|summary| summary.get("findings"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if summary_findings > 0 || !findings.is_empty() {
        return false;
    }
    if evidence_promotion_discloses_unanalyzed_working_tree(check_json) {
        return false;
    }
    if evidence_promotion_discloses_no_scope(check_json) {
        return false;
    }
    if check_json
        .get("limitations")
        .and_then(Value::as_array)
        .is_some_and(|limitations| !limitations.is_empty())
    {
        return false;
    }
    if evidence_promotion_emits_any_harness_limitation(check_json) {
        return false;
    }
    if check_json
        .get("preview_languages")
        .and_then(Value::as_array)
        .is_some_and(|advisories| !advisories.is_empty())
    {
        return false;
    }
    if !json_non_empty_string_field_paths(check_json, "static_limit_kind").is_empty() {
        return false;
    }
    true
}

/// Whether any harness projection in the check output emits a limitation
/// with the given code.
fn evidence_promotion_emits_any_harness_limitation_kind(check_json: &Value, kind: &str) -> bool {
    check_json
        .get("test_harnesses")
        .and_then(Value::as_array)
        .is_some_and(|harnesses| {
            harnesses.iter().any(|harness| {
                harness
                    .get("limitations")
                    .and_then(Value::as_array)
                    .is_some_and(|limitations| {
                        limitations.iter().any(|limitation| {
                            limitation.get("code").and_then(Value::as_str) == Some(kind)
                        })
                    })
            })
        })
}

/// Whether any harness projection in the check output emits any
/// limitation at all.
fn evidence_promotion_emits_any_harness_limitation(check_json: &Value) -> bool {
    check_json
        .get("test_harnesses")
        .and_then(Value::as_array)
        .is_some_and(|harnesses| {
            harnesses.iter().any(|harness| {
                harness
                    .get("limitations")
                    .and_then(Value::as_array)
                    .is_some_and(|limitations| !limitations.is_empty())
            })
        })
}

fn evidence_promotion_discloses_unanalyzed_working_tree(check_json: &Value) -> bool {
    check_json
        .get("unanalyzed_working_tree")
        .and_then(Value::as_bool)
        == Some(true)
}

fn evidence_promotion_discloses_no_scope(check_json: &Value) -> bool {
    if check_json.get("no_scope_provided").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    check_json
        .get("scope_disclosures")
        .and_then(Value::as_array)
        .is_some_and(|disclosures| {
            disclosures.iter().any(|disclosure| {
                disclosure.get("scope_status").and_then(Value::as_str) == Some("no_scope_provided")
                    || disclosure.get("category").and_then(Value::as_str)
                        == Some("no_scope_disclosure")
            })
        })
}

pub(crate) fn evidence_promotion_external_failure_kind(violations: &[String]) -> String {
    evidence_promotion_failure_kind(violations, false)
}

pub(crate) fn evidence_promotion_pure_failure_kind(violations: &[String]) -> String {
    evidence_promotion_failure_kind(violations, true)
}

fn evidence_promotion_failure_kind(violations: &[String], pure_case: bool) -> String {
    let joined = violations.join("\n").to_ascii_lowercase();
    if joined.contains("runtime budget exceeded") || joined.contains("timed out") {
        "runtime_budget_exceeded".to_string()
    } else if joined.contains("artifact bytes") || joined.contains("artifact budget") {
        "artifact_budget_exceeded".to_string()
    } else if joined.contains("clone failed") || joined.contains("fetch of exact commit failed") {
        "network_unavailable".to_string()
    } else if joined.contains("promoted to exposed")
        || joined.contains("classification `exposed`")
        || joined.contains("exceeds maximum")
    {
        "unexpected_promotion".to_string()
    } else if joined.contains("static_limit_kind")
        || joined.contains("must_emit_limitation")
        || joined.contains("must_not_emit_limitation")
        || joined.contains("named limitation")
        || joined.contains("test_harnesses limitations")
    {
        "unexpected_limitation".to_string()
    } else if pure_case
        && (joined.contains("re-bless")
            || joined.contains("expected/check.json")
            || joined.contains("must_report_clean")
            || joined.contains("must_not_report_clean")
            || joined.contains("must_disclose_scope")
            || joined.contains("must_disclose_no_scope")
            || joined.contains("must_disclose_unanalyzed_working_tree")
            || joined.contains("must_not_emit_repair_packet")
            || joined.contains("must_disclose_witness")
            || joined.contains("must_not_claim_no_tests_found")
            || joined.contains("must_have_verify_command")
            || joined.contains("must_not_have_verify_command")
            || joined.contains("must_have_receipt_command")
            || joined.contains("must_not_have_receipt_command")
            || joined.contains("must_emit_repair_packet")
            || joined.contains("must_not_emit_limitation")
            || joined.contains("must_disclose_repair_packet_detail")
            || joined.contains("must_not_have_contradictory_packet_messaging")
            || joined.contains("expected_class")
            || joined.contains("expected_completeness"))
    {
        "golden_drift".to_string()
    } else if joined.contains("missing")
        || joined.contains("unknown tier")
        || joined.contains("path does not exist")
        || joined.contains("patch did not apply")
        || joined.contains("failed to resolve patch")
        || joined.contains("unsupported language")
        || joined.contains("checkout")
        || joined.contains("setup")
    {
        "setup_failure".to_string()
    } else {
        "semantic_failure".to_string()
    }
}

fn directory_size_bytes(path: &Path) -> Result<u64, String> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let metadata = fs::symlink_metadata(&current).map_err(|err| {
            format!(
                "failed to inspect artifact path {}: {err}",
                normalize_path(&current)
            )
        })?;
        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        } else if metadata.is_dir() {
            for entry in fs::read_dir(&current).map_err(|err| {
                format!(
                    "failed to read artifact directory {}: {err}",
                    normalize_path(&current)
                )
            })? {
                let entry = entry.map_err(|err| {
                    format!(
                        "failed to read artifact directory entry under {}: {err}",
                        normalize_path(&current)
                    )
                })?;
                stack.push(entry.path());
            }
        }
    }
    Ok(total)
}

fn excerpt_for_report(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= limit {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..limit])
    }
}

pub(crate) fn write_evidence_promotion_external_report(
    runs: &[EvidencePromotionExternalRun],
    violations: &[String],
) -> Result<(), String> {
    ensure_reports_dir()?;
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let json_runs = runs
        .iter()
        .map(|run| {
            serde_json::json!({
                "id": run.id,
                "status": run.status,
                "result_kind": run.result_kind,
                "runtime_ms": run.runtime_ms,
                "artifact_bytes": run.artifact_bytes,
                "external_case": run.external_case.as_ref().map(EvidencePromotionExternalLaunch::to_json),
                "checkout": run.checkout,
                "violations": run.violations,
            })
        })
        .collect::<Vec<_>>();
    let json_report = serde_json::json!({
        "schema_version": "0.1",
        "kind": "evidence_promotion_pinned_external",
        "status": status,
        "cases_total": runs.len(),
        "cases_passed": runs.iter().filter(|run| run.violations.is_empty()).count(),
        "cases_failed": runs.iter().filter(|run| !run.violations.is_empty()).count(),
        "violations": violations,
        "runs": json_runs,
    });
    write_report(
        EVIDENCE_PROMOTION_EXTERNAL_JSON,
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&json_report)
                .map_err(|err| format!("serialize pinned external report: {err}"))?
        ),
    )?;

    let mut markdown = format!(
        "# Evidence Promotion Pinned External\n\nStatus: `{status}`\n\nCases: `{}` total, `{}` passed, `{}` failed.\n\n",
        runs.len(),
        runs.iter().filter(|run| run.violations.is_empty()).count(),
        runs.iter().filter(|run| !run.violations.is_empty()).count()
    );
    if violations.is_empty() {
        markdown.push_str("## Violations\n\nNone detected.\n\n");
    } else {
        markdown.push_str("## Violations\n\n");
        for violation in violations {
            markdown.push_str("- ");
            markdown.push_str(violation);
            markdown.push('\n');
        }
        markdown.push('\n');
    }
    if runs.iter().any(|run| run.external_case.is_some()) {
        markdown.push_str("## Launch Points\n\n");
        markdown.push_str("| Case | Repository | Commit | Patch | Command | Runtime budget seconds | Artifact budget bytes |\n");
        markdown.push_str("|---|---|---|---|---|---:|---:|\n");
        for run in runs.iter().filter(|run| run.external_case.is_some()) {
            let Some(external_case) = &run.external_case else {
                continue;
            };
            markdown.push_str(&format!(
                "| `{}` | {} | `{}` | `{}` | `{}` | {} | {} |\n",
                run.id,
                markdown_cell(&external_case.repo),
                external_case.commit,
                markdown_cell(&external_case.patch),
                markdown_cell(&external_case.command),
                external_case.runtime_budget_seconds,
                external_case.artifact_budget_bytes
            ));
        }
        markdown.push('\n');
    }
    markdown.push_str("## Cases\n\n");
    markdown.push_str("| Case | Status | Result kind | Runtime ms | Artifact bytes |\n");
    markdown.push_str("|---|---:|---:|---:|---:|\n");
    for run in runs {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} |\n",
            run.id, run.status, run.result_kind, run.runtime_ms, run.artifact_bytes
        ));
    }
    markdown.push('\n');
    write_report(EVIDENCE_PROMOTION_EXTERNAL_MD, &markdown)?;
    Ok(())
}

fn load_evidence_promotion_corpus_case_metadata(
    corpus_path: &Path,
) -> Result<Vec<EvidencePromotionCorpusCaseMeta>, String> {
    let corpus_text = read_text_lossy(corpus_path)?;
    let corpus_value: Value = parse_json_rejecting_duplicate_keys(&corpus_text)
        .map_err(|err| format!("failed to parse {}: {err}", normalize_path(corpus_path)))?;
    let Some(cases) = corpus_value.get("cases").and_then(Value::as_array) else {
        return Err(format!(
            "{} has no `cases` array",
            normalize_path(corpus_path)
        ));
    };
    Ok(cases
        .iter()
        .map(|case| EvidencePromotionCorpusCaseMeta {
            id: case
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<missing-id>")
                .to_string(),
            language: case
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            tier: case
                .get("tier")
                .and_then(Value::as_str)
                .unwrap_or("<missing-tier>")
                .to_string(),
            external_case: if case.get("tier").and_then(Value::as_str) == Some("pinned_external") {
                EvidencePromotionExternalLaunch::from_case_json(case)
            } else {
                None
            },
        })
        .collect())
}

fn evidence_promotion_case_violations(id: &str, violations: &[String]) -> Vec<String> {
    let needle = format!("`{id}`");
    violations
        .iter()
        .filter(|violation| violation.contains(&needle))
        .cloned()
        .collect()
}

fn evidence_promotion_unmatched_violations(
    metadata: &[EvidencePromotionCorpusCaseMeta],
    violations: &[String],
) -> Vec<String> {
    violations
        .iter()
        .filter(|violation| {
            !metadata
                .iter()
                .any(|case| violation.contains(&format!("`{}`", case.id)))
        })
        .cloned()
        .collect()
}

fn build_evidence_promotion_corpus_summary_cases(
    metadata: &[EvidencePromotionCorpusCaseMeta],
    pure_violations: &[String],
    external_runs: &[EvidencePromotionExternalRun],
    options: &EvidencePromotionHonestyOptions,
) -> Vec<CorpusSummaryCase> {
    let mut cases = Vec::new();
    for case in metadata {
        if case.tier == "pinned_external" {
            let selected = options
                .only_case
                .as_ref()
                .is_none_or(|selected| selected == &case.id);
            if options.run_pinned_external && selected {
                if let Some(run) = external_runs.iter().find(|run| run.id == case.id) {
                    cases.push(CorpusSummaryCase {
                        id: case.id.clone(),
                        language: case.language.clone(),
                        tier: case.tier.clone(),
                        status: run.status.clone(),
                        result_kind: run.result_kind.clone(),
                        message: if run.violations.is_empty() {
                            "case passed semantic expectations".to_string()
                        } else {
                            run.violations.join("; ")
                        },
                        runtime_ms: Some(run.runtime_ms),
                        artifact_bytes: Some(run.artifact_bytes),
                        external_case: run
                            .external_case
                            .clone()
                            .or_else(|| case.external_case.clone()),
                    });
                } else {
                    cases.push(CorpusSummaryCase {
                        id: case.id.clone(),
                        language: case.language.clone(),
                        tier: case.tier.clone(),
                        status: "fail".to_string(),
                        result_kind: "setup_failure".to_string(),
                        message: "selected pinned_external case did not produce a run record"
                            .to_string(),
                        runtime_ms: None,
                        artifact_bytes: None,
                        external_case: case.external_case.clone(),
                    });
                }
            } else {
                cases.push(CorpusSummaryCase {
                    id: case.id.clone(),
                    language: case.language.clone(),
                    tier: case.tier.clone(),
                    status: "not_run".to_string(),
                    result_kind: "not_run".to_string(),
                    message: if options.run_pinned_external {
                        "case filtered by --case".to_string()
                    } else {
                        "pinned external cases require --pinned-external".to_string()
                    },
                    runtime_ms: None,
                    artifact_bytes: None,
                    external_case: case.external_case.clone(),
                });
            }
        } else {
            let case_violations = evidence_promotion_case_violations(&case.id, pure_violations);
            cases.push(CorpusSummaryCase {
                id: case.id.clone(),
                language: case.language.clone(),
                tier: case.tier.clone(),
                status: if case_violations.is_empty() {
                    "pass".to_string()
                } else {
                    "fail".to_string()
                },
                result_kind: if case_violations.is_empty() {
                    "pass".to_string()
                } else {
                    evidence_promotion_pure_failure_kind(&case_violations)
                },
                message: if case_violations.is_empty() {
                    "case passed semantic expectations".to_string()
                } else {
                    case_violations.join("; ")
                },
                runtime_ms: None,
                artifact_bytes: None,
                external_case: None,
            });
        }
    }

    for violation in evidence_promotion_unmatched_violations(metadata, pure_violations) {
        cases.push(CorpusSummaryCase {
            id: "__corpus__".to_string(),
            language: "unknown".to_string(),
            tier: "corpus".to_string(),
            status: "fail".to_string(),
            result_kind: evidence_promotion_pure_failure_kind(std::slice::from_ref(&violation)),
            message: violation,
            runtime_ms: None,
            artifact_bytes: None,
            external_case: None,
        });
    }
    for run in external_runs
        .iter()
        .filter(|run| run.id.starts_with("__") && !run.violations.is_empty())
    {
        cases.push(CorpusSummaryCase {
            id: run.id.clone(),
            language: "unknown".to_string(),
            tier: "pinned_external".to_string(),
            status: run.status.clone(),
            result_kind: run.result_kind.clone(),
            message: run.violations.join("; "),
            runtime_ms: Some(run.runtime_ms),
            artifact_bytes: Some(run.artifact_bytes),
            external_case: run.external_case.clone(),
        });
    }

    cases
}

fn write_evidence_promotion_corpus_summary_report(
    corpus_path: &Path,
    pure_violations: &[String],
    external_runs: &[EvidencePromotionExternalRun],
    options: &EvidencePromotionHonestyOptions,
) -> Result<(), String> {
    ensure_reports_dir()?;
    let metadata = load_evidence_promotion_corpus_case_metadata(corpus_path)?;
    let cases = build_evidence_promotion_corpus_summary_cases(
        &metadata,
        pure_violations,
        external_runs,
        options,
    );
    let failed = cases.iter().filter(|case| case.status == "fail").count();
    let passed = cases.iter().filter(|case| case.status == "pass").count();
    let not_run = cases.iter().filter(|case| case.status == "not_run").count();
    let status = if failed == 0 { "pass" } else { "fail" };

    let json_cases = cases
        .iter()
        .map(|case| {
            serde_json::json!({
                "id": case.id,
                "language": case.language,
                "tier": case.tier,
                "status": case.status,
                "result_kind": case.result_kind,
                "message": case.message,
                "runtime_ms": case.runtime_ms,
                "artifact_bytes": case.artifact_bytes,
                "external_case": case.external_case.as_ref().map(EvidencePromotionExternalLaunch::to_json),
            })
        })
        .collect::<Vec<_>>();
    let json_report = serde_json::json!({
        "schema_version": "0.1",
        "kind": "corpus_summary",
        "corpus": "evidence-promotion-honesty",
        "status": status,
        "cases_total": cases.len(),
        "cases_passed": passed,
        "cases_failed": failed,
        "cases_not_run": not_run,
        "failure_kinds": [
            "semantic_failure",
            "golden_drift",
            "setup_failure",
            "network_unavailable",
            "runtime_budget_exceeded",
            "artifact_budget_exceeded",
            "unexpected_limitation",
            "unexpected_promotion"
        ],
        "cases": json_cases,
    });
    write_report(
        CORPUS_SUMMARY_JSON,
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&json_report)
                .map_err(|err| format!("serialize corpus summary report: {err}"))?
        ),
    )?;

    let mut markdown = format!(
        "# Corpus Summary\n\nStatus: `{status}`\n\nCorpus: `evidence-promotion-honesty`\n\nCases: `{}` total, `{passed}` passed, `{failed}` failed, `{not_run}` not run.\n\n",
        cases.len()
    );
    markdown.push_str("## Failure Kinds\n\n");
    for kind in [
        "semantic_failure",
        "golden_drift",
        "setup_failure",
        "network_unavailable",
        "runtime_budget_exceeded",
        "artifact_budget_exceeded",
        "unexpected_limitation",
        "unexpected_promotion",
    ] {
        markdown.push_str("- `");
        markdown.push_str(kind);
        markdown.push_str("`\n");
    }
    markdown.push_str("\n## Cases\n\n");
    markdown.push_str("| Case | Tier | Status | Result kind | Message |\n");
    markdown.push_str("|---|---|---:|---|---|\n");
    for case in &cases {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} |\n",
            case.id,
            case.tier,
            case.status,
            case.result_kind,
            markdown_cell(&case.message)
        ));
    }
    if cases.iter().any(|case| case.external_case.is_some()) {
        markdown.push_str("\n## Pinned External Launches\n\n");
        markdown.push_str("| Case | Repository | Commit | Patch | Command | Runtime budget seconds | Artifact budget bytes |\n");
        markdown.push_str("|---|---|---|---|---|---:|---:|\n");
        for case in cases.iter().filter(|case| case.external_case.is_some()) {
            let Some(external_case) = &case.external_case else {
                continue;
            };
            markdown.push_str(&format!(
                "| `{}` | {} | `{}` | `{}` | `{}` | {} | {} |\n",
                case.id,
                markdown_cell(&external_case.repo),
                external_case.commit,
                markdown_cell(&external_case.patch),
                markdown_cell(&external_case.command),
                external_case.runtime_budget_seconds,
                external_case.artifact_budget_bytes
            ));
        }
    }
    markdown.push('\n');
    write_report(CORPUS_SUMMARY_MD, &markdown)?;
    Ok(())
}

pub(crate) fn check_evidence_promotion_honesty(args: &[String]) -> Result<(), String> {
    let options = parse_evidence_promotion_honesty_args(args)?;
    let corpus_path = Path::new(EVIDENCE_PROMOTION_HONESTY_CORPUS);
    let mut pure_violations = Vec::new();
    validate_evidence_promotion_honesty_corpus_at(corpus_path, &mut pure_violations)?;
    let mut external_runs = Vec::new();
    if options.run_pinned_external {
        external_runs = run_evidence_promotion_pinned_external_cases(corpus_path, &options)?;
    }
    write_evidence_promotion_corpus_summary_report(
        corpus_path,
        &pure_violations,
        &external_runs,
        &options,
    )?;
    let mut violations = pure_violations.clone();
    for run in &external_runs {
        violations.extend(run.violations.iter().cloned());
    }

    let report = if violations.is_empty() {
        "pass: corpus tiers valid; all charter members at expected class; no clean-guard case lost its findings; scope-guard cases kept report scope headers; no promoted case carries exposed; all controls retain exposed".to_string()
    } else {
        format!("FAIL: {}", violations.join("; "))
    };
    ensure_reports_dir()?;
    let mut body = format!(
        "# check-evidence-promotion-honesty\n\nStatus: {}\n\n",
        if violations.is_empty() {
            "pass"
        } else {
            "fail"
        }
    );
    body.push_str("## Why This Matters\n\n");
    body.push_str(
        "A finding may not be promoted to `exposed` unless its evidence STRUCTURALLY \
         matches the seam. Each confirmed fake-clean is pinned as a charter member that \
         must stay non-promoted. Honest re-bless of a charter fixture to `exposed` in the \
         golden would bypass `goldens check`; this gate reads the byte-pinned golden and \
         asserts the semantic expectation independently.\n\n",
    );
    if violations.is_empty() {
        body.push_str("## Violations\n\nNone detected.\n\n");
    } else {
        body.push_str("## Violations\n\n");
        for v in &violations {
            body.push_str("```text\n");
            body.push_str(v);
            body.push_str("\n```\n\n");
        }
        body.push_str("## Fix Kind\n\n```text\nAuthorDecisionRequired\n```\n\n");
        body.push_str("## Recommended Fixes\n\n");
        body.push_str(
            "1. If a non-promoted case now shows `exposed`, the classifier changed — \
               revert the production change or add a stricter corpus entry.\n\
             2. If a control case lost `exposed`, the gate has over-corrected — revert \
               the production change or update the control.\n\
             3. If a scope-guard case lost report-level scope fields, restore the \
               golden's schema_version/tool/mode/root/base header or remove the scope \
               guard only after updating the governing spec.\n\
             4. To register a new fake-clean, add a `must_not_promote` assertion to \
               fixtures/evidence-promotion-honesty-corpus/corpus.json.\n",
        );
    }
    body.push_str(&format!("\n## Detail\n\n{report}\n\n"));
    body.push_str("## Rerun\n\n```bash\ncargo xtask check-evidence-promotion-honesty\n```\n");
    write_report("evidence-promotion-honesty.md", &body)?;

    if violations.is_empty() {
        println!("{}", report);
        Ok(())
    } else {
        Err(format!(
            "check-evidence-promotion-honesty failed; see target/ripr/reports/evidence-promotion-honesty.md\n{}",
            violations.join("\n")
        ))
    }
}

pub(crate) fn validate_evidence_promotion_honesty_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let corpus_path = Path::new(EVIDENCE_PROMOTION_HONESTY_CORPUS);
    if !corpus_path.exists() {
        violations.push(format!(
            "evidence promotion honesty corpus is missing {}",
            normalize_path(corpus_path)
        ));
        return Ok(());
    }
    validate_evidence_promotion_honesty_corpus_at(corpus_path, violations)
}

fn evidence_promotion_known_class(class: &str) -> bool {
    matches!(
        class,
        "exposed"
            | "weakly_exposed"
            | "reachable_unrevealed"
            | "no_static_path"
            | "infection_unknown"
            | "propagation_unknown"
            | "static_unknown"
    )
}

/// Classification severity ordering: exposed > weakly_exposed > reachable_unrevealed/no_static_path/*_unknown
fn evidence_class_severity(class: &str) -> u8 {
    match class {
        "exposed" => 3,
        "weakly_exposed" => 2,
        "reachable_unrevealed" | "no_static_path" => 1,
        _ => 0, // infection_unknown, propagation_unknown, static_unknown
    }
}

fn json_bool_field_paths(value: &Value, field: &str, expected: bool) -> Vec<String> {
    fn walk(value: &Value, field: &str, expected: bool, path: &str, paths: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let child_path = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    if key == field && child.as_bool() == Some(expected) {
                        paths.push(child_path.clone());
                    }
                    walk(child, field, expected, &child_path, paths);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let child_path = if path.is_empty() {
                        format!("[{index}]")
                    } else {
                        format!("{path}[{index}]")
                    };
                    walk(child, field, expected, &child_path, paths);
                }
            }
            _ => {}
        }
    }

    let mut paths = Vec::new();
    walk(value, field, expected, "", &mut paths);
    paths
}

fn json_non_empty_string_field_paths(value: &Value, field: &str) -> Vec<String> {
    fn walk(value: &Value, field: &str, path: &str, paths: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let child_path = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    if key == field && child.as_str().is_some_and(|value| !value.trim().is_empty())
                    {
                        paths.push(child_path.clone());
                    }
                    walk(child, field, &child_path, paths);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let child_path = if path.is_empty() {
                        format!("[{index}]")
                    } else {
                        format!("{path}[{index}]")
                    };
                    walk(child, field, &child_path, paths);
                }
            }
            _ => {}
        }
    }

    let mut paths = Vec::new();
    walk(value, field, "", &mut paths);
    paths
}

fn json_non_empty_string_field_values(value: &Value, field: &str) -> Vec<String> {
    fn walk(value: &Value, field: &str, values: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if key == field
                        && let Some(text) = child
                            .as_str()
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                    {
                        let text = text.to_string();
                        if !values.contains(&text) {
                            values.push(text);
                        }
                    }
                    walk(child, field, values);
                }
            }
            Value::Array(items) => {
                for child in items {
                    walk(child, field, values);
                }
            }
            _ => {}
        }
    }

    let mut values = Vec::new();
    walk(value, field, &mut values);
    values
}

fn json_non_empty_array_field_paths(value: &Value, field: &str) -> Vec<String> {
    fn walk(value: &Value, field: &str, path: &str, paths: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let child_path = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    if key == field && child.as_array().is_some_and(|items| !items.is_empty()) {
                        paths.push(child_path.clone());
                    }
                    walk(child, field, &child_path, paths);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let child_path = if path.is_empty() {
                        format!("[{index}]")
                    } else {
                        format!("{path}[{index}]")
                    };
                    walk(child, field, &child_path, paths);
                }
            }
            _ => {}
        }
    }

    let mut paths = Vec::new();
    walk(value, field, "", &mut paths);
    paths
}

fn evidence_promotion_non_empty_string_field<'a>(case: &'a Value, field: &str) -> Option<&'a str> {
    case.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn evidence_promotion_positive_u64_field(case: &Value, field: &str) -> bool {
    case.get(field)
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0)
}

fn is_exact_git_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Parse JSON while rejecting duplicate object keys (issue #2277).
///
/// `serde_json::from_str::<Value>` keeps the LAST duplicate key silently, so a
/// hand-spliced corpus object with repeated `id`/`language`/... keys parses
/// fine while silently dropping an earlier case's pin — exactly the failure
/// the evidence-promotion corpus exists to catch. This visitor fails closed
/// on the first duplicate key instead.
fn parse_json_rejecting_duplicate_keys(text: &str) -> Result<Value, String> {
    use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
    use std::fmt;

    struct NoDupValue;

    impl<'de> Visitor<'de> for NoDupValue {
        type Value = Value;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("any JSON value with unique object keys")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
            Ok(Value::Bool(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
            Ok(Value::from(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
            Ok(Value::from(value))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
            Ok(serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number))
        }

        fn visit_str<E>(self, value: &str) -> Result<Value, E> {
            Ok(Value::String(value.to_string()))
        }

        fn visit_string<E>(self, value: String) -> Result<Value, E> {
            Ok(Value::String(value))
        }

        fn visit_none<E>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }

        fn visit_unit<E>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_any(NoDupValue)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(item) = seq.next_element_seed(NoDupSeed)? {
                items.push(item);
            }
            Ok(Value::Array(items))
        }

        fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut object = serde_json::Map::with_capacity(map.size_hint().unwrap_or(0));
            while let Some(key) = map.next_key::<String>()? {
                if object.contains_key(&key) {
                    return Err(de::Error::custom(format!(
                        "duplicate key `{key}` in JSON object"
                    )));
                }
                let value = map.next_value_seed(NoDupSeed)?;
                object.insert(key, value);
            }
            Ok(Value::Object(object))
        }
    }

    struct NoDupSeed;

    impl<'de> DeserializeSeed<'de> for NoDupSeed {
        type Value = Value;

        fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_any(NoDupValue)
        }
    }

    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = NoDupSeed
        .deserialize(&mut deserializer)
        .map_err(|err| err.to_string())?;
    deserializer.end().map_err(|err| err.to_string())?;
    Ok(value)
}

pub(crate) fn validate_evidence_promotion_honesty_corpus_at(
    corpus_path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !corpus_path.exists() {
        violations.push(format!(
            "evidence promotion honesty corpus is missing {}",
            normalize_path(corpus_path)
        ));
        return Ok(());
    }

    let corpus_text = read_text_lossy(corpus_path)?;
    let corpus_value: Value = parse_json_rejecting_duplicate_keys(&corpus_text)
        .map_err(|err| format!("failed to parse {}: {err}", normalize_path(corpus_path)))?;

    let Some(cases) = corpus_value.get("cases").and_then(Value::as_array) else {
        violations.push(format!(
            "{} has no `cases` array",
            normalize_path(corpus_path)
        ));
        return Ok(());
    };

    // Parity: track languages with non-promoted cases and control cases
    let mut non_promoted_languages: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let mut control_languages: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let mut seen_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for case in cases {
        let id = case
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("<missing-id>");

        // Duplicate id check
        if !seen_ids.insert(id.to_string()) {
            violations.push(format!(
                "evidence promotion honesty corpus has duplicate case id `{id}`"
            ));
        }

        let language = case
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let source_fixture = case
            .get("source_fixture")
            .and_then(Value::as_str)
            .unwrap_or("");
        let source_report = case
            .get("source_report")
            .and_then(Value::as_str)
            .unwrap_or("");
        let tier = case.get("tier").and_then(Value::as_str).unwrap_or("");
        let assertions = match evidence_promotion_case_assertions(case) {
            Ok(assertions) => assertions,
            Err(err) => {
                violations.push(err);
                Vec::new()
            }
        };
        let must_not_promote = assertions.iter().any(|assertion| {
            matches!(
                assertion,
                EvidencePromotionSemanticAssertion::MustNotPromote
            )
        });
        let must_not_report_clean = assertions.iter().any(|assertion| {
            matches!(
                assertion,
                EvidencePromotionSemanticAssertion::MustNotReportClean
            )
        });
        let pins_zero_findings = assertions.iter().any(|assertion| {
            matches!(
                assertion,
                EvidencePromotionSemanticAssertion::ExpectedFindingCount { count: 0 }
            )
        });
        let pins_non_vacuous_outcome = must_not_report_clean || pins_zero_findings;
        if case.get("assertions").is_some() && must_not_promote && !pins_non_vacuous_outcome {
            violations.push(format!(
                "evidence promotion honesty case `{id}`: `must_not_promote` requires \
                 `must_not_report_clean` or `expected_finding_count=0`, so an empty \
                 findings re-bless cannot pass vacuously"
            ));
        }

        match tier {
            "pure" => {
                for external_field in [
                    "external_repo",
                    "external_command",
                    "external_commit",
                    "external_patch",
                    "runtime_budget_seconds",
                    "artifact_budget_bytes",
                ] {
                    if case.get(external_field).is_some() {
                        violations.push(format!(
                            "evidence promotion honesty case `{id}`: tier `pure` must not \
                             carry pinned-external metadata field `{external_field}`"
                        ));
                    }
                }
            }
            "pinned_external" => {
                let mut missing_or_invalid = Vec::new();
                if evidence_promotion_non_empty_string_field(case, "external_repo").is_none() {
                    missing_or_invalid.push("external_repo");
                }
                if evidence_promotion_non_empty_string_field(case, "external_command").is_none() {
                    missing_or_invalid.push("external_command");
                }
                match evidence_promotion_non_empty_string_field(case, "external_commit") {
                    Some(commit) if is_exact_git_commit(commit) => {}
                    _ => missing_or_invalid.push("external_commit"),
                }
                match evidence_promotion_non_empty_string_field(case, "external_patch") {
                    Some(path) if Path::new(path).exists() => {}
                    _ => missing_or_invalid.push("external_patch"),
                }
                if !evidence_promotion_positive_u64_field(case, "runtime_budget_seconds") {
                    missing_or_invalid.push("runtime_budget_seconds");
                }
                if !evidence_promotion_positive_u64_field(case, "artifact_budget_bytes") {
                    missing_or_invalid.push("artifact_budget_bytes");
                }
                if !missing_or_invalid.is_empty() {
                    violations.push(format!(
                        "evidence promotion honesty case `{id}`: tier `pinned_external` \
                         requires exact external metadata fields external_repo, \
                         external_command, external_commit (40-hex git commit), existing external_patch, \
                         runtime_budget_seconds, and artifact_budget_bytes; missing or invalid: {}",
                        missing_or_invalid.join(", ")
                    ));
                }
                continue;
            }
            "" => violations.push(format!(
                "evidence promotion honesty case `{id}`: `tier` is required and must be \
                 one of `pure` or `pinned_external`"
            )),
            other => violations.push(format!(
                "evidence promotion honesty case `{id}`: unknown tier `{other}`; expected \
                 `pure` or `pinned_external`"
            )),
        }

        if source_fixture.is_empty() == source_report.is_empty() {
            violations.push(format!(
                "evidence promotion honesty case `{id}`: pure cases require exactly one of `source_fixture` or `source_report`"
            ));
            continue;
        }

        let (source_artifact, check_json_path) = if source_fixture.is_empty() {
            let report_path = PathBuf::from(source_report);
            if !report_path.exists() {
                violations.push(format!(
                    "evidence promotion honesty case `{id}`: source_report `{source_report}` path does not exist"
                ));
                continue;
            }
            (source_report, report_path)
        } else {
            // Parity: source fixture must exist.
            let fixture_dir = Path::new(source_fixture);
            if !fixture_dir.exists() {
                violations.push(format!(
                    "evidence promotion honesty case `{id}`: source_fixture `{source_fixture}` does not exist"
                ));
                continue;
            }

            // Parity: source fixture must have expected/check.json.
            let check_json_path = fixture_dir.join("expected/check.json");
            if !check_json_path.exists() {
                violations.push(format!(
                    "evidence promotion honesty case `{id}`: `{}` is missing expected/check.json",
                    normalize_path(fixture_dir)
                ));
                continue;
            }

            // Parity: source fixture must NOT be in the manifest-only denylist
            // (it must stay covered by `goldens check`).
            if is_manifest_only_fixture_dir(fixture_dir) {
                violations.push(format!(
                    "evidence promotion honesty case `{id}`: source_fixture `{source_fixture}` is a manifest-only fixture dir; only regular fixtures with golden check.json may be charter members"
                ));
                continue;
            }

            (source_fixture, check_json_path)
        };

        // Read the golden check.json (byte-pinned source of truth)
        let check_json_text = read_text_lossy(&check_json_path)?;
        let check_json: Value = serde_json::from_str(&check_json_text).map_err(|err| {
            format!(
                "failed to parse {}: {err}",
                normalize_path(&check_json_path)
            )
        })?;
        let source_human_text = if source_fixture.is_empty() {
            None
        } else {
            let human_path =
                Path::new(source_fixture).join(EVIDENCE_PROMOTION_HUMAN_PROJECTION_PATH);
            human_path
                .exists()
                .then(|| read_text_lossy(&human_path))
                .transpose()?
        };
        if case.get("assertions").is_some() {
            if assertions.iter().any(|assertion| {
                matches!(
                    assertion,
                    EvidencePromotionSemanticAssertion::MustNotPromote
                )
            }) {
                non_promoted_languages.insert(language.to_string());
            }
            if assertions.iter().any(|assertion| {
                matches!(assertion, EvidencePromotionSemanticAssertion::MustPromote)
            }) {
                control_languages.insert(language.to_string());
            }
            violations.extend(evidence_promotion_semantic_violations(
                id,
                Some(source_artifact),
                &assertions,
                &check_json,
                source_human_text.as_deref(),
                !source_fixture.is_empty(),
            ));
            continue;
        }
        let findings = check_json
            .get("findings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let must_not_report_clean = case
            .get("must_not_report_clean")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if must_not_report_clean {
            let summary_findings = check_json
                .get("summary")
                .and_then(|summary| summary.get("findings"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if summary_findings == 0 || findings.is_empty() {
                violations.push(format!(
                    "evidence promotion honesty case `{id}` (fixture `{source_fixture}`): \
                     `must_not_report_clean` requires at least one reported finding, but \
                     summary.findings={summary_findings} and findings.len()={} -- a re-bless \
                     made a known gap read clean (false-clean regression)",
                    findings.len()
                ));
            }
        }

        // Semantic assertion (RIPR-SPEC-0108): a charter case may require the
        // report-level scope header to remain visible. This is a first-run
        // honesty guard: a known limitation case must not be re-blessed into an
        // artifact that still has findings but no machine-readable statement of
        // which tool/mode/root/base produced them.
        let must_disclose_scope = case
            .get("must_disclose_scope")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if must_disclose_scope {
            let missing_scope_fields = ["schema_version", "tool", "mode", "root", "base"]
                .iter()
                .filter_map(|field| {
                    let present = check_json
                        .get(*field)
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty());
                    (!present).then_some(*field)
                })
                .collect::<Vec<_>>();
            if !missing_scope_fields.is_empty() {
                violations.push(format!(
                    "evidence promotion honesty case `{id}` (fixture `{source_fixture}`): \
                     `must_disclose_scope` requires report-level scope fields \
                     schema_version/tool/mode/root/base, but missing or empty field(s): {} -- \
                     a re-bless kept evidence without preserving the analyzed-scope header \
                     (first-run trust regression)",
                    missing_scope_fields.join(", ")
                ));
            }
        }

        let must_remain_non_promoted = case
            .get("must_remain_non_promoted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let expected_promoted = case
            .get("expected_promoted")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        // Semantic assertion (RIPR-SPEC-0108 + 0114/0115): a charter case may
        // require that a specific named limitation is still emitted. This guards
        // against a dishonest re-bless that silently drops `static_limit_kind`
        // back to a bare `no_static_path` — the exact fail-closed regression the
        // transitive-reach limitation was added to prevent. Independent of the
        // promotion checks below, so a case can assert both at once.
        let must_emit_limitation = case
            .get("must_emit_limitation")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if must_emit_limitation {
            let expected_limit_kind = case
                .get("expected_limit_kind")
                .and_then(Value::as_str)
                .unwrap_or("");
            if expected_limit_kind.is_empty() {
                violations.push(format!(
                    "evidence promotion honesty case `{id}`: `must_emit_limitation` is true \
                     but `expected_limit_kind` is missing or empty"
                ));
            } else {
                let has_limit = findings.iter().any(|f| {
                    f.get("static_limit_kind").and_then(Value::as_str) == Some(expected_limit_kind)
                });
                if !has_limit {
                    violations.push(format!(
                        "evidence promotion honesty case `{id}` (fixture `{source_fixture}`): \
                         `must_emit_limitation` requires a finding with static_limit_kind \
                         `{expected_limit_kind}` but none is present — a re-bless silently dropped \
                         the named limitation (fail-closed regression)"
                    ));
                }
            }
        }

        // Semantic assertion (RIPR-SPEC-0108): named limitation cases may
        // require every projection surface to stay non-packet-ready. This is
        // independent from classification: a `no_static_path` result that
        // quietly grows `repair_packet_ready=true` is still a false delegation
        // authority regression.
        let must_not_emit_repair_packet = case
            .get("must_not_emit_repair_packet")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if must_not_emit_repair_packet {
            let packet_ready_paths =
                json_bool_field_paths(&check_json, "repair_packet_ready", true);
            if !packet_ready_paths.is_empty() {
                violations.push(format!(
                    "evidence promotion honesty case `{id}` (fixture `{source_fixture}`): \
                     `must_not_emit_repair_packet` forbids `repair_packet_ready=true`, \
                     but found it at {} -- a named limitation became delegatable without \
                     the required repair-packet contract",
                    packet_ready_paths.join(", ")
                ));
            }
        }

        // Semantic assertion (RIPR-SPEC-0108 + 0115): a charter case may require
        // that a finding's `evidence` discloses the concrete transitive-reach
        // WITNESS — the "Where to look" pointer naming the witnessing test and
        // entry symbol. This guards against a re-bless that drops the witness
        // line back to the bare 0114 limitation message, regressing the
        // first-run-trust UX RIPR-SPEC-0115 added. The recognized prose begins
        // with the prefix shared in `crate::domain::TRANSITIVE_REACH_WITNESS_PREFIX`
        // (ripr crate); kept here as a literal because xtask cannot import that
        // pub(crate) const — the assertion itself fails loudly if the prose drifts.
        let must_disclose_witness = case
            .get("must_disclose_witness")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if must_disclose_witness {
            const WITNESS_PREFIX: &str = "For example, the test ";
            let discloses = findings.iter().any(|f| {
                f.get("evidence")
                    .and_then(Value::as_array)
                    .is_some_and(|ev| {
                        ev.iter()
                            .filter_map(Value::as_str)
                            .any(|line| line.starts_with(WITNESS_PREFIX))
                    })
            });
            if !discloses {
                violations.push(format!(
                    "evidence promotion honesty case `{id}` (fixture `{source_fixture}`): \
                     `must_disclose_witness` requires a finding whose evidence names the \
                     transitive-reach witness (prefix `{WITNESS_PREFIX}`) but none does — a \
                     re-bless dropped the RIPR-SPEC-0115 'Where to look' pointer (fail-closed regression)"
                ));
            }
        }

        if must_remain_non_promoted {
            non_promoted_languages.insert(language.to_string());

            let expected_max_class = case
                .get("expected_max_class")
                .and_then(Value::as_str)
                .unwrap_or("weakly_exposed");
            let max_severity = evidence_class_severity(expected_max_class);

            for finding in &findings {
                let class = finding
                    .get("classification")
                    .and_then(Value::as_str)
                    .unwrap_or("static_unknown");
                let finding_id = finding
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("<no-id>");

                // Hard invariant: no finding may be promoted to `exposed`
                if class == "exposed" {
                    violations.push(format!(
                        "evidence promotion honesty case `{id}` (fixture `{source_fixture}`): \
                         finding `{finding_id}` has classification `exposed` but \
                         `must_remain_non_promoted` is true — dishonest re-bless detected; \
                         revert the golden or remove this charter member"
                    ));
                }

                // Soft invariant: respect expected_max_class
                if evidence_class_severity(class) > max_severity {
                    violations.push(format!(
                        "evidence promotion honesty case `{id}` (fixture `{source_fixture}`): \
                         finding `{finding_id}` has classification `{class}` which exceeds \
                         expected_max_class `{expected_max_class}`"
                    ));
                }
            }
        } else if expected_promoted {
            control_languages.insert(language.to_string());

            // Control: at least one finding must be `exposed`
            let has_exposed = findings
                .iter()
                .any(|f| f.get("classification").and_then(Value::as_str) == Some("exposed"));
            if !has_exposed {
                violations.push(format!(
                    "evidence promotion honesty control case `{id}` (fixture `{source_fixture}`): \
                     `expected_promoted` is true but no finding has classification `exposed` — \
                     the gate has over-corrected or the fixture needs re-blessing"
                ));
            }
        }
    }

    // Parity: each of {python, typescript, rust} must have >= 1 non-promoted case
    for lang in ["python", "typescript", "rust"] {
        if !non_promoted_languages.contains(lang) {
            violations.push(format!(
                "evidence promotion honesty corpus must include at least one \
                 `must_not_promote` assertion for language `{lang}`"
            ));
        }
    }

    // Parity: each of {python, typescript, rust} must have >= 1 control case
    // Note: python may not have a positive control yet, so we only require rust+typescript.
    // The requirement from the spec is rust and typescript.
    for lang in ["rust", "typescript"] {
        if !control_languages.contains(lang) {
            violations.push(format!(
                "evidence promotion honesty corpus must include at least one \
                 `must_promote` control assertion for language `{lang}`"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod harness_limitation_assertion_tests {
    use super::*;
    use serde_json::json;

    fn check_with_harness_limitation(code: &str) -> Value {
        json!({
            "findings": [],
            "test_harnesses": [
                {
                    "registration_id": "mimic-suite",
                    "limitations": [
                        {"code": code, "file": "tests/mimic.rs", "line": 5}
                    ]
                }
            ]
        })
    }

    #[test]
    fn must_emit_limitation_accepts_harness_projection_kind() {
        let check_json = check_with_harness_limitation("registration_unreachable");
        assert!(evidence_promotion_emits_any_harness_limitation_kind(
            &check_json,
            "registration_unreachable"
        ));
    }

    #[test]
    fn must_emit_limitation_rejects_absent_harness_kind() {
        let check_json = check_with_harness_limitation("registration_unreachable");
        assert!(!evidence_promotion_emits_any_harness_limitation_kind(
            &check_json,
            "unanchored_trial_path"
        ));
    }

    #[test]
    fn harness_projection_kinds_are_kind_specific() {
        let check_json = check_with_harness_limitation("static_unknown");
        assert!(evidence_promotion_emits_any_harness_limitation_kind(
            &check_json,
            "static_unknown"
        ));
        assert!(!evidence_promotion_emits_any_harness_limitation_kind(
            &check_json,
            "registration_unreachable"
        ));
    }

    #[test]
    fn harness_projection_without_limitations_emits_nothing() {
        let check_json = json!({
            "test_harnesses": [
                {"registration_id": "mimic-suite", "limitations": []}
            ]
        });
        assert!(!evidence_promotion_emits_any_harness_limitation(
            &check_json
        ));
        assert!(!evidence_promotion_emits_any_harness_limitation_kind(
            &check_json,
            "registration_unreachable"
        ));
    }

    #[test]
    fn missing_harness_projection_emits_nothing() {
        let check_json = json!({"findings": []});
        assert!(!evidence_promotion_emits_any_harness_limitation(
            &check_json
        ));
    }
}

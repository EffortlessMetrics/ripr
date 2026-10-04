//! Classified-seam cache body with each distinct related test written once.
//!
//! Full evidence repeats the same related-test records across many seams: on
//! regex, 10,000 seams carry 438,134 related-test entries but only 1,155
//! distinct records, and those repeats were 94% of a 429 MB cache file. The
//! body now stores a `related_tests` table of distinct records and gives each
//! seam the table indices of its tests, in its original order.
//!
//! Records are deduplicated by their own serialized JSON, so a decoded seam's
//! related tests serialize exactly as the stored ones did. Field lists are
//! destructured exhaustively: a new `ClassifiedSeam` or `TestGripEvidence`
//! field fails to compile here until the cache body carries it.

use crate::analysis::new_test_target::NewTestTargetAdmission;
use crate::analysis::seam_classification::ClassifiedSeam;
use crate::analysis::seams::{RepoSeam, SeamGripClass, SeamId};
use crate::analysis::test_grip_evidence::{RelatedTestGrip, TestGripEvidence};
use crate::domain::{MissingDiscriminatorFact, StageEvidence, ValueFact};
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

#[derive(Serialize)]
struct TableRef<'a> {
    related_tests: Vec<&'a RelatedTestGrip>,
    seams: Vec<SeamRef<'a>>,
}

#[derive(Serialize)]
struct SeamRef<'a> {
    seam: &'a RepoSeam,
    evidence: EvidenceRef<'a>,
    class: &'a SeamGripClass,
}

#[derive(Serialize)]
struct EvidenceRef<'a> {
    seam_id: &'a SeamId,
    related_tests: Vec<u32>,
    reach: &'a StageEvidence,
    activate: &'a StageEvidence,
    propagate: &'a StageEvidence,
    observe: &'a StageEvidence,
    discriminate: &'a StageEvidence,
    observed_values: &'a [ValueFact],
    missing_discriminators: &'a [MissingDiscriminatorFact],
    #[serde(skip_serializing_if = "Option::is_none")]
    new_test_target: Option<&'a NewTestTargetAdmission>,
}

#[derive(Deserialize)]
struct TableOwned {
    related_tests: Vec<RelatedTestGrip>,
    seams: Vec<SeamOwned>,
}

#[derive(Deserialize)]
struct SeamOwned {
    seam: RepoSeam,
    evidence: EvidenceOwned,
    class: SeamGripClass,
}

#[derive(Deserialize)]
struct EvidenceOwned {
    seam_id: SeamId,
    related_tests: Vec<u32>,
    reach: StageEvidence,
    activate: StageEvidence,
    propagate: StageEvidence,
    observe: StageEvidence,
    discriminate: StageEvidence,
    observed_values: Vec<ValueFact>,
    missing_discriminators: Vec<MissingDiscriminatorFact>,
    #[serde(default)]
    new_test_target: Option<NewTestTargetAdmission>,
}

pub(super) fn serialize<S: Serializer>(
    seams: &[ClassifiedSeam],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut related_tests: Vec<&RelatedTestGrip> = Vec::new();
    let mut index_of: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut seam_refs = Vec::with_capacity(seams.len());
    for classified in seams {
        let ClassifiedSeam {
            seam,
            evidence,
            class,
        } = classified;
        let TestGripEvidence {
            seam_id,
            related_tests: seam_tests,
            reach,
            activate,
            propagate,
            observe,
            discriminate,
            observed_values,
            missing_discriminators,
            new_test_target,
        } = evidence;
        let mut indices = Vec::with_capacity(seam_tests.len());
        for test in seam_tests {
            let key = serde_json::to_vec(test).map_err(S::Error::custom)?;
            let next = u32::try_from(related_tests.len()).map_err(S::Error::custom)?;
            let index = *index_of.entry(key).or_insert_with(|| {
                related_tests.push(test);
                next
            });
            indices.push(index);
        }
        seam_refs.push(SeamRef {
            seam,
            evidence: EvidenceRef {
                seam_id,
                related_tests: indices,
                reach,
                activate,
                propagate,
                observe,
                discriminate,
                observed_values,
                missing_discriminators,
                new_test_target: new_test_target.as_ref(),
            },
            class,
        });
    }
    TableRef {
        related_tests,
        seams: seam_refs,
    }
    .serialize(serializer)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<ClassifiedSeam>, D::Error> {
    let TableOwned {
        related_tests: table,
        seams,
    } = TableOwned::deserialize(deserializer)?;
    let mut classified = Vec::with_capacity(seams.len());
    for SeamOwned {
        seam,
        evidence,
        class,
    } in seams
    {
        let EvidenceOwned {
            seam_id,
            related_tests: indices,
            reach,
            activate,
            propagate,
            observe,
            discriminate,
            observed_values,
            missing_discriminators,
            new_test_target,
        } = evidence;
        let mut related_tests = Vec::with_capacity(indices.len());
        for index in indices {
            let test = usize::try_from(index)
                .ok()
                .and_then(|index| table.get(index))
                .ok_or_else(|| {
                    D::Error::custom(format!(
                        "related test index {index} outside a table of {}",
                        table.len()
                    ))
                })?;
            related_tests.push(test.clone());
        }
        classified.push(ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests,
                reach,
                activate,
                propagate,
                observe,
                discriminate,
                observed_values,
                missing_discriminators,
                new_test_target,
            },
            class,
        });
    }
    Ok(classified)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{TestKind, TestTargetEvidence};
    use crate::domain::{
        Confidence, OracleKind, OracleStrength, RelationConfidence, RelationReason, StageState,
        SymbolId,
    };
    use std::path::PathBuf;

    #[derive(Serialize, Deserialize)]
    struct Body {
        #[serde(with = "super")]
        seams: Vec<ClassifiedSeam>,
    }

    pub(in crate::analysis::seam_cache) fn related(
        name: &str,
        reason: RelationReason,
    ) -> RelatedTestGrip {
        RelatedTestGrip {
            test_name: name.to_owned(),
            file: PathBuf::from("tests/it.rs"),
            line: 7,
            test_target: Some(TestTargetEvidence::from_index(
                SymbolId(format!("tests/it.rs::{name}")),
                PathBuf::from("tests/it.rs"),
                7,
                TestKind::Integration,
                reason,
                "sha256:workspace".to_owned(),
            )),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            evidence_summary: "exact value assertion".to_owned(),
            relation_reason: reason,
            relation_confidence: RelationConfidence::Medium,
        }
    }

    pub(in crate::analysis::seam_cache) fn seam(
        line: usize,
        related_tests: Vec<RelatedTestGrip>,
    ) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            PathBuf::from("src/foo.rs"),
            "src/foo.rs::foo",
            SeamKind::PredicateBoundary,
            line * 10,
            line,
            "x > 5".to_owned(),
            RequiredDiscriminator::BoundaryValue {
                description: "x > 5".to_owned(),
            },
            ExpectedSink::ReturnValue,
        );
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests,
            reach: StageEvidence::new(StageState::Yes, Confidence::High, "reach"),
            activate: StageEvidence::new(StageState::Unknown, Confidence::Medium, "activate"),
            propagate: StageEvidence::new(StageState::Unknown, Confidence::Medium, "propagate"),
            observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observe"),
            discriminate: StageEvidence::new(StageState::No, Confidence::Low, "discriminate"),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
            new_test_target: None,
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::Ungripped,
        }
    }

    #[test]
    fn table_round_trip_keeps_every_seam_and_shares_repeated_tests() -> Result<(), String> {
        let a = related("a", RelationReason::SameModule);
        let b = related("b", RelationReason::SameModule);
        // Same test under another relation is a distinct record, not a repeat.
        let a_direct = related("a", RelationReason::DirectOwnerCall);
        let seams = vec![
            seam(1, vec![a.clone(), b.clone()]),
            seam(2, vec![b.clone(), a.clone(), a_direct]),
            seam(3, Vec::new()),
            seam(4, vec![a]),
        ];
        let encoded = serde_json::to_value(Body {
            seams: seams.clone(),
        })
        .map_err(|err| err.to_string())?;
        let table = encoded["seams"]["related_tests"]
            .as_array()
            .ok_or("missing related test table")?;
        assert_eq!(table.len(), 3);
        assert_eq!(
            encoded["seams"]["seams"][1]["evidence"]["related_tests"],
            serde_json::json!([1, 0, 2])
        );
        let decoded: Body = serde_json::from_value(encoded).map_err(|err| err.to_string())?;
        assert_eq!(
            serde_json::to_value(&decoded.seams).map_err(|err| err.to_string())?,
            serde_json::to_value(&seams).map_err(|err| err.to_string())?
        );
        Ok(())
    }

    #[test]
    fn index_outside_the_table_is_a_decode_error() -> Result<(), String> {
        let mut encoded = serde_json::to_value(Body {
            seams: vec![seam(1, vec![related("a", RelationReason::SameModule)])],
        })
        .map_err(|err| err.to_string())?;
        encoded["seams"]["seams"][0]["evidence"]["related_tests"] = serde_json::json!([1]);
        let Err(err) = serde_json::from_value::<Body>(encoded) else {
            return Err("an out-of-range index must not decode".to_owned());
        };
        assert!(
            err.to_string()
                .contains("related test index 1 outside a table of 1")
        );
        Ok(())
    }
}

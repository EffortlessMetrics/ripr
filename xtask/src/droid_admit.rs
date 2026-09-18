//! Trusted Droid admission classifier for #1654 PR 1.
//!
//! The privileged Droid jobs run from default-branch workflow bytes. This
//! classifier turns the candidate-tied event payload into a typed admission:
//! same-repository PR subjects with a fresh, retarget-free head are admitted
//! as inert data; everything else is rejected with a receipt. Malformed
//! input is an error (fail closed), never a silent skip.

use serde_json::Value;
use std::collections::BTreeMap;

const ADMITTED_ACTIONS: &[&str] = &[
    "opened",
    "synchronize",
    "ready_for_review",
    "reopened",
    "labeled",
    "unlabeled",
];

const ADMITTED_ASSOCIATIONS: &[&str] = &["OWNER", "MEMBER", "COLLABORATOR"];

const MAX_TITLE_CHARS: usize = 8192;
const MAX_BODY_CHARS: usize = 65536;

/// Fresh subject facts re-fetched through the API by base-owned workflow
/// steps. The classifier compares the event payload against these so a
/// stale or retargeted subject cannot ride an older event.
pub(crate) struct FreshFacts {
    pub(crate) head_sha: String,
    pub(crate) base_sha: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    Admitted(AdmissionSubject),
    Rejected(AdmissionSubject, String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AdmissionSubject {
    pub(crate) repository: String,
    pub(crate) pr_number: u64,
    pub(crate) head_sha: String,
    pub(crate) base_sha: String,
    pub(crate) action: String,
    pub(crate) association: String,
}

/// Pure admission decision over one event payload. Every rejection carries
/// the bound subject so the receipt names exactly what was refused.
pub(crate) fn admit_event(
    event_name: &str,
    event: &Value,
    expected_repo: &str,
    fresh: &FreshFacts,
) -> Result<Admission, String> {
    if event_name != "pull_request_target" {
        return Err(format!(
            "unsupported event {event_name:?}; this classifier admits pull_request_target subjects only"
        ));
    }
    let repository = event
        .get("repository")
        .and_then(|repo| repo.get("full_name"))
        .and_then(Value::as_str)
        .ok_or_else(|| "event carries no repository.full_name".to_string())?;
    if repository != expected_repo {
        return Err(format!(
            "event repository {repository:?} does not match expected {expected_repo:?}"
        ));
    }
    let action = event
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| "event carries no action".to_string())?;
    if !ADMITTED_ACTIONS.contains(&action) {
        return Ok(reject(
            repository,
            0,
            "",
            "",
            action,
            "",
            "unsupported-action",
        ));
    }
    let pull = event
        .get("pull_request")
        .ok_or_else(|| "event carries no pull_request subject".to_string())?;
    let pr_number = pull
        .get("number")
        .and_then(Value::as_u64)
        .filter(|number| *number > 0)
        .ok_or_else(|| "event pull_request carries no usable number".to_string())?;
    let head_repo = pull
        .pointer("/head/repo/full_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "event pull_request carries no head repository".to_string())?;
    let base_repo = pull
        .pointer("/base/repo/full_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "event pull_request carries no base repository".to_string())?;
    if head_repo != expected_repo || base_repo != expected_repo {
        return Ok(reject(
            repository,
            pr_number,
            "",
            "",
            action,
            "",
            "fork-or-cross-repo",
        ));
    }
    let head_sha = pull
        .pointer("/head/sha")
        .and_then(Value::as_str)
        .ok_or_else(|| "event pull_request carries no head sha".to_string())?;
    let base_sha = pull
        .pointer("/base/sha")
        .and_then(Value::as_str)
        .ok_or_else(|| "event pull_request carries no base sha".to_string())?;
    check_sha(head_sha, "head")?;
    check_sha(base_sha, "base")?;
    check_sha(&fresh.head_sha, "fresh head")?;
    check_sha(&fresh.base_sha, "fresh base")?;
    if head_sha != fresh.head_sha {
        return Ok(reject(
            repository,
            pr_number,
            head_sha,
            base_sha,
            action,
            "",
            "stale-head",
        ));
    }
    if base_sha != fresh.base_sha {
        return Ok(reject(
            repository,
            pr_number,
            head_sha,
            base_sha,
            action,
            "",
            "retargeted-base",
        ));
    }
    let association = pull
        .get("author_association")
        .and_then(Value::as_str)
        .ok_or_else(|| "event pull_request carries no author association".to_string())?;
    if !ADMITTED_ASSOCIATIONS.contains(&association) {
        return Ok(reject(
            repository,
            pr_number,
            head_sha,
            base_sha,
            action,
            association,
            "untrusted-actor",
        ));
    }
    let title = pull
        .get("title")
        .and_then(Value::as_str)
        .ok_or_else(|| "event pull_request carries no title".to_string())?;
    if title.chars().count() > MAX_TITLE_CHARS {
        return Ok(reject(
            repository,
            pr_number,
            head_sha,
            base_sha,
            action,
            association,
            "oversized-subject",
        ));
    }
    if let Some(body) = pull.get("body").and_then(Value::as_str)
        && body.chars().count() > MAX_BODY_CHARS
    {
        return Ok(reject(
            repository,
            pr_number,
            head_sha,
            base_sha,
            action,
            association,
            "oversized-subject",
        ));
    }
    if title.contains("[skip-review]") || has_label(pull, "release-check") {
        return Ok(reject(
            repository,
            pr_number,
            head_sha,
            base_sha,
            action,
            association,
            "review-suppressed",
        ));
    }
    Ok(Admission::Admitted(AdmissionSubject {
        repository: repository.to_string(),
        pr_number,
        head_sha: head_sha.to_string(),
        base_sha: base_sha.to_string(),
        action: action.to_string(),
        association: association.to_string(),
    }))
}

fn reject(
    repository: &str,
    pr_number: u64,
    head_sha: &str,
    base_sha: &str,
    action: &str,
    association: &str,
    reason: &str,
) -> Admission {
    Admission::Rejected(
        AdmissionSubject {
            repository: repository.to_string(),
            pr_number,
            head_sha: head_sha.to_string(),
            base_sha: base_sha.to_string(),
            action: action.to_string(),
            association: association.to_string(),
        },
        reason.to_string(),
    )
}

fn check_sha(value: &str, role: &str) -> Result<(), String> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(());
    }
    Err(format!("{role} sha is not a 40-character hex digest"))
}

fn has_label(pull: &Value, name: &str) -> bool {
    pull.get("labels")
        .and_then(Value::as_array)
        .is_some_and(|labels| {
            labels
                .iter()
                .any(|label| label.get("name").and_then(Value::as_str) == Some(name))
        })
}

/// CLI shell: parse argv, read the event file, write the receipt, and
/// report `admitted=<bool>`. Rejections exit zero with a receipt so the run
/// records the refusal; malformed input exits non-zero (fail closed).
pub(crate) fn droid_admit(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let text = std::fs::read_to_string(&parsed.event_path)
        .map_err(|error| format!("cannot read event file {}: {error}", parsed.event_path))?;
    if text.len() > 4 * 1_048_576 {
        return Err("event file exceeds the 4 MiB bound".to_string());
    }
    let event: Value = serde_json::from_str(&text)
        .map_err(|error| format!("event file is not valid JSON: {error}"))?;
    let admission = admit_event(
        &parsed.event_name,
        &event,
        &parsed.repo,
        &FreshFacts {
            head_sha: parsed.fresh_head,
            base_sha: parsed.fresh_base,
        },
    )?;
    let (decision, subject) = match &admission {
        Admission::Admitted(subject) => ("admitted", subject),
        Admission::Rejected(subject, _) => ("rejected", subject),
    };
    let reason = match &admission {
        Admission::Admitted(_) => Value::Null,
        Admission::Rejected(_, reason) => Value::String(reason.clone()),
    };
    let receipt = serde_json::json!({
        "schema": "ripr.droid-admission/v1",
        "decision": decision,
        "event_name": parsed.event_name,
        "subject": {
            "repository": subject.repository,
            "pr": subject.pr_number,
            "head": subject.head_sha,
            "base": subject.base_sha,
            "action": subject.action,
            "association": subject.association,
        },
        "reason": reason,
    });
    let rendered = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("cannot render receipt: {error}"))?;
    std::fs::write(&parsed.receipt_path, format!("{rendered}\n"))
        .map_err(|error| format!("cannot write receipt {}: {error}", parsed.receipt_path))?;
    let admitted = decision == "admitted";
    println!("admitted={admitted}");
    println!("receipt={}", parsed.receipt_path);
    if let Ok(github_output) = std::env::var("GITHUB_OUTPUT")
        && !github_output.is_empty()
    {
        let append = format!("admitted={admitted}\nreceipt={}\n", parsed.receipt_path);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&github_output)
            .and_then(|mut file| {
                use std::io::Write as _;
                file.write_all(append.as_bytes())
            })
            .map_err(|error| format!("cannot append {github_output}: {error}"))?;
    }
    Ok(())
}

struct AdmitArgs {
    event_name: String,
    event_path: String,
    repo: String,
    fresh_head: String,
    fresh_base: String,
    receipt_path: String,
}

fn parse_args(args: &[String]) -> Result<AdmitArgs, String> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args.get(index).cloned().unwrap_or_default();
        let value = args.get(index + 1).cloned().unwrap_or_default();
        if !flag.starts_with("--") || value.is_empty() || value.starts_with("--") {
            return Err("expected --<flag> <value> pairs; usage: droid-admit --event-name NAME --event PATH --repo OWNER/NAME --fresh-head SHA --fresh-base SHA --receipt PATH"
                .to_string());
        }
        values.insert(flag, value);
        index += 2;
    }
    let take = |flag: &str| {
        values
            .get(flag)
            .cloned()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("missing required {flag}"))
    };
    Ok(AdmitArgs {
        event_name: take("--event-name")?,
        event_path: take("--event")?,
        repo: take("--repo")?,
        fresh_head: take("--fresh-head")?,
        fresh_base: take("--fresh-base")?,
        receipt_path: take("--receipt")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const REPO: &str = "EffortlessMetrics/ripr";
    const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn fresh() -> FreshFacts {
        FreshFacts {
            head_sha: HEAD.to_string(),
            base_sha: BASE.to_string(),
        }
    }

    fn event(action: &str) -> Value {
        json!({
            "action": action,
            "repository": {"full_name": REPO},
            "pull_request": {
                "number": 1718,
                "title": "vscode: route server extraction",
                "body": "Closes the trust seam.",
                "author_association": "OWNER",
                "head": {"sha": HEAD, "repo": {"full_name": REPO}},
                "base": {"sha": BASE, "repo": {"full_name": REPO}},
                "labels": [],
            }
        })
    }

    #[test]
    fn admits_a_fresh_same_repo_subject() {
        let admission = admit_event("pull_request_target", &event("opened"), REPO, &fresh());
        assert!(
            matches!(&admission, Ok(Admission::Admitted(_))),
            "expected admission: {admission:?}"
        );
        if let Ok(Admission::Admitted(subject)) = &admission {
            assert_eq!(subject.pr_number, 1718);
            assert_eq!(subject.head_sha, HEAD);
        }
    }

    #[test]
    fn rejects_unsupported_event_names_as_errors() {
        let admission = admit_event("pull_request", &event("opened"), REPO, &fresh());
        assert!(
            admission.is_err(),
            "pull_request event must fail closed: {admission:?}"
        );
        if let Err(error) = &admission {
            assert!(error.contains("pull_request_target"), "{error}");
        }
    }

    #[test]
    fn rejects_wrong_repository_as_error() {
        let admission = admit_event(
            "pull_request_target",
            &event("opened"),
            "Other/Repo",
            &fresh(),
        );
        assert!(
            admission.is_err(),
            "wrong repository must fail closed: {admission:?}"
        );
        if let Err(error) = &admission {
            assert!(error.contains("does not match"), "{error}");
        }
    }

    #[test]
    fn rejects_edited_actions() {
        let admission = admit_event("pull_request_target", &event("edited"), REPO, &fresh());
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "unsupported-action"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_fork_heads() {
        let mut hostile = event("synchronize");
        hostile["pull_request"]["head"]["repo"]["full_name"] = json!("attacker/fork");
        let admission = admit_event("pull_request_target", &hostile, REPO, &fresh());
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "fork-or-cross-repo"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_stale_heads() {
        let stale = FreshFacts {
            head_sha: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
            base_sha: BASE.to_string(),
        };
        let admission = admit_event("pull_request_target", &event("synchronize"), REPO, &stale);
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "stale-head"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_retargeted_bases() {
        let moved = FreshFacts {
            head_sha: HEAD.to_string(),
            base_sha: "dddddddddddddddddddddddddddddddddddddddd".to_string(),
        };
        let admission = admit_event("pull_request_target", &event("synchronize"), REPO, &moved);
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "retargeted-base"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_untrusted_actors() {
        let mut hostile = event("opened");
        hostile["pull_request"]["author_association"] = json!("NONE");
        let admission = admit_event("pull_request_target", &hostile, REPO, &fresh());
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "untrusted-actor"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_suppressed_reviews_with_a_receipt() {
        let mut suppressed = event("opened");
        suppressed["pull_request"]["title"] = json!("feat: x [skip-review]");
        let admission = admit_event("pull_request_target", &suppressed, REPO, &fresh());
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "review-suppressed"),
            "{admission:?}"
        );
        let mut labeled = event("opened");
        labeled["pull_request"]["labels"] = json!([{"name": "release-check"}]);
        let admission = admit_event("pull_request_target", &labeled, REPO, &fresh());
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "review-suppressed"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_oversized_subjects() {
        let mut hostile = event("opened");
        hostile["pull_request"]["title"] = json!("x".repeat(MAX_TITLE_CHARS + 1));
        let admission = admit_event("pull_request_target", &hostile, REPO, &fresh());
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "oversized-subject"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_malformed_shas_as_errors() {
        let mut hostile = event("opened");
        hostile["pull_request"]["head"]["sha"] = json!("not-a-sha");
        let admission = admit_event("pull_request_target", &hostile, REPO, &fresh());
        assert!(
            admission.is_err(),
            "malformed sha must fail closed: {admission:?}"
        );
        if let Err(error) = &admission {
            assert!(error.contains("head sha"), "{error}");
        }
    }

    #[test]
    fn rejects_missing_subjects_as_errors() {
        let admission = admit_event(
            "pull_request_target",
            &json!({"action": "opened"}),
            REPO,
            &fresh(),
        );
        assert!(
            admission.is_err(),
            "missing subject must fail closed: {admission:?}"
        );
        if let Err(error) = &admission {
            assert!(!error.is_empty());
        }
    }
}

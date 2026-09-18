//! Trusted Droid admission classifier for #1654.
//!
//! The privileged Droid jobs run from default-branch workflow bytes. This
//! classifier turns candidate-tied subjects into typed admissions:
//!
//! - `auto-review` (PR 1): the `pull_request_target` event payload binds a
//!   same-repository PR with a fresh, retarget-free head.
//! - `dispatch` (PR 2): the `workflow_run` consumer admits only what an API
//!   refetch proves — repository, mention, association, recency, and fresh
//!   SHAs. The dispatcher slip is a routing hint, never authority.
//!
//! Everything else is rejected with a receipt. Malformed input is an error
//! (fail closed), never a silent skip.

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
    pub(crate) mode: String,
    pub(crate) repository: String,
    pub(crate) pr_number: u64,
    pub(crate) issue_number: u64,
    pub(crate) comment_id: u64,
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
        return Ok(reject_auto(
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
        return Ok(reject_auto(
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
        return Ok(reject_auto(
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
        return Ok(reject_auto(
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
        return Ok(reject_auto(
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
        return Ok(reject_auto(
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
        return Ok(reject_auto(
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
        return Ok(reject_auto(
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
        mode: "auto-review".to_string(),
        repository: repository.to_string(),
        pr_number,
        issue_number: 0,
        comment_id: 0,
        head_sha: head_sha.to_string(),
        base_sha: base_sha.to_string(),
        action: action.to_string(),
        association: association.to_string(),
    }))
}

struct Deny<'a> {
    mode: &'a str,
    repository: &'a str,
    pr_number: u64,
    issue_number: u64,
    comment_id: u64,
    head_sha: &'a str,
    base_sha: &'a str,
    action: &'a str,
    association: &'a str,
}

fn deny(fields: Deny<'_>, reason: &str) -> Admission {
    Admission::Rejected(
        AdmissionSubject {
            mode: fields.mode.to_string(),
            repository: fields.repository.to_string(),
            pr_number: fields.pr_number,
            issue_number: fields.issue_number,
            comment_id: fields.comment_id,
            head_sha: fields.head_sha.to_string(),
            base_sha: fields.base_sha.to_string(),
            action: fields.action.to_string(),
            association: fields.association.to_string(),
        },
        reason.to_string(),
    )
}

fn reject_auto(
    repository: &str,
    pr_number: u64,
    head_sha: &str,
    base_sha: &str,
    action: &str,
    association: &str,
    reason: &str,
) -> Admission {
    deny(
        Deny {
            mode: "auto-review",
            repository,
            pr_number,
            issue_number: 0,
            comment_id: 0,
            head_sha,
            base_sha,
            action,
            association,
        },
        reason,
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

pub(crate) const DEFAULT_REPLAY_WINDOW_HOURS: u64 = 12;
const FUTURE_SKEW_SECS: u64 = 900;

/// Dispatch-mode admission over API-refetched facts. The dispatcher slip is
/// a routing hint only: repository, mention, association, recency, and SHAs
/// must all verify against this refetch, or the subject is rejected.
pub(crate) fn admit_dispatch(
    facts: &Value,
    expected_repo: &str,
    now_epoch_secs: u64,
    replay_window_hours: u64,
) -> Result<Admission, String> {
    let repository = facts
        .get("repository")
        .and_then(Value::as_str)
        .ok_or_else(|| "facts carry no repository".to_string())?;
    if repository != expected_repo {
        return Err(format!(
            "facts repository {repository:?} does not match expected {expected_repo:?}"
        ));
    }
    let mode = facts
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| "facts carry no mode".to_string())?;
    if mode != "pr-mention" && mode != "comment" && mode != "issue" {
        return Err(format!("unsupported dispatch mode {mode:?}"));
    }
    let pr_number = facts.get("pr").and_then(Value::as_u64).unwrap_or(0);
    let issue_number = facts.get("issue").and_then(Value::as_u64).unwrap_or(0);
    let comment_id = facts.get("comment_id").and_then(Value::as_u64).unwrap_or(0);
    if mode == "issue" && issue_number == 0 {
        return Err("issue mode carries no issue number".to_string());
    }
    if mode != "issue" && pr_number == 0 {
        return Err(format!("{mode} mode carries no PR number"));
    }
    if mode == "comment" && comment_id == 0 {
        return Err("comment mode carries no comment id".to_string());
    }
    let head_sha = facts.get("head").and_then(Value::as_str).unwrap_or("");
    let base_sha = facts.get("base").and_then(Value::as_str).unwrap_or("");
    let action = facts.get("action").and_then(Value::as_str).unwrap_or(mode);
    let association = facts
        .get("association")
        .and_then(Value::as_str)
        .ok_or_else(|| "facts carry no author association".to_string())?;
    let reject = |reason: &str| {
        deny(
            Deny {
                mode,
                repository,
                pr_number,
                issue_number,
                comment_id,
                head_sha,
                base_sha,
                action,
                association,
            },
            reason,
        )
    };
    if !ADMITTED_ASSOCIATIONS.contains(&association) {
        return Ok(reject("untrusted-actor"));
    }
    let created_at = facts
        .get("created_at")
        .and_then(Value::as_str)
        .ok_or_else(|| "facts carry no creation time".to_string())?;
    if let Some(reason) = check_created_at(created_at, now_epoch_secs, replay_window_hours)? {
        return Ok(reject(reason));
    }
    let title = facts.get("title").and_then(Value::as_str).unwrap_or("");
    let body = facts.get("body").and_then(Value::as_str).unwrap_or("");
    if title.chars().count() > MAX_TITLE_CHARS || body.chars().count() > MAX_BODY_CHARS {
        return Ok(reject("oversized-subject"));
    }
    let mentioned = title.contains("@droid") || body.contains("@droid");
    if !mentioned {
        return Ok(reject("missing-mention"));
    }
    if let Some(labels) = facts.get("labels").and_then(Value::as_array)
        && labels
            .iter()
            .any(|label| label.get("name").and_then(Value::as_str) == Some("release-check"))
    {
        return Ok(reject("review-suppressed"));
    }
    if mode != "issue" {
        check_sha(head_sha, "head")?;
        check_sha(base_sha, "base")?;
        // Slip hints are optional: when a dispatcher supplies them they must
        // match the refetch, proving the subject did not move in between.
        // Direct refetch mode omits them; the refetch itself is then fresh
        // and the replay window bounds its age.
        let slip_head = facts.get("slip_head").and_then(Value::as_str).unwrap_or("");
        if !slip_head.is_empty() {
            check_sha(slip_head, "slip head")?;
            if head_sha != slip_head {
                return Ok(reject("stale-head"));
            }
        }
        let slip_base = facts.get("slip_base").and_then(Value::as_str).unwrap_or("");
        if !slip_base.is_empty() {
            check_sha(slip_base, "slip base")?;
            if base_sha != slip_base {
                return Ok(reject("retargeted-base"));
            }
        }
    }
    Ok(Admission::Admitted(AdmissionSubject {
        mode: mode.to_string(),
        repository: repository.to_string(),
        pr_number,
        issue_number,
        comment_id,
        head_sha: head_sha.to_string(),
        base_sha: base_sha.to_string(),
        action: action.to_string(),
        association: association.to_string(),
    }))
}

/// Returns `Ok(None)` for a fresh subject, `Ok(Some(reason))` for an
/// out-of-window replay, or `Err` for a malformed timestamp.
fn check_created_at(
    created_at: &str,
    now_epoch_secs: u64,
    replay_window_hours: u64,
) -> Result<Option<&'static str>, String> {
    if !is_rfc3339_utc(created_at) {
        return Err(format!(
            "creation time {created_at:?} is not a UTC RFC 3339 timestamp"
        ));
    }
    let window_secs = replay_window_hours.saturating_mul(3600);
    let cutoff = epoch_to_rfc3339(now_epoch_secs.saturating_sub(window_secs));
    if created_at < cutoff.as_str() {
        return Ok(Some("stale-request"));
    }
    let future_limit = epoch_to_rfc3339(now_epoch_secs.saturating_add(FUTURE_SKEW_SECS));
    if created_at > future_limit.as_str() {
        return Ok(Some("stale-request"));
    }
    Ok(None)
}

fn is_rfc3339_utc(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[19] != b'Z' {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let digit = matches!(index, 0..=3 | 5..=6 | 8..=9 | 11..=12 | 14..=15 | 17..=18);
        let separator = matches!(
            (index, byte),
            (4, b'-') | (7, b'-') | (10, b'T') | (13, b':') | (16, b':') | (19, b'Z')
        );
        if digit != byte.is_ascii_digit() || (!digit && !separator) {
            return false;
        }
    }
    true
}

fn two_digits(value: u64) -> String {
    format!("{:02}", value.min(99))
}

/// Days-to-civil conversion over whole days; std-only so the classifier
/// keeps zero date dependencies.
pub(crate) fn epoch_to_rfc3339(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let clock = epoch_secs % 86_400;
    let shift = days + 719_468;
    let era = shift.div_euclid(146_097);
    let day_of_era = shift.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_part + 2) / 5 + 1).cast_unsigned();
    let month = (if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    })
    .cast_unsigned();
    if month <= 2 {
        year += 1;
    }
    format!(
        "{year:04}-{}-{}T{}:{}:{}Z",
        two_digits(month),
        two_digits(day),
        two_digits(clock / 3600),
        two_digits((clock % 3600) / 60),
        two_digits(clock % 60)
    )
}

/// CLI shell: parse argv, read the event or facts file, write the receipt,
/// and report `admitted=<bool>` plus `mode=<mode>`. Rejections exit zero
/// with a receipt so the run records the refusal; malformed input exits
/// non-zero (fail closed).
pub(crate) fn droid_admit(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let admission = if parsed.mode == "dispatch" {
        let facts_path = parsed
            .facts_path
            .as_ref()
            .ok_or_else(|| "dispatch mode requires --facts PATH".to_string())?;
        let text = std::fs::read_to_string(facts_path)
            .map_err(|error| format!("cannot read facts file {facts_path}: {error}"))?;
        if text.len() > 4 * 1_048_576 {
            return Err("facts file exceeds the 4 MiB bound".to_string());
        }
        let facts: Value = serde_json::from_str(&text)
            .map_err(|error| format!("facts file is not valid JSON: {error}"))?;
        admit_dispatch(
            &facts,
            &parsed.repo,
            current_epoch_secs()?,
            parsed.replay_window_hours,
        )?
    } else if parsed.mode == "auto-review" {
        let event_path = parsed
            .event_path
            .as_ref()
            .ok_or_else(|| "auto-review mode requires --event PATH".to_string())?;
        let text = std::fs::read_to_string(event_path)
            .map_err(|error| format!("cannot read event file {event_path}: {error}"))?;
        if text.len() > 4 * 1_048_576 {
            return Err("event file exceeds the 4 MiB bound".to_string());
        }
        let event: Value = serde_json::from_str(&text)
            .map_err(|error| format!("event file is not valid JSON: {error}"))?;
        admit_event(
            &parsed.event_name,
            &event,
            &parsed.repo,
            &FreshFacts {
                head_sha: parsed.fresh_head.clone(),
                base_sha: parsed.fresh_base.clone(),
            },
        )?
    } else {
        return Err(format!(
            "unsupported --mode {:?}; expected auto-review or dispatch",
            parsed.mode
        ));
    };
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
            "mode": subject.mode,
            "repository": subject.repository,
            "pr": subject.pr_number,
            "issue": subject.issue_number,
            "comment": subject.comment_id,
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
    println!("mode={}", subject.mode);
    println!("head={}", subject.head_sha);
    println!("receipt={}", parsed.receipt_path);
    if let Ok(github_output) = std::env::var("GITHUB_OUTPUT")
        && !github_output.is_empty()
    {
        let append = format!(
            "admitted={admitted}\nmode={}\nhead={}\nreceipt={}\n",
            subject.mode, subject.head_sha, parsed.receipt_path
        );
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

fn current_epoch_secs() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system clock is before the epoch: {error}"))
}

struct AdmitArgs {
    mode: String,
    event_name: String,
    event_path: Option<String>,
    facts_path: Option<String>,
    repo: String,
    fresh_head: String,
    fresh_base: String,
    replay_window_hours: u64,
    receipt_path: String,
}

fn parse_args(args: &[String]) -> Result<AdmitArgs, String> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args.get(index).cloned().unwrap_or_default();
        let value = args.get(index + 1).cloned().unwrap_or_default();
        if !flag.starts_with("--") || value.is_empty() || value.starts_with("--") {
            return Err(usage());
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
    let mode = values
        .get("--mode")
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto-review".to_string());
    let replay_window_hours = values
        .get("--replay-window-hours")
        .map(|value| {
            value.parse::<u64>().map_err(|error| {
                format!("--replay-window-hours must be a non-negative integer: {error}")
            })
        })
        .transpose()?
        .unwrap_or(DEFAULT_REPLAY_WINDOW_HOURS);
    Ok(AdmitArgs {
        event_name: take("--event-name")?,
        event_path: values.get("--event").cloned(),
        facts_path: values.get("--facts").cloned(),
        repo: take("--repo")?,
        fresh_head: values.get("--fresh-head").cloned().unwrap_or_default(),
        fresh_base: values.get("--fresh-base").cloned().unwrap_or_default(),
        mode,
        replay_window_hours,
        receipt_path: take("--receipt")?,
    })
}

fn usage() -> String {
    "expected --<flag> <value> pairs; usage: droid-admit --mode auto-review|dispatch --event-name NAME --repo OWNER/NAME --receipt PATH [--event PATH --fresh-head SHA --fresh-base SHA | --facts PATH [--replay-window-hours N]]"
        .to_string()
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

    const NOW: u64 = 1_787_000_000;

    fn dispatch_facts() -> Value {
        json!({
            "mode": "comment",
            "repository": REPO,
            "pr": 1720,
            "comment_id": 5_725_000_001u64,
            "action": "created",
            "title": "",
            "body": "please take a look @droid",
            "association": "MEMBER",
            "created_at": epoch_to_rfc3339(NOW - 3600),
            "head": HEAD,
            "base": BASE,
            "slip_head": HEAD,
            "slip_base": BASE,
            "labels": [],
        })
    }

    fn admit_dispatch_now(facts: &Value) -> Result<Admission, String> {
        admit_dispatch(facts, REPO, NOW, DEFAULT_REPLAY_WINDOW_HOURS)
    }

    #[test]
    fn pins_epoch_to_rfc3339_vectors() {
        assert_eq!(epoch_to_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(epoch_to_rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn admits_a_fresh_comment_dispatch() {
        let admission = admit_dispatch_now(&dispatch_facts());
        assert!(
            matches!(&admission, Ok(Admission::Admitted(_))),
            "expected dispatch admission: {admission:?}"
        );
        if let Ok(Admission::Admitted(subject)) = &admission {
            assert_eq!(subject.mode, "comment");
            assert_eq!(subject.comment_id, 5_725_000_001);
        }
    }

    #[test]
    fn admits_issue_mode_without_shas() {
        let mut facts = dispatch_facts();
        facts["mode"] = json!("issue");
        facts["issue"] = json!(1563);
        facts["pr"] = json!(0);
        facts["body"] = json!("@droid reconcile the rows");
        let admission = admit_dispatch_now(&facts);
        assert!(
            matches!(&admission, Ok(Admission::Admitted(_))),
            "expected issue admission: {admission:?}"
        );
    }

    #[test]
    fn rejects_forged_mention_removal() {
        let mut facts = dispatch_facts();
        facts["body"] = json!("never mind, handled manually");
        let admission = admit_dispatch_now(&facts);
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "missing-mention"),
            "{admission:?}"
        );
    }

    #[test]
    fn admits_direct_refetch_without_slip_hints() {
        let mut facts = dispatch_facts();
        facts["slip_head"] = json!("");
        facts["slip_base"] = json!("");
        let admission = admit_dispatch_now(&facts);
        assert!(
            matches!(&admission, Ok(Admission::Admitted(_))),
            "direct refetch is its own freshness proof: {admission:?}"
        );
    }

    #[test]
    fn rejects_moved_heads_since_dispatch() {
        let mut facts = dispatch_facts();
        facts["head"] = json!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let admission = admit_dispatch_now(&facts);
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "stale-head"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_ancient_replays() {
        let mut facts = dispatch_facts();
        facts["created_at"] = json!("2020-01-01T00:00:00Z");
        let admission = admit_dispatch_now(&facts);
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "stale-request"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_untrusted_comment_actors() {
        let mut facts = dispatch_facts();
        facts["association"] = json!("NONE");
        let admission = admit_dispatch_now(&facts);
        assert!(
            matches!(&admission, Ok(Admission::Rejected(_, reason)) if reason == "untrusted-actor"),
            "{admission:?}"
        );
    }

    #[test]
    fn rejects_malformed_timestamps_as_errors() {
        let mut facts = dispatch_facts();
        facts["created_at"] = json!("18 Sep 2026");
        let admission = admit_dispatch_now(&facts);
        assert!(
            admission.is_err(),
            "malformed timestamp must fail closed: {admission:?}"
        );
    }

    #[test]
    fn rejects_unknown_modes_as_errors() {
        let mut facts = dispatch_facts();
        facts["mode"] = json!("schedule");
        let admission = admit_dispatch_now(&facts);
        assert!(
            admission.is_err(),
            "unknown mode must fail closed: {admission:?}"
        );
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

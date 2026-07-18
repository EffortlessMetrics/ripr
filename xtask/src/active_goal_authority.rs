use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const INVENTORY: &str = "fixtures/active-goal-authority-audit/consumers.toml";
const ISSUE_CONTRACTS: &str = "fixtures/active-goal-authority-audit/issue-contracts.json";
const MARKERS: [&str; 4] = [
    ".ripr/goals/active.toml",
    "active_goal",
    "active goal",
    "current goal",
];
const CLASSIFICATIONS: [&str; 8] = [
    "historical_campaign_evidence",
    "stable_campaign_definition",
    "explicit_campaign_filter",
    "live_selection_or_authorization_remove",
    "compatibility_reader_only",
    "derived_current_view",
    "obsolete",
    "unknown_needs_decision",
];

#[derive(Clone, Debug, Default)]
struct Rule {
    id: String,
    selector: String,
    classification: String,
    owner: String,
    dependent_issue: String,
    compatibility_period: String,
    current_behavior: String,
    fields: String,
    authority: String,
    target_behavior: String,
    positive_proof: String,
    negative_proof: String,
}

#[derive(Debug)]
struct Audit {
    rows: Vec<(String, Rule)>,
    unclassified: Vec<String>,
    unused_rules: Vec<String>,
    contradictions: Vec<String>,
    semantic_digest: String,
    framework_blockers: Vec<String>,
}

pub(crate) fn run() -> Result<(), String> {
    run_at(Path::new("."), Path::new("target/ripr/reports"))
}

pub(crate) fn run_at(root: &Path, reports: &Path) -> Result<(), String> {
    let audit = audit_root(root)?;
    fs::create_dir_all(reports).map_err(|err| format!("create reports directory: {err}"))?;
    fs::write(
        reports.join("active-goal-authority-audit.json"),
        render_json(&audit)?,
    )
    .map_err(|err| format!("write authority audit JSON: {err}"))?;
    fs::write(
        reports.join("active-goal-authority-audit.md"),
        render_markdown(&audit),
    )
    .map_err(|err| format!("write authority audit Markdown: {err}"))?;
    println!(
        "active-goal authority audit: {} consumers, {} blockers",
        audit.rows.len(),
        audit.unclassified.len() + audit.contradictions.len() + audit.framework_blockers.len()
    );
    if audit.unclassified.is_empty()
        && audit.contradictions.is_empty()
        && audit.framework_blockers.is_empty()
    {
        Ok(())
    } else {
        Err(format!(
            "active-goal authority audit blocked: {} unclassified discoveries, {} contradictions, {} framework blockers; inspect {}",
            audit.unclassified.len(),
            audit.contradictions.len(),
            audit.framework_blockers.len(),
            reports.join("active-goal-authority-audit.json").display()
        ))
    }
}

fn audit_root(root: &Path) -> Result<Audit, String> {
    let rules = parse_rules(
        &fs::read_to_string(root.join(INVENTORY))
            .map_err(|err| format!("read {INVENTORY}: {err}"))?,
    )?;
    let issue_rows = issue_contract_rows(root)?;
    let discovered = discover(root)?;
    let mut rows = Vec::new();
    let mut used = BTreeSet::new();
    let mut unclassified = Vec::new();
    let mut contradictions = Vec::new();
    for path in discovered {
        let matches: Vec<&Rule> = rules
            .iter()
            .filter(|rule| selector_matches(&rule.selector, &path))
            .collect();
        let longest = matches.iter().map(|rule| rule.selector.len()).max();
        let specific: Vec<&Rule> = matches
            .into_iter()
            .filter(|rule| Some(rule.selector.len()) == longest)
            .collect();
        match specific.as_slice() {
            [] => unclassified.push(path),
            [rule] => {
                used.insert(rule.id.clone());
                if rule.classification == "unknown_needs_decision" {
                    contradictions.push(format!("{} remains unknown under {}", path, rule.id));
                }
                rows.push((path, (*rule).clone()));
            }
            _ => contradictions.push(format!(
                "{path} matches multiple inventory rules: {}",
                specific
                    .iter()
                    .map(|rule| rule.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
    rows.extend(issue_rows);
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    unclassified.sort();
    contradictions.sort();
    let unused_rules: Vec<String> = rules
        .iter()
        .filter(|rule| !used.contains(&rule.id))
        .map(|rule| rule.id.clone())
        .collect();
    let framework_blockers = vec![
        "occurrence_inventory_not_proven: path-level rules must be replaced by reviewed (path, anchor, marker_kind, normalized_marker_hash) rows; broad live selectors are not migration evidence".to_string(),
        "issue_snapshots_not_proven: current individual snapshots are required for #1631-#1639, #1643-#1650, #1692, #1697, and #1701; range aggregates are not migration evidence".to_string(),
    ];
    let mut semantic = semantic_text(&rows, &unclassified, &unused_rules, &contradictions);
    for blocker in &framework_blockers {
        semantic.push_str(&format!("framework_blocker|{blocker}\n"));
    }
    let semantic_digest = format!("{:x}", Sha256::digest(semantic.as_bytes()));
    Ok(Audit {
        rows,
        unclassified,
        unused_rules,
        contradictions,
        semantic_digest,
        framework_blockers,
    })
}

fn issue_contract_rows(root: &Path) -> Result<Vec<(String, Rule)>, String> {
    let text = fs::read_to_string(root.join(ISSUE_CONTRACTS))
        .map_err(|err| format!("read {ISSUE_CONTRACTS}: {err}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("parse {ISSUE_CONTRACTS}: {err}"))?;
    let contracts = value
        .get("contracts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{ISSUE_CONTRACTS} must contain contracts[]"))?;
    if contracts.is_empty() {
        return Err(format!(
            "{ISSUE_CONTRACTS} must capture at least one issue contract"
        ));
    }
    let mut rows = Vec::new();
    let mut seen_issues = BTreeSet::new();
    for contract in contracts {
        for field in [
            "issue",
            "classification",
            "owner",
            "dependent_issue",
            "compatibility_period",
            "current_behavior",
            "fields",
            "authority",
            "target_behavior",
            "positive_proof",
            "negative_proof",
        ] {
            if contract
                .get(field)
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(format!("issue contract is missing non-empty {field}"));
            }
        }
        let get = |field: &str| {
            contract
                .get(field)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let issue = get("issue");
        if !seen_issues.insert(issue.clone()) {
            return Err(format!("duplicate issue contract identity {issue}"));
        }
        let rule = validate_rule(Rule {
            id: format!("issue-contract-{issue}"),
            selector: format!("github:{issue}"),
            classification: get("classification"),
            owner: get("owner"),
            dependent_issue: get("dependent_issue"),
            compatibility_period: get("compatibility_period"),
            current_behavior: get("current_behavior"),
            fields: get("fields"),
            authority: get("authority"),
            target_behavior: get("target_behavior"),
            positive_proof: get("positive_proof"),
            negative_proof: get("negative_proof"),
        })?;
        rows.push((format!("github:{issue}"), rule));
    }
    Ok(rows)
}

fn discover(root: &Path) -> Result<Vec<String>, String> {
    if root.join(".git").exists() {
        let mut paths = Vec::new();
        for normalized in tracked_paths(root)? {
            if super::should_skip_path(&normalized) {
                continue;
            }
            let path = root.join(&normalized);
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            if contains_marker(&text) {
                paths.push(normalized);
            }
        }
        paths.sort();
        return Ok(paths);
    }
    let mut paths = Vec::new();
    walk(root, root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    let output = crate::run::run_output_bytes_in_dir("git", &["ls-files", "-z"], root)?;
    parse_tracked_paths(&output)
}

fn parse_tracked_paths(output: &[u8]) -> Result<Vec<String>, String> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            std::str::from_utf8(path)
                .map(str::to_string)
                .map_err(|err| format!("git ls-files path was not UTF-8: {err}"))
        })
        .collect()
}

fn walk(root: &Path, directory: &Path, found: &mut Vec<String>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .map_err(|err| format!("read directory {}: {err}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("read directory entry: {err}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|err| format!("strip root: {err}"))?;
        let normalized = normalize_path(relative)?;
        if path.is_dir() {
            if normalized == ".git" || normalized == "target" || normalized.starts_with(".git/") {
                continue;
            }
            walk(root, &path, found)?;
        } else if path.is_file() {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if contains_marker(&text) {
                found.push(normalized);
            }
        }
    }
    Ok(())
}

fn normalize_path(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}

fn contains_marker(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    MARKERS.iter().any(|marker| lowered.contains(marker))
}

fn parse_rules(text: &str) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();
    let mut current: Option<Rule> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[consumer]]" {
            if let Some(rule) = current.take() {
                rules.push(validate_rule(rule)?);
            }
            current = Some(Rule::default());
            continue;
        }
        let Some(rule) = current.as_mut() else {
            return Err(format!(
                "inventory line {} is outside [[consumer]]",
                index + 1
            ));
        };
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid inventory line {}", index + 1))?;
        let value = value
            .trim()
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| format!("inventory line {} must use a quoted value", index + 1))?;
        match key.trim() {
            "id" => rule.id = value.to_string(),
            "selector" => rule.selector = value.to_string(),
            "classification" => rule.classification = value.to_string(),
            "owner" => rule.owner = value.to_string(),
            "dependent_issue" => rule.dependent_issue = value.to_string(),
            "compatibility_period" => rule.compatibility_period = value.to_string(),
            "current_behavior" => rule.current_behavior = value.to_string(),
            "fields" => rule.fields = value.to_string(),
            "authority" => rule.authority = value.to_string(),
            "target_behavior" => rule.target_behavior = value.to_string(),
            "positive_proof" => rule.positive_proof = value.to_string(),
            "negative_proof" => rule.negative_proof = value.to_string(),
            other => {
                return Err(format!(
                    "unknown inventory key {other} on line {}",
                    index + 1
                ));
            }
        }
    }
    if let Some(rule) = current {
        rules.push(validate_rule(rule)?);
    }
    if rules.is_empty() {
        return Err("consumer inventory is empty".to_string());
    }
    let mut ids = BTreeSet::new();
    for rule in &rules {
        if !ids.insert(rule.id.as_str()) {
            return Err(format!("duplicate consumer inventory id {}", rule.id));
        }
    }
    Ok(rules)
}

fn validate_rule(rule: Rule) -> Result<Rule, String> {
    let required = [
        &rule.id,
        &rule.selector,
        &rule.classification,
        &rule.owner,
        &rule.dependent_issue,
        &rule.compatibility_period,
        &rule.current_behavior,
        &rule.fields,
        &rule.authority,
        &rule.target_behavior,
        &rule.positive_proof,
        &rule.negative_proof,
    ];
    if required.iter().any(|value| value.is_empty()) {
        return Err(format!(
            "inventory rule {} has empty required fields",
            rule.id
        ));
    }
    if !CLASSIFICATIONS.contains(&rule.classification.as_str()) {
        return Err(format!(
            "inventory rule {} has unknown classification {}",
            rule.id, rule.classification
        ));
    }
    Ok(rule)
}

fn selector_matches(selector: &str, path: &str) -> bool {
    selector
        .strip_suffix("/**")
        .map_or(path == selector, |prefix| {
            path == prefix || path.starts_with(&format!("{prefix}/"))
        })
}

fn semantic_text(
    rows: &[(String, Rule)],
    unclassified: &[String],
    unused: &[String],
    contradictions: &[String],
) -> String {
    let mut text = String::new();
    for (path, rule) in rows {
        text.push_str(&format!(
            "{path}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
            rule.id,
            rule.classification,
            rule.owner,
            rule.dependent_issue,
            rule.compatibility_period,
            rule.current_behavior,
            rule.fields,
            rule.authority,
            rule.target_behavior,
            rule.positive_proof,
            rule.negative_proof
        ));
    }
    for value in unclassified {
        text.push_str(&format!("unclassified|{value}\n"));
    }
    for value in unused {
        text.push_str(&format!("unused|{value}\n"));
    }
    for value in contradictions {
        text.push_str(&format!("contradiction|{value}\n"));
    }
    text
}

fn render_json(audit: &Audit) -> Result<String, String> {
    let rows: Vec<_> = audit.rows.iter().map(|(path, rule)| serde_json::json!({
        "path": path, "inventory_id": rule.id, "classification": rule.classification,
        "owner": rule.owner, "dependent_issue": rule.dependent_issue,
        "compatibility_period": rule.compatibility_period, "current_behavior": rule.current_behavior,
        "fields_consumed": rule.fields, "authority_effect": rule.authority,
        "target_behavior": rule.target_behavior, "positive_proof": rule.positive_proof,
        "negative_proof": rule.negative_proof
    })).collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "schema_version":"0.1", "semantic_digest":audit.semantic_digest,
        "migration_ready": audit.unclassified.is_empty() && audit.contradictions.is_empty() && audit.framework_blockers.is_empty(),
        "consumers":rows, "unclassified_discoveries":audit.unclassified,
        "unused_inventory_rows":audit.unused_rules, "contradictions":audit.contradictions,
        "framework_blockers": audit.framework_blockers,
        "blockers": audit.unclassified.len() + audit.contradictions.len() + audit.framework_blockers.len(),
        "non_claims":["live_work_selection","mutation_authority","campaign_priority","github_state_freshness"]
    }))
    .map(|text| text + "\n")
    .map_err(|err| format!("serialize authority audit JSON: {err}"))
}

fn render_markdown(audit: &Audit) -> String {
    let ready = audit.unclassified.is_empty()
        && audit.contradictions.is_empty()
        && audit.framework_blockers.is_empty();
    let mut out = format!(
        "# Active-goal authority audit\n\nSchema: `0.1`\n\nSemantic digest: `{}`\n\nMigration ready: `{ready}`\n\nThis report inventories tracked singleton consumers. It does not select work, authorize mutation, rank campaigns, or read live GitHub state.\n\n## Consumers\n\n| Path | Classification | Owner | Follow-up | Authority effect |\n| --- | --- | --- | --- | --- |\n",
        audit.semantic_digest
    );
    for (path, rule) in &audit.rows {
        out.push_str(&format!(
            "| `{path}` | `{}` | `{}` | `{}` | {} |\n",
            rule.classification, rule.owner, rule.dependent_issue, rule.authority
        ));
    }
    out.push_str("\n## Blockers\n\n");
    if ready {
        out.push_str("None.\n");
    }
    for value in &audit.unclassified {
        out.push_str(&format!("- unclassified: `{value}`\n"));
    }
    for value in &audit.contradictions {
        out.push_str(&format!("- contradiction: {value}\n"));
    }
    for value in &audit.framework_blockers {
        out.push_str(&format!("- framework: {value}\n"));
    }
    out.push_str("\n## Unused inventory rows\n\n");
    if audit.unused_rules.is_empty() {
        out.push_str("None.\n");
    } else {
        for value in &audit.unused_rules {
            out.push_str(&format!("- `{value}`\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        audit_root, discover, issue_contract_rows, parse_rules, parse_tracked_paths, run_at,
        selector_matches,
    };
    use std::fs;
    use std::path::Path;

    #[test]
    fn nul_delimited_tracked_paths_preserve_newlines() -> Result<(), String> {
        let paths = parse_tracked_paths(b"plain.rs\0line\nbreak.rs\0")?;
        if paths != ["plain.rs", "line\nbreak.rs"] {
            return Err(format!("tracked paths changed identity: {paths:?}"));
        }
        Ok(())
    }

    #[test]
    fn tracked_paths_reject_non_utf8_identity() -> Result<(), String> {
        if parse_tracked_paths(b"valid.rs\0invalid-\xff.rs\0").is_ok() {
            return Err("non-UTF-8 tracked path was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn fallback_paths_reject_non_utf8_identity() -> Result<(), String> {
        #[cfg(unix)]
        let path = {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;
            std::path::PathBuf::from(OsString::from_vec(vec![0xff]))
        };
        #[cfg(windows)]
        let path = {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;
            std::path::PathBuf::from(OsString::from_wide(&[0xd800]))
        };
        if super::normalize_path(&path).is_ok() {
            return Err("non-UTF-8 fallback path was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn selectors_are_exact_or_directory_scoped() -> Result<(), String> {
        if !selector_matches("docs/handoffs/**", "docs/handoffs/old.md") {
            return Err("directory-scoped selector did not match a child".to_string());
        }
        if selector_matches("docs/handoffs/**", "docs/handoff.md") {
            return Err("directory-scoped selector escaped its directory".to_string());
        }
        Ok(())
    }

    #[test]
    fn repository_inventory_is_complete_and_deterministic() -> Result<(), String> {
        let first = audit_root(Path::new("../"))?;
        if !first.unclassified.is_empty() {
            return Err(format!("unclassified: {:?}", first.unclassified));
        }
        if !first.contradictions.is_empty() {
            return Err(format!("contradictions: {:?}", first.contradictions));
        }
        let fixture = Path::new("../fixtures/active-goal-authority-audit/historical-reference");
        let fixture_first = audit_root(fixture)?;
        let fixture_second = audit_root(fixture)?;
        if fixture_first.semantic_digest != fixture_second.semantic_digest {
            return Err("equivalent inputs changed semantic digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn ignored_and_untracked_residue_does_not_change_digest() -> Result<(), String> {
        let root = Path::new("../");
        let residue = root.join(format!(
            ".active-goal-audit-residue-{}/active_goal_reader.txt",
            std::process::id()
        ));
        let parent = residue
            .parent()
            .ok_or_else(|| "residue path has no parent".to_string())?;
        fs::create_dir_all(parent).map_err(|err| format!("create residue directory: {err}"))?;
        fs::write(&residue, ".ripr/goals/active.toml ready = true")
            .map_err(|err| format!("write residue: {err}"))?;
        let discovered = discover(root)?;
        if let Err(err) = fs::remove_dir_all(parent)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!("remove residue: {err}"));
        }
        if discovered
            .iter()
            .any(|path| path.contains(".active-goal-audit-residue-"))
        {
            return Err("untracked residue entered repository discovery".to_string());
        }
        Ok(())
    }

    #[test]
    fn hidden_singleton_and_legacy_ready_block_migration() -> Result<(), String> {
        let audit = audit_root(Path::new(
            "../fixtures/active-goal-authority-audit/hidden-singleton",
        ))?;
        if !audit
            .unclassified
            .iter()
            .any(|path| path == "hidden_reader.rs")
        {
            return Err("hidden_reader.rs did not block migration".to_string());
        }
        if audit
            .unclassified
            .iter()
            .any(|path| path.ends_with("active.toml"))
        {
            return Err("fixture unexpectedly contains active.toml".to_string());
        }
        let source = fs::read_to_string(
            "../fixtures/active-goal-authority-audit/hidden-singleton/hidden_reader.rs",
        )
        .map_err(|err| format!("read hidden reader: {err}"))?;
        if !source.contains("const LEGACY_STATUS: &str = \"ready\";") {
            return Err("legacy ready control is missing or changed".to_string());
        }
        if source
            .replace("\"ready\"", "\"pending\"")
            .contains("const LEGACY_STATUS: &str = \"ready\";")
        {
            return Err("legacy ready control did not discriminate changed status".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_consumer_and_issue_identities_are_rejected() -> Result<(), String> {
        let duplicate_rule =
            include_str!("../../fixtures/active-goal-authority-audit/consumers.toml");
        let first = duplicate_rule
            .split("[[consumer]]")
            .nth(1)
            .ok_or_else(|| "inventory has no consumer".to_string())?;
        let duplicated = format!("[[consumer]]{first}[[consumer]]{first}");
        if parse_rules(&duplicated).is_ok() {
            return Err("duplicate consumer id was accepted".to_string());
        }
        let root = Path::new("../fixtures/active-goal-authority-audit/duplicate-issues");
        if issue_contract_rows(root).is_ok() {
            return Err("duplicate issue identity was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn blocked_command_returns_error_after_writing_both_reports() -> Result<(), String> {
        let fixture = Path::new("../fixtures/active-goal-authority-audit/hidden-singleton");
        let reports = fixture.join("target/ripr/reports");
        let result = run_at(fixture, &reports);
        if result.is_ok() {
            return Err("blocked command unexpectedly returned success".to_string());
        }
        for name in [
            "active-goal-authority-audit.json",
            "active-goal-authority-audit.md",
        ] {
            if !reports.join(name).is_file() {
                return Err(format!("blocked command did not preserve {name}"));
            }
        }
        fs::remove_dir_all(fixture.join("target"))
            .map_err(|err| format!("remove fixture reports: {err}"))?;
        Ok(())
    }

    #[test]
    fn historical_reference_is_classified_without_authority() -> Result<(), String> {
        let audit = audit_root(Path::new(
            "../fixtures/active-goal-authority-audit/historical-reference",
        ))?;
        if !audit.unclassified.is_empty() {
            return Err(format!(
                "historical fixture unclassified: {:?}",
                audit.unclassified
            ));
        }
        if !audit
            .rows
            .iter()
            .all(|(_, rule)| rule.classification == "historical_campaign_evidence")
        {
            return Err("historical fixture gained non-historical authority".to_string());
        }
        Ok(())
    }
}

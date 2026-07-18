use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Occurrence {
    path: String,
    anchor: String,
    marker_kind: String,
    normalized_marker_hash: String,
}

#[derive(Clone, Debug)]
struct RustOwner {
    start: usize,
    end: usize,
    anchor: String,
}

#[derive(Debug)]
struct Audit {
    rows: Vec<(String, Rule)>,
    occurrences: Vec<Occurrence>,
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
    let occurrences = discover_occurrences(root)?;
    let discovered = occurrences
        .iter()
        .map(|occurrence| occurrence.path.clone())
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    let mut used = BTreeSet::new();
    let mut unclassified = Vec::new();
    let mut contradictions = duplicate_occurrence_contradictions(&occurrences);
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
    let mut semantic = semantic_text(
        &rows,
        &occurrences,
        &unclassified,
        &unused_rules,
        &contradictions,
    );
    for blocker in &framework_blockers {
        semantic.push_str(&format!("framework_blocker|{blocker}\n"));
    }
    let semantic_digest = format!("{:x}", Sha256::digest(semantic.as_bytes()));
    Ok(Audit {
        rows,
        occurrences,
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

#[cfg(test)]
fn discover(root: &Path) -> Result<Vec<String>, String> {
    Ok(discover_occurrences(root)?
        .into_iter()
        .map(|occurrence| occurrence.path)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn discover_occurrences(root: &Path) -> Result<Vec<Occurrence>, String> {
    let paths = if root.join(".git").exists() {
        tracked_paths(root)?
    } else {
        let mut paths = Vec::new();
        walk_paths(root, root, &mut paths)?;
        paths
    };
    let mut occurrences = Vec::new();
    for normalized in paths {
        if super::should_skip_path(&normalized) {
            continue;
        }
        let path = root.join(&normalized);
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        occurrences.extend(occurrences_in_text(&normalized, &text));
    }
    occurrences.sort();
    Ok(occurrences)
}

fn occurrences_in_text(path: &str, text: &str) -> Vec<Occurrence> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut line_starts = vec![0];
    line_starts.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
    let rust_owners = if Path::new(path).extension().and_then(|value| value.to_str()) == Some("rs")
    {
        collect_rust_owners(text)
    } else {
        Vec::new()
    };
    let mut occurrences = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let lowered = line.to_ascii_lowercase();
        for marker in MARKERS {
            for (column, _) in marker_matches(&lowered, marker) {
                let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
                let anchor = structural_anchor(
                    path,
                    &lines,
                    index,
                    line_starts[index] + column,
                    &rust_owners,
                );
                let marker_kind = marker_kind(marker).to_string();
                let normalized_marker_hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));
                occurrences.push(Occurrence {
                    path: path.to_string(),
                    anchor,
                    marker_kind,
                    normalized_marker_hash,
                });
            }
        }
    }
    occurrences
}

fn marker_matches<'a>(text: &'a str, marker: &'a str) -> impl Iterator<Item = (usize, &'a str)> {
    text.match_indices(marker).filter(move |(start, _)| {
        if marker == ".ripr/goals/active.toml" {
            return true;
        }
        let before = text[..*start].chars().next_back();
        let end = *start + marker.len();
        let after = text[end..].chars().next();
        let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
        before.is_none_or(|ch| !is_word(ch)) && after.is_none_or(|ch| !is_word(ch))
    })
}

fn duplicate_occurrence_contradictions(occurrences: &[Occurrence]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut contradictions = BTreeSet::new();
    for occurrence in occurrences {
        let identity = (
            occurrence.path.as_str(),
            occurrence.anchor.as_str(),
            occurrence.marker_kind.as_str(),
            occurrence.normalized_marker_hash.as_str(),
        );
        if !seen.insert(identity) {
            contradictions.insert(format!(
                "duplicate occurrence identity in {} at {} ({})",
                occurrence.path, occurrence.anchor, occurrence.marker_kind
            ));
        }
    }
    contradictions.into_iter().collect()
}

fn marker_kind(marker: &str) -> &'static str {
    match marker {
        ".ripr/goals/active.toml" => "singleton_manifest_path",
        "active_goal" => "active_goal_identifier",
        "active goal" => "active_goal_phrase",
        "current goal" => "current_goal_phrase",
        _ => "unknown_marker",
    }
}

fn structural_anchor(
    path: &str,
    lines: &[&str],
    index: usize,
    byte_offset: usize,
    rust_owners: &[RustOwner],
) -> String {
    let extension = Path::new(path).extension().and_then(|value| value.to_str());
    if extension == Some("rs") {
        return rust_owners
            .iter()
            .filter(|owner| owner.start <= byte_offset && byte_offset < owner.end)
            .min_by_key(|owner| owner.end - owner.start)
            .map_or_else(
                || "rust-document-root".to_string(),
                |owner| owner.anchor.clone(),
            );
    }
    if extension == Some("toml") {
        return toml_anchor(lines, index);
    }
    if extension == Some("md") {
        return markdown_anchor(lines, index);
    }
    for candidate in lines[..=index].iter().rev() {
        let trimmed = candidate.trim();
        let anchored = match extension {
            Some("yml" | "yaml") => trimmed.ends_with(':') || trimmed.starts_with("- name:"),
            Some("json") => trimmed.starts_with('"') && trimmed.contains(':'),
            _ => false,
        };
        if anchored {
            return trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
        }
    }
    "document-root".to_string()
}

fn collect_rust_owners(text: &str) -> Vec<RustOwner> {
    use ra_ap_syntax::ast::{self, AstNode, HasName};
    use ra_ap_syntax::{Edition, SourceFile};

    let parse = SourceFile::parse(text, Edition::Edition2024);
    let tree = parse.tree();
    let mut owners = Vec::new();
    for node in tree.syntax().descendants() {
        let leaf = if let Some(item) = ast::Fn::cast(node.clone()) {
            item.name().map(|name| format!("fn {}", name.text()))
        } else if let Some(item) = ast::Const::cast(node.clone()) {
            item.name().map(|name| format!("const {}", name.text()))
        } else if let Some(item) = ast::Static::cast(node.clone()) {
            item.name().map(|name| format!("static {}", name.text()))
        } else {
            None
        };
        let Some(leaf) = leaf else {
            continue;
        };
        let mut chain = vec![leaf];
        for parent in node.ancestors().skip(1) {
            let owner = if let Some(item) = ast::Impl::cast(parent.clone()) {
                item.self_ty().map(|self_ty| {
                    item.trait_().map_or_else(
                        || format!("impl {}", self_ty.syntax().text()),
                        |trait_ty| {
                            format!(
                                "impl {} for {}",
                                trait_ty.syntax().text(),
                                self_ty.syntax().text()
                            )
                        },
                    )
                })
            } else if let Some(item) = ast::Trait::cast(parent.clone()) {
                item.name().map(|name| format!("trait {}", name.text()))
            } else if let Some(item) = ast::Module::cast(parent) {
                item.name().map(|name| format!("mod {}", name.text()))
            } else {
                None
            };
            if let Some(owner) = owner {
                chain.push(owner);
            }
        }
        chain.reverse();
        let range = node.text_range();
        owners.push(RustOwner {
            start: u32::from(range.start()) as usize,
            end: u32::from(range.end()) as usize,
            anchor: chain.join(" > "),
        });
    }
    owners
}

fn markdown_anchor(lines: &[&str], index: usize) -> String {
    let mut headings = Vec::<String>::new();
    let mut repeated = BTreeMap::<(usize, String), usize>::new();
    for candidate in &lines[..=index] {
        let trimmed = candidate.trim();
        let level = trimmed.chars().take_while(|ch| *ch == '#').count();
        if level == 0 || !trimmed.chars().nth(level).is_some_and(char::is_whitespace) {
            continue;
        }
        headings.truncate(level.saturating_sub(1));
        let count = repeated.entry((level, trimmed.to_string())).or_default();
        *count += 1;
        headings.push(format!("{trimmed}[{count}]"));
    }
    if headings.is_empty() {
        "document-root".to_string()
    } else {
        headings.join(" > ")
    }
}

fn toml_anchor(lines: &[&str], index: usize) -> String {
    let mut table = "document-root";
    let mut identity = None;
    for candidate in lines[..=index].iter().rev() {
        let trimmed = candidate.trim();
        if identity.is_none()
            && ["id", "name", "spec", "selector"]
                .iter()
                .any(|key| trimmed.starts_with(&format!("{key} =")))
        {
            identity = Some(trimmed);
        }
        if trimmed.starts_with('[') {
            table = trimmed;
            break;
        }
    }
    identity.map_or_else(|| table.to_string(), |value| format!("{table} {value}"))
}

fn walk_paths(root: &Path, directory: &Path, found: &mut Vec<String>) -> Result<(), String> {
    if root.join(".git").exists() {
        return Err("walk_paths is only for non-git fixture roots".to_string());
    }
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
            walk_paths(root, &path, found)?;
        } else if path.is_file() {
            found.push(normalized);
        }
    }
    Ok(())
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

fn normalize_path(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
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
    occurrences: &[Occurrence],
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
    for occurrence in occurrences {
        text.push_str(&format!(
            "occurrence|{}|{}|{}|{}\n",
            occurrence.path,
            occurrence.anchor,
            occurrence.marker_kind,
            occurrence.normalized_marker_hash
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
    let occurrences: Vec<_> = audit
        .occurrences
        .iter()
        .map(|occurrence| {
            serde_json::json!({
                "path": occurrence.path,
                "anchor": occurrence.anchor,
                "marker_kind": occurrence.marker_kind,
                "normalized_marker_hash": occurrence.normalized_marker_hash,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "schema_version":"0.2", "semantic_digest":audit.semantic_digest,
        "migration_ready": audit.unclassified.is_empty() && audit.contradictions.is_empty() && audit.framework_blockers.is_empty(),
        "consumers":rows, "occurrences": occurrences,
        "occurrence_count": audit.occurrences.len(),
        "unclassified_discoveries":audit.unclassified,
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
        "# Active-goal authority audit\n\nSchema: `0.2`\n\nSemantic digest: `{}`\n\nMigration ready: `{ready}`\n\nThis report inventories tracked singleton consumers. It does not select work, authorize mutation, rank campaigns, or read live GitHub state.\n\n## Occurrences\n\n| Path | Anchor | Marker kind | Normalized marker hash |\n| --- | --- | --- | --- |\n",
        audit.semantic_digest
    );
    for occurrence in &audit.occurrences {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            occurrence.path,
            occurrence.anchor.replace('|', "\\|"),
            occurrence.marker_kind,
            occurrence.normalized_marker_hash
        ));
    }
    out.push_str("\n## Consumers\n\n| Path | Classification | Owner | Follow-up | Authority effect |\n| --- | --- | --- | --- | --- |\n");
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
        audit_root, discover, duplicate_occurrence_contradictions, issue_contract_rows,
        occurrences_in_text, parse_rules, parse_tracked_paths, render_json, run_at,
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
    fn occurrence_identity_is_structural_and_content_sensitive() -> Result<(), String> {
        let before = occurrences_in_text(
            "docs/example.md",
            "# Authority\n\nRead .ripr/goals/active.toml before work.\n",
        );
        let moved = occurrences_in_text(
            "docs/example.md",
            "# Authority\n\n\nRead .ripr/goals/active.toml before work.\n",
        );
        if before != moved {
            return Err("line movement changed occurrence identity".to_string());
        }
        let changed = occurrences_in_text(
            "docs/example.md",
            "# Authority\n\nNever read .ripr/goals/active.toml before work.\n",
        );
        if before.first().map(|item| &item.normalized_marker_hash)
            == changed.first().map(|item| &item.normalized_marker_hash)
        {
            return Err("authority-bearing content change preserved its hash".to_string());
        }
        Ok(())
    }

    #[test]
    fn reviewed_path_additions_and_duplicate_markers_cannot_hide() -> Result<(), String> {
        let baseline = occurrences_in_text(
            "docs/reviewed.md",
            "# Authority\n\nRead .ripr/goals/active.toml.\n",
        );
        let added = occurrences_in_text(
            "docs/reviewed.md",
            "# Authority\n\nRead .ripr/goals/active.toml.\n\n# Recovery\n\nRead current goal.\n",
        );
        if added.len() != baseline.len() + 1 {
            return Err("new occurrence in a reviewed path was absorbed".to_string());
        }
        let duplicates = occurrences_in_text(
            "docs/reviewed.md",
            "# Authority\n\n.ripr/goals/active.toml and .ripr/goals/active.toml\n",
        );
        if duplicate_occurrence_contradictions(&duplicates).len() != 1 {
            return Err("same-anchor duplicate markers did not fail closed".to_string());
        }
        Ok(())
    }

    #[test]
    fn rust_anchors_use_syntax_owners_and_markers_require_boundaries() -> Result<(), String> {
        let source = r#"
impl First {
    pub(crate) async fn load(&self) { let value = "active_goal"; }
}
impl Second {
    fn load(&self) { let value = "active_goal"; }
}
trait TraitA { fn load(&self); }
trait TraitB { fn load(&self); }
impl TraitA for Shared {
    fn load(&self) { let value = "active_goal"; }
}
impl TraitB for Shared {
    fn load(&self) { let value = "active_goal"; }
}
fn noise() { let inactive_goal = "active goals"; let active_goal_id = 1; }
"#;
        let occurrences = occurrences_in_text("src/lib.rs", source);
        if occurrences.len() != 4 {
            return Err(format!("identifier boundaries changed: {occurrences:?}"));
        }
        if occurrences[0].anchor != "impl First > fn load"
            || occurrences[1].anchor != "impl Second > fn load"
            || occurrences[2].anchor != "impl TraitA for Shared > fn load"
            || occurrences[3].anchor != "impl TraitB for Shared > fn load"
        {
            return Err(format!("Rust syntax owners changed: {occurrences:?}"));
        }
        let crlf = occurrences_in_text("src/lib.rs", &source.replace('\n', "\r\n"));
        if occurrences != crlf {
            return Err(format!(
                "line-ending style changed Rust occurrence identity: {crlf:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn occurrence_projection_preserves_legacy_consumers() -> Result<(), String> {
        let audit = audit_root(Path::new("../"))?;
        let json = render_json(&audit)?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|err| format!("parse report: {err}"))?;
        if value.get("schema_version").and_then(|item| item.as_str()) != Some("0.2") {
            return Err("occurrence report schema is not 0.2".to_string());
        }
        if value
            .get("consumers")
            .and_then(|item| item.as_array())
            .is_none()
        {
            return Err("legacy consumers projection disappeared".to_string());
        }
        let occurrences = value
            .get("occurrences")
            .and_then(|item| item.as_array())
            .ok_or_else(|| "occurrence projection is missing".to_string())?;
        if occurrences.is_empty() {
            return Err("occurrence projection is vacuous".to_string());
        }
        for field in ["path", "anchor", "marker_kind", "normalized_marker_hash"] {
            if occurrences[0].get(field).is_none() {
                return Err(format!("occurrence projection is missing {field}"));
            }
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
        if first.contradictions.is_empty()
            || first
                .contradictions
                .iter()
                .any(|value| !value.starts_with("duplicate occurrence identity in "))
        {
            return Err(format!(
                "occurrence contradictions changed shape: {:?}",
                first.contradictions
            ));
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

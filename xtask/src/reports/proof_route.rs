//! `cargo xtask proof route` — the read-only proof-route report from the
//! proof-routing operating model (docs/PROOF_ROUTING.md, slice 3).
//!
//! The command maps the files changed between a base and head revision onto
//! the proof packs in `policy/proof-packs.toml` and reports which CI lanes
//! (from `policy/ci-lane-whitelist.toml`) the change would require, which
//! stay advisory, which would be skipped with a reason, and which are pinned
//! as never routed. Routing state is `advisory-report-only`: this command
//! never executes a proof command and changes no CI behavior.

use crate::policy::proof_packs::{ProofPack, load_proof_packs};
use crate::run::run_output_owned;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_BASE: &str = "origin/main";
const DEFAULT_HEAD: &str = "HEAD";
const PROOF_ROUTE_SCHEMA_VERSION: &str = "0.1";
const PROOF_ROUTE_ROUTING_STATE: &str = "advisory-report-only";
const UNKNOWN_SURFACE_REASON: &str = "unknown_surface_full_proof";
const NO_MATCHED_SURFACE_REASON: &str = "no_matched_surface";
const NEVER_ROUTED_REASON: &str = "never_routed";
const PROOF_ROUTE_JSON: &str = "proof-route.json";
const PROOF_ROUTE_MD: &str = "proof-route.md";
const PROOF_USAGE: &str = "usage: cargo xtask proof route [--base <rev>] [--head <rev>]";

/// Static lane-cost markers. A lane is `heavy` when any of its commands needs
/// a workspace build or full test run, `medium` when it drives the Node or
/// VSIX toolchain, and `light` otherwise.
const HEAVY_COMMAND_MARKERS: &[&str] = &[
    "cargo check",
    "cargo clippy",
    "cargo test",
    "llvm-cov",
    "nextest",
];
const MEDIUM_COMMAND_MARKERS: &[&str] = &["npm", "vsce"];

pub(crate) fn proof(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{PROOF_USAGE}");
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(format!("missing proof subcommand; {PROOF_USAGE}"));
    };
    match subcommand.as_str() {
        "route" => proof_route(rest),
        other => Err(format!("unknown proof subcommand `{other}`; {PROOF_USAGE}")),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProofRouteOptions {
    base: String,
    head: String,
}

impl Default for ProofRouteOptions {
    fn default() -> Self {
        Self {
            base: DEFAULT_BASE.to_string(),
            head: DEFAULT_HEAD.to_string(),
        }
    }
}

fn parse_route_options(args: &[String]) -> Result<ProofRouteOptions, String> {
    let mut options = ProofRouteOptions::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--base" => {
                index += 1;
                options.base = non_empty_arg(args, index, "--base")?.to_string();
            }
            "--head" => {
                index += 1;
                options.head = non_empty_arg(args, index, "--head")?.to_string();
            }
            other => {
                return Err(format!(
                    "unknown proof route argument `{other}`; {PROOF_USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}; {PROOF_USAGE}"));
    };
    if value.trim().is_empty() {
        return Err(format!("proof route {flag} requires a non-empty value"));
    }
    Ok(value)
}

fn proof_route(args: &[String]) -> Result<(), String> {
    let options = parse_route_options(args)?;
    let packs = load_proof_packs()?;
    let lanes = load_ci_lanes()?;
    let base_sha = resolve_commit_sha(&options.base)?;
    let head_sha = resolve_commit_sha(&options.head)?;
    let changed_files = changed_files(&options.base, &options.head)?;
    let route = route_proof(&packs, &lanes, &changed_files);

    let json_value = proof_route_json(&options, &base_sha, &head_sha, &changed_files, &route);
    let json_text = serde_json::to_string_pretty(&json_value)
        .map_err(|err| format!("serialize proof route report: {err}"))?;
    crate::write_report(PROOF_ROUTE_JSON, &format!("{json_text}\n"))?;
    crate::write_report(
        PROOF_ROUTE_MD,
        &proof_route_markdown(&options, &base_sha, &head_sha, &changed_files, &route),
    )?;
    println!("Wrote target/ripr/reports/{PROOF_ROUTE_JSON}");
    println!("Wrote target/ripr/reports/{PROOF_ROUTE_MD}");
    Ok(())
}

fn git_output(args: &[&str]) -> Result<String, String> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    run_output_owned("git", &owned)
}

fn resolve_commit_sha(rev: &str) -> Result<String, String> {
    let spec = format!("{rev}^{{commit}}");
    git_output(&["rev-parse", "--verify", spec.as_str()])
        .map(|output| output.trim().to_string())
        .map_err(|err| format!("bad proof route revision {rev:?}: {err}"))
}

fn changed_files(base: &str, head: &str) -> Result<Vec<String>, String> {
    let range = format!("{base}...{head}");
    let output = git_output(&["diff", "--name-only", range.as_str()])?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

/// One `[[lane]]` entry from `policy/ci-lane-whitelist.toml`, reduced to the
/// fields proof routing needs.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CiLane {
    id: String,
    commands: Vec<String>,
}

fn load_ci_lanes() -> Result<Vec<CiLane>, String> {
    let mut violations = Vec::new();
    let document =
        crate::read_ci_ledger_document(crate::PROOF_PACK_LANE_WHITELIST_PATH, &mut violations);
    let lanes = document
        .as_ref()
        .map(|document| parse_ci_lanes(document, &mut violations))
        .unwrap_or_default();
    if violations.is_empty() {
        Ok(lanes)
    } else {
        Err(format!(
            "{} is not usable for proof routing:\n{}\nrun `cargo xtask check-ci-lane-whitelist` for the full report",
            crate::PROOF_PACK_LANE_WHITELIST_PATH,
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn parse_ci_lanes(document: &crate::CiLedgerDocument, violations: &mut Vec<String>) -> Vec<CiLane> {
    let path = crate::PROOF_PACK_LANE_WHITELIST_PATH;
    let mut lanes = Vec::new();
    for table in crate::ci_tables(document, "lane") {
        let Some(id) = crate::ci_required_non_empty_table_string(path, table, "id", violations)
        else {
            continue;
        };
        let commands =
            crate::ci_required_table_array(path, table, "commands", violations).unwrap_or_default();
        lanes.push(CiLane { id, commands });
    }
    lanes
}

fn estimated_lane_cost(commands: &[String]) -> &'static str {
    let contains_any = |markers: &[&str]| {
        commands
            .iter()
            .any(|command| markers.iter().any(|marker| command.contains(marker)))
    };
    if contains_any(HEAVY_COMMAND_MARKERS) {
        "heavy"
    } else if contains_any(MEDIUM_COMMAND_MARKERS) {
        "medium"
    } else {
        "light"
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatchedPack {
    id: String,
    ci_lane: Option<String>,
    matched_files: Vec<String>,
    required_commands: Vec<String>,
    advisory_commands: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LaneRoute {
    id: String,
    estimated_cost: &'static str,
    packs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkippedLane {
    id: String,
    estimated_cost: &'static str,
    reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProofRoute {
    matched_packs: Vec<MatchedPack>,
    unmatched_files: Vec<String>,
    full_proof: bool,
    release_proof_required: bool,
    required_lanes: Vec<LaneRoute>,
    advisory_lanes: Vec<LaneRoute>,
    skipped_lanes: Vec<SkippedLane>,
    never_routed_lanes: Vec<LaneRoute>,
}

/// Pure routing decision: changed files -> matched packs -> lane verdicts.
///
/// - A file matching no pack triggers the full proof
///   (`unknown_surface_policy = "full-proof"`): every pack's lane is required.
/// - A surface in more than one pack runs the union of the matched packs.
/// - The lane of a `never_routed` pack (release proof) is required whenever
///   the pack matches or the full proof triggers, and is otherwise reported
///   as never routed — it is never listed as skipped.
fn route_proof(packs: &[ProofPack], lanes: &[CiLane], changed_files: &[String]) -> ProofRoute {
    let mut matched_files_by_pack: Vec<Vec<String>> = vec![Vec::new(); packs.len()];
    let mut unmatched_files = Vec::new();
    for file in changed_files {
        let mut matched_any = false;
        for (index, pack) in packs.iter().enumerate() {
            if pack
                .paths
                .iter()
                .any(|glob| crate::glob_matches(glob, file))
                && let Some(files) = matched_files_by_pack.get_mut(index)
            {
                files.push(file.clone());
                matched_any = true;
            }
        }
        if !matched_any {
            unmatched_files.push(file.clone());
        }
    }
    let full_proof = !unmatched_files.is_empty();

    let matched_packs: Vec<MatchedPack> = packs
        .iter()
        .zip(&matched_files_by_pack)
        .filter(|(_, files)| !files.is_empty())
        .map(|(pack, files)| MatchedPack {
            id: pack.id.clone(),
            ci_lane: pack.ci_lane.clone(),
            matched_files: files.clone(),
            required_commands: pack.required_commands.clone(),
            advisory_commands: pack.advisory_commands.clone(),
        })
        .collect();

    // Packs whose proof the change must pay: the matched packs, or every pack
    // when an unknown surface routes to the full proof.
    let routed_packs: Vec<&ProofPack> = packs
        .iter()
        .zip(&matched_files_by_pack)
        .filter(|(_, files)| full_proof || !files.is_empty())
        .map(|(pack, _)| pack)
        .collect();

    // Release proof is required whenever a never-routed (release) pack
    // matched or the unknown-surface full proof triggered.
    let release_proof_required = routed_packs.iter().any(|pack| pack.never_routed);

    let mut required: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pack in &routed_packs {
        if let Some(lane) = &pack.ci_lane {
            required
                .entry(lane.clone())
                .or_default()
                .push(pack.id.clone());
        }
    }

    // Advisory lanes: lanes that host a routed pack's advisory commands and
    // are not already required. Advisory stays visible without blocking.
    let mut advisory: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for lane in lanes {
        if required.contains_key(&lane.id) {
            continue;
        }
        let mut pack_ids = Vec::new();
        for pack in &routed_packs {
            if pack.advisory_commands.iter().any(|command| {
                lane.commands
                    .iter()
                    .any(|lane_command| lane_command == command)
            }) {
                pack_ids.push(pack.id.clone());
            }
        }
        if !pack_ids.is_empty() {
            advisory.insert(lane.id.clone(), pack_ids);
        }
    }

    let mut never_routed_packs_by_lane: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pack in packs.iter().filter(|pack| pack.never_routed) {
        if let Some(lane) = &pack.ci_lane {
            never_routed_packs_by_lane
                .entry(lane.clone())
                .or_default()
                .push(pack.id.clone());
        }
    }

    let mut required_lanes = Vec::new();
    let mut advisory_lanes = Vec::new();
    let mut skipped_lanes = Vec::new();
    let mut never_routed_lanes = Vec::new();
    let mut emitted = BTreeSet::new();
    for lane in lanes {
        emitted.insert(lane.id.clone());
        let estimated_cost = estimated_lane_cost(&lane.commands);
        if let Some(pack_ids) = required.get(&lane.id) {
            required_lanes.push(LaneRoute {
                id: lane.id.clone(),
                estimated_cost,
                packs: pack_ids.clone(),
            });
        } else if let Some(pack_ids) = advisory.get(&lane.id) {
            advisory_lanes.push(LaneRoute {
                id: lane.id.clone(),
                estimated_cost,
                packs: pack_ids.clone(),
            });
        } else if let Some(pack_ids) = never_routed_packs_by_lane.get(&lane.id) {
            never_routed_lanes.push(LaneRoute {
                id: lane.id.clone(),
                estimated_cost,
                packs: pack_ids.clone(),
            });
        } else {
            skipped_lanes.push(SkippedLane {
                id: lane.id.clone(),
                estimated_cost,
                reason: NO_MATCHED_SURFACE_REASON,
            });
        }
    }
    // A pack lane missing from the whitelist stays visible rather than being
    // dropped silently; `check-proof-packs` reports the manifest drift.
    for (lane_id, pack_ids) in &required {
        if !emitted.contains(lane_id) {
            required_lanes.push(LaneRoute {
                id: lane_id.clone(),
                estimated_cost: "light",
                packs: pack_ids.clone(),
            });
        }
    }

    ProofRoute {
        matched_packs,
        unmatched_files,
        full_proof,
        release_proof_required,
        required_lanes,
        advisory_lanes,
        skipped_lanes,
        never_routed_lanes,
    }
}

fn lane_routes_json(lanes: &[LaneRoute]) -> Value {
    Value::Array(
        lanes
            .iter()
            .map(|lane| {
                json!({
                    "id": lane.id,
                    "estimated_cost": lane.estimated_cost,
                    "packs": lane.packs,
                })
            })
            .collect(),
    )
}

fn proof_route_json(
    options: &ProofRouteOptions,
    base_sha: &str,
    head_sha: &str,
    changed_files: &[String],
    route: &ProofRoute,
) -> Value {
    let matched_packs: Vec<Value> = route
        .matched_packs
        .iter()
        .map(|pack| {
            json!({
                "id": pack.id,
                "ci_lane": pack.ci_lane,
                "matched_files": pack.matched_files,
                "required_commands": pack.required_commands,
                "advisory_commands": pack.advisory_commands,
            })
        })
        .collect();
    let skipped_lanes: Vec<Value> = route
        .skipped_lanes
        .iter()
        .map(|lane| {
            json!({
                "id": lane.id,
                "reason": lane.reason,
                "estimated_cost": lane.estimated_cost,
            })
        })
        .collect();
    json!({
        "schema_version": PROOF_ROUTE_SCHEMA_VERSION,
        "routing_state": PROOF_ROUTE_ROUTING_STATE,
        "base": {"rev": options.base, "sha": base_sha},
        "head": {"rev": options.head, "sha": head_sha},
        "changed_files": changed_files,
        "matched_packs": matched_packs,
        "unmatched_files": route.unmatched_files,
        "full_proof": route.full_proof,
        "full_proof_reason": if route.full_proof {
            Value::String(UNKNOWN_SURFACE_REASON.to_string())
        } else {
            Value::Null
        },
        "release_proof_required": route.release_proof_required,
        "required_lanes": lane_routes_json(&route.required_lanes),
        "advisory_lanes": lane_routes_json(&route.advisory_lanes),
        "skipped_lanes": skipped_lanes,
        "never_routed_lanes": lane_routes_json(&route.never_routed_lanes),
    })
}

fn backticked_list(values: &[String]) -> String {
    if values.is_empty() {
        return "(none)".to_string();
    }
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn proof_route_markdown(
    options: &ProofRouteOptions,
    base_sha: &str,
    head_sha: &str,
    changed_files: &[String],
    route: &ProofRoute,
) -> String {
    let mut body = String::from("# ripr proof route report\n\n");
    body.push_str("Status: advisory\n");
    body.push_str(&format!("Routing state: {PROOF_ROUTE_ROUTING_STATE}\n"));
    body.push_str(&format!(
        "Release proof required: {}\n\n",
        route.release_proof_required
    ));
    body.push_str(
        "This report maps changed files onto the proof packs in `policy/proof-packs.toml` and \
         the CI lanes in `policy/ci-lane-whitelist.toml`. It is read-only evidence: no proof \
         command is executed and no CI behavior changes because of this report.\n\n",
    );
    body.push_str("Range:\n\n");
    body.push_str(&format!("- base: `{}` (`{base_sha}`)\n", options.base));
    body.push_str(&format!("- head: `{}` (`{head_sha}`)\n\n", options.head));

    body.push_str(&format!("## Changed files ({})\n\n", changed_files.len()));
    if changed_files.is_empty() {
        body.push_str("No files changed between base and head.\n\n");
    } else {
        for file in changed_files {
            body.push_str(&format!("- `{file}`\n"));
        }
        body.push('\n');
    }

    body.push_str("## Matched packs\n\n");
    if route.matched_packs.is_empty() {
        body.push_str("No proof pack matched the changed files.\n\n");
    } else {
        body.push_str(
            "| Pack | CI lane | Matched files | Required commands | Advisory commands |\n",
        );
        body.push_str("| --- | --- | --- | --- | --- |\n");
        for pack in &route.matched_packs {
            body.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                pack.id,
                pack.ci_lane.as_deref().unwrap_or("(missing)"),
                backticked_list(&pack.matched_files),
                backticked_list(&pack.required_commands),
                backticked_list(&pack.advisory_commands),
            ));
        }
        body.push('\n');
    }

    body.push_str("## Unmatched files\n\n");
    if route.unmatched_files.is_empty() {
        body.push_str("All changed files matched at least one proof pack.\n\n");
    } else {
        for file in &route.unmatched_files {
            body.push_str(&format!("- `{file}`\n"));
        }
        body.push_str(&format!(
            "\nUnknown surfaces route to the full proof (`{UNKNOWN_SURFACE_REASON}`): every \
             pack's lane is required for this change.\n\n",
        ));
    }

    body.push_str("## Lane routing\n\n");
    body.push_str(&format!(
        "Routing state is `{PROOF_ROUTE_ROUTING_STATE}`: skipped entries below are recorded \
         reasons only, every CI lane still runs as configured, and no CI behavior is changed \
         by this report.\n\n",
    ));
    body.push_str("| Lane | Decision | Estimated cost | Detail |\n");
    body.push_str("| --- | --- | --- | --- |\n");
    for lane in &route.required_lanes {
        let detail = if route.full_proof {
            format!("{UNKNOWN_SURFACE_REASON}; packs: {}", lane.packs.join(", "))
        } else {
            format!("packs: {}", lane.packs.join(", "))
        };
        body.push_str(&format!(
            "| {} | required | {} | {detail} |\n",
            lane.id, lane.estimated_cost
        ));
    }
    for lane in &route.advisory_lanes {
        body.push_str(&format!(
            "| {} | advisory | {} | packs: {} |\n",
            lane.id,
            lane.estimated_cost,
            lane.packs.join(", ")
        ));
    }
    for lane in &route.never_routed_lanes {
        body.push_str(&format!(
            "| {} | never-routed | {} | {NEVER_ROUTED_REASON}; release proof is never routed away (packs: {}) |\n",
            lane.id,
            lane.estimated_cost,
            lane.packs.join(", ")
        ));
    }
    for lane in &route.skipped_lanes {
        body.push_str(&format!(
            "| {} | skipped | {} | {} |\n",
            lane.id, lane.estimated_cost, lane.reason
        ));
    }
    body.push('\n');

    body.push_str(&format!(
        "Rerun: `cargo xtask proof route --base {} --head {}`\n",
        options.base, options.head
    ));
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::proof_packs::parse_proof_packs;
    use std::path::{Path, PathBuf};

    fn repo_root() -> Result<PathBuf, String> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
            format!(
                "failed to resolve repo root from {}",
                manifest_dir.display()
            )
        })
    }

    fn read_repo_file(relative: &str) -> Result<String, String> {
        let path = repo_root()?.join(relative);
        std::fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))
    }

    fn manifest_packs() -> Result<Vec<ProofPack>, String> {
        let text = read_repo_file("policy/proof-packs.toml")?;
        let (document, mut violations) =
            crate::parse_ci_ledger_document(crate::PROOF_PACK_MANIFEST_PATH, &text);
        let packs = parse_proof_packs(&document, &mut violations);
        if violations.is_empty() {
            Ok(packs)
        } else {
            Err(format!(
                "manifest fixture should parse cleanly: {violations:?}"
            ))
        }
    }

    fn whitelist_lanes() -> Result<Vec<CiLane>, String> {
        let text = read_repo_file("policy/ci-lane-whitelist.toml")?;
        let (document, mut violations) =
            crate::parse_ci_ledger_document(crate::PROOF_PACK_LANE_WHITELIST_PATH, &text);
        let lanes = parse_ci_lanes(&document, &mut violations);
        if violations.is_empty() {
            Ok(lanes)
        } else {
            Err(format!(
                "lane whitelist fixture should parse cleanly: {violations:?}"
            ))
        }
    }

    fn route_for(files: &[&str]) -> Result<ProofRoute, String> {
        let packs = manifest_packs()?;
        let lanes = whitelist_lanes()?;
        let changed: Vec<String> = files.iter().map(|file| (*file).to_string()).collect();
        Ok(route_proof(&packs, &lanes, &changed))
    }

    fn matched_pack_ids(route: &ProofRoute) -> Vec<&str> {
        route
            .matched_packs
            .iter()
            .map(|pack| pack.id.as_str())
            .collect()
    }

    fn required_lane_ids(route: &ProofRoute) -> Vec<&str> {
        route
            .required_lanes
            .iter()
            .map(|lane| lane.id.as_str())
            .collect()
    }

    fn skipped_lane_ids(route: &ProofRoute) -> Vec<&str> {
        route
            .skipped_lanes
            .iter()
            .map(|lane| lane.id.as_str())
            .collect()
    }

    #[test]
    fn docs_only_diff_routes_to_docs_spec_pack_only() -> Result<(), String> {
        let route = route_for(&["docs/specs/SPEC-0001-example.md"])?;
        assert_eq!(matched_pack_ids(&route), vec!["docs-spec"]);
        assert_eq!(required_lane_ids(&route), vec!["docs"]);
        assert!(!route.full_proof);
        assert!(!route.release_proof_required);
        assert!(route.unmatched_files.is_empty());
        // The release lane is never listed as skipped.
        assert!(!skipped_lane_ids(&route).contains(&"release-readiness-proof"));
        assert!(
            route
                .never_routed_lanes
                .iter()
                .any(|lane| lane.id == "release-readiness-proof")
        );
        Ok(())
    }

    #[test]
    fn output_diff_routes_to_output_contracts_pack() -> Result<(), String> {
        let route = route_for(&["crates/ripr/src/output/human.rs"])?;
        let matched = matched_pack_ids(&route);
        assert!(
            matched.contains(&"output-contracts"),
            "matched: {matched:?}"
        );
        // The manifest places src/output in static-language too; a surface in
        // more than one pack runs the union of the matched packs.
        assert!(matched.contains(&"static-language"), "matched: {matched:?}");
        let required = required_lane_ids(&route);
        assert!(
            required.contains(&"output-contracts"),
            "required: {required:?}"
        );
        assert!(
            required.contains(&"static-language"),
            "required: {required:?}"
        );
        assert!(!route.full_proof);
        Ok(())
    }

    #[test]
    fn mixed_diff_routes_to_the_union_of_matched_packs() -> Result<(), String> {
        let route = route_for(&[
            "docs/specs/SPEC-0001-example.md",
            "crates/ripr/src/output/json.rs",
        ])?;
        let matched = matched_pack_ids(&route);
        for expected in ["docs-spec", "static-language", "output-contracts"] {
            assert!(matched.contains(&expected), "matched: {matched:?}");
        }
        let required = required_lane_ids(&route);
        for expected in ["docs", "static-language", "output-contracts"] {
            assert!(required.contains(&expected), "required: {required:?}");
        }
        assert!(!route.full_proof);
        Ok(())
    }

    #[test]
    fn unknown_surface_routes_to_full_proof() -> Result<(), String> {
        let route = route_for(&["scripts/unknown-surface.bin"])?;
        assert!(route.full_proof);
        assert_eq!(route.unmatched_files, vec!["scripts/unknown-surface.bin"]);
        assert!(route.matched_packs.is_empty());
        // Full proof requires every pack's lane, including release proof.
        let required = required_lane_ids(&route);
        for expected in [
            "docs",
            "static-language",
            "output-contracts",
            "traceability",
            "routed-rust-small",
            "vscode-e2e",
            "release-readiness-proof",
        ] {
            assert!(required.contains(&expected), "required: {required:?}");
        }
        assert!(route.release_proof_required);
        assert!(!skipped_lane_ids(&route).contains(&"release-readiness-proof"));
        Ok(())
    }

    #[test]
    fn xtask_diff_routes_to_xtask_report_pack() -> Result<(), String> {
        let route = route_for(&["xtask/src/reports/proof_route.rs"])?;
        assert_eq!(matched_pack_ids(&route), vec!["xtask-report"]);
        assert_eq!(required_lane_ids(&route), vec!["routed-rust-small"]);
        assert!(!route.full_proof);
        assert!(!route.release_proof_required);
        Ok(())
    }

    #[test]
    fn analysis_diff_routes_to_analysis_fixture_pack() -> Result<(), String> {
        let route = route_for(&["crates/ripr/src/analysis/probes.rs"])?;
        assert_eq!(matched_pack_ids(&route), vec!["analysis-fixture"]);
        assert_eq!(required_lane_ids(&route), vec!["routed-rust-small"]);
        assert!(!route.full_proof);
        assert!(!route.release_proof_required);
        Ok(())
    }

    #[test]
    fn release_file_routes_release_pack_and_is_never_skipped() -> Result<(), String> {
        let route = route_for(&["Cargo.toml"])?;
        assert_eq!(matched_pack_ids(&route), vec!["release-package"]);
        assert_eq!(required_lane_ids(&route), vec!["release-readiness-proof"]);
        assert!(!route.full_proof);
        assert!(route.release_proof_required);
        assert!(!skipped_lane_ids(&route).contains(&"release-readiness-proof"));
        assert!(route.never_routed_lanes.is_empty());
        Ok(())
    }

    #[test]
    fn changelog_routes_release_pack_and_is_never_skipped() -> Result<(), String> {
        let route = route_for(&["CHANGELOG.md"])?;
        let matched = matched_pack_ids(&route);
        // CHANGELOG.md is markdown and a release surface: docs-spec and
        // release-package both match and run as a union.
        assert!(matched.contains(&"release-package"), "matched: {matched:?}");
        assert!(matched.contains(&"docs-spec"), "matched: {matched:?}");
        assert!(route.release_proof_required);
        let required = required_lane_ids(&route);
        assert!(
            required.contains(&"release-readiness-proof"),
            "required: {required:?}"
        );
        assert!(!skipped_lane_ids(&route).contains(&"release-readiness-proof"));
        Ok(())
    }

    #[test]
    fn json_and_markdown_agree_on_lane_decisions() -> Result<(), String> {
        let route = route_for(&["docs/specs/SPEC-0001-example.md", "CHANGELOG.md"])?;
        let options = ProofRouteOptions::default();
        let changed = vec![
            "docs/specs/SPEC-0001-example.md".to_string(),
            "CHANGELOG.md".to_string(),
        ];
        let json_value = proof_route_json(&options, "base-sha", "head-sha", &changed, &route);
        let markdown = proof_route_markdown(&options, "base-sha", "head-sha", &changed, &route);

        // Both renderers derive from the same ProofRoute; every lane decision
        // in the JSON must appear with the same decision label in the MD.
        let lane_sets: [(&str, &str); 4] = [
            ("required_lanes", "required"),
            ("advisory_lanes", "advisory"),
            ("never_routed_lanes", "never-routed"),
            ("skipped_lanes", "skipped"),
        ];
        for (key, decision) in lane_sets {
            let lanes = json_value
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| format!("proof-route json should carry `{key}`"))?;
            for lane in lanes {
                let id = lane
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("`{key}` entries should carry an id"))?;
                let row = format!("| {id} | {decision} |");
                if !markdown.contains(&row) {
                    return Err(format!("markdown should contain row `{row}`"));
                }
            }
        }
        assert_eq!(
            json_value
                .get("release_proof_required")
                .and_then(Value::as_bool),
            Some(route.release_proof_required)
        );
        assert!(markdown.contains(&format!(
            "Release proof required: {}",
            route.release_proof_required
        )));
        Ok(())
    }

    #[test]
    fn pack_path_globs_match_expected_shapes() -> Result<(), String> {
        // `**/*.md` matches markdown at the repo root and nested.
        assert!(crate::glob_matches("**/*.md", "README.md"));
        assert!(crate::glob_matches("**/*.md", "docs/learnings/notes.md"));
        assert!(!crate::glob_matches("**/*.md", "docs/notes.txt"));
        // `docs/specs/**` matches nested spec files.
        assert!(crate::glob_matches("docs/specs/**", "docs/specs/a/b.md"));
        assert!(!crate::glob_matches("docs/specs/**", "docs/SPECS.md"));
        // Exact paths match only themselves.
        assert!(crate::glob_matches("Cargo.toml", "Cargo.toml"));
        assert!(!crate::glob_matches("Cargo.toml", "crates/ripr/Cargo.toml"));
        Ok(())
    }

    #[test]
    fn estimated_lane_cost_uses_the_static_command_map() {
        let heavy = vec!["cargo test --workspace".to_string()];
        let heavy_coverage = vec![
            "cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info".to_string(),
        ];
        let medium = vec!["npm run compile".to_string()];
        let medium_vsce = vec!["npx @vscode/vsce package".to_string()];
        let light = vec!["cargo xtask check-doc-index".to_string()];
        let empty: Vec<String> = Vec::new();
        assert_eq!(estimated_lane_cost(&heavy), "heavy");
        assert_eq!(estimated_lane_cost(&heavy_coverage), "heavy");
        assert_eq!(estimated_lane_cost(&medium), "medium");
        assert_eq!(estimated_lane_cost(&medium_vsce), "medium");
        assert_eq!(estimated_lane_cost(&light), "light");
        assert_eq!(estimated_lane_cost(&empty), "light");
    }

    #[test]
    fn report_outputs_carry_the_routing_contract() -> Result<(), String> {
        let route = route_for(&["docs/specs/SPEC-0001-example.md"])?;
        let options = ProofRouteOptions::default();
        let json_value = proof_route_json(
            &options,
            "base-sha",
            "head-sha",
            &["docs/specs/SPEC-0001-example.md".to_string()],
            &route,
        );
        assert_eq!(
            json_value.get("schema_version").and_then(Value::as_str),
            Some(PROOF_ROUTE_SCHEMA_VERSION)
        );
        assert_eq!(
            json_value.get("routing_state").and_then(Value::as_str),
            Some(PROOF_ROUTE_ROUTING_STATE)
        );
        assert_eq!(
            json_value.get("full_proof").and_then(Value::as_bool),
            Some(false)
        );
        let markdown = proof_route_markdown(
            &options,
            "base-sha",
            "head-sha",
            &["docs/specs/SPEC-0001-example.md".to_string()],
            &route,
        );
        assert!(markdown.contains(PROOF_ROUTE_ROUTING_STATE));
        assert!(markdown.contains("| docs | required |"));
        assert!(markdown.contains("never-routed"));
        Ok(())
    }

    #[test]
    fn route_options_parse_defaults_and_reject_unknown_args() -> Result<(), String> {
        let parsed = parse_route_options(&[])?;
        assert_eq!(parsed.base, DEFAULT_BASE);
        assert_eq!(parsed.head, DEFAULT_HEAD);
        let parsed = parse_route_options(&[
            "--base".to_string(),
            "HEAD~1".to_string(),
            "--head".to_string(),
            "HEAD".to_string(),
        ])?;
        assert_eq!(parsed.base, "HEAD~1");
        assert_eq!(parsed.head, "HEAD");
        for args in [
            vec!["--what".to_string()],
            vec!["--base".to_string()],
            vec!["--base".to_string(), " ".to_string()],
        ] {
            if parse_route_options(&args).is_ok() {
                return Err(format!("expected parse failure for {args:?}"));
            }
        }
        Ok(())
    }
}

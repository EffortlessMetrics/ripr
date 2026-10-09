//! Shared owner for the blind-journey installed-language fixture contracts:
//! RIPR-SPEC-0207 (installed Rust, #4516), RIPR-SPEC-0209 (installed Python,
//! #4518) and RIPR-SPEC-0210 (installed TypeScript, #4519).
//!
//! Extracted as a behavior-preserving parameterization of the previously
//! copied per-language validators in `general_validators.rs`: every violation
//! message, every fail-closed rule (SHA-256 rebinding, on-disk enumeration,
//! scenario-to-snapshot binding, fixture-root path confinement) and every
//! check ordering is byte-identical to the per-language implementations this
//! module replaces. The language label, fixture root, spec decision and
//! schema version are data, and the two parameterized rule differences — the
//! TypeScript designated-snapshot head binding and the Python empty-inventory
//! rejection plus environment-binding contract — are explicit contract
//! fields pinned by the tests at the bottom of this file and by the
//! unchanged per-language test modules in `general_validators.rs`.

use super::*;

pub(crate) const INSTALLED_RUST_FIXTURE_SCHEMA_VERSION: &str =
    "blind_journey_installed_rust_fixture.v1";
pub(crate) const INSTALLED_TYPESCRIPT_FIXTURE_SCHEMA_VERSION: &str =
    "blind_journey_installed_typescript_fixture.v1";
pub(crate) const INSTALLED_PYTHON_FIXTURE_SCHEMA_VERSION: &str =
    "blind_journey_installed_python_fixture.v1";

/// How a scripted scenario's candidate head/tree identity must bind the
/// recorded manifest snapshots. TypeScript names a designated snapshot per
/// scenario because its journey rows bind distinct variant snapshots; Rust
/// and Python accept any recorded snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstalledJourneyHeadBinding {
    AnySnapshot,
    DesignatedSnapshot,
}

/// One installed-language blind-journey fixture contract. Every field is
/// behavior, not convenience: the gate renders its violation messages and
/// fail-closed rules from these values, so a wrong label, root or schema
/// version is itself a contract violation the committed-fixture tests catch.
pub(crate) struct InstalledJourneyFixtureContract {
    /// Language word rendered inside every violation message, e.g. `rust`.
    pub(crate) label: &'static str,
    /// Fixture directory this contract owns, relative to the workspace root.
    pub(crate) fixture_root: &'static str,
    /// Spec decision the fixture SPEC.md must name.
    pub(crate) spec_decision: &'static str,
    /// Manifest schema_version this fixture binds.
    pub(crate) schema_version: &'static str,
    /// How scenario candidate head/tree identities must bind the snapshots.
    pub(crate) head_binding: InstalledJourneyHeadBinding,
    /// Whether an empty journey_scenario_ids inventory fails closed.
    pub(crate) reject_empty_scenario_inventory: bool,
    /// Language-specific manifest checks appended after the shared snapshot
    /// and edit-cage rules; languages without extra rules pass a no-op.
    pub(crate) extra_manifest_violations: fn(&Value) -> Vec<String>,
}

fn no_extra_manifest_violations(_manifest: &Value) -> Vec<String> {
    Vec::new()
}

pub(crate) const INSTALLED_RUST_CONTRACT: InstalledJourneyFixtureContract =
    InstalledJourneyFixtureContract {
        label: "rust",
        fixture_root: "fixtures/blind_journey_installed_rust",
        spec_decision: "RIPR-SPEC-0207",
        schema_version: INSTALLED_RUST_FIXTURE_SCHEMA_VERSION,
        head_binding: InstalledJourneyHeadBinding::AnySnapshot,
        reject_empty_scenario_inventory: false,
        extra_manifest_violations: no_extra_manifest_violations,
    };

pub(crate) const INSTALLED_TYPESCRIPT_CONTRACT: InstalledJourneyFixtureContract =
    InstalledJourneyFixtureContract {
        label: "typescript",
        fixture_root: "fixtures/blind_journey_installed_typescript",
        spec_decision: "RIPR-SPEC-0210",
        schema_version: INSTALLED_TYPESCRIPT_FIXTURE_SCHEMA_VERSION,
        head_binding: InstalledJourneyHeadBinding::DesignatedSnapshot,
        reject_empty_scenario_inventory: false,
        extra_manifest_violations: no_extra_manifest_violations,
    };

pub(crate) const INSTALLED_PYTHON_CONTRACT: InstalledJourneyFixtureContract =
    InstalledJourneyFixtureContract {
        label: "python",
        fixture_root: "fixtures/blind_journey_installed_python",
        spec_decision: "RIPR-SPEC-0209",
        schema_version: INSTALLED_PYTHON_FIXTURE_SCHEMA_VERSION,
        head_binding: InstalledJourneyHeadBinding::AnySnapshot,
        reject_empty_scenario_inventory: true,
        extra_manifest_violations: installed_python_environment_binding_violations,
    };

fn installed_journey_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// One recorded git identity is a lowercase 40-hex SHA-1 object id; anything
/// else cannot be the reproducible commit or tree identity the fixture claims.
pub(crate) fn installed_journey_git_identity_wellformed(value: Option<&Value>) -> bool {
    value.and_then(Value::as_str).is_some_and(|identity| {
        identity.len() == 40
            && identity
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

/// Every snapshot file binding recomputed against the bytes on disk, plus the
/// edit-cage invariants the scripted journeys rely on, plus the contract's
/// language-specific manifest rules.
pub(crate) fn installed_journey_manifest_violations(
    contract: &InstalledJourneyFixtureContract,
    root: &Path,
    manifest: &Value,
) -> Vec<String> {
    let mut violations = Vec::new();
    let fixture_root = match root.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            violations.push(format!(
                "blind journey installed {} fixture root cannot be resolved: {error}",
                contract.label
            ));
            return violations;
        }
    };
    if json_string_field(manifest, "schema_version").as_deref() != Some(contract.schema_version) {
        violations.push(format!(
            "blind journey installed {} manifest schema_version must be {}",
            contract.label, contract.schema_version
        ));
    }
    let Some(snapshots) = manifest
        .get("repository")
        .and_then(|repository| repository.get("snapshots"))
        .and_then(Value::as_object)
    else {
        violations.push(format!(
            "blind journey installed {} manifest is missing repository.snapshots",
            contract.label
        ));
        return violations;
    };
    for (name, snapshot) in snapshots {
        let snapshot_dir = root.join("repository").join(name);
        let confined_dir = match snapshot_dir.canonicalize() {
            Ok(resolved) if resolved.starts_with(&fixture_root) => resolved,
            Ok(_) => {
                violations.push(format!(
                    "blind journey installed {} snapshot `{name}` escapes the fixture root",
                    contract.label
                ));
                continue;
            }
            Err(error) => {
                violations.push(format!(
                    "blind journey installed {} snapshot `{name}` directory cannot be \
                     resolved: {error}",
                    contract.label
                ));
                continue;
            }
        };
        if !installed_journey_git_identity_wellformed(snapshot.get("commit"))
            || !installed_journey_git_identity_wellformed(snapshot.get("tree"))
        {
            violations.push(format!(
                "blind journey installed {} snapshot `{name}` must record well-formed commit \
                 and tree identities",
                contract.label
            ));
        }
        let Some(files) = snapshot.get("files").and_then(Value::as_object) else {
            violations.push(format!(
                "blind journey installed {} snapshot `{name}` is missing its file digest \
                 bindings",
                contract.label
            ));
            continue;
        };
        for (relative, digest) in files {
            let Some(expected) = digest.as_str() else {
                violations.push(format!(
                    "blind journey installed {} snapshot `{name}` file `{relative}` records a \
                     non-string digest",
                    contract.label
                ));
                continue;
            };
            let path = confined_dir.join(relative);
            let path = match path.canonicalize() {
                Ok(resolved) if resolved.starts_with(&fixture_root) => resolved,
                Ok(_) => {
                    violations.push(format!(
                        "blind journey installed {} snapshot `{name}` file `{relative}` escapes \
                         the fixture root",
                        contract.label
                    ));
                    continue;
                }
                Err(error) => {
                    violations.push(format!(
                        "blind journey installed {} snapshot `{name}` file `{relative}` cannot \
                         be read: {error}",
                        contract.label
                    ));
                    continue;
                }
            };
            let body = match std::fs::read(&path) {
                Ok(body) => body,
                Err(error) => {
                    violations.push(format!(
                        "blind journey installed {} snapshot `{name}` file `{relative}` cannot \
                         be read: {error}",
                        contract.label
                    ));
                    continue;
                }
            };
            if installed_journey_sha256_hex(&body) != expected {
                violations.push(format!(
                    "blind journey installed {} snapshot `{name}` file `{relative}` drifted \
                     from its recorded digest",
                    contract.label
                ));
            }
        }
        let mut on_disk = Vec::new();
        installed_journey_collect_snapshot_files(
            contract,
            &confined_dir,
            Path::new(""),
            &mut on_disk,
            &mut violations,
            name,
        );
        for relative in on_disk {
            if !files.contains_key(relative.as_str()) {
                violations.push(format!(
                    "blind journey installed {} snapshot `{name}` file `{relative}` is not \
                     bound in the manifest",
                    contract.label
                ));
            }
        }
    }
    let journey = manifest.get("journey");
    let string_list = |key: &str| {
        journey
            .and_then(|journey| journey.get(key))
            .and_then(Value::as_array)
            .map(|entries| entries.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let cage = string_list("expected_edit_cage");
    let forbidden = string_list("forbidden_edits");
    let edit_target = journey
        .and_then(|journey| journey.get("selected_repair"))
        .and_then(|repair| repair.get("edit_target"))
        .and_then(Value::as_str);
    match edit_target {
        Some(target) if cage.contains(&target) => {}
        Some(_) => {
            violations.push(format!(
                "blind journey installed {} selected repair edit target must lie inside the \
                 expected edit cage",
                contract.label
            ));
        }
        None => {
            violations.push(format!(
                "blind journey installed {} manifest is missing \
                 journey.selected_repair.edit_target",
                contract.label
            ));
        }
    }
    if cage.iter().any(|entry| forbidden.contains(entry)) {
        violations.push(format!(
            "blind journey installed {} expected edit cage and forbidden edits must not \
             overlap",
            contract.label
        ));
    }
    violations.extend((contract.extra_manifest_violations)(manifest));
    violations
}

/// Fail-closed inventory of one retained snapshot directory: every regular
/// file is reported with its manifest-relative `/`-separated path so a
/// behavior-affecting snapshot file cannot stay unbound from the manifest.
fn installed_journey_collect_snapshot_files(
    contract: &InstalledJourneyFixtureContract,
    dir: &Path,
    prefix: &Path,
    files: &mut Vec<String>,
    violations: &mut Vec<String>,
    snapshot: &str,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            violations.push(format!(
                "blind journey installed {} snapshot `{snapshot}` directory {} cannot be \
                 read: {error}",
                contract.label,
                normalize_path(dir)
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                violations.push(format!(
                    "blind journey installed {} snapshot `{snapshot}` directory {} cannot be \
                     enumerated: {error}",
                    contract.label,
                    normalize_path(dir)
                ));
                continue;
            }
        };
        let relative = prefix.join(entry.file_name());
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                violations.push(format!(
                    "blind journey installed {} snapshot `{snapshot}` path {} cannot be \
                     inspected: {error}",
                    contract.label,
                    normalize_path(&relative)
                ));
                continue;
            }
        };
        if file_type.is_dir() {
            installed_journey_collect_snapshot_files(
                contract,
                &entry.path(),
                &relative,
                files,
                violations,
                snapshot,
            );
        } else if file_type.is_file() {
            files.push(normalize_path(&relative));
        } else {
            violations.push(format!(
                "blind journey installed {} snapshot `{snapshot}` path {} is not a regular \
                 file, so it is not digest-bound",
                contract.label,
                normalize_path(&relative)
            ));
        }
    }
}

/// Every scripted scenario the manifest names must bind its recorded
/// candidate identities to the same manifest snapshots, so a renamed row
/// cannot claim an unrelated repository while this gate stays green.
pub(crate) fn installed_journey_scenario_binding_violations(
    contract: &InstalledJourneyFixtureContract,
    manifest: &Value,
    corpus: &Value,
) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(ids) = manifest
        .get("journey_scenario_ids")
        .and_then(Value::as_array)
    else {
        return violations;
    };
    let named: BTreeSet<&str> = ids.iter().filter_map(Value::as_str).collect();
    if named.is_empty() {
        return violations;
    }
    let Some(snapshots) = manifest
        .get("repository")
        .and_then(|repository| repository.get("snapshots"))
        .and_then(Value::as_object)
    else {
        return violations;
    };
    let Some(base_commit) = snapshots
        .get("base")
        .and_then(|snapshot| snapshot.get("commit"))
        .and_then(Value::as_str)
    else {
        violations.push(format!(
            "blind journey installed {} manifest is missing the base snapshot commit",
            contract.label
        ));
        return violations;
    };
    let Some(scenarios) = corpus.get("scenarios").and_then(Value::as_array) else {
        return violations;
    };
    for scenario in scenarios {
        let Some(id) = scenario.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !named.contains(id) {
            continue;
        }
        let candidate = scenario
            .get("journey")
            .and_then(|journey| journey.get("candidate"));
        let candidate_base = candidate
            .and_then(|candidate| candidate.get("base"))
            .and_then(Value::as_str);
        if candidate_base != Some(base_commit) {
            violations.push(format!(
                "blind journey installed {} scenario `{id}` candidate base must bind the \
                 base snapshot commit",
                contract.label
            ));
        }
        let head = candidate
            .and_then(|candidate| candidate.get("head"))
            .and_then(Value::as_str);
        let tree = candidate
            .and_then(|candidate| candidate.get("tree"))
            .and_then(Value::as_str);
        match contract.head_binding {
            InstalledJourneyHeadBinding::AnySnapshot => {
                let head_binds_a_snapshot = snapshots.values().any(|snapshot| {
                    snapshot.get("commit").and_then(Value::as_str) == head
                        && snapshot.get("tree").and_then(Value::as_str) == tree
                });
                if !head_binds_a_snapshot {
                    violations.push(format!(
                        "blind journey installed {} scenario `{id}` candidate head and tree \
                         must bind one manifest snapshot",
                        contract.label
                    ));
                }
            }
            InstalledJourneyHeadBinding::DesignatedSnapshot => {
                let bindings = manifest
                    .get("scenario_snapshot_bindings")
                    .and_then(Value::as_object);
                let Some(designated) = bindings
                    .and_then(|bindings| bindings.get(id))
                    .and_then(Value::as_str)
                else {
                    violations.push(format!(
                        "blind journey installed {} scenario `{id}` must name its designated \
                         snapshot in scenario_snapshot_bindings",
                        contract.label
                    ));
                    continue;
                };
                if designated == "base" {
                    violations.push(format!(
                        "blind journey installed {} scenario `{id}` must not designate the \
                         unchanged base snapshot as its candidate head",
                        contract.label
                    ));
                    continue;
                }
                let Some(snapshot) = snapshots.get(designated) else {
                    violations.push(format!(
                        "blind journey installed {} scenario `{id}` designates unknown \
                         snapshot `{designated}`",
                        contract.label
                    ));
                    continue;
                };
                let head_binds_designated = snapshot.get("commit").and_then(Value::as_str) == head
                    && snapshot.get("tree").and_then(Value::as_str) == tree;
                if !head_binds_designated {
                    violations.push(format!(
                        "blind journey installed {} scenario `{id}` candidate head and tree \
                         must bind its designated snapshot `{designated}`",
                        contract.label
                    ));
                }
            }
        }
    }
    violations
}

/// Every scripted scenario the manifest names must exist in the committed
/// RIPR-SPEC-0205 executor corpus, so the fixture cannot claim journey rows
/// the gate does not execute.
pub(crate) fn installed_journey_missing_scenarios(
    contract: &InstalledJourneyFixtureContract,
    manifest: &Value,
    corpus: &Value,
) -> Vec<String> {
    let mut missing = Vec::new();
    let Some(ids) = manifest
        .get("journey_scenario_ids")
        .and_then(Value::as_array)
    else {
        missing.push(format!(
            "blind journey installed {} manifest is missing journey_scenario_ids",
            contract.label
        ));
        return missing;
    };
    if contract.reject_empty_scenario_inventory && ids.is_empty() {
        missing.push(format!(
            "blind journey installed {} journey_scenario_ids must name at least one scenario",
            contract.label
        ));
        return missing;
    }
    let corpus_ids: BTreeSet<String> = corpus
        .get("scenarios")
        .and_then(Value::as_array)
        .map(|scenarios| {
            scenarios
                .iter()
                .filter_map(|scenario| json_string_field(scenario, "id"))
                .collect()
        })
        .unwrap_or_default();
    for id in ids {
        let Some(id) = id.as_str() else {
            missing.push(format!(
                "blind journey installed {} journey_scenario_ids must be strings",
                contract.label
            ));
            continue;
        };
        if !corpus_ids.contains(id) {
            missing.push(format!(
                "blind journey installed {} scenario `{id}` is missing from the blind \
                 journey execute corpus",
                contract.label
            ));
        }
    }
    missing
}

/// The shared outer `check-fixture-contracts` owner for one installed-language
/// fixture: fixture presence, the SPEC.md decision and section headings,
/// manifest validation against the bytes on disk, and scenario existence and
/// snapshot binding against the RIPR-SPEC-0205 executor corpus.
pub(crate) fn validate_installed_journey_fixture(
    contract: &InstalledJourneyFixtureContract,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new(contract.fixture_root);
    for required in ["SPEC.md", "manifest.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "blind journey installed {} fixture is missing {}",
                contract.label,
                normalize_path(&path)
            ));
        }
    }
    let spec_path = root.join("SPEC.md");
    if spec_path.exists() {
        let body = read_text_lossy(&spec_path)?;
        if !body.contains(contract.spec_decision) {
            violations.push(format!(
                "blind journey installed {} SPEC.md must name its {} decision",
                contract.label, contract.spec_decision
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !body.contains(heading) {
                violations.push(format!(
                    "blind journey installed {} SPEC.md must contain the `{heading}` section",
                    contract.label
                ));
            }
        }
    }
    let manifest_path = root.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(());
    }
    let manifest = match read_json_value(&manifest_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(format!(
                "blind journey installed {} manifest is invalid: {err}",
                contract.label
            ));
            return Ok(());
        }
    };
    for violation in installed_journey_manifest_violations(contract, root, &manifest) {
        violations.push(violation);
    }
    let corpus_path = Path::new("fixtures/blind_journey_execute/corpus.json");
    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    for missing in installed_journey_missing_scenarios(contract, &manifest, &corpus) {
        violations.push(missing);
    }
    for violation in installed_journey_scenario_binding_violations(contract, &manifest, &corpus) {
        violations.push(violation);
    }
    Ok(())
}

/// Environment-binding contract for the installed-Python journey
/// (RIPR-SPEC-0209): the focused verification command must be the documented
/// module form, and the bare form must stay a recorded historical artifact
/// at documented strength, never the recommendation.
pub(crate) fn installed_python_environment_binding_violations(manifest: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    let journey = manifest.get("journey");
    let binding = journey.and_then(|journey| journey.get("environment_binding"));
    let current = binding
        .and_then(|binding| binding.get("current_verify_command"))
        .and_then(Value::as_str);
    let historical = binding
        .and_then(|binding| binding.get("historical_bare_command"))
        .and_then(Value::as_str);
    let focused = journey
        .and_then(|journey| journey.get("selected_repair"))
        .and_then(|repair| repair.get("focused_verification_command"))
        .and_then(Value::as_str);
    match (current, focused) {
        (Some(current), Some(focused)) if current == focused => {}
        (Some(_), Some(_)) => {
            violations.push(
                "blind journey installed python focused verification command must equal the \
                 documented current module form"
                    .to_string(),
            );
        }
        _ => {
            violations.push(
                "blind journey installed python manifest must record both \
                 journey.environment_binding.current_verify_command and \
                 journey.selected_repair.focused_verification_command"
                    .to_string(),
            );
        }
    }
    match (current, historical) {
        (Some(current), Some(historical))
            if current.starts_with("python -m pytest ") && historical.starts_with("pytest ") =>
        {
            if historical == current {
                violations.push(
                    "blind journey installed python historical bare command must differ from \
                     the current module form"
                        .to_string(),
                );
            }
        }
        _ => {
            violations.push(
                "blind journey installed python manifest must record the module-form current \
                 command and the bare-form historical command"
                    .to_string(),
            );
        }
    }
    violations
}

#[cfg(test)]
mod installed_journey_contract_tests {
    use super::*;

    fn all_contracts() -> [&'static InstalledJourneyFixtureContract; 3] {
        [
            &INSTALLED_RUST_CONTRACT,
            &INSTALLED_TYPESCRIPT_CONTRACT,
            &INSTALLED_PYTHON_CONTRACT,
        ]
    }

    #[test]
    fn contract_table_binds_three_distinct_languages() -> Result<(), String> {
        let mut labels = BTreeSet::new();
        let mut schema_versions = BTreeSet::new();
        let mut roots = BTreeSet::new();
        for contract in all_contracts() {
            if !labels.insert(contract.label) {
                return Err(format!(
                    "duplicate contract label `{}` in the shared contract table",
                    contract.label
                ));
            }
            if !schema_versions.insert(contract.schema_version) {
                return Err(format!(
                    "duplicate schema version `{}` in the shared contract table",
                    contract.schema_version
                ));
            }
            if !roots.insert(contract.fixture_root) {
                return Err(format!(
                    "duplicate fixture root `{}` in the shared contract table",
                    contract.fixture_root
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn violation_messages_render_the_exact_committed_language_bytes() -> Result<(), String> {
        // Literal expectations pin the rendered bytes independently of the
        // contract table: a corrupted label must fail this test, not define
        // its own oracle.
        let manifest: Value = serde_json::json!({"schema_version": "not.a.real.schema"});
        let corpus: Value = serde_json::json!({"scenarios": []});
        let cases = [
            (
                &INSTALLED_RUST_CONTRACT,
                "blind journey installed rust manifest is missing journey_scenario_ids",
            ),
            (
                &INSTALLED_TYPESCRIPT_CONTRACT,
                "blind journey installed typescript manifest is missing journey_scenario_ids",
            ),
            (
                &INSTALLED_PYTHON_CONTRACT,
                "blind journey installed python manifest is missing journey_scenario_ids",
            ),
        ];
        for (contract, expected) in cases {
            let missing = installed_journey_missing_scenarios(contract, &manifest, &corpus);
            if missing.len() != 1 || missing[0] != expected {
                return Err(format!(
                    "the {} contract must render `{expected}` exactly, got: {missing:?}",
                    contract.label
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn typescript_contract_requires_the_designated_snapshot_binding() -> Result<(), String> {
        let manifest: Value = serde_json::json!({
            "repository": {"snapshots": {
                "base": {
                    "commit": "9e62e8091324eb65e214be59a45cd596a8192219",
                    "tree": "3b633ff09d7fca5e33f9ba5e15a5afe025c26b30"
                },
                "head": {
                    "commit": "7ee422a670ba383a600785d46eadc432d27bbfd2",
                    "tree": "453b32d284b9a120f92dbe254825f8d581e79369"
                }
            }},
            "journey_scenario_ids": ["journey_row"]
        });
        let corpus: Value = serde_json::json!({"scenarios": [{
            "id": "journey_row",
            "journey": {"candidate": {
                "base": "9e62e8091324eb65e214be59a45cd596a8192219",
                "head": "7ee422a670ba383a600785d46eadc432d27bbfd2",
                "tree": "453b32d284b9a120f92dbe254825f8d581e79369"
            }}
        }]});
        // AnySnapshot contracts keep accepting a candidate that binds any
        // recorded snapshot.
        for contract in [&INSTALLED_RUST_CONTRACT, &INSTALLED_PYTHON_CONTRACT] {
            let violations =
                installed_journey_scenario_binding_violations(contract, &manifest, &corpus);
            if !violations.is_empty() {
                return Err(format!(
                    "the {} contract must accept any-snapshot binding, got: {violations:?}",
                    contract.label
                ));
            }
        }
        // The TypeScript contract fails closed until the manifest names the
        // designated snapshot for the scenario.
        let typescript_violations = installed_journey_scenario_binding_violations(
            &INSTALLED_TYPESCRIPT_CONTRACT,
            &manifest,
            &corpus,
        );
        if !typescript_violations
            .iter()
            .any(|violation| violation.contains("designated"))
        {
            return Err(format!(
                "the typescript contract must require the designated snapshot binding, got: \
                 {typescript_violations:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn python_contract_rejects_an_empty_scenario_inventory() -> Result<(), String> {
        let manifest: Value = serde_json::json!({"journey_scenario_ids": []});
        let corpus: Value = serde_json::json!({"scenarios": []});
        let python_missing =
            installed_journey_missing_scenarios(&INSTALLED_PYTHON_CONTRACT, &manifest, &corpus);
        if !python_missing
            .iter()
            .any(|violation| violation.contains("must name at least one scenario"))
        {
            return Err(format!(
                "the python contract must reject an empty scenario inventory, got: \
                 {python_missing:?}"
            ));
        }
        // The rust and typescript contracts keep their historical acceptance
        // of an empty inventory; the shared owner must not leak the python
        // rejection into them.
        for contract in [&INSTALLED_RUST_CONTRACT, &INSTALLED_TYPESCRIPT_CONTRACT] {
            let missing = installed_journey_missing_scenarios(contract, &manifest, &corpus);
            if !missing.is_empty() {
                return Err(format!(
                    "the {} contract must keep accepting an empty inventory, got: {missing:?}",
                    contract.label
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn extra_manifest_rules_are_python_specific() -> Result<(), String> {
        let manifest: Value = serde_json::json!({"journey": {}});
        let python_extras = (INSTALLED_PYTHON_CONTRACT.extra_manifest_violations)(&manifest);
        if python_extras.is_empty() {
            return Err(
                "the python contract must add environment-binding rules for a manifest without \
                 journey.environment_binding"
                    .to_string(),
            );
        }
        for contract in [&INSTALLED_RUST_CONTRACT, &INSTALLED_TYPESCRIPT_CONTRACT] {
            let extras = (contract.extra_manifest_violations)(&manifest);
            if !extras.is_empty() {
                return Err(format!(
                    "the {} contract must not add manifest rules, got: {extras:?}",
                    contract.label
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn python_environment_binding_rules_accept_the_committed_shape() -> Result<(), String> {
        let conforming: Value = serde_json::json!({
            "journey": {
                "environment_binding": {
                    "current_verify_command": "python -m pytest tests/test_pricing.py",
                    "historical_bare_command": "pytest tests/test_pricing.py"
                },
                "selected_repair": {
                    "focused_verification_command": "python -m pytest tests/test_pricing.py"
                }
            }
        });
        let extras = installed_python_environment_binding_violations(&conforming);
        if !extras.is_empty() {
            return Err(format!(
                "the committed python environment-binding shape must be accepted, got: {extras:?}"
            ));
        }
        Ok(())
    }
}

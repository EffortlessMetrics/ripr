fn dependency_paths(
    text: &str,
    root: &Path,
    deadline: Option<Instant>,
) -> Result<Vec<PathBuf>, String> {
    let joined = text.replace("\\\n", " ");
    let first = joined
        .lines()
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .ok_or("depfile lacks a dependency rule")?;
    let (_, dependencies) = first
        .split_once(": ")
        .ok_or("depfile lacks a dependency separator")?;
    let mut paths = Vec::new();
    let mut token = String::new();
    let mut chars = dependencies.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\\'
            && chars
                .peek()
                .is_some_and(|next| next.is_whitespace() || matches!(next, '\\' | '#' | ':'))
        {
            token.push(chars.next().ok_or("incomplete dependency escape")?);
        } else if character.is_whitespace() {
            if !token.is_empty() {
                remaining(deadline)?;
                paths.push(
                    root.join(&token)
                        .canonicalize()
                        .map_err(|error| format!("resolve dependency {token}: {error}"))?,
                );
                token.clear();
            }
        } else {
            token.push(character);
        }
    }
    if !token.is_empty() {
        remaining(deadline)?;
        paths.push(
            root.join(&token)
                .canonicalize()
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(paths)
}

fn artifact_records(
    stdout: &str,
    metadata: &Value,
    root: &Path,
    command: &str,
    profile: &Value,
    deadline: Option<Instant>,
) -> Result<(String, Vec<Value>), String> {
    let words = command.split_whitespace().skip(2).collect::<Vec<_>>();
    let mut package_name = None;
    let mut target = None;
    let mut filter = None;
    let mut index = 0;
    while index < words.len() {
        match words[index] {
            "-p" | "--package" => {
                index += 1;
                package_name = words.get(index).copied();
            }
            "--bin" | "--test" => {
                let kind = if words[index] == "--bin" {
                    "bin"
                } else {
                    "test"
                };
                index += 1;
                target = Some((kind, words.get(index).copied()));
            }
            "--lib" => target = Some(("lib", None)),
            "--locked" | "--offline" => {}
            word if !word.starts_with('-') && filter.is_none() => filter = Some(word),
            _ => return Err("unsupported source-bound producer selection".to_string()),
        }
        index += 1;
    }
    let package_name = package_name.ok_or("producer selection lacks an exact package")?;
    let tests = stdout
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect::<Vec<_>>();
    if tests.is_empty()
        || tests
            .iter()
            .any(|name| filter.is_some_and(|filter| !name.contains(filter)))
    {
        return Err("actual enumeration does not match its ledger filter".to_string());
    }
    let packages = metadata["packages"]
        .as_array()
        .ok_or("Cargo metadata lacks packages")?;
    let mut finished = 0;
    let mut graph = Vec::new();
    let mut result = Vec::new();
    for line in stdout.lines() {
        remaining(deadline)?;
        let record = match serde_json::from_str::<Value>(line) {
            Ok(record) => record,
            Err(_)
                if line.is_empty() || line.ends_with(": test") || line.ends_with(": benchmark") =>
            {
                continue;
            }
            Err(_) => return Err("unexpected or malformed Cargo producer output".to_string()),
        };
        match record["reason"].as_str() {
            Some("build-finished") if record["success"] == true => finished += 1,
            Some("compiler-message" | "build-script-executed") => {}
            Some("compiler-artifact") => {
                graph.push(record.clone());
                if record["profile"]["test"] != true {
                    continue;
                }
                let Some(package) = packages
                    .iter()
                    .find(|package| package["id"] == record["package_id"])
                else {
                    continue;
                };
                if package["name"].as_str() != Some(package_name) {
                    return Err("enumeration test artifact belongs to another package".to_string());
                }
                if let Some((kind, name)) = target
                    && (!record["target"]["kind"].as_array().is_some_and(|kinds| {
                        kinds.iter().any(|value| value.as_str() == Some(kind))
                    }) || name
                        .is_some_and(|name| record["target"]["name"].as_str() != Some(name)))
                {
                    return Err("enumeration test artifact belongs to another target".to_string());
                }
                let manifest = Path::new(
                    record["manifest_path"]
                        .as_str()
                        .ok_or("artifact lacks manifest")?,
                );
                if !manifest.starts_with(root)
                    || package["manifest_path"].as_str().map(Path::new) != Some(manifest)
                {
                    return Err("Cargo artifact names another source root".to_string());
                }
                if !package["targets"]
                    .as_array()
                    .is_some_and(|targets| targets.contains(&record["target"]))
                {
                    return Err("Cargo artifact target differs from current metadata".to_string());
                }
                let features = package["features"]
                    .as_object()
                    .ok_or("package lacks features")?;
                let mut expected_features = std::collections::BTreeSet::new();
                let mut pending = if features.contains_key("default") {
                    vec!["default".to_string()]
                } else {
                    Vec::new()
                };
                while let Some(feature) = pending.pop() {
                    if !expected_features.insert(feature.clone()) {
                        continue;
                    }
                    for value in features
                        .get(&feature)
                        .and_then(Value::as_array)
                        .ok_or("invalid package feature")?
                    {
                        let value = value.as_str().ok_or("invalid feature member")?;
                        if features.contains_key(value) {
                            pending.push(value.to_string());
                        }
                    }
                }
                let actual_features = record["features"]
                    .as_array()
                    .ok_or("artifact lacks features")?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(ToString::to_string)
                            .ok_or("invalid artifact feature")
                    })
                    .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
                if actual_features != expected_features || record["profile"] != *profile {
                    return Err(
                        "Cargo test artifact features or profile differ from selected context"
                            .to_string(),
                    );
                }
                if record["executable"].as_str().is_none() {
                    return Err("test artifact lacks executable".to_string());
                }
                result.push(record);
            }
            _ => return Err("unexpected Cargo producer record".to_string()),
        }
    }
    if finished != 1 || result.is_empty() {
        return Err("Cargo producer lacks successful build and test artifacts".to_string());
    }
    let package = packages
        .iter()
        .find(|package| package["name"].as_str() == Some(package_name))
        .ok_or("selected package is absent")?;
    let expected_targets = package["targets"]
        .as_array()
        .ok_or("package lacks targets")?
        .iter()
        .filter(|item| {
            item["test"] == true
                && item["kind"].as_array().is_some_and(|kinds| {
                    kinds.iter().any(|kind| {
                        matches!(kind.as_str(), Some("lib" | "bin" | "test"))
                            && target.is_none_or(|(selected, name)| {
                                kind.as_str() == Some(selected)
                                    && name.is_none_or(|name| item["name"].as_str() == Some(name))
                            })
                    })
                })
        })
        .collect::<Vec<_>>();
    if expected_targets.len() != result.len()
        || expected_targets
            .iter()
            .any(|target| !result.iter().any(|artifact| artifact["target"] == **target))
    {
        return Err(
            "actual Cargo producer omitted or duplicated a selected test target".to_string(),
        );
    }
    Ok((json_sha(&graph)?, result))
}

fn artifacts(
    stdout: &str,
    metadata: &Value,
    root: &Path,
    command: &str,
    profile: &Value,
    deadline: Option<Instant>,
) -> Result<(String, Vec<Artifact>), String> {
    let (graph, records) = artifact_records(stdout, metadata, root, command, profile, deadline)?;
    let target_dir = Path::new(
        metadata["target_directory"]
            .as_str()
            .ok_or("metadata lacks target directory")?,
    )
    .canonicalize()
    .map_err(|error| error.to_string())?;
    let mut result = Vec::new();
    for record in records {
        result.push(materialized_artifact(record, root, &target_dir, deadline)?);
    }
    Ok((graph, result))
}

fn materialized_artifact(
    record: Value,
    root: &Path,
    target_dir: &Path,
    deadline: Option<Instant>,
) -> Result<Artifact, String> {
    let manifest = Path::new(
        record["manifest_path"]
            .as_str()
            .ok_or("artifact lacks manifest")?,
    )
    .canonicalize()
    .map_err(|error| error.to_string())?;
    let executable = record["executable"]
        .as_str()
        .ok_or("test artifact lacks executable")?;
    let executable = PathBuf::from(executable);
    let executable_sha256 = file_sha_before(&executable, deadline)?;
    let executable = executable
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !executable.starts_with(target_dir) {
        return Err("Cargo artifact escaped its target directory".to_string());
    }
    let depfile = executable.with_extension("d");
    let dependencies = String::from_utf8(read_regular(&depfile, STREAM_LIMIT as u64)?)
        .map_err(|error| error.to_string())?;
    let source = record["target"]["src_path"]
        .as_str()
        .ok_or("artifact lacks source path")?;
    let source_paths = dependency_paths(&dependencies, root, deadline)?;
    let source = Path::new(source)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !source.starts_with(root)
        || !source_paths.contains(&source)
        || source_paths
            .iter()
            .any(|path| !path.starts_with(root) && !path.starts_with(target_dir))
    {
        return Err("artifact depfile is not bound to its current source paths".to_string());
    }
    let package_root = manifest.parent().ok_or("manifest has no parent")?;
    let manifest_dependencies = dependencies
        .lines()
        .filter_map(|line| line.strip_prefix("# env-dep:CARGO_MANIFEST_DIR="))
        .collect::<Vec<_>>();
    if manifest_dependencies
        .iter()
        .any(|value| Path::new(value) != package_root)
    {
        return Err("artifact was compiled for another manifest directory".to_string());
    }
    Ok(Artifact {
        compiler_record: record,
        executable,
        executable_sha256,
        depfile_sha256: file_sha_before(&depfile, deadline)?,
        depfile,
    })
}

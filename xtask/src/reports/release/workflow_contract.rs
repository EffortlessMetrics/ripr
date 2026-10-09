use super::missing_required_needles;

/// The generated template uploads the whole report tree. Check the actual
/// ripr job's named artifact step, so a cleanup command, SARIF path, comment,
/// or another step cannot stand in for a recursive artifact upload root.
pub(super) fn github_workflow_yaml_missing(text: &str) -> Vec<String> {
    let required = [
        "continue-on-error: true",
        "$GITHUB_STEP_SUMMARY",
        "target/ripr/reports",
        "RIPR_UPLOAD_SARIF",
        "actions/upload-artifact",
    ];
    let mut missing = missing_required_needles(text, &required);
    missing.extend(qualified_release_steps_missing(text));
    let (mut in_jobs, mut in_ripr, mut in_steps) = (false, false, false);
    let (mut upload_step, mut in_with, mut in_path) = (false, false, false);
    let (mut upload_action, mut ripr_root, mut ci_root) = (false, false, false);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        match indent {
            0 => {
                in_jobs = trimmed == "jobs:";
                in_ripr = false;
                in_steps = false;
            }
            2 if in_jobs => {
                in_ripr = trimmed == "ripr:";
                in_steps = false;
            }
            4 if in_ripr => in_steps = trimmed == "steps:",
            6 if in_steps && trimmed.starts_with("- ") => {
                if upload_step {
                    break;
                }
                upload_step = trimmed == "- name: Upload RIPR report artifacts";
                in_with = false;
                in_path = false;
            }
            8 if in_steps && upload_step => {
                if let Some(reference) = trimmed.strip_prefix("uses: actions/upload-artifact@") {
                    upload_action =
                        !reference.trim().is_empty() && !reference.trim_start().starts_with('#');
                }
                in_with = trimmed == "with:";
                in_path = false;
            }
            10 if in_steps && upload_step && in_with => in_path = trimmed == "path: |",
            12 if in_steps && upload_step && in_with && in_path => match trimmed {
                "target/ripr" => ripr_root = true,
                "target/ci" => ci_root = true,
                _ => {}
            },
            _ => {}
        }
    }
    for (present, expected) in [
        (
            upload_action,
            "report artifact upload action in jobs.ripr.steps",
        ),
        (ripr_root, "report artifact upload path: target/ripr"),
        (ci_root, "report artifact upload path: target/ci"),
    ] {
        if !present {
            missing.push(expected.to_string());
        }
    }
    missing
}

// Frozen scoped bytes of the independently replayed 0.10.0 adapter.
// Root execution configuration and complete step bodies are pinned; only
// empty separator lines and CRLF line endings are normalized. Meaningful
// trailing spaces remain pinned, including shell continuation lines.
// This is an exact admitted-template
// contract, not an interpretation of general YAML or shell semantics.
// Intentional adapter changes require a new installed-release replay.
fn qualified_release_steps_missing(text: &str) -> Vec<String> {
    use sha2::{Digest, Sha256};
    let mut steps: Vec<(String, String)> = Vec::new();
    let (mut in_jobs, mut in_ripr, mut in_steps) = (false, false, false);
    let mut step: Option<(String, Vec<String>)> = None;
    let (mut ripr_jobs, mut step_blocks) = (0, 0);
    let mut job_fields = Vec::new();
    let (mut seen_jobs, mut unexpected_root) = (false, false);
    let mut unexpected_job = false;
    let flush = |step: &mut Option<(String, Vec<String>)>, steps: &mut Vec<(String, String)>| {
        if let Some((name, mut lines)) = step.take() {
            while lines.last().is_some_and(String::is_empty) {
                lines.pop();
            }
            steps.push((name, lines.join("\n")));
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if (trimmed.is_empty() || indent > 6)
                && let Some((_, lines)) = &mut step
            {
                lines.push(line.to_string());
            }
            continue;
        }
        if seen_jobs && indent == 0 && trimmed != "Rerun without --dry-run to apply." {
            unexpected_root = true;
        }
        match indent {
            0 => {
                flush(&mut step, &mut steps);
                in_jobs = trimmed == "jobs:";
                seen_jobs |= in_jobs;
                in_ripr = false;
                in_steps = false;
            }
            2 if in_jobs => {
                flush(&mut step, &mut steps);
                in_ripr = trimmed == "ripr:";
                unexpected_job |= !in_ripr;
                ripr_jobs += usize::from(in_ripr);
                in_steps = false;
            }
            4 if in_ripr => {
                flush(&mut step, &mut steps);
                in_steps = trimmed == "steps:";
                step_blocks += usize::from(in_steps);
                job_fields.push(trimmed);
            }
            6 if in_steps && trimmed.starts_with("- ") => {
                flush(&mut step, &mut steps);
                let name = trimmed.strip_prefix("- name: ").unwrap_or("");
                step = Some((name.to_string(), vec![line.to_string()]));
            }
            _ if in_steps && indent > 6 => {
                if let Some((_, lines)) = &mut step {
                    lines.push(line.to_string());
                }
            }
            _ => {
                flush(&mut step, &mut steps);
                in_steps = false;
            }
        }
    }
    flush(&mut step, &mut steps);
    let source_lines = text.lines().collect::<Vec<_>>();
    let names = source_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == "name: RIPR")
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let roots = source_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == "jobs:")
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let root_ok = names.len() == 1
        && roots.len() == 1
        && names[0] < roots[0]
        && format!(
            "{:x}",
            Sha256::digest(
                source_lines[names[0]..roots[0]]
                    .join("\n")
                    .trim_end_matches('\n')
                    .as_bytes()
            )
        ) == "7279796c36575b11941c6b4bb959c5158665afc726b29e902c687eed5f30a069";
    let mut missing = Vec::new();
    if !root_ok
        || unexpected_root
        || unexpected_job
        || ripr_jobs != 1
        || step_blocks != 1
        || job_fields
            != [
                "name: RIPR advisory reports",
                "runs-on: ubuntu-latest",
                "continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}",
                "steps:",
            ]
    {
        missing.push(
            "qualified release execution configuration and unique jobs.ripr.steps mapping"
                .to_string(),
        );
    }
    let admitted_steps = steps
        .iter()
        .map(|(_, body)| body.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if steps.len() != 42
        || format!("{:x}", Sha256::digest(admitted_steps.as_bytes()))
            != "9688b4c138b1922cdf225d45f0bdabd29cb51b1478c0ba64efb74e870f21901f"
    {
        missing.push("qualified release checkout, cleanup and complete ordered step inventory in jobs.ripr.steps".to_string());
    }
    let mut positions = Vec::new();
    for (name, digest) in [
        (
            "Install ripr",
            "85e1b6098ccd15bda3688e95c00270e195f2a956676a5f0451c29e5720b85863",
        ),
        (
            "Verify installed RIPR compatibility",
            "a54b5fead50c39729364e57ebd63b41c2e1f885d5a1f31dfa82774efab383bcb",
        ),
        (
            "Add RIPR advisory summary",
            "582e2b830c133d31114438603fe5f21775b7bb2c5594012a38eaf1c1336a7be6",
        ),
    ] {
        let matches = steps
            .iter()
            .enumerate()
            .filter(|(_, (found, _))| found == name)
            .collect::<Vec<_>>();
        if matches.len() == 1
            && format!("{:x}", Sha256::digest(matches[0].1.1.as_bytes())) == digest
        {
            positions.push(matches[0].0);
        } else {
            missing.push(format!(
                "qualified installed 0.10.0 step in jobs.ripr.steps: {name}"
            ));
        }
    }
    let first = steps
        .iter()
        .position(|(name, _)| name == "Generate RIPR pilot packet");
    let summary = steps
        .iter()
        .position(|(name, _)| name == "Add RIPR advisory summary");
    let producer_ok = match (first, summary) {
        (Some(first), Some(summary)) if summary > first && summary - first == 33 => {
            let body = steps[first..summary]
                .iter()
                .map(|(_, body)| body.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            format!("{:x}", Sha256::digest(body.as_bytes()))
                == "5ce23fa0e9b2d478464313329145676465424a8eee20787a8564d5baa059ebac"
                && steps[first..summary]
                    .iter()
                    .all(|(name, _)| steps.iter().filter(|(found, _)| found == name).count() == 1)
                && positions.len() == 3
                && positions[0] < positions[1]
                && positions[1] < first
                && positions[2] == summary
        }
        _ => false,
    };
    if !producer_ok {
        missing.push(
            "qualified ordered installed 0.10.0 producer commands and artifacts in jobs.ripr.steps"
                .to_string(),
        );
    }
    missing
}

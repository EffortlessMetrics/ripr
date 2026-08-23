use std::collections::BTreeMap;
use std::path::Path;

pub(crate) fn check_network_policy() -> Result<(), String> {
    let ordinary_result = crate::check_network_policy_impl();
    let orphan_result = network_policy_orphan_violations();

    match (ordinary_result, orphan_result) {
        (Ok(()), Ok(violations)) if violations.is_empty() => Ok(()),
        (Ok(()), Ok(violations)) => Err(render_network_policy_failure(&violations)),
        (Err(error), Ok(violations)) if violations.is_empty() => Err(error),
        (Err(error), Ok(violations)) => Err(append_network_policy_violations(error, &violations)),
        (Ok(()), Err(error)) => Err(error),
        (Err(policy_error), Err(orphan_error)) => Err(format!(
            "{policy_error}; failed to evaluate orphaned network-policy rows: {orphan_error}"
        )),
    }
}

fn network_policy_orphan_violations() -> Result<Vec<String>, String> {
    let allowlist = crate::read_count_policy_allowlist("policy/network_allowlist.txt")?;
    let patterns = crate::network_policy_patterns();
    let mut counts = BTreeMap::<(String, String), usize>::new();

    for path in crate::tracked_files()? {
        if !crate::is_network_policy_candidate(&path) {
            continue;
        }
        let text = crate::read_text_lossy(Path::new(&path))?;
        for pattern in &patterns {
            let count = text.matches(pattern).count();
            if count > 0 {
                counts.insert((path.clone(), pattern.clone()), count);
            }
        }
    }

    Ok(orphan_violations_from_counts(&allowlist, &counts))
}

fn orphan_violations_from_counts(
    allowlist: &BTreeMap<(String, String), usize>,
    counts: &BTreeMap<(String, String), usize>,
) -> Vec<String> {
    let mut violations = allowlist
        .iter()
        .filter_map(|((path, pattern), maximum)| {
            let actual = counts
                .get(&(path.clone(), pattern.clone()))
                .copied()
                .unwrap_or(0);
            (*maximum > 0 && actual == 0)
                .then(|| format!("{path} | {pattern} | orphaned max_count={maximum}"))
        })
        .collect::<Vec<_>>();
    violations.sort();
    violations
}

fn render_network_policy_failure(violations: &[String]) -> String {
    let body = violations
        .iter()
        .map(|violation| format!("- {violation}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("network policy failed:\n{body}")
}

fn append_network_policy_violations(mut error: String, violations: &[String]) -> String {
    while error.ends_with('\n') {
        let _ = error.pop();
    }
    for violation in violations {
        error.push_str("\n- ");
        error.push_str(violation);
    }
    error
}

#[cfg(test)]
mod tests {
    use super::orphan_violations_from_counts;
    use std::collections::BTreeMap;

    #[test]
    fn orphan_detection_reports_only_positive_zero_count_rows() {
        let allowlist = BTreeMap::from([
            (("live.rs".to_string(), "curl".to_string()), 2),
            (("orphan.rs".to_string(), "curl".to_string()), 3),
            (("zero.rs".to_string(), "curl".to_string()), 0),
        ]);
        let counts = BTreeMap::from([(("live.rs".to_string(), "curl".to_string()), 1)]);

        assert_eq!(
            orphan_violations_from_counts(&allowlist, &counts),
            vec!["orphan.rs | curl | orphaned max_count=3"]
        );
    }
}

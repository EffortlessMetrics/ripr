use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const NETWORK_POLICY_PATH: &str = "policy/network_allowlist.txt";

pub(crate) fn check_network_policy() -> Result<(), String> {
    let ordinary_result = crate::check_network_policy_impl();
    let semantic_result = network_policy_semantic_violations();

    match (ordinary_result, semantic_result) {
        (Ok(()), Ok(violations)) if violations.is_empty() => Ok(()),
        (Ok(()), Ok(violations)) => Err(render_network_policy_failure(&violations)),
        (Err(error), Ok(violations)) if violations.is_empty() => Err(error),
        (Err(error), Ok(violations)) => Err(append_network_policy_violations(error, &violations)),
        (Ok(()), Err(error)) => Err(error),
        (Err(policy_error), Err(semantic_error)) => Err(format!(
            "{policy_error}; failed to evaluate semantic network-policy rows: {semantic_error}"
        )),
    }
}

fn network_policy_semantic_violations() -> Result<Vec<String>, String> {
    let policy_text = crate::read_text_lossy(Path::new(NETWORK_POLICY_PATH))?;
    let mut violations = duplicate_key_violations_from_text(&policy_text);
    let allowlist = crate::read_count_policy_allowlist(NETWORK_POLICY_PATH)?;
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

    violations.extend(orphan_violations_from_counts(&allowlist, &counts));
    violations.sort();
    Ok(violations)
}

fn duplicate_key_violations_from_text(policy: &str) -> Vec<String> {
    let mut seen = BTreeSet::<(String, String)>::new();
    let mut violations = Vec::new();

    for (index, raw_line) in policy.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut fields = line.split('|').map(str::trim);
        let Some(path) = fields.next() else {
            continue;
        };
        let Some(pattern) = fields.next() else {
            continue;
        };
        if path.is_empty() || pattern.is_empty() {
            continue;
        }

        let key = (path.to_string(), pattern.to_string());
        if !seen.insert(key) {
            violations.push(format!(
                "{path} | {pattern} | duplicate semantic key at line {}",
                index + 1
            ));
        }
    }

    violations
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
    use super::{duplicate_key_violations_from_text, orphan_violations_from_counts};
    use std::collections::BTreeMap;

    #[test]
    fn duplicate_detection_uses_trimmed_path_and_pattern_as_semantic_key() {
        let policy = "a.rs|curl|1|source|first\n  a.rs | curl |2|swarm|duplicate\n";
        assert_eq!(
            duplicate_key_violations_from_text(policy),
            vec!["a.rs | curl | duplicate semantic key at line 2"]
        );
    }

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

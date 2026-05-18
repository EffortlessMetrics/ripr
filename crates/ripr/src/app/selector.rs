use crate::domain::Finding;

pub(in crate::app) fn select_finding<'a>(
    findings: &'a [Finding],
    selector: &str,
) -> Option<&'a Finding> {
    findings
        .iter()
        .find(|finding| finding.id == selector || selector_matches_location(selector, finding))
}

pub(in crate::app) fn selector_matches_location(selector: &str, finding: &Finding) -> bool {
    let file = finding.probe.location.file.to_string_lossy();
    let line = finding.probe.location.line;
    selector == format!("{file}:{line}")
        || selector.ends_with(&format!(":{line}")) && selector.contains(file.as_ref())
}

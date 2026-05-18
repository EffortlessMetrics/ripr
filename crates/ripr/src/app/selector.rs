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
    let line = finding.probe.location.line.to_string();
    let Some((selector_file, selector_line)) = selector.rsplit_once(':') else {
        return false;
    };

    selector_line == line
        && (selector_file == file.as_ref()
            || selector_file.ends_with(&format!("/{file}"))
            || selector_file.ends_with(&format!("\\{file}")))
}

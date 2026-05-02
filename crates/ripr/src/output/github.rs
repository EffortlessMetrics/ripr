use crate::app::CheckOutput;

pub fn render(output: &CheckOutput) -> String {
    let mut out = String::new();
    for finding in &output.findings {
        let title = format!("ripr {}", finding.class.as_str());
        let mut message = finding
            .recommended_next_step
            .as_deref()
            .unwrap_or("Static RIPR exposure finding")
            .to_string();
        let stop_reasons = finding.effective_stop_reasons();
        if !stop_reasons.is_empty() {
            let reasons = stop_reasons
                .iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            message.push_str(" Stop reason: ");
            message.push_str(&reasons);
        }
        let annotation_level = match finding.class.severity() {
            "info" => "notice",
            "note" => "notice",
            _ => "warning",
        };
        out.push_str(&format!(
            "::{annotation_level} file={},line={},title={}::{}\n",
            finding.probe.location.file.display(),
            finding.probe.location.line,
            escape_cmd(&title),
            escape_cmd(&message)
        ));
    }
    if output.findings.is_empty() {
        out.push_str("::notice title=ripr::No static mutation exposure findings found\n");
    }
    out
}

fn escape_cmd(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(',', "%2C")
        .replace(':', "%3A")
}

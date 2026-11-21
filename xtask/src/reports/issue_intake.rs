const USAGE: &str = "usage: cargo xtask issue-intake --issue <number>";
const BODY_EXCERPT_LIMIT: usize = 1000;

#[derive(Debug)]
struct GithubIssue {
    title: String,
    labels: Vec<GithubLabel>,
    body: Option<String>,
}

#[derive(Debug)]
struct GithubLabel {
    name: String,
}

fn parse_issue_number(args: &[String]) -> Result<u32, String> {
    let mut issue_number = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--issue" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for --issue\n{USAGE}"))?;
                let parsed = value
                    .parse::<u32>()
                    .map_err(|error| format!("invalid --issue `{value}`: {error}\n{USAGE}"))?;
                if issue_number.replace(parsed).is_some() {
                    return Err(format!("duplicate --issue\n{USAGE}"));
                }
                index += 2;
            }
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }

    issue_number.ok_or_else(|| USAGE.to_string())
}

fn body_excerpt(body: Option<&str>) -> Option<String> {
    let body = body?;
    if body.len() <= BODY_EXCERPT_LIMIT {
        return Some(body.to_string());
    }
    let mut end = BODY_EXCERPT_LIMIT;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    Some(format!("{}...", &body[..end]))
}

fn decode_issue(gh_output: &str) -> Result<GithubIssue, String> {
    let parsed: serde_json::Value = serde_json::from_str(gh_output)
        .map_err(|error| format!("failed to parse gh issue view output: {error}"))?;
    let title = parsed
        .get("title")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "gh issue view output is missing string `title`".to_string())?
        .to_string();
    let labels = parsed
        .get("labels")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "gh issue view output is missing array `labels`".to_string())?
        .iter()
        .map(|label| {
            let name = label
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    "gh issue view output has a label without string `name`".to_string()
                })?;
            Ok(GithubLabel {
                name: name.to_string(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let body_value = parsed
        .get("body")
        .ok_or_else(|| "gh issue view output is missing `body`".to_string())?;
    let body = (!body_value.is_null())
        .then(|| body_value.as_str().map(str::to_string))
        .flatten();
    if !body_value.is_null() && body.is_none() {
        return Err("gh issue view output has non-string `body`".to_string());
    }
    Ok(GithubIssue {
        title,
        labels,
        body,
    })
}

pub(crate) fn issue_intake(args: &[String]) -> Result<(), String> {
    let issue_number = parse_issue_number(args)?;
    let issue_number_text = issue_number.to_string();
    let gh_output = crate::run::run_output(
        "gh",
        &[
            "issue",
            "view",
            &issue_number_text,
            "--json",
            "title,labels,body",
            "--repo",
            "EffortlessMetrics/ripr-swarm",
        ],
    )
    .map_err(|error| format!("failed to run gh issue view: {error}"))?;

    let issue = decode_issue(&gh_output)?;
    let packet_json = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": "1.0",
        "issue_number": issue_number,
        "title": issue.title,
        "labels": issue.labels.into_iter().map(|label| label.name).collect::<Vec<_>>(),
        "body_excerpt": body_excerpt(issue.body.as_deref()),
        "initial_readiness": "unknown"
    }))
    .map_err(|error| format!("failed to serialize intake packet: {error}"))?;

    let out_path = std::path::PathBuf::from(format!(
        "target/ripr/reports/issue-intake-{issue_number}.json"
    ));
    super::write_parented_file(&out_path, "issue intake packet", packet_json.as_bytes())?;

    println!("wrote {}", out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_issue_number_accepts_only_a_single_positive_u32() -> Result<(), String> {
        let issue = parse_issue_number(&["--issue".to_string(), "1644".to_string()])?;
        if issue != 1644 {
            return Err(format!("expected 1644, got {issue}"));
        }
        for args in [
            vec![],
            vec!["--issue".to_string()],
            vec!["--issue".to_string(), "nope".to_string()],
            vec!["--unknown".to_string()],
            vec![
                "--issue".to_string(),
                "1".to_string(),
                "--issue".to_string(),
                "2".to_string(),
            ],
        ] {
            if parse_issue_number(&args).is_ok() {
                return Err(format!("malformed arguments were accepted: {args:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn body_excerpt_preserves_utf8_boundaries_and_null_bodies() -> Result<(), String> {
        if body_excerpt(None).is_some() {
            return Err("null issue body fabricated an excerpt".to_string());
        }
        let body = "\u{00e9}".repeat(600);
        let excerpt = body_excerpt(Some(&body)).ok_or_else(|| "missing excerpt".to_string())?;
        if !excerpt.ends_with("...") || !excerpt.is_char_boundary(excerpt.len()) {
            return Err("UTF-8 body excerpt was not boundary-safe".to_string());
        }
        if excerpt.trim_end_matches("...").len() > BODY_EXCERPT_LIMIT {
            return Err("body excerpt exceeded its byte limit".to_string());
        }
        Ok(())
    }

    #[test]
    fn decode_issue_requires_source_owned_fields() -> Result<(), String> {
        let issue = decode_issue(r#"{"title":"Issue","labels":[{"name":"bug"}],"body":null}"#)?;
        let label = issue
            .labels
            .first()
            .ok_or_else(|| "typed issue payload omitted the label".to_string())?;
        if issue.title != "Issue" || label.name != "bug" || issue.body.is_some() {
            return Err("typed issue payload was decoded incorrectly".to_string());
        }
        if decode_issue(r#"{"labels":[],"body":null}"#).is_ok() {
            return Err("missing title was converted into a fake value".to_string());
        }
        Ok(())
    }
}

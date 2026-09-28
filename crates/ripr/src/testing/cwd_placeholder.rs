//! Renderer working-directory projection for checked-in expectations.
//!
//! Issue #3872 anchors funnel redirect targets at the resolved `--root`, so
//! rendered commands embed the renderer machine directory. Checked-in
//! expectations pin the anchored shape with a `<cwd>/` placeholder for that
//! prefix — never a real machine directory. The spelling and the projection
//! rule live here so every golden, corpus, and fixture projection agrees.
//! (This module — not a path-included production file — is the home for the
//! helpers: `xtask` compiles `agent::loop_commands` into its own crate via
//! `#[path]`, where cross-module test callers do not exist.)

use crate::agent::loop_commands::display_path;
use std::path::PathBuf;

/// The machine prefix that anchored redirect targets embed: the renderer
/// working directory with stable separators and a trailing slash.
///
/// An unreadable working directory degrades to a prefix that matches
/// nothing, so the projection becomes a no-op and the comparison fails
/// loudly instead of passing on unprojected machine paths.
#[cfg(test)]
pub(crate) fn renderer_cwd_prefix() -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    format!("{}/", display_path(&cwd))
}

/// Replace every renderer-working-directory prefix inside `value` with
/// `<cwd>/`, recursively. The anchor sits mid-string inside rendered
/// commands, so this projects occurrences, not just a leading prefix.
#[cfg(test)]
pub(crate) fn project_renderer_cwd(value: &mut serde_json::Value) {
    let prefix = renderer_cwd_prefix();
    project_cwd_prefix(value, &prefix);
}

#[cfg(test)]
fn project_cwd_prefix(value: &mut serde_json::Value, prefix: &str) {
    match value {
        serde_json::Value::String(text) => {
            *text = project_text_with_prefix(text, prefix);
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .for_each(|item| project_cwd_prefix(item, prefix)),
        serde_json::Value::Object(map) => map
            .values_mut()
            .for_each(|item| project_cwd_prefix(item, prefix)),
        _ => {}
    }
}

/// Project one rendered string: the bare machine prefix maps to `<cwd>/`,
/// and so does the `shell_arg`-quoted Bash redirect form — but only when the
/// quote comes from the machine prefix. Redirect targets render through
/// `shell_arg`, so a checkout path carrying a space (or `+`, `@`, …) reaches
/// expectations quoted — `> '<prefix>tail'` — while the pinned goldens carry
/// the unquoted `<cwd>/tail` shape. When the tail itself needs quoting (for
/// example a root carrying a space, `> '<prefix>repo root/…'`), the quoted
/// placeholder is the correct machine-independent shape and the token is
/// left quoted. Only the `> ` redirect position unquotes: the PowerShell-safe
/// form intentionally quotes its `WriteAllText('…')` path argument on every
/// checkout, and its goldens pin that quoted shape. Artifact tails never
/// contain a quote, so the first closing quote after the anchored prefix
/// ends the token; a token with no closing quote keeps the bare-prefix
/// projection only, matching previous behavior.
#[cfg(test)]
pub(crate) fn project_cwd_text(text: &str) -> String {
    project_text_with_prefix(text, &renderer_cwd_prefix())
}

/// Mirror of `shell_arg`'s bare-token rule: the placeholder tail renders
/// unquoted exactly when the raw tail would.
#[cfg(test)]
fn tail_renders_bare(tail: &str) -> bool {
    !tail.is_empty()
        && tail
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-' | ':'))
}

/// Project a selected workspace root (#3999/#4001) the same way: commands
/// bound to `root` carry it as the bare `--root` token and as the prefix of
/// every anchored target, and both map to `<root>`. Goldens whose producer
/// binds a fixture workspace (rather than the renderer working directory)
/// use this projection; `<cwd>` stays the renderer-directory placeholder.
#[cfg(test)]
pub(crate) fn project_root_text(text: &str, root: &std::path::Path) -> String {
    let prefix = format!(
        "{}/",
        crate::agent::loop_commands::bound_root(&root.to_string_lossy())
    );
    project_text_with_placeholder(text, &prefix, "<root>")
}

#[cfg(test)]
fn project_text_with_prefix(text: &str, prefix: &str) -> String {
    project_text_with_placeholder(text, prefix, "<cwd>")
}

#[cfg(test)]
fn project_text_with_placeholder(text: &str, prefix: &str, placeholder: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    // Mirror `shell_arg`'s single-quote escaping so a prefix that itself
    // carries a quote still opens the needle identically.
    let quoted_open = format!("> '{}", prefix.replace('\'', r"'\''"));
    while let Some(at) = rest.find(quoted_open.as_str()) {
        let after = &rest[at + quoted_open.len()..];
        let Some(end) = after.find('\'') else {
            break;
        };
        let tail = &after[..end];
        out.push_str(&rest[..at]);
        if tail_renders_bare(tail) {
            out.push_str(&format!("> {placeholder}/"));
            out.push_str(tail);
        } else {
            out.push_str(&format!("> '{placeholder}/"));
            out.push_str(tail);
            out.push('\'');
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    project_bound_root_token(
        &out.replace(prefix, &format!("{placeholder}/")),
        prefix,
        placeholder,
    )
}

/// Project the bound `--root` token itself (#3999): generated commands name
/// the selected root as one absolute path with no trailing slash, so after
/// the `<cwd>/` prefix pass the remaining bare occurrences are root tokens.
/// The `shell_arg`-quoted form (a checkout path carrying a space) unquotes to
/// the same `<cwd>` placeholder; a longer sibling path that merely starts
/// with the same bytes is left alone.
#[cfg(test)]
fn project_bound_root_token(text: &str, prefix: &str, placeholder: &str) -> String {
    let bare = prefix.trim_end_matches('/');
    if bare.is_empty() {
        return text.to_string();
    }
    let quoted = format!("'{}'", bare.replace('\'', r"'\''"));
    let text = text.replace(&quoted, placeholder);
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find(bare) {
        let after = &rest[at + bare.len()..];
        let continues_path = after
            .chars()
            .next()
            .is_some_and(|ch| ch.is_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'));
        out.push_str(&rest[..at]);
        out.push_str(if continues_path { bare } else { placeholder });
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::project_text_with_prefix;
    use crate::agent::loop_commands::shell_arg;

    #[test]
    fn quoted_anchored_target_projects_to_unquoted_placeholder() {
        // Synthetic `/srv` prefixes: hostile enough to quote, with no real
        // machine path for the local-context policy to flag.
        let prefix = "/srv/checkout with space/repo/";
        let rendered = format!("> {}", shell_arg(&format!("{prefix}target/ripr/out.json")));
        assert_eq!(
            rendered, "> '/srv/checkout with space/repo/target/ripr/out.json'",
            "the hostile checkout path must reach the test quoted"
        );
        assert_eq!(
            project_text_with_prefix(&rendered, prefix),
            "> <cwd>/target/ripr/out.json"
        );
    }

    #[test]
    fn bare_prefix_and_quote_free_checkout_keep_prior_shape() {
        let prefix = "/srv/checkout/";
        assert_eq!(
            project_text_with_prefix(&format!("> {prefix}target/out.json"), prefix),
            "> <cwd>/target/out.json"
        );
        assert_eq!(
            project_text_with_prefix("no anchor here", prefix),
            "no anchor here"
        );
        // A quoted token with no closing quote keeps the bare projection.
        assert_eq!(
            project_text_with_prefix(&format!("> '{prefix}target/out.json"), prefix),
            "> '<cwd>/target/out.json"
        );
    }

    #[test]
    fn hostile_tail_keeps_its_quoted_shape() {
        // The quote here comes from the tail (`repo root`), not the machine
        // prefix: the quoted placeholder is the stable golden shape.
        let prefix = "/srv/checkout/";
        let rendered = format!(
            "> {}",
            shell_arg(&format!("{prefix}repo root/target/out.json"))
        );
        assert_eq!(
            rendered, "> '/srv/checkout/repo root/target/out.json'",
            "the hostile tail must reach the test quoted"
        );
        assert_eq!(
            project_text_with_prefix(&rendered, prefix),
            "> '<cwd>/repo root/target/out.json'"
        );
    }

    #[test]
    fn bound_root_token_projects_bare_and_quoted_forms() {
        let prefix = "/srv/checkout/";
        assert_eq!(
            project_text_with_prefix(
                "ripr check --root /srv/checkout --mode draft > /srv/checkout/target/out.json",
                prefix
            ),
            "ripr check --root <cwd> --mode draft > <cwd>/target/out.json"
        );
        assert_eq!(
            project_text_with_prefix("`ripr pilot --root /srv/checkout`", prefix),
            "`ripr pilot --root <cwd>`"
        );
        // A sibling checkout sharing the prefix bytes is not the root.
        assert_eq!(
            project_text_with_prefix("--root /srv/checkout2 --json", prefix),
            "--root /srv/checkout2 --json"
        );
        let spaced = "/srv/checkout with space/";
        assert_eq!(
            project_text_with_prefix("--root '/srv/checkout with space' --json", spaced),
            "--root <cwd> --json"
        );
    }

    #[test]
    fn powershell_write_all_text_keeps_its_quoted_shape() {
        // The PowerShell-safe form quotes its path argument on every
        // checkout; its goldens pin that shape, so the projection must not
        // strip those quotes — only the Bash `> ` redirect unquotes.
        let prefix = "/srv/checkout with space/repo/";
        let rendered = format!("[System.IO.File]::WriteAllText('{prefix}target/out.json', $x)");
        assert_eq!(
            project_text_with_prefix(&rendered, prefix),
            "[System.IO.File]::WriteAllText('<cwd>/target/out.json', $x)"
        );
    }
}

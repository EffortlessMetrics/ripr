use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::PathBuf;

use super::{
    ChangedFile, DIFF_FILE_LIMIT_ENV, ParsedDiff, is_new_path_marker, parse_old_path_marker,
    parser_state,
};

pub(super) fn parse_unbounded(input: &str) -> ParsedDiff {
    match parse_lines(input.lines(), |_| Ok::<(), Infallible>(())) {
        Ok(parsed) => parsed,
        Err(never) => match never {},
    }
}

pub(super) fn parse_bounded_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
    limit: usize,
) -> Result<ParsedDiff, String> {
    parse_lines(lines, |count| {
        if count <= limit {
            return Ok(());
        }
        Err(format!(
            "diff_scope_oversized: at least {count} changed files exceed the \
             {DIFF_FILE_LIMIT_ENV} limit ({limit}); parsing stopped before the \
             remaining file bodies and analysis was not run. Repair route: reduce \
             the diff scope, split the extraction PR, run a narrower diff, or \
             raise the limit via {DIFF_FILE_LIMIT_ENV}=<number>."
        ))
    })
}

/// Both entry points use the same grammar and accepted-path map. The admission
/// check runs at the two registration sites, before reading another input line.
/// The unbounded caller has an infallible policy rather than a fallback that
/// could accidentally turn a refused parse into an empty successful result.
fn parse_lines<'a, E>(
    lines: impl Iterator<Item = &'a str>,
    mut admit_file_count: impl FnMut(usize) -> Result<(), E>,
) -> Result<ParsedDiff, E> {
    let mut files: BTreeMap<PathBuf, ChangedFile> = BTreeMap::new();
    let mut state = parser_state::ParserState::default();
    // One-line lookahead preserves plain ---/+++ section boundaries without
    // collecting the whole input. The input &str itself is already in memory;
    // this file-count policy does not bound its bytes or a single large hunk.
    let mut lines = lines.peekable();
    while let Some(raw) = lines.next() {
        if state.handle_diff_boundary(raw) {
            continue;
        }

        if state.handle_binary_files_sentinel(raw) {
            state.record_binary_deletion(raw);
            continue;
        }

        if state.handle_submodule_mode(raw) {
            continue;
        }

        if state.handle_submodule_index(raw) {
            continue;
        }

        if state.handle_rename_metadata(raw, &mut files) {
            admit_file_count(files.len())?;
            continue;
        }

        if state.combined_quarantine()
            && parse_old_path_marker(raw)
            && lines.peek().is_some_and(|next| is_new_path_marker(next))
        {
            // Unprefixed plain markers may follow a combined hunk without a
            // diff --git boundary; quarantined parent columns must stay inert.
            state.close_combined_quarantine();
        }

        if state.in_hunk()
            && parse_old_path_marker(raw)
            && lines.peek().is_some_and(|next| is_new_path_marker(next))
        {
            state.close_hunk();
        }

        if state.register_path_marker(raw, &mut files) {
            admit_file_count(files.len())?;
            continue;
        }

        if state.handle_hunk_header(raw) {
            continue;
        }

        state.consume_hunk_line(raw, &mut files);
    }

    Ok(ParsedDiff {
        changed_files: files.into_values().collect(),
        deleted_file_count: state.deleted_file_count(),
        submodule_file_count: state.submodule_file_count(),
        renamed_file_count: state.renamed_file_count(),
        pure_rename_file_count: state.pure_rename_file_count(),
        pure_rename_paths: state.pure_rename_paths(),
        limitations: state.limitations(),
    })
}

#[cfg(test)]
mod tests;

//! Shared `/`-separated path-glob matcher.
//!
//! One semantic owner for the glob dialect used by `path`-selector
//! suppressions (#1441) and Rust `generated_file_patterns`:
//!
//! - `**` as a whole segment matches zero or more `/`-separated segments;
//! - `*` matches any run of characters within one segment;
//! - `?` matches exactly one character within one segment;
//! - every other character matches itself (case-sensitive);
//! - empty and `.` segments are ignored on both sides.
//!
//! Both levels use the iterative single-backtrack-point wildcard algorithm.
//! Every non-wildcard token consumes exactly one unit (a character, or a
//! whole segment), so resuming from the most recent wildcard is complete and
//! the match runs in `O(pattern * subject)` steps per level instead of the
//! exponential time of naive recursive backtracking.

/// Returns whether `path` matches the `/`-separated glob `pattern`.
pub(crate) fn path_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<Vec<char>> = normalized_segments(pattern)
        .map(|segment| segment.chars().collect())
        .collect();
    let path_segments: Vec<Vec<char>> = normalized_segments(path)
        .map(|segment| segment.chars().collect())
        .collect();
    segments_match(&pattern_segments, &path_segments)
}

/// Returns whether one path segment (for example a file name) matches a
/// single-segment glob using `*` and `?`.
pub(crate) fn segment_glob_matches(pattern: &str, segment: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let segment: Vec<char> = segment.chars().collect();
    chars_match(&pattern, &segment)
}

fn normalized_segments(value: &str) -> impl Iterator<Item = &str> {
    value
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
}

fn segments_match(pattern: &[Vec<char>], path: &[Vec<char>]) -> bool {
    let is_double_star = |segment: &Vec<char>| segment.as_slice() == ['*', '*'];
    let mut pattern_index = 0;
    let mut path_index = 0;
    // (pattern index just after the latest `**`, path index it resumes at)
    let mut backtrack: Option<(usize, usize)> = None;
    while path_index < path.len() {
        match pattern.get(pattern_index) {
            Some(segment) if is_double_star(segment) => {
                pattern_index += 1;
                backtrack = Some((pattern_index, path_index));
                continue;
            }
            Some(segment) if chars_match(segment, &path[path_index]) => {
                pattern_index += 1;
                path_index += 1;
                continue;
            }
            _ => {}
        }
        let Some((resume_pattern, resume_path)) = backtrack else {
            return false;
        };
        pattern_index = resume_pattern;
        path_index = resume_path + 1;
        backtrack = Some((resume_pattern, path_index));
    }
    pattern[pattern_index.min(pattern.len())..]
        .iter()
        .all(is_double_star)
}

fn chars_match(pattern: &[char], segment: &[char]) -> bool {
    let mut pattern_index = 0;
    let mut segment_index = 0;
    // (pattern index just after the latest `*`, segment index it resumes at)
    let mut backtrack: Option<(usize, usize)> = None;
    while let Some(&actual) = segment.get(segment_index) {
        match pattern.get(pattern_index) {
            Some('*') => {
                pattern_index += 1;
                backtrack = Some((pattern_index, segment_index));
                continue;
            }
            Some(&expected) if expected == '?' || expected == actual => {
                pattern_index += 1;
                segment_index += 1;
                continue;
            }
            _ => {}
        }
        let Some((resume_pattern, resume_segment)) = backtrack else {
            return false;
        };
        pattern_index = resume_pattern;
        segment_index = resume_segment + 1;
        backtrack = Some((resume_pattern, segment_index));
    }
    pattern[pattern_index.min(pattern.len())..]
        .iter()
        .all(|token| *token == '*')
}

#[cfg(test)]
mod tests {
    use super::{path_glob_matches, segment_glob_matches};
    use std::time::{Duration, Instant};

    /// The pre-fix recursive matcher, kept only as a differential oracle on
    /// inputs small enough for its exponential worst case to stay cheap.
    fn reference_path_match(pattern: &str, path: &str) -> bool {
        fn segments(value: &str) -> Vec<&str> {
            value
                .split('/')
                .filter(|segment| !segment.is_empty() && *segment != ".")
                .collect()
        }
        fn segs(pattern: &[&str], path: &[&str]) -> bool {
            let Some((head, rest)) = pattern.split_first() else {
                return path.is_empty();
            };
            if *head == "**" {
                return (0..=path.len()).any(|skip| segs(rest, &path[skip..]));
            }
            let Some((segment, remaining)) = path.split_first() else {
                return false;
            };
            chars(
                &head.chars().collect::<Vec<_>>(),
                &segment.chars().collect::<Vec<_>>(),
            ) && segs(rest, remaining)
        }
        fn chars(pattern: &[char], segment: &[char]) -> bool {
            match pattern.split_first() {
                None => segment.is_empty(),
                Some(('*', rest)) => (0..=segment.len()).any(|skip| chars(rest, &segment[skip..])),
                Some(('?', rest)) => segment
                    .split_first()
                    .is_some_and(|(_, remaining)| chars(rest, remaining)),
                Some((expected, rest)) => {
                    segment.split_first().is_some_and(|(actual, remaining)| {
                        actual == expected && chars(rest, remaining)
                    })
                }
            }
        }
        segs(&segments(pattern), &segments(path))
    }

    #[test]
    fn path_glob_star_double_star_and_question_semantics() {
        let cases: &[(&str, &str, bool)] = &[
            ("docs/**", "docs/status/gen.md", true),
            ("docs/**", "docs", true),
            ("**", "", true),
            ("**", "a/b/c", true),
            ("**/**", "a", true),
            ("**/gen.rs", "a/b/gen.rs", true),
            ("**/gen.rs", "gen.rs", true),
            ("**/gen.rs", "a/b/gen.rs/x", false),
            ("src/**/tests.rs", "src/tests.rs", true),
            ("src/**/tests.rs", "src/a/b/tests.rs", true),
            ("src/**/b/**/c.rs", "src/b/x/b/y/c.rs", true),
            ("src/**/b/**/c.rs", "src/x/y/c.rs", false),
            ("src/*.rs", "src/lib.rs", true),
            ("src/*.rs", "src/nested/lib.rs", false),
            ("src/*", "src/", false),
            ("*", "", false),
            ("", "", true),
            ("", "a", false),
            ("src/li?.rs", "src/lib.rs", true),
            ("src/li?.rs", "src/line.rs", false),
            ("src/?", "src/", false),
            ("src/lib.rs", "src/lib.rs", true),
            ("src/lib.rs", "src/lib.rs.bak", false),
            ("src", "src/lib.rs", false),
            ("./src/*.rs", "src/lib.rs", true),
            ("src/*.rs", "./src/lib.rs", true),
            ("src//*.rs", "src/lib.rs", true),
            ("src/*.rs/", "src/lib.rs", true),
            ("src/*", "src/a", true),
            ("src/*lib", "src/lib", true),
            ("src/lib*", "src/lib", true),
            ("src/*b*", "src/abc", true),
            ("src/*b*", "src/ac", false),
            ("src/a*b*c", "src/abbbc", true),
            ("src/a*b*c", "src/abcb", false),
            ("src/***", "src/x", true),
            ("src/**x", "src/x", true),
            ("src/**x", "src/a/x", false),
            ("SRC/*.rs", "src/lib.rs", false),
            ("src/é?.rs", "src/éa.rs", true),
        ];
        for (pattern, path, expected) in cases {
            assert_eq!(
                path_glob_matches(pattern, path),
                *expected,
                "pattern `{pattern}` against `{path}`"
            );
            assert_eq!(
                reference_path_match(pattern, path),
                *expected,
                "reference oracle disagrees on `{pattern}` against `{path}`"
            );
        }
    }

    #[test]
    fn segment_glob_matches_file_names() {
        assert!(segment_glob_matches("*.generated.rs", "api.generated.rs"));
        assert!(!segment_glob_matches("*.generated.rs", "api.rs"));
        assert!(segment_glob_matches("gen_?.rs", "gen_a.rs"));
        assert!(!segment_glob_matches("gen_?.rs", "gen_ab.rs"));
        assert!(segment_glob_matches("*", ""));
        assert!(!segment_glob_matches("?", ""));
        assert!(segment_glob_matches("", ""));
    }

    #[test]
    fn path_glob_agrees_with_reference_matcher_on_exhaustive_small_inputs() {
        let pattern_segments = ["**", "*", "a", "?", "a*", "*b", "a?b", "*a*"];
        let path_segments = ["a", "b", "ab", "ba", "aab", "abb"];
        let mut patterns = vec![String::new()];
        for _ in 0..3 {
            let mut next = Vec::new();
            for prefix in &patterns {
                for segment in pattern_segments {
                    next.push(if prefix.is_empty() {
                        segment.to_string()
                    } else {
                        format!("{prefix}/{segment}")
                    });
                }
            }
            patterns.extend(next);
            patterns.sort();
            patterns.dedup();
        }
        let mut paths = vec![String::new()];
        for _ in 0..3 {
            let mut next = Vec::new();
            for prefix in &paths {
                for segment in path_segments {
                    next.push(if prefix.is_empty() {
                        segment.to_string()
                    } else {
                        format!("{prefix}/{segment}")
                    });
                }
            }
            paths.extend(next);
            paths.sort();
            paths.dedup();
        }
        let mut compared = 0usize;
        for pattern in &patterns {
            for path in &paths {
                assert_eq!(
                    path_glob_matches(pattern, path),
                    reference_path_match(pattern, path),
                    "pattern `{pattern}` against `{path}`"
                );
                compared += 1;
            }
        }
        assert!(compared > 100_000, "compared only {compared} pairs");
    }

    #[test]
    fn pathological_star_patterns_finish_quickly() {
        let started = Instant::now();
        let char_pattern = format!("src/{}b.rs", "*a".repeat(20) + "*");
        let long_name = format!("src/{}.rs", "a".repeat(40));
        assert!(!path_glob_matches(&char_pattern, &long_name));

        let segment_pattern = format!("{}/b.rs", "**/a".repeat(20));
        let long_path = format!("{}/c.rs", vec!["a"; 40].join("/"));
        assert!(!path_glob_matches(&segment_pattern, &long_path));

        let name_pattern = "*a".repeat(20) + "*b";
        assert!(!segment_glob_matches(&name_pattern, &"a".repeat(40)));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "pathological globs took {:?}",
            started.elapsed()
        );
    }
}

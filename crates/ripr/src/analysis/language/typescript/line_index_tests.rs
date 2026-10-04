//! The per-source line index (`SourceText`) must answer every lookup exactly
//! as the old per-call newline scan did, and extraction on a large module
//! must no longer rescan the file for every node.

use super::*;

#[test]
fn line_index_matches_reference_scan_on_edge_case_sources() {
    // Deterministic pseudo-random sources over newline, CR and multibyte
    // characters, on top of the named edge cases.
    let alphabet = ['a', '\n', '\r', ' ', '\u{e9}', '\u{20ac}', '\u{1f600}'];
    let mut state: u64 = 0x7a5c_71e5_0dd5_0001;
    let mut generated = Vec::new();
    for len in [1_usize, 2, 7, 31, 200] {
        let mut text = String::new();
        for _ in 0..len {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let pick = usize::try_from(state >> 33).unwrap_or_default() % alphabet.len();
            text.push(alphabet[pick]);
        }
        generated.push(text);
    }
    let named = [
        "",
        "x",
        "\n",
        "\n\n\n",
        "line1\nline2\nline3\n",
        "no trailing newline",
        "trailing newline\n",
        "crlf\r\nlines\r\n",
        "\r\n\r\n",
        "lone\rcarriage\rreturns",
        "\u{e9}\n\u{fc}\u{20ac}\n\u{1f600}x\r\n",
        "\u{1f600}\u{1f600}\n\u{1f600}",
    ];
    let mut compared = 0_usize;
    let mut non_boundary = 0_usize;
    for text in named
        .iter()
        .copied()
        .chain(generated.iter().map(String::as_str))
    {
        let source = SourceText::new(text);
        // The index dereferences to exactly the source it was built from.
        assert_eq!(&*source, text);
        // Every offset through the end (the offset equal to the length is an
        // oxc span end at EOF), just past it, and the widest u32 span bound.
        let offsets = (0..=text.len() + 3).chain([u32::MAX as usize, usize::MAX]);
        for offset in offsets {
            if offset < text.len() && !text.is_char_boundary(offset) {
                non_boundary += 1;
            }
            assert_eq!(
                source.line_for_offset(offset),
                line_for_offset(text, offset),
                "offset {offset} in {text:?}"
            );
            compared += 1;
        }
    }
    // The corpus really exercised multibyte interiors, not only boundaries.
    assert!(non_boundary > 20, "non-boundary offsets: {non_boundary}");
    assert!(compared > 500, "compared offsets: {compared}");
}

/// Four lines per function; each function is one extracted owner.
fn generated_large_typescript_module(functions: usize) -> String {
    let mut text = String::new();
    for index in 0..functions {
        text.push_str(&format!(
            "export function f{index}(x: number): number {{\n  if (x > {index}) return x + {index};\n  return x - 1;\n}}\n"
        ));
    }
    text
}

/// Three lines per test; each test carries one `toBe` assertion.
fn generated_large_typescript_test_file(tests: usize) -> String {
    let mut text = String::from("import { f0 } from '../src/large';\n");
    for index in 0..tests {
        text.push_str(&format!(
            "test('case {index}', () => {{\n  expect(f0({index})).toBe({index});\n}});\n"
        ));
    }
    text
}

#[test]
fn owner_and_test_extraction_stay_linear_on_a_large_module() {
    let functions = 5_000;
    let owner_source = generated_large_typescript_module(functions);
    assert_eq!(owner_source.lines().count(), 20_000);
    let test_source = generated_large_typescript_test_file(functions);
    assert_eq!(test_source.lines().count(), 15_001);

    let started = std::time::Instant::now();
    let owners = extract_owners(Path::new("src/large.ts"), &owner_source);
    let tests = extract_tests(Path::new("tests/large.test.ts"), &test_source);
    let elapsed = started.elapsed();

    // The fixtures parsed into one owner per function and one test per
    // registration, with late-file lines computed correctly.
    assert_eq!(owners.len(), functions);
    let last_owner = owners
        .iter()
        .find(|owner| owner.name == "f4999")
        .map(|owner| (owner.start_line, owner.end_line));
    assert_eq!(last_owner, Some((19_997, 20_000)));
    assert_eq!(tests.len(), functions);
    let last_test = tests
        .iter()
        .find(|test| test.name == "case 4999")
        .map(|test| {
            (
                test.line,
                test.assertions
                    .iter()
                    .map(|assertion| assertion.line)
                    .collect::<Vec<_>>(),
            )
        });
    assert_eq!(last_test, Some((14_999, vec![15_000])));

    // Generous bound. In a debug build on these fixtures the per-lookup
    // rescan took about 46s (owners) + 30s (tests), quadrupling with each
    // doubling of the file, and the indexed path about 0.2s + 0.2s.
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "extraction took {elapsed:?}"
    );
}

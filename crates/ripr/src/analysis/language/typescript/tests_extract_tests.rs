//! Focused extraction tests for active Jest/Vitest declaration forms.

use super::*;

#[test]
fn extracts_active_test_modifiers_with_assertions() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
test.only("focused", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
it.concurrent("parallel", () => {
    expect(add(1, 2)).toBe(3);
});
test.sequential("serial", () => {
    expect(normalize("x")).toBe("x");
});
"#,
    );

    assert_eq!(tests.len(), 3);
    assert_eq!(tests[0].local_name, "focused");
    assert_eq!(tests[1].local_name, "parallel");
    assert_eq!(tests[2].local_name, "serial");
    assert!(tests.iter().all(|test| test.assertions.len() == 1));
}

#[test]
fn recurses_active_describe_modifiers() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
describe.only("focused suite", () => {
    test("focused case", () => {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
describe.concurrent("parallel suite", () => {
    it("parallel case", () => {
        expect(add(1, 2)).toBe(3);
    });
});
describe.sequential("serial suite", () => {
    test("serial case", () => {
        expect(normalize("x")).toBe("x");
    });
});
"#,
    );

    assert_eq!(tests.len(), 3);
    assert_eq!(tests[0].name, "focused suite focused case");
    assert_eq!(tests[1].name, "parallel suite parallel case");
    assert_eq!(tests[2].name, "serial suite serial case");
    assert!(tests.iter().all(|test| test.describe_names.len() == 1));
}

#[test]
fn recurses_bare_and_modified_parameterized_suites() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
describe.each([[100], [150]])("amount %i", () => {
    test("discount", () => {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
describe.concurrent.each([[1], [2]])("parallel %i", () => {
    it.only("adds", () => {
        expect(add(1, 2)).toBe(3);
    });
});
"#,
    );

    assert_eq!(tests.len(), 2);
    assert_eq!(tests[0].name, "amount %i discount");
    assert_eq!(tests[0].describe_names, vec!["amount %i".to_string()]);
    assert_eq!(tests[1].name, "parallel %i adds");
    assert_eq!(tests[1].describe_names, vec!["parallel %i".to_string()]);
    assert!(tests.iter().all(|test| test.assertions.len() == 1));
}

#[test]
fn recognizes_active_modifier_chains_before_each_in_either_order() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
test.only.each([
    [100, 90],
    [150, 140],
])("discounts %#", (amount, expected) => {
    expect(applyDiscount(amount, 100)).toBe(expected);
});
it.concurrent.only.each([
    [1, 2, 3],
])("adds concurrently %#", (left, right, expected) => {
    expect(add(left, right)).toBe(expected);
});
test.only.sequential.each([
    ["x", "x"],
])("normalizes sequentially %#", (value, expected) => {
    expect(normalize(value)).toBe(expected);
});
"#,
    );

    assert_eq!(tests.len(), 3);
    assert_eq!(tests[0].local_name, "discounts %#");
    assert_eq!(tests[1].local_name, "adds concurrently %#");
    assert_eq!(tests[2].local_name, "normalizes sequentially %#");
    assert!(tests.iter().all(|test| test.assertions.len() == 1));
}

#[test]
fn keeps_repeated_and_mixed_active_modifier_chains_discoverable() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
test.only.only("repeated focus", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
test.concurrent.concurrent("repeated concurrent", () => {
    expect(add(1, 2)).toBe(3);
});
test.concurrent.sequential("mixed execution flags", () => {
    expect(add(2, 3)).toBe(5);
});
describe.sequential.concurrent("mixed suite flags", () => {
    test("nested active", () => {
        expect(normalize("x")).toBe("x");
    });
});
"#,
    );

    assert_eq!(tests.len(), 4);
    assert_eq!(tests[0].local_name, "repeated focus");
    assert_eq!(tests[1].local_name, "repeated concurrent");
    assert_eq!(tests[2].local_name, "mixed execution flags");
    assert_eq!(tests[3].name, "mixed suite flags nested active");
    assert!(tests.iter().all(|test| test.assertions.len() == 1));
}

#[test]
fn keeps_disabled_conditional_expected_failure_and_unknown_declarations_uncredited() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
test.skip("skipped", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
it.todo("todo");
describe.skip("disabled suite", () => {
    test("nested", () => {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
test.runIf(true)("conditional", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
test.skipIf(false)("conditional skip", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
test.fails("expected failure", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
runner.only("unknown root", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
test.retry("unknown modifier", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
test.only.skip("active then disabled", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
test.skip.only("disabled then active", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
describe.only.skip("active then disabled suite", () => {
    test("nested", () => {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
"#,
    );

    assert!(tests.is_empty());
}

#[test]
fn extracted_active_test_reaches_direct_owner_relation() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
import { applyDiscount } from "../src/pricing";

test.only("discount boundary", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
"#,
    );
    assert_eq!(tests.len(), 1);

    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 20,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        params: Vec::new(),
        exported_as_default: false,
        class_default_export: false,
        module_entries: Vec::new(),
        arity: None,
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::DirectOwnerCall
    );
    assert_eq!(candidates[0].test.name, "discount boundary");
}

/// A mock call chained after another call (`jest.mock("a").mock("b")`) must
/// still record the owner-module registration: the chained callee's object
/// is descended into, so the owner-module mock guard keeps applying. A plain
/// member mock on an unrelated receiver is still not a runner mock call.
#[test]
fn collects_mock_chained_after_a_mock_call() {
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"
jest.mock("../src/pricing").mock("../src/other");
unrelated.mock("../src/pricing");

test("chained mock registration", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert!(
        tests[0]
            .mocks_in_file
            .iter()
            .any(|mock| mock == "../src/pricing"),
        "a registration chained after jest.mock(...) must be collected, got {:?}",
        tests[0].mocks_in_file
    );
    assert_eq!(
        tests[0].mocks_in_file.len(),
        1,
        "the chained ../src/other argument and the unrelated receiver mock are not runner mock registrations, got {:?}",
        tests[0].mocks_in_file
    );
}

/// mocha BDD `context` / `specify` and the TDD / Vitest / `node:test` `suite`
/// register tests exactly like `describe` / `it` (#4548).
#[test]
fn recognizes_mocha_context_specify_and_suite_roots() {
    let tests = extract_tests(
        Path::new("test/pricing.spec.js"),
        r#"
describe("pricing", function () {
    context("with a coupon", function () {
        specify("applies the discount", function () {
            expect(applyDiscount(100, 100)).toBe(90);
        });
    });
});
suite("totals", function () {
    test("adds", function () {
        expect(add(1, 2)).toBe(3);
    });
    suite.only("focused", function () {
        specify.only("normalizes", function () {
            expect(normalize("x")).toBe("x");
        });
    });
});
"#,
    );

    let names: Vec<&str> = tests.iter().map(|test| test.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "pricing with a coupon applies the discount",
            "totals adds",
            "totals focused normalizes",
        ]
    );
    assert!(tests.iter().all(|test| test.assertions.len() == 1));
}

/// `node:test` / Vitest options objects sit between the title and the
/// callback; the callback (and its receiver) is argument 2. A trailing
/// timeout after the callback keeps argument 1 as the body (#4548).
#[test]
fn reads_callback_after_options_object_and_before_timeout() {
    let source = r#"
describe("pricing", { concurrency: 1 }, () => {
    it("discounts", { timeout: 50 }, (t) => {
        t.is(applyDiscount(100, 100), 90);
    });
});
test("adds", () => {
    expect(add(1, 2)).toBe(3);
}, 5000);
"#;
    let tests = extract_tests(Path::new("test/pricing.test.mjs"), source);

    assert_eq!(tests.len(), 2, "{tests:?}");
    assert_eq!(tests[0].name, "pricing discounts");
    // The `t` receiver comes from the callback after the options object.
    assert_eq!(tests[0].assertions.len(), 1, "{:?}", tests[0].assertions);
    assert_eq!(tests[0].assertions[0].matcher, "is");
    assert_eq!(tests[1].name, "adds");
    assert_eq!(tests[1].assertions.len(), 1);
    assert!(
        detect_partial_test_extraction(Path::new("test/pricing.test.mjs"), source, &tests)
            .is_none(),
        "tests inside an options-object describe are extracted, not dropped"
    );
}

/// A `describe` whose title is not a string literal still has its body
/// walked (#4548); it is named by the computed-title placeholder (#4593).
#[test]
fn walks_describe_with_non_literal_title() {
    let source = r#"
describe(Div.name, () => {
    it("renders", () => {
        expect(render(Div)).toBe("<div></div>");
    });
});
describe(`${label} suite`, () => {
    test("formats", () => {
        expect(format(1)).toBe("1");
    });
});
"#;
    let tests = extract_tests(Path::new("test/div.test.ts"), source);

    let names: Vec<&str> = tests.iter().map(|test| test.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "<computed title, line 2> renders",
            "<computed title, line 7> formats"
        ]
    );
    assert_eq!(
        tests[0].describe_names,
        vec!["<computed title, line 2>".to_string()]
    );
    assert!(
        detect_partial_test_extraction(Path::new("test/div.test.ts"), source, &tests).is_none(),
        "tests inside a non-literal describe are extracted, not dropped"
    );
}

/// Negative: skipped mocha spellings stay uncredited and are not reported as
/// dropped registrations.
#[test]
fn keeps_skipped_mocha_and_suite_forms_uncredited() {
    let source = r#"
context.skip("skipped context", function () {
    specify("nested", function () {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
xcontext("x context", function () {
    it("nested", function () {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
specify.skip("skipped specify", function () {
    expect(applyDiscount(100, 100)).toBe(90);
});
xit("x it", function () {
    expect(applyDiscount(100, 100)).toBe(90);
});
suite.skip("skipped suite", function () {
    test("nested", function () {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
describe.skip("skipped with options", { timeout: 5 }, () => {
    it("nested", () => {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
"#;
    let tests = extract_tests(Path::new("test/pricing.spec.js"), source);

    assert!(tests.is_empty(), "{tests:?}");
}

/// Negative (#4638 review): a `node:test` / Vitest options object that skips,
/// marks todo, or inverts (`fails`) the registration registers no running
/// discriminator — exactly like `.skip` / `.todo` / `.fails`. Neither the
/// test nor a skipped describe's body is extracted, and a skipped
/// registration is not reported as a dropped test.
#[test]
fn options_object_skip_todo_fails_registrations_stay_uncredited() {
    let source = r#"
test("skipped", { skip: true }, () => {
    assert.strictEqual(isAdult(18), true);
});
it("todo", { todo: true }, () => {
    assert.strictEqual(isAdult(18), true);
});
it("skip reason", { skip: "flaky" }, (t) => {
    t.is(isAdult(18), true);
});
test("fails", { fails: true }, () => {
    expect(isAdult(18)).toBe(true);
});
test("dynamic skip", { skip: process.env.CI }, () => {
    expect(isAdult(18)).toBe(true);
});
test("spread options", { ...options }, () => {
    expect(isAdult(18)).toBe(true);
});
test("computed key", { [key]: true }, () => {
    expect(isAdult(18)).toBe(true);
});
test("shorthand", { skip }, () => {
    expect(isAdult(18)).toBe(true);
});
test("legacy trailing options", () => {
    expect(isAdult(18)).toBe(true);
}, { skip: true });
test({ skip: true }, () => {
    assert.strictEqual(isAdult(18), true);
});
test.each([[18]])("each skipped %i", { skip: true }, (age) => {
    expect(isAdult(age)).toBe(true);
});
describe("skipped suite", { skip: "flaky" }, () => {
    it("nested", () => {
        expect(isAdult(18)).toBe(true);
    });
});
"#;
    let file = Path::new("test/calc.test.js");
    let tests = extract_tests(file, source);

    assert!(tests.is_empty(), "{tests:?}");
    // The top-level skipped registrations are not "dropped" (a `.skip` call
    // is not reported either).
    let top_level_only = source
        .split("describe(\"skipped suite\"")
        .next()
        .unwrap_or_default();
    assert!(
        detect_partial_test_extraction(file, top_level_only, &[]).is_none(),
        "options-skipped registrations are not dropped registrations"
    );
    // A describe skipped through its options object discloses exactly what
    // `describe.skip` discloses for the same body.
    let skipped_by_options = r#"
describe("skipped suite", { skip: true }, () => {
    it("nested", () => {
        expect(isAdult(18)).toBe(true);
    });
});
"#;
    let skipped_by_modifier = r#"
describe.skip("skipped suite", () => {
    it("nested", () => {
        expect(isAdult(18)).toBe(true);
    });
});
"#;
    assert_eq!(
        detect_partial_test_extraction(file, skipped_by_options, &[]).map(|gap| gap.shape),
        detect_partial_test_extraction(file, skipped_by_modifier, &[]).map(|gap| gap.shape),
    );
}

/// Positive control (#4638 review): options that leave the registration
/// running (`skip: false`, `todo: undefined`, `only`, `timeout`) keep it
/// extracted.
#[test]
fn options_object_with_inactive_skip_values_stays_extracted() {
    let source = r#"
describe("suite", { skip: false, concurrency: 1 }, () => {
    it("runs", { todo: undefined, timeout: 50 }, () => {
        expect(isAdult(18)).toBe(true);
    });
    test("focused", { only: true, fails: false }, () => {
        expect(isAdult(18)).toBe(true);
    });
});
"#;
    let file = Path::new("test/calc.test.js");
    let tests = extract_tests(file, source);

    let names: Vec<&str> = tests.iter().map(|test| test.name.as_str()).collect();
    assert_eq!(names, vec!["suite runs", "suite focused"]);
    assert!(detect_partial_test_extraction(file, source, &tests).is_none());
}

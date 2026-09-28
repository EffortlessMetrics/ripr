//! Tests for the TypeScript boundary input derivation (#4215 follow-up).

use super::*;
use std::path::Path;

const CHANGED_LINE: &str = "  if (amount >= DISCOUNT_THRESHOLD) {";

/// The boundary input for `changed_line` inside `owner_name`, extracted from
/// `source` exactly as the adapter extracts it. The outer `None` means the
/// owner was not extracted (for example a parse failure), so a negative case
/// asserts `Some(None)` and can never pass on a broken source alone.
fn input_for(
    source: &str,
    owner_name: &str,
    changed_line: &str,
) -> Option<Option<TypeScriptBoundaryInput>> {
    let owners = extract_owners(Path::new("src/pricing.ts"), source);
    let owner = owners.iter().find(|owner| owner.name == owner_name)?;
    // The adapter passes the diff's line number; the last line with this
    // text stands in for it here. A line-shape case whose text is not in the
    // module sits where the module's own comparison is.
    let line_of = |wanted: &str| {
        source
            .lines()
            .enumerate()
            .filter(|(_, text)| text.trim_end() == wanted.trim_end())
            .map(|(index, _)| index + 1)
            .last()
    };
    let line = line_of(changed_line).or_else(|| line_of(CHANGED_LINE))?;
    Some(ts_boundary_input_in_source(
        source,
        line,
        changed_line,
        owner,
    ))
}

fn pricing_module(prelude: &str, body_extra: &str, postlude: &str) -> String {
    format!(
        "{prelude}\nexport function discountedTotal(amount: number): number {{\n{body_extra}\n  if (amount >= DISCOUNT_THRESHOLD) {{\n    return amount - Math.floor(amount / 10);\n  }}\n  return amount;\n}}\n{postlude}\n"
    )
}

fn resolved(value: i64) -> Option<TypeScriptBoundaryInput> {
    Some(TypeScriptBoundaryInput {
        parameter: "amount".to_string(),
        index: 0,
        operand: "DISCOUNT_THRESHOLD".to_string(),
        value,
    })
}

#[test]
fn same_file_integer_const_resolves_to_the_boundary_input() {
    let source = pricing_module("export const DISCOUNT_THRESHOLD = 10000;", "", "");
    // Setup: the owner parsed with its single plain parameter.
    let owners = extract_owners(Path::new("src/pricing.ts"), &source);
    let owner = owners.iter().find(|owner| owner.name == "discountedTotal");
    assert_eq!(
        owner.map(|owner| owner.params.clone()),
        Some(vec!["amount".to_string()])
    );

    let input = input_for(&source, "discountedTotal", CHANGED_LINE).flatten();
    assert_eq!(input, resolved(10000));
    assert_eq!(
        input.map(|input| input.evidence_line()),
        Some(
            "typescript_boundary_input: parameter=amount;index=0;operand=DISCOUNT_THRESHOLD;value=10000"
                .to_string()
        )
    );
}

#[test]
fn benign_module_shapes_still_resolve() {
    for (label, prelude, body_extra, postlude) in [
        (
            "underscore literal",
            "const DISCOUNT_THRESHOLD: number = 10_000;",
            "",
            "",
        ),
        (
            "comments and strings name both",
            "// DISCOUNT_THRESHOLD = 5 and amount = 1 in a comment\nexport const DISCOUNT_THRESHOLD = 10000;",
            "  /* let amount = 2; */ const label = \"amount = DISCOUNT_THRESHOLD\";",
            "",
        ),
        (
            "reads in calls, templates, typeof, and re-exports",
            "const DISCOUNT_THRESHOLD = 10000;\nexport { DISCOUNT_THRESHOLD };\nexport const cap = Math.max(1, DISCOUNT_THRESHOLD);",
            "  log(`${amount} vs ${DISCOUNT_THRESHOLD}`, typeof DISCOUNT_THRESHOLD, format(amount));",
            "export default DISCOUNT_THRESHOLD;",
        ),
        (
            "a member of the same name is a different property",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  const limits = order.amount + config.DISCOUNT_THRESHOLD;",
            "",
        ),
    ] {
        let source = pricing_module(prelude, body_extra, postlude);
        assert_eq!(
            input_for(&source, "discountedTotal", CHANGED_LINE).flatten(),
            resolved(10000),
            "{label}: {source}"
        );
    }
}

#[test]
fn constant_written_first_resolves_with_the_parameter_side() {
    let source = "export const DISCOUNT_THRESHOLD = 10000;\nexport function discountedTotal(amount: number): number {\n  if (DISCOUNT_THRESHOLD <= amount) {\n    return 1;\n  }\n  return amount;\n}\n";
    assert_eq!(
        input_for(
            source,
            "discountedTotal",
            "  if (DISCOUNT_THRESHOLD <= amount) {"
        )
        .flatten(),
        resolved(10000)
    );
}

#[test]
fn literal_boundary_binds_the_parameter_position() {
    let source = "export function applyRate(rate: number, amount: number): number {\n  if (amount >= 100) {\n    return rate;\n  }\n  return 0;\n}\n";
    assert_eq!(
        input_for(source, "applyRate", "  if (amount >= 100) {").flatten(),
        Some(TypeScriptBoundaryInput {
            parameter: "amount".to_string(),
            index: 1,
            operand: "100".to_string(),
            value: 100,
        })
    );
}

#[test]
fn rebindable_or_non_literal_constants_stay_unresolved() {
    for (label, prelude, body_extra, postlude) in [
        ("let", "export let DISCOUNT_THRESHOLD = 10000;", "", ""),
        ("var", "export var DISCOUNT_THRESHOLD = 10000;", "", ""),
        (
            "float",
            "export const DISCOUNT_THRESHOLD = 10000.5;",
            "",
            "",
        ),
        ("exponent", "export const DISCOUNT_THRESHOLD = 1e4;", "", ""),
        ("hex", "export const DISCOUNT_THRESHOLD = 0x2710;", "", ""),
        (
            "bigint",
            "export const DISCOUNT_THRESHOLD = 10000n;",
            "",
            "",
        ),
        (
            "computed",
            "export const DISCOUNT_THRESHOLD = limit();",
            "",
            "",
        ),
        (
            "as const",
            "export const DISCOUNT_THRESHOLD = 10000 as const;",
            "",
            "",
        ),
        ("negative", "export const DISCOUNT_THRESHOLD = -1;", "", ""),
        (
            "declare only",
            "export declare const DISCOUNT_THRESHOLD: number;",
            "",
            "",
        ),
        ("no declaration", "", "", ""),
        (
            "imported from another module",
            "import { DISCOUNT_THRESHOLD } from './config';",
            "",
            "",
        ),
        (
            "imported under an alias",
            "import { LIMIT as DISCOUNT_THRESHOLD } from './config';",
            "",
            "",
        ),
        (
            "declared twice at the top level",
            "const DISCOUNT_THRESHOLD = 10000;\nconst DISCOUNT_THRESHOLD = 5;",
            "",
            "",
        ),
        (
            "destructured at the top level",
            "export const DISCOUNT_THRESHOLD = 10000;\nconst [DISCOUNT_THRESHOLD] = [5];",
            "",
            "",
        ),
        (
            "shadowed by a local const in the owner",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  const DISCOUNT_THRESHOLD = 5;",
            "",
        ),
        (
            "shadowed by a nested block let",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  { let DISCOUNT_THRESHOLD = 5; }",
            "",
        ),
        (
            "shadowed by a nested arrow parameter",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  const check = (DISCOUNT_THRESHOLD: number) => amount;",
            "",
        ),
        (
            "shadowed by a bare arrow parameter",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  const check = DISCOUNT_THRESHOLD => amount;",
            "",
        ),
        (
            "shadowed by a catch binding",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  try { run(); } catch (DISCOUNT_THRESHOLD) { log(); }",
            "",
        ),
        (
            "shadowed by a for-of binding",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  for (const DISCOUNT_THRESHOLD of [1]) { log(); }",
            "",
        ),
        (
            "shadowed by object destructuring",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  const { DISCOUNT_THRESHOLD } = settings;",
            "",
        ),
        (
            "shadowed by renamed destructuring",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  const { limit: DISCOUNT_THRESHOLD } = settings;",
            "",
        ),
        (
            "nested function of the same name",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  function DISCOUNT_THRESHOLD() { return 5; }",
            "",
        ),
        (
            "nested generator of the same name",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  function* DISCOUNT_THRESHOLD() { yield 5; }",
            "",
        ),
        (
            "enum member of the same name",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "",
            "enum Limits { DISCOUNT_THRESHOLD = 5 }",
        ),
        (
            "namespace const of the same name",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "",
            "namespace Limits { export const DISCOUNT_THRESHOLD = 5; }",
        ),
        (
            "class of the same name elsewhere",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "",
            "class DISCOUNT_THRESHOLD {}",
        ),
        (
            "object key write of the same name",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "",
            "exports.DISCOUNT_THRESHOLD = 5;",
        ),
        (
            "object literal key of the same name",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "",
            "export const settings = { DISCOUNT_THRESHOLD: 5 };",
        ),
        (
            "eval in the module",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  eval(code);",
            "",
        ),
        (
            "escaped identifier",
            "export const DISCOUNT_THRESHOLD = 10000;",
            "  const \\u0044ISCOUNT_THRESHOLD = 5;",
            "",
        ),
    ] {
        let source = pricing_module(prelude, body_extra, postlude);
        assert_eq!(
            input_for(&source, "discountedTotal", CHANGED_LINE),
            Some(None),
            "{label}: {source}"
        );
    }
}

#[test]
fn owner_parameter_shadow_of_the_constant_stays_unresolved() {
    let source = "export const DISCOUNT_THRESHOLD = 10000;\nexport function discountedTotal(amount: number, DISCOUNT_THRESHOLD = 5): number {\n  if (amount >= DISCOUNT_THRESHOLD) {\n    return 1;\n  }\n  return amount;\n}\n";
    assert_eq!(
        input_for(source, "discountedTotal", CHANGED_LINE),
        Some(None)
    );
}

#[test]
fn written_or_rebound_parameters_stay_unresolved() {
    for (label, body_extra) in [
        ("assignment", "  amount = Math.round(amount);"),
        ("compound assignment", "  amount += 1;"),
        ("postfix update", "  amount++;"),
        ("prefix update", "  --amount;"),
        ("var redeclaration", "  var amount = 2;"),
        ("nested block let", "  { let amount = 2; log(amount); }"),
        ("array destructuring write", "  [amount] = [1];"),
        ("object destructuring write", "  ({ amount } = settings);"),
        ("for-of write", "  for (amount of [1]) { log(); }"),
        ("for-in write", "  for (amount in settings) { log(); }"),
        (
            "nested closure parameter",
            "  const f = (amount: number) => amount;",
        ),
        (
            "nested function parameter",
            "  function inner(amount: number) { return amount; }",
        ),
        ("arguments alias", "  arguments[0] = 5;"),
    ] {
        let source = pricing_module("export const DISCOUNT_THRESHOLD = 10000;", body_extra, "");
        assert_eq!(
            input_for(&source, "discountedTotal", CHANGED_LINE),
            Some(None),
            "{label}: {source}"
        );
    }
}

#[test]
fn comparisons_without_a_single_plain_parameter_stay_unresolved() {
    let module = "export const LIMIT = 10;\n";
    for (label, signature, line) in [
        (
            "both sides are parameters",
            "(amount: number, limit: number)",
            "  if (amount >= limit) {",
        ),
        (
            "member receiver",
            "(order: Order)",
            "  if (order.amount >= LIMIT) {",
        ),
        (
            "length receiver",
            "(user: string)",
            "  if (user.length >= LIMIT) {",
        ),
        (
            "destructured parameter",
            "({ amount }: Order)",
            "  if (amount >= LIMIT) {",
        ),
        (
            "rest parameter",
            "(...amount: number[])",
            "  if (amount >= LIMIT) {",
        ),
        (
            "nullish boundary",
            "(amount?: number)",
            "  if (amount ?? 10) {",
        ),
        (
            "float literal",
            "(amount: number)",
            "  if (amount >= 10.5) {",
        ),
    ] {
        let source = format!(
            "{module}export function check{signature}: number {{\n{line}\n    return 1;\n  }}\n  return 0;\n}}\n"
        );
        assert_eq!(
            input_for(&source, "check", line),
            Some(None),
            "{label}: {source}"
        );
    }
}

#[test]
fn unparseable_module_stays_unresolved() {
    // The owner is extracted from a parseable copy; the module read at
    // derivation time does not parse.
    let good = pricing_module("export const DISCOUNT_THRESHOLD = 10000;", "", "");
    let owners = extract_owners(Path::new("src/pricing.ts"), &good);
    let owner = owners.iter().find(|owner| owner.name == "discountedTotal");
    assert!(owner.is_some(), "setup: owner must be extracted");
    let broken = pricing_module("export const DISCOUNT_THRESHOLD = 10000;", "", "const = ;");
    assert_eq!(
        owner.and_then(|owner| ts_boundary_input_in_source(&broken, 4, CHANGED_LINE, owner)),
        None
    );
    assert_eq!(
        owner.and_then(|owner| ts_boundary_input_in_source(&good, 4, CHANGED_LINE, owner)),
        resolved(10000)
    );
}

/// Review finding on #4429: operand extraction keeps only the token nearest
/// the comparison, so arithmetic, a sign, or a member read around either side
/// used to derive the operand's value as the boundary input. Each shape moves
/// the real boundary, so no input is derived.
#[test]
fn comparisons_with_arithmetic_or_signed_sides_stay_unresolved() {
    let source = pricing_module(
        "export const DISCOUNT_THRESHOLD = 10000;\nexport const OFFSET = 5;",
        "",
        "",
    );
    // Control: the plain comparison still resolves, so the negatives below
    // fail on the changed line, not on the module.
    assert_eq!(
        input_for(&source, "discountedTotal", CHANGED_LINE),
        Some(resolved(10000))
    );
    for changed in [
        "  if (OFFSET + amount >= DISCOUNT_THRESHOLD) {",
        "  if (amount >= DISCOUNT_THRESHOLD + 1) {",
        "  if (amount * 2 > DISCOUNT_THRESHOLD) {",
        "  if (2 * amount >= DISCOUNT_THRESHOLD) {",
        "  if (amount - OFFSET >= DISCOUNT_THRESHOLD) {",
        "  if (DISCOUNT_THRESHOLD <= amount < OFFSET) {",
        "  if (amount >= -5) {",
        "  if (order.amount >= DISCOUNT_THRESHOLD) {",
        "  if (amount >= DISCOUNT_THRESHOLD",
        "  amount >= DISCOUNT_THRESHOLD ? 1 : 0;",
        "  if (amount > OFFSET) { log('amount >= DISCOUNT_THRESHOLD ;'); }",
    ] {
        assert_eq!(
            input_for(&source, "discountedTotal", changed),
            Some(None),
            "{changed}"
        );
    }
}

#[test]
fn whole_side_comparisons_accept_common_line_shapes() {
    for (line, expected) in [
        ("  if (amount >= LIMIT) {", true),
        ("  return amount >= LIMIT ? 1 : 0;", true),
        ("  const big = LIMIT <= amount && flag;", true),
        // Something before the comparison can skip evaluating it.
        ("if (ok || amount === 5_000) {", false),
        ("  return ready ? amount >= LIMIT : false;", false),
        ("  if (flag && amount >= LIMIT) {", false),
        ("  items.forEach((x) => amount >= LIMIT);", false),
        // Only `if (`, `return`, or `const|let|var NAME =` may open the
        // statement before the comparison (second review on #4429).
        ("  if (express) return amount >= LIMIT ? 0 : 900;", false),
        ("  if (skip) return 0; const big = amount >= LIMIT;", false),
        ("  if (a) {} else if (amount >= LIMIT) {", false),
        ("  if (ready) if (amount >= LIMIT) {", false),
        ("  const o = { f() { return amount >= LIMIT; } };", false),
        ("  try { g(); } catch { return amount >= LIMIT; }", false),
        ("  let big = amount >= LIMIT;", true),
        ("  if (amount >= LIMIT + 1) {", false),
        ("  if (x + amount >= LIMIT) {", false),
        ("  if (amount >= -LIMIT) {", false),
        ("  if (amount >= LIMIT || amount >= LIMIT) {", false),
        // The expression may continue on a neighbouring line.
        ("  if (amount >= LIMIT", false),
        ("  amount >= LIMIT ? 1 : 0;", false),
        ("  return amount >= LIMIT", false),
        // Another comparison on the line may be the changed one.
        ("  if (amount >= LIMIT || amount >= CAP) {", false),
        ("  if (amount >= LIMIT && total > 0) {", false),
        // Literal and comment spans are not tokenized as code.
        (
            "  msg = amount > CAP ? \"over: amount >= LIMIT, deny\" : ok;",
            false,
        ),
        (
            "  msg = amount > CAP ? `over: amount >= LIMIT, deny` : ok;",
            false,
        ),
        ("  ok = amount > CAP; // note: amount >= LIMIT ;", false),
        ("  ok = amount > CAP; /* amount >= LIMIT ; */", false),
    ] {
        let operand = if line.contains("5_000") {
            "5_000"
        } else {
            "LIMIT"
        };
        assert_eq!(
            super::boundary_input::comparison_has_whole_sides(line, "amount", operand),
            expected,
            "{line}"
        );
    }
}

/// A compared value derived from the parameter (`scaled = amount * 10`) is
/// not the parameter, so no input is derived for `fee(3)`-style calls; the
/// derivation only binds a comparison side that is the owner's own
/// read-only positional parameter.
#[test]
fn comparison_on_a_value_derived_from_the_parameter_stays_unresolved() {
    let source = "export function fee(amount: number): number {\n  const scaled = amount * 10;\n  if (scaled > 30) {\n    return 5;\n  }\n  return 0;\n}\n";
    let owners = extract_owners(Path::new("src/pricing.ts"), source);
    // Setup: the owner parsed with its single plain parameter.
    assert_eq!(
        owners
            .iter()
            .find(|owner| owner.name == "fee")
            .map(|owner| owner.params.clone()),
        Some(vec!["amount".to_string()])
    );
    assert_eq!(input_for(source, "fee", "  if (scaled > 30) {"), Some(None));
}

/// Source challenge on #4429: an earlier guard returns before the changed
/// comparison, so the derived input (`shipping(5000)`) never executes it.
/// Operand equality alone must not become a boundary input.
#[test]
fn changed_line_behind_an_early_exit_or_nested_block_stays_unresolved() {
    let changed = "  if (amount > 5000) {";
    let shipping = |before: &str, wrap: (&str, &str)| {
        format!(
            "export function shipping(amount: number): number {{\n{before}{}{changed}\n    return 1;\n  }}\n{}  return 2;\n}}\n",
            wrap.0, wrap.1
        )
    };
    let positive = shipping("  const fee = 1;\n", ("", ""));
    assert_eq!(
        input_for(&positive, "shipping", changed),
        Some(Some(TypeScriptBoundaryInput {
            parameter: "amount".to_string(),
            index: 0,
            operand: "5000".to_string(),
            value: 5000,
        })),
        "control: an unguarded top-level comparison still derives"
    );
    for (label, source) in [
        (
            "early return guard",
            shipping("  if (amount === 5000) return 0;\n", ("", "")),
        ),
        (
            "early throw",
            shipping(
                "  if (amount < 0) {\n    throw new Error(\"neg\");\n  }\n",
                ("", ""),
            ),
        ),
        (
            "earlier loop that may not finish for the input",
            shipping("  while (amount === 5000) {}\n", ("", "")),
        ),
        (
            "earlier do-while loop",
            shipping("  do {} while (amount === 5000);\n", ("", "")),
        ),
        (
            "earlier for loop",
            shipping("  for (let i = 0; i < amount; i++) {}\n", ("", "")),
        ),
        (
            "enclosing if",
            shipping("", ("  if (amount < 100) {\n", "  }\n")),
        ),
        (
            "enclosing loop",
            shipping("", ("  for (const x of [1]) {\n", "  }\n")),
        ),
        (
            "enclosing callback",
            shipping("", ("  [1].forEach(() => {\n", "  });\n")),
        ),
        (
            "changed line appears twice",
            shipping(&format!("{changed}\n    return 3;\n  }}\n"), ("", "")),
        ),
    ] {
        assert_eq!(
            input_for(&source, "shipping", changed),
            Some(None),
            "{label}: {source}"
        );
    }
    // The changed line must start a fresh statement: a braceless body or an
    // `else` branch may skip it (independent review on #4429). The continued
    // expressions are already refused by the line checks; they stay here as
    // guards for the same skip.
    let owner = |body: &str| {
        format!("export function shipping(amount: number): number {{\n{body}  return 2;\n}}\n")
    };
    for (label, body, line) in [
        (
            "else branch of a one-line if",
            "  if (amount === 5000) log();\n  else if (amount > 5000) {\n    return 1;\n  }\n",
            "  else if (amount > 5000) {",
        ),
        (
            "else on the line after a block",
            "  if (amount === 5000) {\n    log();\n  }\n  else if (amount > 5000) {\n    return 1;\n  }\n",
            "  else if (amount > 5000) {",
        ),
        (
            "braceless if body",
            "  if (amount !== 5000)\n    if (amount > 5000) {\n      return 1;\n    }\n",
            "    if (amount > 5000) {",
        ),
        (
            "braceless loop body",
            "  for (const x of [1])\n    if (amount > 5000) {\n      return 1;\n    }\n",
            "    if (amount > 5000) {",
        ),
        (
            "ternary continued",
            "  const fee = amount === 5000 ? 0 :\n    (amount > 5000) ? 1 : 2;\n",
            "    (amount > 5000) ? 1 : 2;",
        ),
        (
            "short circuit continued",
            "  const fee = amount !== 5000 &&\n    (amount > 5000);\n",
            "    (amount > 5000);",
        ),
        (
            "arrow body continued",
            "  const cb = (x: number) =>\n    (amount > 5000);\n",
            "    (amount > 5000);",
        ),
    ] {
        let source = owner(body);
        assert_eq!(
            input_for(&source, "shipping", line),
            Some(None),
            "{label}: {source}"
        );
    }
}

/// Second review on #4429: a changed line inside a block comment holds no
/// code, so the next code line must not stand in for it. The adapter's line
/// number is authoritative; the module's own line must hold the comparison.
#[test]
fn changed_line_inside_a_block_comment_stays_unresolved() {
    let source = "export function shipping(amount: number): number {\n  /*\n  if (amount >= 5000) {\n  */\n  if (amount > 9000) {\n    return 0;\n  }\n  return 500;\n}\n";
    let owners = extract_owners(Path::new("src/shipping.ts"), source);
    let owner = owners.iter().find(|owner| owner.name == "shipping");
    assert!(owner.is_some(), "setup: owner must be extracted");
    let derive = |line: usize, text: &str| {
        owner.and_then(|owner| ts_boundary_input_in_source(source, line, text, owner))
    };
    assert_eq!(derive(3, "  if (amount >= 5000) {"), None);
    // Fifth review: a tree that does not match the diff (here the diff says
    // `>=` where the module says `>`) fails closed, since the reach rules
    // read this tree.
    assert_eq!(derive(5, "  if (amount >= 9000) {"), None);
    // Control: the real code line with its own operand derives.
    assert_eq!(
        derive(5, "  if (amount > 9000) {").map(|input| input.value),
        Some(9000)
    );
}

/// Third review on #4429: the stacked `{` must be the owner's own body. A
/// curried arrow or a returned function runs its body only when the result
/// is called, and a generator only when iterated.
#[test]
fn nested_function_bodies_and_generators_stay_unresolved() {
    let changed = "  if (amount > 5000) {";
    let tail = "\n    return 0;\n  }\n  return 500;\n";
    for (label, head, close) in [
        (
            "curried arrow",
            "export const shipping = (amount: number) => (): number => {",
            "};",
        ),
        (
            "returned function",
            "export const shipping = (amount: number) => function () {",
            "};",
        ),
        (
            "returned async arrow",
            "export const shipping = (amount: number) => async () => {",
            "};",
        ),
        (
            "generator",
            "export function* shipping(amount: number) {",
            "}",
        ),
        (
            "type literal return then thunk",
            "export const shipping = (amount: number): { (): number } => () => {",
            "};",
        ),
        (
            "generic type literal return then async thunk",
            "export const shipping = (amount: number): Promise<{ ok: boolean }> => async () => {",
            "};",
        ),
    ] {
        let source = format!("{head}\n{changed}{tail}{close}\n");
        let owners = extract_owners(Path::new("src/shipping.ts"), &source);
        let owner = owners.iter().find(|owner| owner.name == "shipping");
        assert!(owner.is_some(), "setup: {label} owner must be extracted");
        assert_eq!(
            owner.and_then(|owner| ts_boundary_input_in_source(&source, 2, changed, owner)),
            None,
            "{label}: {source}"
        );
    }
    // Controls: a block-bodied arrow owner and a plain return type derive.
    for head in [
        "export const shipping = (amount: number): number => {",
        "export function shipping(amount: number): Promise<number> | number {",
    ] {
        let source = format!("{head}\n{changed}{tail}}}\n");
        let owners = extract_owners(Path::new("src/shipping.ts"), &source);
        let owner = owners.iter().find(|owner| owner.name == "shipping");
        assert_eq!(
            owner
                .and_then(|owner| ts_boundary_input_in_source(&source, 2, changed, owner))
                .map(|input| input.value),
            Some(5000),
            "control: {source}"
        );
    }
}

#[test]
fn changed_line_after_an_await_stays_unresolved() {
    let changed = "  if (amount > 5000) {";
    let owner = |before: &str| {
        format!(
            "export async function shipping(amount: number): Promise<number> {{\n{before}{changed}\n    return 1;\n  }}\n  return 2;\n}}\n"
        )
    };
    assert!(
        matches!(
            input_for(&owner("  const fee = 1;\n"), "shipping", changed),
            Some(Some(_))
        ),
        "control: an async owner without an earlier await still derives"
    );
    assert_eq!(
        input_for(&owner("  await settle(amount);\n"), "shipping", changed),
        Some(None),
        "a rejected or never-settling await can stop the boundary input first"
    );
}

//! Boundary input derivation for a changed TypeScript predicate (#4215 follow-up).
//!
//! A changed comparison `amount >= DISCOUNT_THRESHOLD` names the missing
//! discriminator `amount == DISCOUNT_THRESHOLD`. The repair-packet projection
//! can only hand an agent a concrete target when it knows which call input
//! hits that boundary. This module derives that input from the owner's own
//! module, and only when nothing in the module could make the derived input
//! wrong:
//!
//! - one side of the discriminator is the owner's positional parameter
//!   (plain binding, no rest or destructuring), and the owner never writes,
//!   redeclares, or shadows that name after its signature;
//! - the other side is a plain integer literal, or an UPPER_CASE name bound
//!   exactly once in the module, at the top level, by an immutable
//!   `const NAME = <integer literal>` — and every other occurrence of that
//!   name in the module is a plain read (no parameter, local, catch, import,
//!   function, class, enum, or destructuring binding of the same name).
//!
//! - the changed line runs on every call: the module's line at the changed
//!   line number equals the diff's changed line (the tree at `--root` is
//!   read as the diff's post-image), and it sits at the top level of a
//!   block-bodied owner, after no `return`/`throw`/`break`/`continue`/`yield`,
//!   loop, or `await`, and inside no nested block; it starts a fresh
//!   statement, and that statement opens with the comparison (`if (`,
//!   `return`, or `const|let|var NAME =` directly before its first side). A
//!   helper call before the line that throws or never returns for the
//!   boundary value is a static limit this does not see, as is any earlier
//!   statement that throws for the boundary value.
//!
//! The module rules run on the oxc token stream, so comments, strings,
//! templates, and regular expressions never pass for code there; the changed
//! line itself must compare the parameter and the operand as whole sides and
//! carry no literal or comment span at all. Anything the rules do not
//! positively recognize fails closed: no evidence line, and the projection
//! keeps the packet non-delegatable.

use super::*;
use oxc_parser::Kind;
use oxc_parser::config::TokensParserConfig;

/// Evidence prefix consumed by the TypeScript repair-packet projection.
pub(crate) const TYPESCRIPT_BOUNDARY_INPUT_PREFIX: &str = "typescript_boundary_input: ";

/// Evidence prefix for a changed comparison between two owner parameters
/// (#4759), consumed by the same projection.
pub(crate) const TYPESCRIPT_BOUNDARY_PARAMETERS_PREFIX: &str = "typescript_boundary_parameters: ";

/// The statically derived call input that hits a changed predicate boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeScriptBoundaryInput {
    /// The owner parameter the changed comparison reads.
    pub(crate) parameter: String,
    /// Its positional index in the owner signature.
    pub(crate) index: usize,
    /// The other discriminator operand, as written (`DISCOUNT_THRESHOLD`, `100`).
    pub(crate) operand: String,
    /// The integer value that operand resolves to.
    pub(crate) value: i64,
}

impl TypeScriptBoundaryInput {
    pub(crate) fn evidence_line(&self) -> String {
        format!(
            "{TYPESCRIPT_BOUNDARY_INPUT_PREFIX}parameter={};index={};operand={};value={}",
            self.parameter, self.index, self.operand, self.value
        )
    }
}

/// A changed comparison whose two sides are both owner parameters
/// (`amount >= threshold`, #4759). No value is known here: the call's own
/// arguments at the two positions decide whether it hits the boundary, and
/// the projection reads them from the observed call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeScriptBoundaryParameters {
    /// The parameter on the discriminator's left side.
    pub(crate) parameter: String,
    /// Its positional index in the owner signature.
    pub(crate) index: usize,
    /// The parameter on the discriminator's right side.
    pub(crate) operand: String,
    /// Its positional index in the owner signature.
    pub(crate) operand_index: usize,
}

impl TypeScriptBoundaryParameters {
    pub(crate) fn evidence_line(&self) -> String {
        format!(
            "{TYPESCRIPT_BOUNDARY_PARAMETERS_PREFIX}parameter={};index={};operand={};operand_index={}",
            self.parameter, self.index, self.operand, self.operand_index
        )
    }
}

/// The boundary input for a changed predicate line, read from the owner's
/// module through the workspace root, as its evidence line. `None` whenever
/// any rule fails.
pub(crate) fn ts_boundary_fact_for_change(
    probe_shape: &TypeScriptProbeShape,
    line: usize,
    line_text: &str,
    owner: &TypeScriptOwner,
    workspace_root: Option<&Path>,
) -> Option<String> {
    if !probe_shape.specific || probe_shape.family != ProbeFamily::Predicate {
        return None;
    }
    let root = workspace_root?;
    let source = std::fs::read_to_string(root.join(&owner.file)).ok()?;
    ts_boundary_input_in_source(&source, line, line_text, owner)
        .map(|input| input.evidence_line())
        .or_else(|| {
            ts_boundary_parameters_in_source(&source, line, line_text, owner)
                .map(|parameters| parameters.evidence_line())
        })
}

/// One discriminator side as written, with its owner-parameter index.
type DiscriminatorSide = (String, Option<usize>);

/// The changed line's `==` discriminator split into its two sides, with the
/// owner-parameter index of each side. `None` for a nullish discriminator or
/// an owner without a fixed positional signature.
fn discriminator_sides(
    line_text: &str,
    owner: &TypeScriptOwner,
) -> Option<(DiscriminatorSide, DiscriminatorSide)> {
    let (boundary, nullish) = typescript_boundary_discriminator_with_shape(line_text)?;
    if nullish {
        return None;
    }
    let (left, right) = boundary.split_once(" == ")?;
    let (left, right) = (left.trim(), right.trim());
    // `params` is recorded only for a fixed positional signature of plain
    // bindings.
    owner.arity?;
    let param_index = |operand: &str| owner.params.iter().position(|param| param == operand);
    Some((
        (left.to_string(), param_index(left)),
        (right.to_string(), param_index(right)),
    ))
}

/// [`ts_boundary_fact_for_change`] over an already-read module source, for
/// a comparison between one parameter and a literal or module constant.
pub(crate) fn ts_boundary_input_in_source(
    source: &str,
    line: usize,
    line_text: &str,
    owner: &TypeScriptOwner,
) -> Option<TypeScriptBoundaryInput> {
    let ((left, left_index), (right, right_index)) = discriminator_sides(line_text, owner)?;
    // Exactly one side may name a parameter.
    let (parameter, index, operand) = match (left_index, right_index) {
        (Some(index), None) => (left, index, right),
        (None, Some(index)) => (right, index, left),
        _ => return None,
    };
    if !comparison_has_whole_sides(line_text, &parameter, &operand) {
        return None;
    }
    let literal = integer_literal(&operand);
    let constant = if literal.is_none() {
        is_constant_shaped_operand(&operand).then(|| operand.clone())
    } else {
        None
    };
    if literal.is_none() && constant.is_none() {
        return None;
    }
    let request = ModuleRequest {
        parameters: vec![parameter.clone()],
        owner_start_line: owner.start_line,
        owner_end_line: owner.end_line,
        constant,
        changed_line: line,
        changed_text: line_text.to_string(),
    };
    let resolved = module_admits(owner, source, request)?;
    let value = match resolved {
        Some(value) => value,
        None => literal?,
    };
    Some(TypeScriptBoundaryInput {
        parameter,
        index,
        operand,
        value,
    })
}

/// [`ts_boundary_fact_for_change`] over an already-read module source, for
/// a comparison between two distinct owner parameters (#4759). The same
/// module rules hold for both: each is read-only, and the changed line runs
/// on every call.
pub(crate) fn ts_boundary_parameters_in_source(
    source: &str,
    line: usize,
    line_text: &str,
    owner: &TypeScriptOwner,
) -> Option<TypeScriptBoundaryParameters> {
    let ((parameter, index), (operand, operand_index)) = discriminator_sides(line_text, owner)?;
    let (index, operand_index) = (index?, operand_index?);
    if index == operand_index || !comparison_has_whole_sides(line_text, &parameter, &operand) {
        return None;
    }
    let request = ModuleRequest {
        parameters: vec![parameter.clone(), operand.clone()],
        owner_start_line: owner.start_line,
        owner_end_line: owner.end_line,
        constant: None,
        changed_line: line,
        changed_text: line_text.to_string(),
    };
    module_admits(owner, source, request)?;
    Some(TypeScriptBoundaryParameters {
        parameter,
        index,
        operand,
        operand_index,
    })
}

/// Run the module rules for `request` on the owner's module token stream.
/// `None` when any rule fails; otherwise the resolved constant value, if the
/// request names a constant.
fn module_admits(
    owner: &TypeScriptOwner,
    source: &str,
    request: ModuleRequest,
) -> Option<Option<i64>> {
    let file = owner.file.clone();
    let owned_source = source.to_string();
    parse_on_worker(&file, &owned_source, move |file, source, allocator| {
        let ret = Parser::new(allocator, source, source_type_for(file))
            .with_config(TokensParserConfig)
            .parse();
        if ret.panicked || !ret.errors.is_empty() {
            return None;
        }
        let tokens: Vec<Tok> = ret
            .tokens
            .iter()
            .map(|token| Tok {
                kind: token.kind(),
                start: token.start() as usize,
                end: token.end() as usize,
                escaped: token.escaped(),
            })
            .collect();
        let constant = match &request.constant {
            Some(name) => Some(resolve_module_constant(
                &ret.program.body,
                source,
                &tokens,
                name,
            )?),
            None => None,
        };
        (request
            .parameters
            .iter()
            .all(|parameter| parameter_is_read_only(source, &tokens, &request, parameter))
            && changed_line_runs_on_every_call(source, &tokens, &request))
        .then_some(constant)
    })
    .ok()?
}

/// Line-level punctuators, longest first, for [`comparison_has_whole_sides`].
const LINE_PUNCTUATORS: &[&str] = &[
    ">>>=", "===", "!==", ">>>", "**=", "<<=", ">>=", "&&=", "||=", "??=", "&&", "||", "??", "?.",
    "=>", ">=", "<=", "==", "!=", ">>", "<<", "++", "--", "**", "+=", "-=", "*=", "/=", "%=", "&=",
    "|=", "^=",
];

/// Split a changed line into identifier/number words and punctuators.
/// Whitespace is dropped; `.` stays its own punctuator so a member read
/// (`order.amount`) never passes for the bare parameter.
fn line_tokens(line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut rest = line;
    while let Some(ch) = rest.chars().next() {
        if ch.is_whitespace() {
            rest = &rest[ch.len_utf8()..];
            continue;
        }
        let word_len = rest
            .char_indices()
            .find(|(_, ch)| !(ch.is_alphanumeric() || *ch == '_' || *ch == '$'))
            .map_or(rest.len(), |(at, _)| at);
        let len = if word_len > 0 {
            word_len
        } else {
            LINE_PUNCTUATORS
                .iter()
                .find(|punct| rest.starts_with(**punct))
                .map_or(ch.len_utf8(), |punct| punct.len())
        };
        tokens.push(&rest[..len]);
        rest = &rest[len..];
    }
    tokens
}

/// Whether the changed line compares `parameter` and `operand` as the whole
/// operands of one relational or equality operator. Arithmetic, a sign, a
/// member access, or a chained comparison on either side
/// (`OFFSET + amount >= LIMIT`, `amount >= LIMIT + 1`, `amount >= -5`,
/// `2 * amount > LIMIT`) moves the real boundary away from the operand's
/// value, so the derivation fails closed. So does a side at the start or end
/// of the line, since the expression may continue across a line break.
pub(super) fn comparison_has_whole_sides(line: &str, parameter: &str, operand: &str) -> bool {
    const COMPARISONS: &[&str] = &["===", "!==", "==", "!=", ">=", "<=", ">", "<"];
    const BEFORE: &[&str] = &[
        "(", "&&", "||", "?", ":", ",", "=", "=>", "return", "{", ";",
    ];
    const AFTER: &[&str] = &[")", "&&", "||", "?", ":", ";", ",", "}"];
    // The line tokenizer does not know literal or comment spans, so a string,
    // template, regular expression, or comment could carry a look-alike
    // comparison (`"over: amount >= LIMIT"`). Refuse any line that has one.
    if line.contains(['"', '\'', '`', '/']) {
        return false;
    }
    let tokens = line_tokens(line);
    // A second comparison on the line may be the one the diff changed, and
    // the discriminator extraction cannot tell which one it paired.
    if tokens
        .iter()
        .filter(|token| COMPARISONS.contains(*token))
        .count()
        != 1
    {
        return false;
    }
    let mut found = 0usize;
    for (at, token) in tokens.iter().enumerate() {
        if !COMPARISONS.contains(token) || at == 0 {
            continue;
        }
        let (Some(left), Some(right)) = (tokens.get(at - 1), tokens.get(at + 1)) else {
            continue;
        };
        let pair =
            (*left == parameter && *right == operand) || (*left == operand && *right == parameter);
        if !pair {
            continue;
        }
        // Both edges must be visible on the changed line: a side that starts
        // or ends the line may continue on a neighbouring line
        // (`if (amount >= LIMIT` / `+ 1) {`), which this line cannot show.
        let before_ok = at >= 2 && BEFORE.contains(&tokens[at - 2]);
        let after_ok = tokens.get(at + 2).is_some_and(|next| AFTER.contains(next));
        if !(before_ok && after_ok) {
            return false;
        }
        // The comparison must open the line's statement: anything else before
        // it (`ok || `, `c ? x : `, `if (skip) return 0;`, `else if (`, a
        // method or callback body) may skip evaluating it.
        if !statement_opens_with_comparison(&tokens[..at - 1]) {
            return false;
        }
        found += 1;
    }
    found == 1
}

/// Whether `prefix`, the line tokens before a comparison's first side, is
/// one of the statement openings that always evaluate it: `if (`, `return`,
/// or `const|let|var NAME =`.
fn statement_opens_with_comparison(prefix: &[&str]) -> bool {
    let is_name = |word: &str| {
        word.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_' || ch == '$')
            && word
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
    };
    match prefix {
        ["if", "("] | ["return"] => true,
        [declare, name, "="] => matches!(*declare, "const" | "let" | "var") && is_name(name),
        _ => false,
    }
}

struct ModuleRequest {
    /// The owner parameters the changed comparison reads.
    parameters: Vec<String>,
    owner_start_line: usize,
    owner_end_line: usize,
    constant: Option<String>,
    /// 1-based line of the changed comparison in the owner's module.
    changed_line: usize,
    /// The diff's text for that line.
    changed_text: String,
}

#[derive(Clone, Copy)]
struct Tok {
    kind: Kind,
    start: usize,
    end: usize,
    escaped: bool,
}

impl Tok {
    fn text<'s>(&self, source: &'s str) -> &'s str {
        source.get(self.start..self.end).unwrap_or("")
    }
}

/// A plain decimal integer literal (`100`, `10_000`) as `i64`. Floats,
/// exponents, bigints, radix prefixes, and signs are refused.
fn integer_literal(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty()
        || !raw.starts_with(|ch: char| ch.is_ascii_digit())
        || !raw.chars().all(|ch| ch.is_ascii_digit() || ch == '_')
        || raw.ends_with('_')
        || raw.contains("__")
        || (raw.len() > 1 && raw.starts_with('0'))
    {
        return None;
    }
    raw.replace('_', "").parse::<i64>().ok()
}

/// The value of `name` when the module binds it exactly once, at the top
/// level, with an immutable `const NAME = <integer literal>`, and every other
/// occurrence is a plain read. `None` otherwise.
fn resolve_module_constant(
    body: &[Statement<'_>],
    source: &str,
    tokens: &[Tok],
    name: &str,
) -> Option<i64> {
    if module_is_opaque(source, tokens) {
        return None;
    }
    let mut declaration: Option<(usize, i64)> = None;
    let mut allowed_reads: Vec<usize> = Vec::new();
    for statement in body {
        let declaration_of = |decl: &VariableDeclaration<'_>| -> Option<Option<(usize, i64)>> {
            let declarator = decl.declarations.iter().find(|declarator| {
                matches!(&declarator.id, BindingPattern::BindingIdentifier(id) if id.name == name)
            })?;
            let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                return Some(None);
            };
            if decl.kind != oxc_ast::ast::VariableDeclarationKind::Const || decl.declare {
                return Some(None);
            }
            let value = match &declarator.init {
                Some(Expression::NumericLiteral(literal)) => literal
                    .raw
                    .as_ref()
                    .and_then(|raw| integer_literal(raw.as_str())),
                _ => None,
            };
            Some(value.map(|value| (id.span.start as usize, value)))
        };
        let found = match statement {
            Statement::VariableDeclaration(decl) => declaration_of(decl),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::VariableDeclaration(decl)) => declaration_of(decl),
                _ => {
                    // `export { NAME }` / `export { NAME as OTHER }` without a
                    // source re-exports the local binding: a read.
                    if export.source.is_none() {
                        for specifier in &export.specifiers {
                            if let ModuleExportName::IdentifierReference(local) = &specifier.local
                                && local.name == name
                            {
                                allowed_reads.push(local.span.start as usize);
                            }
                        }
                    }
                    None
                }
            },
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::Identifier(ident) = &export.declaration
                    && ident.name == name
                {
                    allowed_reads.push(ident.span.start as usize);
                }
                None
            }
            _ => None,
        };
        match found {
            // A top-level binding of the name that is not a single
            // immutable integer `const`.
            // A second declaration of the same name is ambiguous.
            Some(Some(_)) if declaration.is_some() => return None,
            Some(None) => return None,
            Some(Some(found)) => declaration = Some(found),
            None => {}
        }
    }
    let (declaration_at, value) = declaration?;
    for (at, token) in tokens.iter().enumerate() {
        if !token.kind.is_identifier_name() || token.text(source) != name {
            continue;
        }
        if token.start == declaration_at || allowed_reads.contains(&token.start) {
            continue;
        }
        if !occurrence_is_plain_read(source, tokens, at) {
            return None;
        }
    }
    Some(value)
}

/// `eval`, `with`, or an escaped identifier anywhere in the module can bind
/// or spell a name the token rules do not see.
fn module_is_opaque(source: &str, tokens: &[Tok]) -> bool {
    tokens.iter().any(|token| {
        token.kind == Kind::With
            || (token.kind.is_identifier_name() && (token.escaped || token.text(source) == "eval"))
    })
}

/// `true` when the owner reads `parameter` only: exactly one occurrence in
/// the signature's parameter list, none before it, and every later one a
/// plain read (no write, update, redeclaration, or nested binding of the
/// same name). `arguments`, `eval`, and `with` in the module fail closed.
fn parameter_is_read_only(
    source: &str,
    tokens: &[Tok],
    request: &ModuleRequest,
    parameter: &str,
) -> bool {
    if module_is_opaque(source, tokens) {
        return false;
    }
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let Some(&span_start) = line_starts.get(request.owner_start_line.saturating_sub(1)) else {
        return false;
    };
    let span_end = line_starts
        .get(request.owner_end_line)
        .copied()
        .unwrap_or(source.len());
    let owner_tokens: Vec<usize> = (0..tokens.len())
        .filter(|&at| tokens[at].start >= span_start && tokens[at].end <= span_end)
        .collect();
    if owner_tokens
        .iter()
        .any(|&at| tokens[at].kind.is_identifier_name() && tokens[at].text(source) == "arguments")
    {
        return false;
    }
    let Some(&open) = owner_tokens
        .iter()
        .find(|&&at| tokens[at].kind == Kind::LParen)
    else {
        return false;
    };
    let Some(close) = matching_close(tokens, open) else {
        return false;
    };
    let mut in_signature = 0usize;
    for &at in &owner_tokens {
        let token = &tokens[at];
        if !token.kind.is_identifier_name() || token.text(source) != parameter {
            continue;
        }
        if matches!(
            at.checked_sub(1).map(|prev| tokens[prev].kind),
            Some(Kind::Dot | Kind::QuestionDot)
        ) {
            // `obj.amount` names a property, not the parameter.
            continue;
        }
        if at < open {
            return false;
        }
        if at < close {
            in_signature += 1;
            continue;
        }
        if !occurrence_is_plain_read(source, tokens, at) {
            return false;
        }
    }
    in_signature == 1
}

/// Whether every call of the owner evaluates the changed line: the line
/// lies inside the owner, after the signature, at the top level
/// of a block body, and no earlier body token can leave the function or
/// skip the line (`return`, `throw`, `break`, `continue`, `yield`, a loop
/// or `await` that may not finish, or an enclosing
/// `if`/loop/`switch`/`try`/callback block). An early guard such
/// as `if (amount === 5000) return 0;` would stop the derived input before
/// the changed comparison, so it fails closed. Expression-bodied and curried
/// arrows, returned functions, and generators are not admitted.
fn changed_line_runs_on_every_call(source: &str, tokens: &[Tok], request: &ModuleRequest) -> bool {
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let changed = request.changed_line;
    if changed <= request.owner_start_line || changed > request.owner_end_line {
        return false;
    }
    let Some(&line_start) = line_starts.get(changed - 1) else {
        return false;
    };
    let Some(&span_start) = line_starts.get(request.owner_start_line.saturating_sub(1)) else {
        return false;
    };
    let Some(open) = (0..tokens.len())
        .find(|&at| tokens[at].start >= span_start && tokens[at].kind == Kind::LParen)
    else {
        return false;
    };
    let Some(close) = matching_close(tokens, open) else {
        return false;
    };
    if tokens[close].end > line_start {
        return false;
    }
    // The changed line must start a fresh statement: the previous token ends
    // one (`;`, a closed block, or the body's `{`) and the line itself opens
    // with a statement keyword. A braceless `if`/loop body, an `else`
    // branch, or an expression continued from the previous line (`? :`,
    // `&&`, `=>`) would otherwise pass with a balanced stack.
    let line_end = line_starts.get(changed).copied().unwrap_or(source.len());
    // The module's own line must equal the diff's changed line, which also
    // refuses a line inside a block comment. The reach rules below read the
    // tree at `--root` and assume it is the diff's post-image; a tree that
    // differs elsewhere but coincides on this line is outside the claim.
    let source_line = source.get(line_start..line_end).unwrap_or("");
    if source_line.trim() != request.changed_text.trim() {
        return false;
    }
    // Generators run no body code until iterated.
    if tokens[..open]
        .iter()
        .any(|token| token.start >= span_start && token.kind == Kind::Star)
    {
        return false;
    }
    // Between the parameter list and the body `{` only a plain return type
    // and the owner's own `=>` may appear; a curried arrow (`=> () => {`) or
    // a returned `function` body is not the owner's body.
    let Some(body_open) = (close + 1..tokens.len()).find(|&at| tokens[at].kind == Kind::LCurly)
    else {
        return false;
    };
    let head = &tokens[close + 1..body_open];
    let plain_head = head.iter().enumerate().all(|(at, token)| match token.kind {
        Kind::Arrow => at + 1 == head.len(),
        Kind::Colon
        | Kind::Dot
        | Kind::LAngle
        | Kind::RAngle
        | Kind::LBrack
        | Kind::RBrack
        | Kind::Pipe
        | Kind::Amp
        | Kind::Comma
        | Kind::Question => true,
        kind => kind.is_identifier_name() && kind != Kind::Function && kind != Kind::Async,
    });
    // The chosen `{` must enclose the changed line: a type literal in the
    // return type (`: { (): number } => () => {`) closes before it.
    let encloses_line =
        matching_close(tokens, body_open).is_some_and(|end| tokens[end].start >= line_end);
    if !plain_head || !encloses_line {
        return false;
    }
    let body = &tokens[close + 1..];
    let before_line = body
        .iter()
        .take_while(|token| token.start < line_start)
        .count();
    let starts_statement = body.get(before_line).is_some_and(|first| {
        first.start < line_end
            && matches!(
                first.kind,
                Kind::If | Kind::Return | Kind::Const | Kind::Let | Kind::Var
            )
    });
    let follows_statement = before_line
        .checked_sub(1)
        .and_then(|last| body.get(last))
        .is_some_and(|last| matches!(last.kind, Kind::Semicolon | Kind::LCurly | Kind::RCurly));
    if !(starts_statement && follows_statement) {
        return false;
    }
    let mut open_groups: Vec<Kind> = Vec::new();
    for token in &body[..before_line] {
        match token.kind {
            // An earlier exit, or a loop or `await` that may not finish for
            // the boundary input, can keep the changed line from running.
            Kind::Return
            | Kind::Throw
            | Kind::Break
            | Kind::Continue
            | Kind::Yield
            | Kind::While
            | Kind::Do
            | Kind::For
            | Kind::Await => {
                return false;
            }
            Kind::LParen | Kind::LBrack | Kind::LCurly | Kind::TemplateHead => {
                open_groups.push(token.kind);
            }
            Kind::RParen | Kind::RBrack | Kind::RCurly | Kind::TemplateTail
                if open_groups.pop().is_none() =>
            {
                return false;
            }
            _ => {}
        }
    }
    open_groups == [Kind::LCurly]
}

/// Index of the token closing the group opened at `open`.
fn matching_close(tokens: &[Tok], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (at, token) in tokens.iter().enumerate().skip(open) {
        match token.kind {
            Kind::LParen | Kind::LBrack | Kind::LCurly | Kind::TemplateHead => depth += 1,
            Kind::RParen | Kind::RBrack | Kind::RCurly | Kind::TemplateTail => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

/// Index of the token opening the innermost group around `at`.
fn enclosing_open(tokens: &[Tok], at: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in (0..at).rev() {
        match tokens[index].kind {
            Kind::RParen | Kind::RBrack | Kind::RCurly => depth += 1,
            Kind::LParen | Kind::LBrack | Kind::LCurly => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
            }
            // Template parts interleave with braces; refuse to guess.
            Kind::TemplateHead | Kind::TemplateMiddle | Kind::TemplateTail => return None,
            _ => {}
        }
    }
    None
}

/// Whether the identifier token at `at` is a plain read of its binding.
///
/// Positive recognition only: the previous token must put the name in
/// expression position (an operator, `return`, `typeof`, an arrow body, a
/// template substitution, ...) and the next token must not assign, update,
/// or open an arrow body. A name after `(`, `[`, or `,` counts only when a
/// binary operator follows (no parameter, destructuring, or catch pattern
/// can continue that way) or when it is a whole argument of a call.
/// Everything else — declarations, parameters, patterns, object keys, labels,
/// type positions after `:` — fails closed.
fn occurrence_is_plain_read(source: &str, tokens: &[Tok], at: usize) -> bool {
    let kind_at = |index: Option<usize>| index.and_then(|i| tokens.get(i)).map(|token| token.kind);
    let prev = kind_at(at.checked_sub(1));
    let next = kind_at(Some(at + 1));
    // Any write or arrow parameter fails closed — including a same-named
    // member write (`exports.NAME = 5`, `this.NAME += 1`), which cannot
    // rebind a module `const` but is not worth reasoning about.
    if next.is_some_and(|kind| is_assignment(kind) || matches!(kind, Kind::Arrow))
        || matches!(prev, Some(Kind::Plus2 | Kind::Minus2))
        || matches!(next, Some(Kind::Plus2 | Kind::Minus2))
    {
        return false;
    }
    if matches!(prev, Some(Kind::Dot | Kind::QuestionDot)) {
        // A member name (`obj.NAME`) read is not an occurrence of the binding.
        return true;
    }
    let Some(prev) = prev else {
        return false;
    };
    match prev {
        Kind::Star => {
            // `a * NAME` and `yield* NAME` read; `function* NAME` and a
            // generator method `*NAME()` bind.
            matches!(
                kind_at(at.checked_sub(2)),
                Some(
                    Kind::Ident
                        | Kind::RParen
                        | Kind::RBrack
                        | Kind::Yield
                        | Kind::Decimal
                        | Kind::Float
                )
            )
        }
        Kind::Default => kind_at(at.checked_sub(2)) == Some(Kind::Export),
        Kind::LParen | Kind::LBrack | Kind::Comma => {
            next.is_some_and(continues_expression)
                || (matches!(next, Some(Kind::RParen | Kind::Comma))
                    && is_call_argument(source, tokens, at))
        }
        kind if is_read_position_operator(kind) => true,
        Kind::Return
        | Kind::Typeof
        | Kind::Await
        | Kind::Throw
        | Kind::Case
        | Kind::Yield
        | Kind::Void
        | Kind::In
        | Kind::Of
        | Kind::New
        | Kind::Delete
        | Kind::Instanceof
        | Kind::Extends
        | Kind::TemplateHead
        | Kind::TemplateMiddle => true,
        _ => false,
    }
}

/// A whole call argument: the enclosing group is `(` directly after a callee
/// (an identifier that is not a declaration name, or a call/index result),
/// and its close is not followed by a function body, arrow, or return type.
fn is_call_argument(source: &str, tokens: &[Tok], at: usize) -> bool {
    let Some(open) = enclosing_open(tokens, at) else {
        return false;
    };
    if tokens[open].kind != Kind::LParen {
        return false;
    }
    let Some(callee) = open.checked_sub(1).map(|index| tokens[index]) else {
        return false;
    };
    let callee_ok = match callee.kind {
        Kind::RParen | Kind::RBrack => true,
        Kind::Ident => {
            // `function name(...)`, `get name(...)`, `async name(...)`, and a
            // generator `*name(...)` declare, they do not call.
            !matches!(
                open.checked_sub(2).map(|index| tokens[index].kind),
                Some(
                    Kind::Function
                        | Kind::Get
                        | Kind::Set
                        | Kind::Async
                        | Kind::Star
                        | Kind::Static
                )
            ) && callee.text(source) != "function"
        }
        _ => false,
    };
    if !callee_ok {
        return false;
    }
    let Some(close) = matching_close(tokens, open) else {
        return false;
    };
    !matches!(
        tokens.get(close + 1).map(|token| token.kind),
        Some(Kind::LCurly | Kind::Arrow | Kind::Colon)
    )
}

fn is_assignment(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Eq
            | Kind::PlusEq
            | Kind::MinusEq
            | Kind::StarEq
            | Kind::Star2Eq
            | Kind::SlashEq
            | Kind::PercentEq
            | Kind::AmpEq
            | Kind::Amp2Eq
            | Kind::PipeEq
            | Kind::Pipe2Eq
            | Kind::CaretEq
            | Kind::ShiftLeftEq
            | Kind::ShiftRightEq
            | Kind::ShiftRight3Eq
            | Kind::Question2Eq
    )
}

/// Operators after which the next token is an expression operand.
fn is_read_position_operator(kind: Kind) -> bool {
    is_assignment(kind)
        || matches!(
            kind,
            Kind::Amp
                | Kind::Amp2
                | Kind::Bang
                | Kind::Caret
                | Kind::Eq2
                | Kind::Eq3
                | Kind::GtEq
                | Kind::LtEq
                | Kind::LAngle
                | Kind::RAngle
                | Kind::Minus
                | Kind::Neq
                | Kind::Neq2
                | Kind::Percent
                | Kind::Pipe
                | Kind::Pipe2
                | Kind::Plus
                | Kind::Question
                | Kind::Question2
                | Kind::ShiftLeft
                | Kind::ShiftRight
                | Kind::ShiftRight3
                | Kind::Slash
                | Kind::Star2
                | Kind::Tilde
                | Kind::Arrow
        )
}

/// Tokens that can only continue an expression after an operand — never a
/// parameter, destructuring pattern, or catch binding. `in` is excluded:
/// `for (NAME in obj)` assigns the name.
fn continues_expression(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Amp
            | Kind::Amp2
            | Kind::Caret
            | Kind::Eq2
            | Kind::Eq3
            | Kind::GtEq
            | Kind::LtEq
            | Kind::LAngle
            | Kind::RAngle
            | Kind::Minus
            | Kind::Neq
            | Kind::Neq2
            | Kind::Percent
            | Kind::Pipe
            | Kind::Pipe2
            | Kind::Plus
            | Kind::Question2
            | Kind::ShiftLeft
            | Kind::ShiftRight
            | Kind::ShiftRight3
            | Kind::Slash
            | Kind::Star
            | Kind::Star2
            | Kind::Instanceof
            | Kind::Dot
            | Kind::QuestionDot
    )
}

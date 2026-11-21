//! A fail-closed matcher for JSON Schema `pattern` constraints.
//!
//! "Fail-closed" is a claim about *construct coverage*, not about resource use:
//! every unsupported construct is reported rather than assumed to match. The
//! only bound enforced is `MAX_GROUP_DEPTH` on parse nesting. Match work and
//! quantifier magnitude are **not** bounded, so a pathological pattern could
//! stall rather than fail visibly. That is acceptable only because the inputs
//! are the repository's own checked-in schemas rather than untrusted text; if
//! this matcher is ever pointed at a pattern the repository does not control,
//! a match-step budget has to come first.
//!
//! The repository's contract validator (`verification_contracts`) is the single
//! authority that decides whether producer bytes satisfy a published schema. A
//! validator that silently ignores `pattern` would report a passing contract
//! while leaving identity fields — commit SHAs, `sha256:` commitments, relative
//! working directories — completely unchecked. That is the false-confidence
//! shape the repository forbids, so this module supports the regular-expression
//! subset the repository's own schemas use and reports every other construct as
//! an error instead of treating it as a match.
//!
//! Supported: anchors `^` `$`, literals, `.`, escapes (`\d \D \w \W \s \S` and
//! punctuation), character classes with ranges and negation, groups including
//! `(?:…)`, lookahead assertions `(?=…)` and `(?!…)`, alternation, and the
//! quantifiers `*` `+` `?` `{n}` `{n,}` `{n,m}`.
//!
//! Unsupported (reported, never assumed to match): backreferences, lookbehind,
//! named groups, inline flags, and any escape outside the list above.

/// One alternative-separated pattern body.
#[derive(Clone, Debug)]
pub(crate) struct Alternation {
    branches: Vec<Sequence>,
}

#[derive(Clone, Debug)]
struct Sequence {
    terms: Vec<Term>,
}

#[derive(Clone, Debug)]
struct Term {
    atom: Atom,
    repeat: Repeat,
}

#[derive(Clone, Copy, Debug)]
struct Repeat {
    min: usize,
    max: Option<usize>,
}

impl Repeat {
    const ONCE: Self = Self {
        min: 1,
        max: Some(1),
    };
}

#[derive(Clone, Debug)]
enum Atom {
    Literal(char),
    AnyExceptNewline,
    Class(CharClass),
    Group(Alternation),
    LookaheadPositive(Alternation),
    LookaheadNegative(Alternation),
    StartAnchor,
    EndAnchor,
}

#[derive(Clone, Debug)]
struct CharClass {
    negated: bool,
    members: Vec<ClassMember>,
}

#[derive(Clone, Debug)]
enum ClassMember {
    Single(char),
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

/// A compiled `pattern`. Compilation fails rather than degrading to "matches
/// everything" when the pattern uses an unsupported construct.
#[derive(Clone, Debug)]
pub(crate) struct SchemaPattern {
    root: Alternation,
}

impl SchemaPattern {
    /// Compile one JSON Schema `pattern` string.
    pub(crate) fn compile(pattern: &str) -> Result<Self, String> {
        let characters: Vec<char> = pattern.chars().collect();
        let mut parser = Parser {
            characters: &characters,
            position: 0,
            depth: 0,
        };
        let root = parser.parse_alternation()?;
        if parser.position != characters.len() {
            return Err(format!(
                "unsupported pattern {pattern:?}: unexpected `{}`",
                parser
                    .characters
                    .get(parser.position)
                    .copied()
                    .unwrap_or(' ')
            ));
        }
        Ok(Self { root })
    }

    /// JSON Schema `pattern` is a partial match, so an unanchored pattern may
    /// start at any position.
    pub(crate) fn is_match(&self, text: &str) -> bool {
        let characters: Vec<char> = text.chars().collect();
        (0..=characters.len())
            .any(|start| match_alternation(&self.root, &characters, start, &mut |_| true))
    }
}

const MAX_GROUP_DEPTH: usize = 16;

struct Parser<'a> {
    characters: &'a [char],
    position: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.characters.get(self.position).copied()
    }

    fn parse_alternation(&mut self) -> Result<Alternation, String> {
        let mut branches = vec![self.parse_sequence()?];
        while self.peek() == Some('|') {
            self.position += 1;
            branches.push(self.parse_sequence()?);
        }
        Ok(Alternation { branches })
    }

    fn parse_sequence(&mut self) -> Result<Sequence, String> {
        let mut terms = Vec::new();
        while let Some(character) = self.peek() {
            if character == '|' || character == ')' {
                break;
            }
            let atom = self.parse_atom()?;
            let repeat = self.parse_repeat()?;
            if repeat.min != 1 || repeat.max != Some(1) {
                match atom {
                    Atom::LookaheadPositive(_) | Atom::LookaheadNegative(_) => {
                        return Err("unsupported pattern: quantified lookahead".to_string());
                    }
                    _ => {}
                }
            }
            terms.push(Term { atom, repeat });
        }
        Ok(Sequence { terms })
    }

    fn parse_atom(&mut self) -> Result<Atom, String> {
        let Some(character) = self.peek() else {
            return Err("unsupported pattern: unexpected end of input".to_string());
        };
        self.position += 1;
        match character {
            '^' => Ok(Atom::StartAnchor),
            '$' => Ok(Atom::EndAnchor),
            '.' => Ok(Atom::AnyExceptNewline),
            '[' => self.parse_class(),
            '(' => self.parse_group(),
            '\\' => self.parse_escape(),
            '*' | '+' | '?' => Err(format!(
                "unsupported pattern: quantifier `{character}` without a preceding atom"
            )),
            ')' | ']' | '{' | '}' => Err(format!(
                "unsupported pattern: unbalanced or unsupported `{character}`"
            )),
            literal => Ok(Atom::Literal(literal)),
        }
    }

    fn parse_group(&mut self) -> Result<Atom, String> {
        if self.depth >= MAX_GROUP_DEPTH {
            return Err("unsupported pattern: group nesting is too deep".to_string());
        }
        let mut kind = GroupKind::Capturing;
        if self.peek() == Some('?') {
            kind = match self.characters.get(self.position + 1).copied() {
                Some(':') => GroupKind::NonCapturing,
                Some('=') => GroupKind::LookaheadPositive,
                Some('!') => GroupKind::LookaheadNegative,
                other => {
                    return Err(format!(
                        "unsupported pattern: group modifier `(?{}`",
                        other.unwrap_or(' ')
                    ));
                }
            };
            self.position += 2;
        }
        self.depth += 1;
        let inner = self.parse_alternation()?;
        self.depth -= 1;
        if self.peek() != Some(')') {
            return Err("unsupported pattern: unterminated group".to_string());
        }
        self.position += 1;
        Ok(match kind {
            GroupKind::Capturing | GroupKind::NonCapturing => Atom::Group(inner),
            GroupKind::LookaheadPositive => Atom::LookaheadPositive(inner),
            GroupKind::LookaheadNegative => Atom::LookaheadNegative(inner),
        })
    }

    fn parse_escape(&mut self) -> Result<Atom, String> {
        let Some(character) = self.peek() else {
            return Err("unsupported pattern: trailing backslash".to_string());
        };
        self.position += 1;
        let member = match character {
            'd' => Some(ClassMember::Digit(false)),
            'D' => Some(ClassMember::Digit(true)),
            'w' => Some(ClassMember::Word(false)),
            'W' => Some(ClassMember::Word(true)),
            's' => Some(ClassMember::Space(false)),
            'S' => Some(ClassMember::Space(true)),
            _ => None,
        };
        if let Some(member) = member {
            return Ok(Atom::Class(CharClass {
                negated: false,
                members: vec![member],
            }));
        }
        if character.is_ascii_alphanumeric() {
            return Err(format!("unsupported pattern: escape `\\{character}`"));
        }
        Ok(Atom::Literal(character))
    }

    fn parse_class(&mut self) -> Result<Atom, String> {
        let mut negated = false;
        if self.peek() == Some('^') {
            negated = true;
            self.position += 1;
        }
        let mut members = Vec::new();
        loop {
            let Some(character) = self.peek() else {
                return Err("unsupported pattern: unterminated character class".to_string());
            };
            self.position += 1;
            if character == ']' {
                break;
            }
            let low = if character == '\\' {
                self.parse_class_escape()?
            } else {
                ClassAtom::Char(character)
            };
            let ranged = self.peek() == Some('-')
                && self
                    .characters
                    .get(self.position + 1)
                    .is_some_and(|next| *next != ']');
            match (low, ranged) {
                (ClassAtom::Char(start), true) => {
                    self.position += 1;
                    let Some(end) = self.peek() else {
                        return Err("unsupported pattern: unterminated class range".to_string());
                    };
                    self.position += 1;
                    let end = if end == '\\' {
                        match self.parse_class_escape()? {
                            ClassAtom::Char(value) => value,
                            ClassAtom::Member(_) => {
                                return Err(
                                    "unsupported pattern: shorthand class as a range bound"
                                        .to_string(),
                                );
                            }
                        }
                    } else {
                        end
                    };
                    if end < start {
                        return Err("unsupported pattern: descending class range".to_string());
                    }
                    members.push(ClassMember::Range(start, end));
                }
                (ClassAtom::Char(single), false) => members.push(ClassMember::Single(single)),
                (ClassAtom::Member(member), _) => members.push(member),
            }
        }
        if members.is_empty() {
            return Err("unsupported pattern: empty character class".to_string());
        }
        Ok(Atom::Class(CharClass { negated, members }))
    }

    fn parse_class_escape(&mut self) -> Result<ClassAtom, String> {
        let Some(character) = self.peek() else {
            return Err("unsupported pattern: trailing backslash in class".to_string());
        };
        self.position += 1;
        Ok(match character {
            'd' => ClassAtom::Member(ClassMember::Digit(false)),
            'D' => ClassAtom::Member(ClassMember::Digit(true)),
            'w' => ClassAtom::Member(ClassMember::Word(false)),
            'W' => ClassAtom::Member(ClassMember::Word(true)),
            's' => ClassAtom::Member(ClassMember::Space(false)),
            'S' => ClassAtom::Member(ClassMember::Space(true)),
            'n' => ClassAtom::Char('\n'),
            'r' => ClassAtom::Char('\r'),
            't' => ClassAtom::Char('\t'),
            other if other.is_ascii_alphanumeric() => {
                return Err(format!("unsupported pattern: class escape `\\{other}`"));
            }
            other => ClassAtom::Char(other),
        })
    }

    fn parse_repeat(&mut self) -> Result<Repeat, String> {
        match self.peek() {
            Some('*') => {
                self.position += 1;
                Ok(Repeat { min: 0, max: None })
            }
            Some('+') => {
                self.position += 1;
                Ok(Repeat { min: 1, max: None })
            }
            Some('?') => {
                self.position += 1;
                Ok(Repeat {
                    min: 0,
                    max: Some(1),
                })
            }
            Some('{') => self.parse_counted_repeat(),
            _ => Ok(Repeat::ONCE),
        }
    }

    fn parse_counted_repeat(&mut self) -> Result<Repeat, String> {
        let start = self.position;
        self.position += 1;
        let min = self.parse_repeat_number()?;
        let Some(min) = min else {
            // `{` that is not a quantifier is not a construct this matcher
            // interprets; refusing keeps the failure visible.
            self.position = start;
            return Err("unsupported pattern: `{` outside a counted quantifier".to_string());
        };
        match self.peek() {
            Some('}') => {
                self.position += 1;
                Ok(Repeat {
                    min,
                    max: Some(min),
                })
            }
            Some(',') => {
                self.position += 1;
                let max = self.parse_repeat_number()?;
                if self.peek() != Some('}') {
                    return Err("unsupported pattern: unterminated counted quantifier".to_string());
                }
                self.position += 1;
                if let Some(max) = max
                    && max < min
                {
                    return Err("unsupported pattern: descending counted quantifier".to_string());
                }
                Ok(Repeat { min, max })
            }
            _ => Err("unsupported pattern: unterminated counted quantifier".to_string()),
        }
    }

    fn parse_repeat_number(&mut self) -> Result<Option<usize>, String> {
        let mut digits = String::new();
        while let Some(character) = self.peek()
            && character.is_ascii_digit()
        {
            digits.push(character);
            self.position += 1;
        }
        if digits.is_empty() {
            return Ok(None);
        }
        digits
            .parse::<usize>()
            .map(Some)
            .map_err(|error| format!("unsupported pattern: quantifier bound {digits:?}: {error}"))
    }
}

enum GroupKind {
    Capturing,
    NonCapturing,
    LookaheadPositive,
    LookaheadNegative,
}

enum ClassAtom {
    Char(char),
    Member(ClassMember),
}

/// Continuation-passing backtracking: `continue_with` receives the position
/// reached after the current construct and reports whether the rest of the
/// pattern matched from there.
fn match_alternation(
    alternation: &Alternation,
    input: &[char],
    position: usize,
    continue_with: &mut dyn FnMut(usize) -> bool,
) -> bool {
    alternation
        .branches
        .iter()
        .any(|branch| match_terms(&branch.terms, input, position, continue_with))
}

fn match_terms(
    terms: &[Term],
    input: &[char],
    position: usize,
    continue_with: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let Some((first, rest)) = terms.split_first() else {
        return continue_with(position);
    };
    match_repeated(first, 0, input, position, &mut |next| {
        match_terms(rest, input, next, continue_with)
    })
}

fn match_repeated(
    term: &Term,
    matched: usize,
    input: &[char],
    position: usize,
    continue_with: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let can_take_more = term.repeat.max.is_none_or(|max| matched < max);
    if can_take_more
        && match_atom(&term.atom, input, position, &mut |next| {
            if next == position {
                // A zero-width match counts once; repeating it would not
                // terminate.
                matched + 1 >= term.repeat.min && continue_with(next)
            } else {
                match_repeated(term, matched + 1, input, next, continue_with)
            }
        })
    {
        return true;
    }
    matched >= term.repeat.min && continue_with(position)
}

fn match_atom(
    atom: &Atom,
    input: &[char],
    position: usize,
    continue_with: &mut dyn FnMut(usize) -> bool,
) -> bool {
    match atom {
        Atom::StartAnchor => position == 0 && continue_with(position),
        Atom::EndAnchor => position == input.len() && continue_with(position),
        Atom::LookaheadPositive(inner) => {
            match_alternation(inner, input, position, &mut |_| true) && continue_with(position)
        }
        Atom::LookaheadNegative(inner) => {
            !match_alternation(inner, input, position, &mut |_| true) && continue_with(position)
        }
        Atom::Group(inner) => match_alternation(inner, input, position, continue_with),
        Atom::Literal(expected) => match input.get(position) {
            Some(actual) if actual == expected => continue_with(position + 1),
            _ => false,
        },
        Atom::AnyExceptNewline => match input.get(position) {
            Some(actual) if *actual != '\n' => continue_with(position + 1),
            _ => false,
        },
        Atom::Class(class) => match input.get(position) {
            Some(actual) if class.matches(*actual) => continue_with(position + 1),
            _ => false,
        },
    }
}

impl CharClass {
    fn matches(&self, candidate: char) -> bool {
        let inside = self.members.iter().any(|member| member.matches(candidate));
        inside != self.negated
    }
}

impl ClassMember {
    fn matches(&self, candidate: char) -> bool {
        match self {
            Self::Single(expected) => candidate == *expected,
            Self::Range(start, end) => candidate >= *start && candidate <= *end,
            Self::Digit(negated) => candidate.is_ascii_digit() != *negated,
            // ECMA-262 defines `\w` as ASCII `[A-Za-z0-9_]`, and does not widen
            // it under Unicode mode. `is_alphanumeric` is Unicode-aware, so it
            // accepted characters the spec excludes — a matcher more permissive
            // than the constraint it claims to enforce, which is the wrong
            // direction for a fail-closed validator.
            Self::Word(negated) => {
                (candidate.is_ascii_alphanumeric() || candidate == '_') != *negated
            }
            Self::Space(negated) => candidate.is_whitespace() != *negated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SchemaPattern;

    fn matches(pattern: &str, text: &str) -> Result<bool, String> {
        Ok(SchemaPattern::compile(pattern)?.is_match(text))
    }

    /// Every `pattern` the repository's published schemas declare must compile,
    /// or the contract validator would be reporting a pass over a constraint it
    /// never evaluated.
    #[test]
    fn published_schema_patterns_compile() -> Result<(), String> {
        for pattern in [
            "^sha256:[0-9a-f]{64}$",
            "^[0-9a-fA-F]{40}$",
            "^[0-9]{4}-[0-9]{2}-[0-9]{2}$",
            "^gap:.+",
            r"^(?![^\\/]+:)(?![\\/])(?!.*(?:^|[\\/])\.\.(?:[\\/]|$)).+$",
        ] {
            SchemaPattern::compile(pattern)?;
        }
        Ok(())
    }

    #[test]
    fn sha256_commitment_pattern_discriminates() -> Result<(), String> {
        let pattern = "^sha256:[0-9a-f]{64}$";
        assert!(matches(pattern, &format!("sha256:{}", "a".repeat(64)))?);
        assert!(!matches(pattern, &format!("sha256:{}", "a".repeat(63)))?);
        assert!(!matches(pattern, &format!("sha256:{}", "A".repeat(64)))?);
        assert!(!matches(pattern, &format!("md5:{}", "a".repeat(64)))?);
        // A partial match must not satisfy an anchored pattern.
        assert!(!matches(pattern, &format!("sha256:{} ", "a".repeat(64)))?);
        Ok(())
    }

    #[test]
    fn head_sha_pattern_requires_forty_hex_characters() -> Result<(), String> {
        let pattern = "^[0-9a-fA-F]{40}$";
        assert!(matches(pattern, &"a".repeat(40))?);
        assert!(matches(pattern, &"A1".repeat(20))?);
        assert!(!matches(pattern, &"a".repeat(39))?);
        assert!(!matches(pattern, &"a".repeat(41))?);
        assert!(!matches(pattern, "not-a-sha")?);
        Ok(())
    }

    #[test]
    fn authorization_date_pattern_requires_iso_shape() -> Result<(), String> {
        let pattern = "^[0-9]{4}-[0-9]{2}-[0-9]{2}$";
        assert!(matches(pattern, "2026-08-08")?);
        assert!(!matches(pattern, "2026-8-8")?);
        assert!(!matches(pattern, "08-08-2026")?);
        Ok(())
    }

    #[test]
    fn canonical_gap_pattern_requires_prefix_and_body() -> Result<(), String> {
        let pattern = "^gap:.+";
        assert!(matches(pattern, "gap:boundary")?);
        assert!(!matches(pattern, "gap:")?);
        assert!(!matches(pattern, "seam:gap:boundary")?);
        Ok(())
    }

    /// The `working_directory` pattern is the one constraint that keeps a
    /// producer-owned command spec from naming an absolute or escaping path.
    #[test]
    fn working_directory_pattern_rejects_absolute_and_escaping_paths() -> Result<(), String> {
        let pattern = r"^(?![^\\/]+:)(?![\\/])(?!.*(?:^|[\\/])\.\.(?:[\\/]|$)).+$";
        assert!(matches(pattern, ".")?);
        assert!(matches(pattern, "crates/ripr")?);
        assert!(matches(pattern, r"crates\ripr")?);
        assert!(matches(pattern, "a..b")?);
        assert!(!matches(pattern, "/absolute")?);
        assert!(!matches(pattern, r"\absolute")?);
        assert!(!matches(pattern, r"drive-letter:\workspace")?);
        assert!(!matches(pattern, "..")?);
        assert!(!matches(pattern, "../escape")?);
        assert!(!matches(pattern, "crates/../escape")?);
        assert!(!matches(pattern, r"crates\..\escape")?);
        assert!(!matches(pattern, "")?);
        Ok(())
    }

    #[test]
    fn alternation_and_optional_quantifiers_match() -> Result<(), String> {
        assert!(matches("^(cat|dog)s?$", "cats")?);
        assert!(matches("^(cat|dog)s?$", "dog")?);
        assert!(!matches("^(cat|dog)s?$", "cows")?);
        assert!(matches("^a{2,}$", "aaa")?);
        assert!(!matches("^a{2,}$", "a")?);
        assert!(matches("^a{2,3}$", "aa")?);
        assert!(!matches("^a{2,3}$", "aaaa")?);
        Ok(())
    }

    /// JSON Schema `pattern` is a partial match: an unanchored pattern may
    /// start anywhere in the value. Every other test here anchors with `^`, so
    /// replacing the `(0..=characters.len())` start-offset sweep with a single
    /// offset of `0` would leave them all green. This is the discriminator for
    /// that sweep.
    #[test]
    fn an_unanchored_pattern_matches_at_a_non_zero_offset() -> Result<(), String> {
        assert!(matches("b+", "aaabbb")?, "match starting after offset 0");
        assert!(matches("bbb$", "aaabbb")?, "end-anchored, non-zero start");
        assert!(
            !matches("zzz", "aaabbb")?,
            "absent substring must not match"
        );

        // An anchored pattern must still refuse a non-zero start, or the sweep
        // would be over-permissive rather than merely present.
        assert!(
            !matches("^bbb", "aaabbb")?,
            "start anchor must pin offset 0"
        );
        assert!(matches("^aaa", "aaabbb")?);
        Ok(())
    }

    /// ECMA-262 defines `\w` as ASCII `[A-Za-z0-9_]` and does not widen it
    /// under Unicode mode. A Unicode-aware implementation accepts characters
    /// the constraint excludes, which for a fail-closed validator is the wrong
    /// direction: the value passes a check the schema intended to reject.
    #[test]
    fn word_class_is_ascii_only() -> Result<(), String> {
        assert!(matches(r"^\w+$", "abc_123")?);
        assert!(
            !matches(r"^\w+$", "abc\u{e9}")?,
            "`\\w` must not accept non-ASCII"
        );
        assert!(
            !matches(r"^\w+$", "\u{3042}")?,
            "`\\w` must not accept non-ASCII"
        );
        assert!(matches(r"^\W$", "\u{e9}")?, "`\\W` is the complement");
        Ok(())
    }

    /// Compilation is fail-closed: an unsupported construct is an error, never
    /// a silently passing constraint.
    #[test]
    fn unsupported_constructs_are_reported_not_ignored() {
        for pattern in [r"(a)\1", "(?<name>a)", r"(?<=a)b", r"\ba", "a{", "[a", "(a"] {
            let outcome = SchemaPattern::compile(pattern);
            assert!(
                outcome.is_err(),
                "pattern {pattern:?} must not compile silently"
            );
        }
    }
}

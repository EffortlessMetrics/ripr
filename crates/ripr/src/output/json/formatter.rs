use std::convert::Infallible;
use std::io::{self, Write};
use std::time::Instant;

/// The sink owns allocation admission. On error, earlier chunks may remain;
/// callers must discard partial output rather than treat it as a complete record.
pub(super) trait JsonSink {
    type Error;

    fn write_str(&mut self, chunk: &str) -> Result<(), Self::Error>;
}

impl JsonSink for String {
    type Error = Infallible;

    fn write_str(&mut self, chunk: &str) -> Result<(), Self::Error> {
        self.push_str(chunk);
        Ok(())
    }
}

pub(super) fn infallible(result: Result<(), Infallible>) {
    match result {
        Ok(()) => {}
        Err(error) => match error {},
    }
}

pub(crate) fn field(out: &mut String, indent: usize, name: &str, value: &str, trailing: bool) {
    infallible(emit_field(out, indent, name, value, trailing));
}

pub(super) fn emit_field<S: JsonSink + ?Sized>(
    out: &mut S,
    indent: usize,
    name: &str,
    value: &str,
    trailing: bool,
) -> Result<(), S::Error> {
    emit_field_prefix(out, indent, name)?;
    out.write_str("\"")?;
    emit_escape(out, value)?;
    out.write_str("\"")?;
    emit_field_end(out, trailing)
}

fn emit_field_prefix<S: JsonSink + ?Sized>(
    out: &mut S,
    indent: usize,
    name: &str,
) -> Result<(), S::Error> {
    for _ in 0..indent {
        out.write_str("  ")?;
    }
    out.write_str("\"")?;
    // Field names are emitted raw, exactly as in the legacy formatter.
    out.write_str(name)?;
    out.write_str("\": ")
}

fn emit_field_end<S: JsonSink + ?Sized>(out: &mut S, trailing: bool) -> Result<(), S::Error> {
    out.write_str(if trailing { ",\n" } else { "\n" })
}

pub(crate) fn number_field(
    out: &mut String,
    indent: usize,
    name: &str,
    value: usize,
    trailing: bool,
) {
    infallible(emit_number_field(out, indent, name, value, trailing));
}

pub(super) fn emit_number_field<S: JsonSink + ?Sized>(
    out: &mut S,
    indent: usize,
    name: &str,
    value: usize,
    trailing: bool,
) -> Result<(), S::Error> {
    emit_field_prefix(out, indent, name)?;
    emit_usize(out, value)?;
    emit_field_end(out, trailing)
}

pub(super) fn emit_usize<S: JsonSink + ?Sized>(
    out: &mut S,
    mut value: usize,
) -> Result<(), S::Error> {
    // A usize has at most BITS decimal digits. Iterator-owned slots avoid any
    // scratch index or length arithmetic, including at usize::MAX.
    let mut scratch = ['\0'; usize::BITS as usize];
    for slot in scratch.iter_mut().rev() {
        *slot = match value % 10 {
            0 => '0',
            1 => '1',
            2 => '2',
            3 => '3',
            4 => '4',
            5 => '5',
            6 => '6',
            7 => '7',
            8 => '8',
            _ => '9',
        };
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let mut digit = [0; 4];
    for ch in scratch.into_iter().filter(|ch| *ch != '\0') {
        out.write_str(ch.encode_utf8(&mut digit))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WireFailureKind {
    Limit,
    Overflow,
    Deadline,
    Writer(io::ErrorKind),
    ZeroProgress,
    OversizedWrite,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WireFailure {
    pub(super) kind: WireFailureKind,
    /// The next/current actual writer callback, including final flush.
    pub(super) ordinal: usize,
}

/// DATA byte measurement only: it reserves no storage or publication authority.
pub(super) struct CountingSink {
    limit: usize,
    bytes: usize,
    failed: Option<WireFailure>,
}

impl CountingSink {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            bytes: 0,
            failed: None,
        }
    }
    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }
}

impl JsonSink for CountingSink {
    type Error = WireFailure;
    fn write_str(&mut self, chunk: &str) -> Result<(), WireFailure> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        let failure = match self.bytes.checked_add(chunk.len()) {
            None => Some(WireFailureKind::Overflow),
            Some(bytes) if bytes > self.limit => Some(WireFailureKind::Limit),
            Some(bytes) => {
                self.bytes = bytes;
                None
            }
        };
        if let Some(kind) = failure {
            let error = WireFailure { kind, ordinal: 0 };
            self.failed = Some(error);
            return Err(error);
        }
        Ok(())
    }
}

/// Finite borrowed DATA emission. This bounds offered wire bytes, not retained
/// analysis, output storage, allocator traffic, syscall time or source currentness.
pub(super) struct BoundedWriteSink<'a, W: Write + ?Sized> {
    writer: &'a mut W,
    limit: usize,
    written: usize,
    ordinal: usize,
    original_deadline: Option<Instant>,
    failed: Option<WireFailure>,
    finished: bool,
}

impl<'a, W: Write + ?Sized> BoundedWriteSink<'a, W> {
    pub(super) fn new(writer: &'a mut W, limit: usize, original_deadline: Option<Instant>) -> Self {
        Self {
            writer,
            limit,
            written: 0,
            ordinal: 0,
            original_deadline,
            failed: None,
            finished: false,
        }
    }
    pub(super) fn written(&self) -> usize {
        self.written
    }
    fn refuse(&mut self, kind: WireFailureKind, ordinal: usize) -> WireFailure {
        let error = WireFailure { kind, ordinal };
        *self.failed.get_or_insert(error)
    }
    fn deadline(&mut self, ordinal: usize) -> Result<(), WireFailure> {
        if self
            .original_deadline
            .is_some_and(|end| Instant::now() >= end)
        {
            return Err(self.refuse(WireFailureKind::Deadline, ordinal));
        }
        Ok(())
    }
    fn callback(&mut self) -> Result<usize, WireFailure> {
        let ordinal = self.ordinal;
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or_else(|| self.refuse(WireFailureKind::Overflow, ordinal))?;
        Ok(ordinal)
    }
    pub(super) fn finish(&mut self) -> Result<usize, WireFailure> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        if self.finished {
            return Ok(self.written);
        }
        self.deadline(self.ordinal)?;
        let ordinal = self.callback()?;
        if let Err(error) = self.writer.flush() {
            return Err(self.refuse(WireFailureKind::Writer(error.kind()), ordinal));
        }
        self.deadline(ordinal)?;
        self.finished = true;
        Ok(self.written)
    }
}

impl<W: Write + ?Sized> JsonSink for BoundedWriteSink<'_, W> {
    type Error = WireFailure;
    fn write_str(&mut self, chunk: &str) -> Result<(), WireFailure> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        if self.finished {
            return Err(self.refuse(WireFailureKind::Finished, self.ordinal));
        }
        // Admit the full offered slice before exposing even a prefix to Write.
        let Some(total) = self.written.checked_add(chunk.len()) else {
            return Err(self.refuse(WireFailureKind::Overflow, self.ordinal));
        };
        if total > self.limit {
            return Err(self.refuse(WireFailureKind::Limit, self.ordinal));
        }
        self.deadline(self.ordinal)?;
        let mut remaining = chunk.as_bytes();
        while !remaining.is_empty() {
            self.deadline(self.ordinal)?;
            let ordinal = self.callback()?;
            let count = match self.writer.write(remaining) {
                Ok(0) => return Err(self.refuse(WireFailureKind::ZeroProgress, ordinal)),
                Ok(count) if count > remaining.len() => {
                    return Err(self.refuse(WireFailureKind::OversizedWrite, ordinal));
                }
                Ok(count) => count,
                Err(error) => {
                    return Err(self.refuse(WireFailureKind::Writer(error.kind()), ordinal));
                }
            };
            self.written = self
                .written
                .checked_add(count)
                .ok_or_else(|| self.refuse(WireFailureKind::Overflow, ordinal))?;
            // Byte slicing permits legitimate short writes inside UTF8.
            remaining = &remaining[count..];
            self.deadline(ordinal)?;
        }
        Ok(())
    }
}

pub(crate) fn float_field(out: &mut String, indent: usize, name: &str, value: f32, trailing: bool) {
    out.push_str(&format!(
        "{}\"{}\": {:.2}{}\n",
        "  ".repeat(indent),
        name,
        value,
        if trailing { "," } else { "" }
    ));
}

pub(crate) fn array_field(
    out: &mut String,
    indent: usize,
    name: &str,
    values: &[String],
    trailing: bool,
) {
    infallible(emit_array_field(out, indent, name, values, trailing));
}

pub(super) fn emit_array_field<S: JsonSink + ?Sized>(
    out: &mut S,
    indent: usize,
    name: &str,
    values: &[String],
    trailing: bool,
) -> Result<(), S::Error> {
    emit_field_prefix(out, indent, name)?;
    out.write_str("[")?;
    for (idx, value) in values.iter().enumerate() {
        if idx != 0 {
            out.write_str(", ")?;
        }
        out.write_str("\"")?;
        emit_escape(out, value)?;
        out.write_str("\"")?;
    }
    out.write_str("]")?;
    emit_field_end(out, trailing)
}

pub(crate) fn escape(value: &str) -> String {
    let mut out = String::new();
    infallible(emit_escape(&mut out, value));
    out
}

pub(super) fn emit_escape<S: JsonSink + ?Sized>(out: &mut S, value: &str) -> Result<(), S::Error> {
    let mut start = 0;
    for (idx, ch) in value.char_indices() {
        let escaped = match ch {
            '\\' => Some("\\\\"),
            '"' => Some("\\\""),
            '\n' => Some("\\n"),
            '\r' => Some("\\r"),
            '\t' => Some("\\t"),
            c if c.is_control() => None,
            _ => continue,
        };
        if start < idx {
            out.write_str(&value[start..idx])?;
        }
        if let Some(escaped) = escaped {
            out.write_str(escaped)?;
        } else {
            let code = ch as u32;
            if code <= 0xFFFF {
                emit_code_unit(out, code)?;
            } else {
                let adjusted = code - 0x10000;
                emit_code_unit(out, 0xD800 + (adjusted >> 10))?;
                emit_code_unit(out, 0xDC00 + (adjusted & 0x3FF))?;
            }
        }
        start = idx + ch.len_utf8();
    }
    if start < value.len() {
        out.write_str(&value[start..])?;
    }
    Ok(())
}

fn emit_code_unit<S: JsonSink + ?Sized>(out: &mut S, code: u32) -> Result<(), S::Error> {
    out.write_str("\\u")?;
    let mut scratch = [0; 4];
    for shift in [12, 8, 4, 0] {
        let digit = b"0123456789abcdef"[((code >> shift) & 0xF) as usize] as char;
        out.write_str(digit.encode_utf8(&mut scratch))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        JsonSink, array_field, emit_array_field, emit_escape, emit_field, escape, field,
        float_field, number_field,
    };

    // Frozen byte oracle from blob 08f0e9fdbdd3cad44476981398f05f061147c113.
    // Deliberately retain the allocating legacy algorithm, independent of emitters.
    fn legacy_escape(value: &str) -> String {
        let mut out = String::new();
        for ch in value.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => {
                    let code = c as u32;
                    if code <= 0xFFFF {
                        out.push_str(&format!("\\u{code:04x}"));
                    } else {
                        let adjusted = code - 0x10000;
                        let high = 0xD800 + (adjusted >> 10);
                        let low = 0xDC00 + (adjusted & 0x3FF);
                        out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
                    }
                }
                c => out.push(c),
            }
        }
        out
    }

    fn legacy_field(out: &mut String, indent: usize, name: &str, value: &str, trailing: bool) {
        out.push_str(&format!(
            "{}\"{}\": \"{}\"{}\n",
            "  ".repeat(indent),
            name,
            legacy_escape(value),
            if trailing { "," } else { "" }
        ));
    }

    fn legacy_array_field(
        out: &mut String,
        indent: usize,
        name: &str,
        values: &[String],
        trailing: bool,
    ) {
        out.push_str(&format!("{}\"{}\": [", "  ".repeat(indent), name));
        for (idx, value) in values.iter().enumerate() {
            out.push_str(&format!("\"{}\"", legacy_escape(value)));
            if idx + 1 != values.len() {
                out.push_str(", ");
            }
        }
        out.push_str(&format!("]{}\n", if trailing { "," } else { "" }));
    }

    fn corpus() -> Vec<String> {
        vec![
            String::new(),
            "plain / ascii".to_string(),
            "a\"b\\c\n\r\t\u{0008}\u{000c}".to_string(),
            (0..=31)
                .chain(127..=159)
                .filter_map(char::from_u32)
                .collect(),
            "é中🦀e\u{0301}\u{200d}\u{2028}\u{2029}\u{feff}\u{ffff}\u{10ffff}".to_string(),
        ]
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Failure {
        Writer(usize),
        Cap,
    }

    struct CheckedSink {
        output: String,
        cap: usize,
        fail_at: Option<usize>,
        partial_write: bool,
        calls: usize,
        reserves: usize,
        failed: Option<Failure>,
    }

    impl CheckedSink {
        fn new(cap: usize, fail_at: Option<usize>, partial_write: bool) -> Self {
            Self {
                output: String::new(),
                cap,
                fail_at,
                partial_write,
                calls: 0,
                reserves: 0,
                failed: None,
            }
        }
    }

    impl JsonSink for CheckedSink {
        type Error = Failure;

        fn write_str(&mut self, chunk: &str) -> Result<(), Self::Error> {
            let ordinal = self.calls;
            self.calls += 1;
            if let Some(error) = self.failed {
                return Err(error);
            }
            if self.fail_at == Some(ordinal) {
                if self.partial_write
                    && let Some(ch) = chunk.chars().next()
                {
                    self.output.push(ch);
                }
                let error = Failure::Writer(ordinal);
                self.failed = Some(error);
                return Err(error);
            }
            if self
                .output
                .len()
                .checked_add(chunk.len())
                .is_none_or(|len| len > self.cap)
            {
                self.failed = Some(Failure::Cap);
                return Err(Failure::Cap);
            }
            self.reserves += 1;
            self.output.reserve_exact(chunk.len());
            self.output.push_str(chunk);
            Ok(())
        }
    }

    fn check_failures(emit: impl Fn(&mut CheckedSink) -> Result<(), Failure>, expected: &str) {
        let mut complete = CheckedSink::new(usize::MAX, None, false);
        assert_eq!(emit(&mut complete), Ok(()));
        assert_eq!(complete.output, expected);
        for ordinal in 0..complete.calls {
            for partial_write in [false, true] {
                let mut sink = CheckedSink::new(usize::MAX, Some(ordinal), partial_write);
                assert_eq!(emit(&mut sink), Err(Failure::Writer(ordinal)));
                assert_eq!(sink.calls, ordinal + 1, "no writes after the first failure");
                assert_eq!(
                    sink.reserves, ordinal,
                    "the refused fragment was not admitted"
                );
                assert!(expected.starts_with(&sink.output));
            }
        }
        for cap in 0..expected.len() {
            let mut sink = CheckedSink::new(cap, None, false);
            assert_eq!(emit(&mut sink), Err(Failure::Cap), "byte cap {cap}");
            assert_eq!(sink.reserves + 1, sink.calls);
            assert!(sink.output.len() <= cap);
            assert!(sink.output.capacity() <= cap);
            assert!(expected.starts_with(&sink.output));
            let before = (sink.output.len(), sink.output.capacity(), sink.reserves);
            assert_eq!(sink.write_str("retry"), Err(Failure::Cap));
            assert_eq!(
                before,
                (sink.output.len(), sink.output.capacity(), sink.reserves)
            );
        }
        let mut exact = CheckedSink::new(expected.len(), None, false);
        assert_eq!(emit(&mut exact), Ok(()));
        assert_eq!(exact.output, expected);
        let before = (exact.output.len(), exact.output.capacity(), exact.reserves);
        assert_eq!(exact.write_str("x"), Err(Failure::Cap));
        assert_eq!(
            before,
            (exact.output.len(), exact.output.capacity(), exact.reserves)
        );
    }

    #[test]
    fn shared_emitters_match_legacy_bytes_and_propagate_every_fragment_failure() {
        let cases = corpus();
        for value in &cases {
            let expected = legacy_escape(value);
            assert_eq!(escape(value), expected);
            check_failures(|sink| emit_escape(sink, value), &expected);
            for indent in [0, 1, 3] {
                for trailing in [false, true] {
                    for name in ["field", "raw\"name\\\n中"] {
                        let mut expected = String::new();
                        legacy_field(&mut expected, indent, name, value, trailing);
                        let mut actual = String::new();
                        field(&mut actual, indent, name, value, trailing);
                        assert_eq!(actual, expected);
                        check_failures(
                            |sink| emit_field(sink, indent, name, value, trailing),
                            &expected,
                        );
                    }
                }
            }
        }
        for values in [&cases[..0], &cases[..1], &cases[2..3], &cases[..]] {
            for indent in [0, 1, 3] {
                for trailing in [false, true] {
                    let mut expected = String::new();
                    legacy_array_field(&mut expected, indent, "raw\"array\n", values, trailing);
                    let mut actual = String::new();
                    array_field(&mut actual, indent, "raw\"array\n", values, trailing);
                    assert_eq!(actual, expected);
                    check_failures(
                        |sink| emit_array_field(sink, indent, "raw\"array\n", values, trailing),
                        &expected,
                    );
                }
            }
        }
    }

    #[test]
    fn escaping_matches_legacy_for_every_unicode_scalar_in_bounded_chunks() {
        let mut chunk = String::new();
        let mut count = 0;
        for ch in (0..=0x10FFFF).filter_map(char::from_u32) {
            count += 1;
            chunk.push(ch);
            if chunk.len() >= 4096 {
                assert_eq!(escape(&chunk), legacy_escape(&chunk));
                chunk.clear();
            }
        }
        assert_eq!(escape(&chunk), legacy_escape(&chunk));
        assert_eq!(count, 1_112_064);
    }

    #[test]
    fn nested_fields_and_arrays_match_legacy_and_abort_on_refusal() {
        let values = corpus();
        let mut expected = "{\n  \"rows\": [\n    {\n".to_string();
        legacy_field(&mut expected, 3, "text", &values[4], true);
        legacy_array_field(&mut expected, 3, "items", &values, false);
        expected.push_str("    },\n    {\n");
        legacy_array_field(&mut expected, 3, "empty", &[], false);
        expected.push_str("    }\n  ]\n}\n");
        check_failures(
            |sink| {
                sink.write_str("{\n  \"rows\": [\n    {\n")?;
                emit_field(sink, 3, "text", &values[4], true)?;
                emit_array_field(sink, 3, "items", &values, false)?;
                sink.write_str("    },\n    {\n")?;
                emit_array_field(sink, 3, "empty", &[], false)?;
                sink.write_str("    }\n  ]\n}\n")
            },
            &expected,
        );
    }

    #[test]
    fn huge_indent_refuses_at_first_chunk_without_constructing_the_output() {
        let mut sink = CheckedSink::new(0, None, false);
        assert_eq!(
            emit_field(&mut sink, usize::MAX, "name", "value", false),
            Err(Failure::Cap)
        );
        assert_eq!(sink.calls, 1);
        assert_eq!(sink.reserves, 0);
        assert_eq!(sink.output.capacity(), 0);
        let mut sink = CheckedSink::new(0, None, false);
        assert_eq!(
            emit_array_field(&mut sink, usize::MAX, "array", &[], false),
            Err(Failure::Cap)
        );
        assert_eq!(sink.calls, 1);
        assert_eq!(sink.reserves, 0);
        assert_eq!(sink.output.capacity(), 0);
    }

    #[test]
    fn unescaped_unicode_is_borrowed_directly_from_input() {
        struct BorrowedSink<'a>(&'a str);
        impl JsonSink for BorrowedSink<'_> {
            type Error = ();

            fn write_str(&mut self, chunk: &str) -> Result<(), Self::Error> {
                assert_eq!(chunk, self.0);
                assert_eq!(chunk.as_ptr(), self.0.as_ptr());
                Err(())
            }
        }
        let value = "plain é中🦀";
        assert_eq!(emit_escape(&mut BorrowedSink(value), value), Err(()));
    }

    #[test]
    fn numeric_helpers_retain_special_float_and_integer_bytes() {
        for value in [0.0, -0.0, 0.125, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for trailing in [false, true] {
                let mut actual = String::new();
                float_field(&mut actual, 2, "score", value, trailing);
                assert_eq!(
                    actual,
                    format!(
                        "    \"score\": {value:.2}{}\n",
                        if trailing { "," } else { "" }
                    )
                );
            }
        }
        for value in [0, 7, usize::MAX] {
            let mut actual = String::new();
            number_field(&mut actual, 0, "count", value, true);
            assert_eq!(actual, format!("\"count\": {value},\n"));
        }
    }

    #[test]
    fn escapes_json() {
        assert_eq!(escape("a\"b\n"), "a\\\"b\\n");
    }

    #[test]
    fn escapes_backslash_and_control_chars() {
        assert_eq!(escape("\\\u{0008}\t"), "\\\\\\u0008\\t");
    }

    #[test]
    fn renders_scalar_fields() {
        let mut out = String::new();

        field(&mut out, 1, "name", "a\"b", true);
        number_field(&mut out, 1, "count", 7, true);
        float_field(&mut out, 1, "score", 0.125, false);

        assert_eq!(
            out,
            "  \"name\": \"a\\\"b\",\n  \"count\": 7,\n  \"score\": 0.12\n"
        );
    }

    #[test]
    fn renders_array_fields_with_and_without_values() {
        let mut out = String::new();

        array_field(
            &mut out,
            1,
            "stop_reasons",
            &["a\"b".to_string(), "c".to_string()],
            true,
        );
        array_field(&mut out, 1, "missing", &[], false);

        assert_eq!(
            out,
            "  \"stop_reasons\": [\"a\\\"b\", \"c\"],\n  \"missing\": []\n"
        );
    }
}

#[cfg(test)]
mod bounded_wire_tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Duration;

    enum Action {
        Short(usize),
        Zero,
        Oversized,
        Error(io::ErrorKind),
        Delayed(Duration, Result<usize, io::ErrorKind>),
    }
    #[derive(Default)]
    struct Script {
        actions: VecDeque<Action>,
        bytes: Vec<u8>,
        calls: usize,
        flushes: usize,
        flush_error: Option<io::ErrorKind>,
        flush_delay: Option<Duration>,
    }
    impl Write for Script {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            let n = match self.actions.pop_front() {
                Some(Action::Zero) => return Ok(0),
                Some(Action::Oversized) => return Ok(bytes.len() + 1),
                Some(Action::Error(kind)) => return Err(io::Error::from(kind)),
                Some(Action::Short(n)) => n.min(bytes.len()),
                Some(Action::Delayed(delay, result)) => {
                    std::thread::sleep(delay);
                    match result {
                        Ok(n) => n.min(bytes.len()),
                        Err(kind) => return Err(io::Error::from(kind)),
                    }
                }
                None => bytes.len(),
            };
            self.bytes.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if let Some(delay) = self.flush_delay {
                std::thread::sleep(delay);
            }
            if let Some(kind) = self.flush_error {
                Err(io::Error::from(kind))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn bounded_integer_matches_frozen_decimal_grammar() {
        for indent in [0, 1, 4] {
            for value in [0, 1, 9, 10, 99, 100, 999, 1000, usize::MAX] {
                for trailing in [false, true] {
                    let mut expected = String::new();
                    frozen_number_field(&mut expected, indent, "line 雪", value, trailing);
                    let mut actual = String::new();
                    number_field(&mut actual, indent, "line 雪", value, trailing);
                    assert_eq!(actual, expected);
                    let mut bytes = Vec::new();
                    let mut sink = BoundedWriteSink::new(&mut bytes, expected.len(), None);
                    assert!(
                        emit_number_field(&mut sink, indent, "line 雪", value, trailing).is_ok()
                    );
                    assert_eq!(sink.finish(), Ok(expected.len()));
                    assert_eq!(bytes, expected.as_bytes());
                }
            }
        }
        let mut literal = String::new();
        infallible(emit_usize(&mut literal, 100));
        assert_eq!(literal, "100");
        let mut writer = Script::default();
        let mut sink = BoundedWriteSink::new(&mut writer, 0, None);
        assert_eq!(
            emit_number_field(&mut sink, usize::MAX, "line", usize::MAX, true)
                .expect_err("huge indent accepted")
                .kind,
            WireFailureKind::Limit
        );
        assert_eq!(writer.calls, 0);
    }

    #[test]
    fn counter_counts_utf8_bytes_and_retains_first_refusal() {
        let mut exact = CountingSink::new(4);
        assert!(exact.write_str("雪").is_ok());
        assert_eq!(exact.bytes(), 3);
        assert!(exact.write_str("x").is_ok());
        assert_eq!(exact.bytes(), 4);
        let failure = exact.write_str("z").expect_err("cap accepted");
        assert_eq!(failure.kind, WireFailureKind::Limit);
        assert_eq!(exact.write_str(""), Err(failure));
        assert_eq!(exact.bytes(), 4);
        let mut overflow = CountingSink::new(usize::MAX);
        overflow.bytes = usize::MAX;
        let error = overflow.write_str("x").expect_err("overflow accepted");
        assert_eq!(error.kind, WireFailureKind::Overflow);
        assert_eq!(overflow.bytes(), usize::MAX);
        let mut chars = CountingSink::new(1);
        assert_eq!(
            chars
                .write_str("雪")
                .expect_err("character counting accepted")
                .kind,
            WireFailureKind::Limit
        );
        assert_eq!(chars.bytes(), 0);
    }

    #[test]
    fn offered_slice_admitted_before_callback_and_short_utf8_writes_progress() {
        let mut writer = Script::default();
        let mut sink = BoundedWriteSink::new(&mut writer, 2, None);
        let error = sink.write_str("雪").expect_err("oversized slice accepted");
        assert_eq!(
            error,
            WireFailure {
                kind: WireFailureKind::Limit,
                ordinal: 0
            }
        );
        assert_eq!(sink.written(), 0);
        assert_eq!(sink.finish(), Err(error));
        assert_eq!(writer.calls, 0);
        assert_eq!(writer.flushes, 0);
        assert!(writer.bytes.is_empty());
        let mut writer = Script {
            actions: VecDeque::from([Action::Short(1), Action::Short(1), Action::Short(1)]),
            ..Script::default()
        };
        let mut sink = BoundedWriteSink::new(&mut writer, 3, None);
        assert!(sink.write_str("雪").is_ok());
        assert_eq!(sink.written(), 3);
        assert_eq!(sink.finish(), Ok(3));
        assert_eq!(sink.finish(), Ok(3));
        assert_eq!(
            sink.write_str(""),
            Err(WireFailure {
                kind: WireFailureKind::Finished,
                ordinal: 4
            })
        );
        assert_eq!(writer.calls, 3);
        assert_eq!(writer.flushes, 1);
        assert_eq!(writer.bytes, "雪".as_bytes());
    }

    #[test]
    fn accepted_short_prefix_charged_and_first_writer_error_is_sticky() {
        let mut writer = Script {
            actions: VecDeque::from([Action::Short(1), Action::Error(io::ErrorKind::BrokenPipe)]),
            ..Script::default()
        };
        let mut sink = BoundedWriteSink::new(&mut writer, 4, None);
        let error = sink.write_str("abcd").expect_err("writer failure ignored");
        assert_eq!(
            error,
            WireFailure {
                kind: WireFailureKind::Writer(io::ErrorKind::BrokenPipe),
                ordinal: 1
            }
        );
        assert_eq!(sink.written(), 1);
        assert_eq!(sink.write_str("later"), Err(error));
        assert_eq!(sink.finish(), Err(error));
        assert_eq!(writer.calls, 2);
        assert_eq!(writer.flushes, 0);
        assert_eq!(writer.bytes, b"a");
    }

    #[test]
    fn zero_oversized_interrupted_and_errors_never_retry_or_flush() {
        for (action, kind) in [
            (Action::Zero, WireFailureKind::ZeroProgress),
            (Action::Oversized, WireFailureKind::OversizedWrite),
            (
                Action::Error(io::ErrorKind::Interrupted),
                WireFailureKind::Writer(io::ErrorKind::Interrupted),
            ),
            (
                Action::Error(io::ErrorKind::PermissionDenied),
                WireFailureKind::Writer(io::ErrorKind::PermissionDenied),
            ),
        ] {
            let mut writer = Script {
                actions: VecDeque::from([action]),
                ..Script::default()
            };
            let mut sink = BoundedWriteSink::new(&mut writer, 4, None);
            let error = sink.write_str("abcd").expect_err("protocol fault accepted");
            assert_eq!(error, WireFailure { kind, ordinal: 0 });
            assert_eq!(sink.written(), 0);
            assert_eq!(sink.write_str(""), Err(error));
            assert_eq!(sink.finish(), Err(error));
            assert_eq!(writer.calls, 1);
            assert_eq!(writer.flushes, 0);
        }
    }

    #[test]
    fn ordinal_and_byte_overflow_refuse_without_callback() {
        let mut writer = Script::default();
        let mut sink = BoundedWriteSink::new(&mut writer, usize::MAX, None);
        sink.ordinal = usize::MAX;
        assert_eq!(
            sink.write_str("x"),
            Err(WireFailure {
                kind: WireFailureKind::Overflow,
                ordinal: usize::MAX
            })
        );
        assert_eq!(sink.written(), 0);
        assert_eq!(writer.calls, 0);
        let mut writer = Script::default();
        let mut sink = BoundedWriteSink::new(&mut writer, usize::MAX, None);
        sink.written = usize::MAX;
        assert_eq!(
            sink.write_str("x")
                .expect_err("byte overflow accepted")
                .kind,
            WireFailureKind::Overflow
        );
        assert_eq!(writer.calls, 0);
    }

    #[test]
    fn original_deadline_valid_bytes_and_writer_fault_order_are_preserved() {
        let mut writer = Script::default();
        let original = Instant::now();
        let mut sink = BoundedWriteSink::new(&mut writer, 2, Some(original));
        assert_eq!(
            sink.write_str("x")
                .expect_err("expired entry accepted")
                .kind,
            WireFailureKind::Deadline
        );
        assert_eq!(writer.calls, 0);
        for (result, kind, charged) in [
            (Ok(1), WireFailureKind::Deadline, 1),
            (Ok(0), WireFailureKind::ZeroProgress, 0),
            (
                Err(io::ErrorKind::BrokenPipe),
                WireFailureKind::Writer(io::ErrorKind::BrokenPipe),
                0,
            ),
        ] {
            let mut writer = Script {
                actions: VecDeque::from([Action::Delayed(Duration::from_millis(20), result)]),
                ..Script::default()
            };
            let original = Instant::now() + Duration::from_millis(10);
            let mut sink = BoundedWriteSink::new(&mut writer, 2, Some(original));
            let error = sink.write_str("xy").expect_err("late completion accepted");
            assert_eq!(error, WireFailure { kind, ordinal: 0 });
            assert_eq!(sink.written(), charged);
            assert_eq!(sink.finish(), Err(error));
            assert_eq!(writer.calls, 1);
            assert_eq!(writer.flushes, 0);
            assert_eq!(writer.bytes.len(), charged);
        }
    }

    #[test]
    fn final_flush_preserves_first_error_and_original_deadline() {
        for flush_error in [None, Some(io::ErrorKind::BrokenPipe)] {
            let mut writer = Script {
                flush_error,
                flush_delay: Some(Duration::from_millis(20)),
                ..Script::default()
            };
            let original = Instant::now() + Duration::from_millis(10);
            let mut sink = BoundedWriteSink::new(&mut writer, 1, Some(original));
            assert!(sink.write_str("x").is_ok());
            let kind = flush_error
                .map(WireFailureKind::Writer)
                .unwrap_or(WireFailureKind::Deadline);
            let error = sink.finish().expect_err("late flush accepted");
            assert_eq!(error, WireFailure { kind, ordinal: 1 });
            assert_eq!(sink.written(), 1);
            assert_eq!(sink.finish(), Err(error));
            assert_eq!(writer.calls, 1);
            assert_eq!(writer.flushes, 1);
        }
    }

    // Exact pre-repair integer oracle, independent of the shared emitter.
    fn frozen_number_field(
        out: &mut String,
        indent: usize,
        name: &str,
        value: usize,
        trailing: bool,
    ) {
        out.push_str(&format!(
            "{}\"{}\": {}{}\n",
            "  ".repeat(indent),
            name,
            value,
            if trailing { "," } else { "" }
        ));
    }
}

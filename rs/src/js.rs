//! Strings the way JavaScript treats them.
//!
//! The TypeScript measures, cuts and trims its strings in UTF-16 code units
//! and by JavaScript's idea of white space, and sends them as
//! `JSON.stringify` writes them. Rust's `str` counts bytes and has a
//! different white space, so the few operations the port needs are spelt out
//! here.

/// `string.length`: UTF-16 code units.
pub fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// White space as `String.prototype.trim` sees it: WhiteSpace and
/// LineTerminator of the language, which is not Rust's `char::is_whitespace`
/// (that has U+0085 and lacks U+FEFF).
pub fn is_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | '\u{a}' | '\u{b}' | '\u{c}' | '\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// `string.trim()`.
pub fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `string.trimEnd()`.
pub fn trim_end(text: &str) -> &str {
    text.trim_end_matches(is_space)
}

/// A line terminator, which `.` in a regular expression does not match.
pub fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `string.slice(0, units)`, or `None` where that would cut a character of
/// two code units in half (JavaScript would keep half of it, a lone
/// surrogate, which no Rust string can hold).
pub fn head_units(text: &str, units: usize) -> Option<&str> {
    let mut seen = 0;
    for (at, c) in text.char_indices() {
        if seen == units {
            return Some(&text[..at]);
        }
        seen += c.len_utf16();
        if seen > units {
            return None;
        }
    }
    Some(text)
}

/// `string.slice(-units)`, or `None` where that would cut a character of two
/// code units in half.
pub fn tail_units(text: &str, units: usize) -> Option<&str> {
    let mut seen = 0;
    for (at, c) in text.char_indices().rev() {
        seen += c.len_utf16();
        if seen == units {
            return Some(&text[at..]);
        }
        if seen > units {
            return None;
        }
    }
    Some(text)
}

/// `JSON.stringify(text)` for a string: `"`, `\` and the control characters
/// escaped, the short forms where JSON has one, lowercase hex for the rest;
/// everything else as it is.
pub fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_are_code_units() {
        assert_eq!(utf16_len("abc"), 3);
        assert_eq!(utf16_len("é"), 1);
        assert_eq!(utf16_len("😀"), 2);
        assert_eq!(utf16_len(""), 0);
    }

    #[test]
    fn trimming_is_javascripts() {
        assert_eq!(trim("\u{feff} a \u{3000}"), "a");
        assert_eq!(trim("\u{85}a\u{85}"), "\u{85}a\u{85}");
        assert_eq!(trim_end(" a \n\t"), " a");
        assert_eq!(trim("\u{2028}a\u{2029}"), "a");
    }

    #[test]
    fn slices_by_code_units() {
        assert_eq!(head_units("abcdef", 3), Some("abc"));
        assert_eq!(head_units("ab", 3), Some("ab"));
        assert_eq!(head_units("aé😀b", 2), Some("aé"));
        assert_eq!(head_units("aé😀b", 3), None);
        assert_eq!(head_units("aé😀b", 4), Some("aé😀"));
        assert_eq!(tail_units("abcdef", 2), Some("ef"));
        assert_eq!(tail_units("ab", 5), Some("ab"));
        assert_eq!(tail_units("a😀b", 2), None);
        assert_eq!(tail_units("a😀b", 3), Some("😀b"));
        assert_eq!(head_units("abc", 0), Some(""));
    }

    #[test]
    fn json_strings_as_javascript_writes_them() {
        assert_eq!(json_string("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(json_string("\n\r\t\u{8}\u{c}"), r#""\n\r\t\b\f""#);
        assert_eq!(json_string("\u{1}\u{1f}\u{7f}"), "\"\\u0001\\u001f\u{7f}\"");
        assert_eq!(json_string("é/😀\u{2028}"), "\"é/😀\u{2028}\"");
    }
}

//! CSS `font-feature-settings` (CSS Fonts 4 §6.12).

/// `normal`, or up to [`Self::MAX`] OpenType features by four-letter tag,
/// each set to a value: 1 turns a feature on, 0 off, and a larger one picks
/// an alternate. A tag named twice keeps its last value, in the place of its
/// first. It applies after `font-variant-numeric` and
/// `font-variant-ligatures`, and wins for the same feature (CSS Fonts 4 §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FontFeatures {
    len: u8,
    features: [(u32, u32); FontFeatures::MAX],
}

impl FontFeatures {
    /// The most features one value names. A run carries these by value
    /// (`TextStyle`, the hosts' runs), so the list is bounded; sixteen is
    /// more than an app picks from a face's character variants, stylistic
    /// sets and figure styles together.
    pub const MAX: usize = 16;

    /// The largest value kept, the most any host's shaper takes (HarfRust's
    /// is 16 bits); a larger one is this, which picks no alternate a face
    /// can have.
    pub const MAX_VALUE: u32 = u16::MAX as u32;

    /// The features as (tag, value), the tag big-endian (`'tnum'` is
    /// `0x746e756d`).
    pub fn features(&self) -> &[(u32, u32)] {
        &self.features[..self.len as usize]
    }

    /// Whether it is `normal`: no feature named.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The row's parse; `None` where [`Self::check`] refuses.
    pub fn parse(css: &str) -> Option<Self> {
        Self::check(css).ok()
    }

    /// `normal`, or a comma-separated list of a quoted four-character tag
    /// (U+0020–U+007E, read as a CSS string: escapes decoded) and an
    /// optional value: a non-negative integer, `on` (1) or `off` (0); a tag
    /// alone is 1. Anything else is refused, as CSS refuses it, and so are
    /// more than [`Self::MAX`] distinct tags. A value past [`Self::MAX_VALUE`]
    /// is that.
    pub fn check(css: &str) -> Result<Self, &'static str> {
        let mut out = Self::default();
        let mut t = Tokens {
            s: css.chars().collect(),
            i: 0,
        };
        t.space()?;
        if t.word().0.eq_ignore_ascii_case("normal") {
            t.space()?;
            return if t.peek(0).is_none() {
                Ok(out)
            } else {
                Err(SHAPE)
            };
        }
        t.i = 0;
        loop {
            t.space()?;
            let tag = t.string()?;
            if tag.len() != 4 || !tag.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
                return Err(
                    "a feature's tag is exactly four printable ASCII characters, as `\"tnum\"`",
                );
            }
            t.space()?;
            let value = t.value()?;
            t.space()?;
            let tag = u32::from_be_bytes(tag.as_bytes().try_into().unwrap_or_default());
            if let Some(f) = out.features[..out.len as usize]
                .iter_mut()
                .find(|(t, _)| *t == tag)
            {
                f.1 = value;
            } else {
                if out.len as usize == Self::MAX {
                    return Err("at most 16 distinct features in one font-feature-settings");
                }
                out.features[out.len as usize] = (tag, value);
                out.len += 1;
            }
            match t.next() {
                None => return Ok(out),
                Some(',') => {}
                Some(_) => return Err(SHAPE),
            }
        }
    }

    /// As CSS serializes it (a value of 1 left unsaid, a tag a CSS string),
    /// what [`Self::parse`] reads back.
    pub fn css(&self) -> String {
        if self.is_empty() {
            return "normal".into();
        }
        self.features()
            .iter()
            .map(|(tag, value)| {
                let mut out = String::from('"');
                for c in tag.to_be_bytes().map(char::from) {
                    if matches!(c, '"' | '\\') {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push('"');
                if *value != 1 {
                    out.push_str(&format!(" {value}"));
                }
                out
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The value set for `tag`, if any.
    pub fn get(&self, tag: &[u8; 4]) -> Option<u32> {
        let tag = u32::from_be_bytes(*tag);
        self.features()
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, v)| *v)
    }
}

const WHITESPACE: [char; 5] = [' ', '\t', '\n', '\r', '\u{c}'];
const SHAPE: &str = "each feature is a quoted tag, as `\"cv05\"`, then an optional value; features are separated by commas";

/// The value's characters, read as CSS Syntax 3 §4 tokenizes them.
struct Tokens {
    s: Vec<char>,
    i: usize,
}

impl Tokens {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.s.get(self.i + ahead).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek(0);
        self.i += usize::from(c.is_some());
        c
    }

    /// Whitespace and comments.
    fn space(&mut self) -> Result<(), &'static str> {
        loop {
            match (self.peek(0), self.peek(1)) {
                (Some(c), _) if WHITESPACE.contains(&c) => self.i += 1,
                (Some('/'), Some('*')) => {
                    self.i += 2;
                    while self.peek(0).is_some()
                        && (self.peek(0), self.peek(1)) != (Some('*'), Some('/'))
                    {
                        self.i += 1;
                    }
                    self.i = (self.i + 2).min(self.s.len());
                }
                _ => return Ok(()),
            }
        }
    }

    /// A string token's value (§4.3.5): an escape is its character, an
    /// escaped newline nothing, and the end of the input closes it.
    fn string(&mut self) -> Result<String, &'static str> {
        let quote = match self.next() {
            Some(q @ ('"' | '\'')) => q,
            _ => return Err(SHAPE),
        };
        let mut out = String::new();
        loop {
            match self.next() {
                None => return Ok(out),
                Some(c) if c == quote => return Ok(out),
                Some('\n' | '\r' | '\u{c}') => {
                    return Err("a feature's tag is a string on one line")
                }
                Some('\\') => match self.peek(0) {
                    None => {}
                    Some('\r') => {
                        self.i += 1;
                        if self.peek(0) == Some('\n') {
                            self.i += 1;
                        }
                    }
                    Some('\n' | '\u{c}') => self.i += 1,
                    Some(_) => out.push(self.escape()),
                },
                Some(c) => out.push(c),
            }
        }
    }

    /// An escape after its backslash (§4.3.7): up to six hex digits and one
    /// whitespace, or the character itself.
    fn escape(&mut self) -> char {
        let mut hex = String::new();
        while hex.len() < 6 && self.peek(0).is_some_and(|c| c.is_ascii_hexdigit()) {
            hex.push(self.next().unwrap_or_default());
        }
        if hex.is_empty() {
            return self.next().unwrap_or(char::REPLACEMENT_CHARACTER);
        }
        if self.peek(0) == Some('\r') && self.peek(1) == Some('\n') {
            self.i += 2;
        } else if self.peek(0).is_some_and(|c| WHITESPACE.contains(&c)) {
            self.i += 1;
        }
        u32::from_str_radix(&hex, 16)
            .ok()
            .filter(|&n| n != 0)
            .and_then(char::from_u32)
            .unwrap_or(char::REPLACEMENT_CHARACTER)
    }

    /// The characters of an identifier or number token, an identifier's
    /// escapes decoded (§4.3.11), and whether there were any: an escaped
    /// digit starts an identifier, not a number.
    fn word(&mut self) -> (String, bool) {
        let (mut out, mut escaped) = (String::new(), false);
        loop {
            match self.peek(0) {
                Some('\\')
                    if self
                        .peek(1)
                        .is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{c}')) =>
                {
                    self.i += 1;
                    out.push(self.escape());
                    escaped = true;
                }
                Some(c)
                    if c.is_ascii_alphanumeric()
                        || matches!(c, '+' | '-' | '.' | '_' | '%')
                        || !c.is_ascii() =>
                {
                    self.i += 1;
                    out.push(c);
                }
                _ => return (out, escaped),
            }
        }
    }

    /// The optional value: `on`, `off`, or an integer token of zero or more,
    /// clamped to [`FontFeatures::MAX_VALUE`]; none is 1.
    fn value(&mut self) -> Result<u32, &'static str> {
        const WHY: &str = "a feature's value is a non-negative integer, `on` or `off`";
        let (word, escaped) = self.word();
        if word.is_empty() {
            return Ok(1);
        }
        match word.to_ascii_lowercase().as_str() {
            "on" => Ok(1),
            "off" => Ok(0),
            _ if escaped => Err(WHY),
            w => {
                let digits = w.strip_prefix('+').unwrap_or(w);
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(WHY);
                }
                Ok(digits
                    .parse::<u32>()
                    .map_or(FontFeatures::MAX_VALUE, |v| v.min(FontFeatures::MAX_VALUE)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_and_writes_it_back() {
        let f = FontFeatures::parse(r#""cv05" 1, 'cv11',"tnum" on, "liga" off, "salt" 3"#).unwrap();
        assert_eq!(f.get(b"cv05"), Some(1));
        assert_eq!(f.get(b"cv11"), Some(1));
        assert_eq!(f.get(b"tnum"), Some(1));
        assert_eq!(f.get(b"liga"), Some(0));
        assert_eq!(f.get(b"salt"), Some(3));
        assert_eq!(f.css(), r#""cv05", "cv11", "tnum", "liga" 0, "salt" 3"#);
        assert_eq!(FontFeatures::parse(&f.css()), Some(f));
        assert!(FontFeatures::parse("normal").unwrap().is_empty());
        assert!(FontFeatures::parse(" /* reset */ normal /* all */ ")
            .unwrap()
            .is_empty());
        assert!(FontFeatures::parse(r"nor\6d al").unwrap().is_empty());
        let escaped = FontFeatures::parse(r#""liga" o\66 f, "calt" \4fN"#).unwrap();
        assert_eq!(
            (escaped.get(b"liga"), escaped.get(b"calt")),
            (Some(0), Some(1))
        );
        assert_eq!(FontFeatures::parse("NORMAL").unwrap().css(), "normal");
        assert_eq!(
            FontFeatures::parse("'salt' 70000").unwrap().get(b"salt"),
            Some(65535)
        );
    }

    /// A tag is a CSS string: escapes are decoded before the four characters
    /// are counted, a comma or quote inside it is part of it, and it is
    /// written back escaped, so the browser reads the same four.
    #[test]
    fn a_tag_is_read_and_written_as_a_css_string() {
        for (css, tag, written) in [
            (r#""\63 v05""#, b"cv05", r#""cv05""#),
            (r#"'\63v05'"#, b"cv05", r#""cv05""#),
            (r#""c\v05""#, b"cv05", r#""cv05""#),
            ("'a,bc' 2", b"a,bc", r#""a,bc" 2"#),
            (r#"'a"bc' +2"#, b"a\"bc", r#""a\"bc" 2"#),
            (r#"'a\\bc'"#, b"a\\bc", r#""a\\bc""#),
            (r#""cv05"/* tail */"#, b"cv05", r#""cv05""#),
            // The end of the input closes a string, as CSS Syntax 3 §4.3.5 has it.
            (r#""cv05"#, b"cv05", r#""cv05""#),
        ] {
            let f = FontFeatures::parse(css).unwrap_or_else(|| panic!("{css}"));
            assert_eq!(f.features()[0].0, u32::from_be_bytes(*tag), "{css}");
            assert_eq!(f.css(), written, "{css}");
            assert_eq!(FontFeatures::parse(&f.css()), Some(f), "{css}");
        }
    }

    #[test]
    fn a_repeated_tag_keeps_its_last_value_in_its_first_place() {
        let f = FontFeatures::parse(r#""tnum", "zero", "tnum" 0"#).unwrap();
        assert_eq!(
            f.features(),
            &[
                (u32::from_be_bytes(*b"tnum"), 0),
                (u32::from_be_bytes(*b"zero"), 1)
            ]
        );
    }

    #[test]
    fn refuses_what_css_refuses_and_more_than_sixteen_features() {
        for bad in [
            "cv05 1",
            r#""cv5" 1"#,
            r#""cv055""#,
            r#""cv05" -1"#,
            r#""cv05" 1.5"#,
            r#""cv05" 1e1"#,
            r#""cv05" yes"#,
            r#""cv05" 1 2"#,
            r#""cv05""cv06""#,
            r#""cv05","#,
            "\"cv\n05\"",
            r#""\0 v05""#,
            "\"cv\u{e9}5\"",
            "normal, \"cv05\"",
            "normal normal",
            r#""cv05" \31"#,
            "",
        ] {
            assert!(FontFeatures::check(bad).is_err(), "{bad}");
        }
        let list = |n: usize| {
            (0..n)
                .map(|i| format!("\"ss{i:02}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        assert_eq!(FontFeatures::parse(&list(16)).unwrap().features().len(), 16);
        assert_eq!(
            FontFeatures::check(&list(17)),
            Err("at most 16 distinct features in one font-feature-settings")
        );
        // A repeat is not a seventeenth feature.
        assert!(FontFeatures::parse(&format!("{}, \"ss00\" 0", list(16))).is_some());
    }
}

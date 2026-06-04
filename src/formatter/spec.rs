/// Consumes a leading run of ASCII digits from `bytes`.
///
/// Returns `(value, consumed)` where `value` is the parsed number (0 if there
/// are no leading digits) and `consumed` is how many bytes were digits. This is
/// the building block that lets the caller resume parsing right after the
/// number it just read.
fn parse_digits(bytes: &[u8]) -> (usize, usize) {
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let s = str::from_utf8(&bytes[..i]).expect("ASCII digits are valid UTF-8");
    (s.parse::<usize>().unwrap_or(0), i)
}

/// Horizontal alignment used when padding a field to its minimum width.
#[derive(Debug, Clone, Copy)]
enum Align {
    Left,
    Right,
}

/// A parsed field format spec: alignment plus optional minimum (pad) and
/// maximum (truncate) widths, measured in characters.
///
/// Spec syntax (the part immediately following `%(name)`):
/// `-` for left alignment, then `min`, then optional `.max`.
/// Examples: `-7` (left-align, pad to 7), `8.3` (right-align, pad to 8,
/// truncate to 3).
#[derive(Debug, Clone, Copy)]
pub(super) struct Spec {
    align: Align,
    min: usize,
    max: usize,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            align: Align::Right,
            min: 0,
            max: 0,
        }
    }
}

impl Spec {
    /// Parses a spec from the start of `s`, consuming only spec characters
    /// (`-`, digits, `.`).
    ///
    /// Returns `(spec, consumed)` where `consumed` is the number of bytes that
    /// belonged to the spec. Anything after that is *not* part of the spec
    /// (it's a following literal), so the caller must resume at `&s[consumed..]`.
    ///
    /// `min` and `max` are independent: `apply` truncates to `max` first, then
    /// pads to `min`, so a contradictory spec like `8.3` truncates to 3 then
    /// pads back out to 8.
    pub(super) fn parse(s: &str) -> (Self, usize) {
        let mut spec = Self::default();
        let bytes = s.as_bytes();
        let mut i = 0;
        if i < bytes.len() && bytes[i] == b'-' {
            spec.align = Align::Left;
            i += 1;
        }

        let (min, j) = parse_digits(&bytes[i..]);
        let mut max = 0;
        i += j;
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
            let (mx, j) = parse_digits(&bytes[i..]);
            i += j;
            max = mx;
        }
        spec.max = max;
        spec.min = min;
        (spec, i)
    }
    /// Applies the spec to `s`: first truncates to `max` characters (if set),
    /// then pads with spaces to `min` characters (if shorter), aligned per
    /// `align`. Widths are counted in characters, not bytes, so multi-byte
    /// UTF-8 is handled correctly.
    pub(super) fn apply(&self, s: &str) -> String {
        let mut s = s.to_string();

        if self.max > 0 {
            s = s.chars().take(self.max).collect();
        }
        let len = s.chars().count();
        if self.min > 0 && len < self.min {
            let addition = " ".repeat(self.min - len);
            match self.align {
                Align::Left => s = format!("{s}{addition}"),
                Align::Right => s = format!("{addition}{s}"),
            }
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse: reports how many bytes the spec consumed ---

    #[test]
    fn parse_empty_spec_consumes_nothing() {
        let (spec, consumed) = Spec::parse("");
        assert_eq!(consumed, 0);
        assert_eq!(spec.min, 0);
        assert_eq!(spec.max, 0);
        assert!(matches!(spec.align, Align::Right));
    }

    #[test]
    fn parse_min_only() {
        let (spec, consumed) = Spec::parse("7");
        assert_eq!(consumed, 1);
        assert_eq!(spec.min, 7);
        assert_eq!(spec.max, 0);
    }

    #[test]
    fn parse_left_align_min_and_max() {
        let (spec, consumed) = Spec::parse("-7.4");
        assert_eq!(consumed, 4);
        assert!(matches!(spec.align, Align::Left));
        // min and max are independent (no clamping).
        assert_eq!(spec.min, 7);
        assert_eq!(spec.max, 4);
    }

    #[test]
    fn parse_stops_at_first_non_spec_char() {
        // The "]  more" is a following literal, not part of the spec.
        let (spec, consumed) = Spec::parse("-7.4]  more");
        assert_eq!(consumed, 4); // only "-7.4" consumed
        assert!(matches!(spec.align, Align::Left));
    }

    #[test]
    fn parse_min_not_clamped_when_below_max() {
        let (spec, consumed) = Spec::parse("3.8");
        assert_eq!(consumed, 3);
        assert_eq!(spec.min, 3);
        assert_eq!(spec.max, 8);
    }

    // --- apply: truncate then pad, counted in characters ---

    #[test]
    fn apply_pads_right_aligned_by_default() {
        let (spec, _) = Spec::parse("5");
        assert_eq!(spec.apply("ab"), "   ab");
    }

    #[test]
    fn apply_pads_left_aligned() {
        let (spec, _) = Spec::parse("-5");
        assert_eq!(spec.apply("ab"), "ab   ");
    }

    #[test]
    fn apply_truncates_to_max() {
        let (spec, _) = Spec::parse("0.3");
        assert_eq!(spec.apply("WARNING"), "WAR");
    }

    #[test]
    fn apply_truncates_then_pads_using_truncated_len() {
        // max=3 truncates "WARNING" -> "WAR", min=8 pads to 8 chars.
        let (spec, _) = Spec::parse("8.3");
        assert_eq!(spec.apply("WARNING"), "     WAR");
        assert_eq!(spec.apply("WARNING").chars().count(), 8);
    }

    #[test]
    fn apply_counts_characters_not_bytes() {
        // "café" is 4 chars but 5 bytes; padding to 6 must add 2 spaces.
        let (spec, _) = Spec::parse("-6");
        assert_eq!(spec.apply("café"), "café  ");
        assert_eq!(spec.apply("café").chars().count(), 6);
    }

    #[test]
    fn apply_noop_when_already_long_enough() {
        let (spec, _) = Spec::parse("3");
        assert_eq!(spec.apply("hello"), "hello");
    }
}

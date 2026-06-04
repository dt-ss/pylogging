//! Pattern-based rendering of log records into strings.
//!
//! [`Formatter`] parses a pattern once into segments; the private `spec`
//! module handles the per-field alignment/width mini-language (`-min.max`).

mod spec;

use crate::types::{Record, RecordExt};
use spec::Spec;
use std::collections::HashMap;

/// A post-processing hook: receives the record and the fully rendered line and
/// returns the final string (e.g. to add ANSI colors).
type Transformer = Box<dyn Fn(&Record, &String) -> String + Send + Sync>;

/// A registered field: a name plus a function that transforms the raw record
/// value before it is padded/aligned (e.g. uppercasing a level).
#[derive(Debug, Clone)]
pub struct FieldFormat {
    name: String,
    resolve: fn(&str) -> &str,
}
impl FieldFormat {
    fn new(name: String, resolve: fn(&str) -> &str) -> Self {
        Self { name, resolve }
    }
}

/// Renders a [`Record`] into a string according to a pattern.
///
/// The pattern is parsed once (at construction) into a list of [`Segment`]s, so
/// [`format`](Formatter::format) does no parsing per call. Patterns mix literal
/// text with field placeholders of the form `%(name)` optionally followed by a
/// format spec (alignment/width), e.g. `"[%(level)-8] %(message)"`.
pub struct Formatter {
    fields: HashMap<String, FieldFormat>,
    transformer: Option<Transformer>,
    segments: Vec<Segment>,
    /// The [`chrono`](https://docs.rs/chrono) `strftime` format used to render
    /// the `timestamp` field (e.g. `"%Y-%m-%d %H:%M:%S"`).
    pub time_format: &'static str,
}

/// One parsed piece of a pattern: either fixed literal text or a field
/// placeholder with its parsed [`Spec`].
#[derive(Debug)]
enum Segment {
    Literal(String),
    Field { name: String, spec: Spec },
}

impl Formatter {
    /// Creates a formatter from a pattern string, parsing it into segments now
    /// so that [`format`](Formatter::format) is allocation-light per call.
    pub fn new(pattern: &str) -> Self {
        Self {
            fields: HashMap::new(),
            transformer: None,
            segments: Self::parse(pattern),
            time_format: "%Y-%m-%d %H:%M:%S",
        }
    }

    /// Registers a transform for a field name. When a `%(name)` placeholder is
    /// rendered, the raw record value is passed through `resolve` before
    /// alignment/width is applied. Returns `&mut Self` for chaining.
    pub fn add_field(&mut self, name: &str, resolve: fn(&str) -> &str) -> &mut Self {
        self.fields.insert(
            name.to_string(),
            FieldFormat::new(name.to_string(), resolve),
        );
        self
    }

    /// Renders `record` into a string, substituting each `%(name)` placeholder
    /// with the record's value (or `""` if absent), applying any registered
    /// field transform and the placeholder's spec. A trailing newline is
    /// appended. If a transformer is set, it post-processes the whole line.
    pub fn format(&self, record: &Record) -> String {
        let mut result = String::with_capacity(64);
        for segment in self.segments.iter() {
            match segment {
                Segment::Literal(literal) => result.push_str(literal),
                Segment::Field { name, spec } => {
                    let raw = match self.fields.get(name) {
                        Some(f) => (f.resolve)(record.get_or_default(f.name.as_str())),
                        None => record.get_or_default(name),
                    };
                    result.push_str(&spec.apply(raw));
                }
            }
        }
        result.push('\n');
        if let Some(transformer) = &self.transformer {
            result = transformer(record, &result);
        }
        result
    }

    /// Installs a post-processing hook that receives the record and the fully
    /// rendered line and returns the final string (e.g. to add ANSI colors).
    pub fn set_transformer(
        &mut self,
        transformer: impl Fn(&Record, &String) -> String + Send + Sync + 'static,
    ) {
        self.transformer = Some(Box::new(move |record, result| transformer(record, result)));
    }

    /// Sets the [`chrono`](https://docs.rs/chrono) `strftime` format used to
    /// render the `timestamp` field.
    pub fn set_time_format(&mut self, time_format: &'static str) {
        self.time_format = time_format;
    }

    /// Splits a pattern into [`Segment`]s. Walks a shrinking `&str`: a `%(name)`
    /// run becomes a `Field` (with its spec parsed via [`Spec::parse`], which
    /// reports how much of the trailing text was spec so the rest stays
    /// literal), everything else becomes `Literal`. An unterminated `%(` is
    /// treated as a literal.
    fn parse(pattern: &str) -> Vec<Segment> {
        let mut segments = Vec::new();
        let mut rest = pattern;
        while !rest.is_empty() {
            if let Some(stripped) = rest.strip_prefix("%(") {
                if let Some(close) = stripped.find(")") {
                    let name = &stripped[..close];
                    let tail = &stripped[close + 1..];

                    let (spec, end) = Spec::parse(tail);

                    segments.push(Segment::Field {
                        name: name.to_string(),
                        spec,
                    });

                    rest = &tail[end..];
                } else {
                    segments.push(Segment::Literal(rest.to_string()));
                    break;
                }
            } else {
                let end = rest.find("%(").unwrap_or(rest.len());
                segments.push(Segment::Literal(rest[..end].to_string()));
                rest = &rest[end..];
            }
        }

        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_fields_and_literals() {
        let formatter = Formatter::new("%(timestamp) - %(message)");
        let record = HashMap::from([
            ("timestamp", "2026-01-01 12:00:00"),
            ("message", "Hello, world!"),
        ]);
        // format appends a trailing newline.
        assert_eq!(
            formatter.format(&record),
            "2026-01-01 12:00:00 - Hello, world!\n"
        );
    }

    #[test]
    fn missing_field_renders_empty() {
        let formatter = Formatter::new("[%(missing)]");
        let record: Record = HashMap::new();
        assert_eq!(formatter.format(&record), "[]\n");
    }

    #[test]
    fn preserves_literal_immediately_after_spec() {
        // Regression: the spec ("-7.4") must not swallow the following "]  ".
        let formatter = Formatter::new("[%(name)-7.4]  %(message)");
        let record = HashMap::from([("name", "ab"), ("message", "hi")]);
        // name: truncate to 4 ("ab" unaffected), left-pad to 7 -> "ab     ";
        // then literal "]  ", then message. The "]  " must survive the spec parse.
        assert_eq!(formatter.format(&record), "[ab     ]  hi\n");
    }

    #[test]
    fn right_aligns_and_truncates_via_spec() {
        let formatter = Formatter::new("%(level)8.3");
        let record = HashMap::from([("level", "WARNING")]);
        // truncate to "WAR", right-align to width 8.
        assert_eq!(formatter.format(&record), "     WAR\n");
    }

    #[test]
    fn field_transform_is_applied_before_spec() {
        let mut formatter = Formatter::new("%(level)");
        formatter.add_field("level", |_| "INFO");
        let record = HashMap::from([("level", "info")]);
        assert_eq!(formatter.format(&record), "INFO\n");
    }

    #[test]
    fn unterminated_placeholder_is_literal() {
        let formatter = Formatter::new("oops %(name");
        let record = HashMap::from([("name", "x")]);
        assert_eq!(formatter.format(&record), "oops %(name\n");
    }

    #[test]
    fn transformer_post_processes_line() {
        let mut formatter = Formatter::new("%(message)");
        formatter.set_transformer(|_record, line| format!(">> {}", line));
        let record = HashMap::from([("message", "hi")]);
        assert_eq!(formatter.format(&record), ">> hi\n");
    }
}

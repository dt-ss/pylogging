use crate::formatter::Formatter;
use crate::types::Record;
use std::io::Write;
use std::sync::Mutex;

/// A sink for log records. Implementors render the record (via their
/// [`Formatter`]) and deliver it somewhere (stdout, a file, ...).
///
/// Handlers are shared across threads ([`Send`] + [`Sync`]) since a logger may
/// be logged to concurrently.
pub trait Handler: Send + Sync {
    /// Renders and writes a single record.
    fn handle(&self, record: &Record);
    /// The formatter this handler uses (the logger reads its `time_format`).
    fn formatter(&self) -> &Formatter;
}

/// A [`Handler`] that writes formatted records to any [`Write`] sink, guarded
/// by a [`Mutex`] so a single stream can be shared across threads.
pub struct StreamHandler<W: Write + Send> {
    formatter: Formatter,
    stream: Mutex<W>,
}

impl<W: Write + Send> StreamHandler<W> {
    /// Creates a handler from an explicit formatter and stream.
    pub fn new(formatter: Formatter, stream: W) -> Self {
        Self {
            formatter,
            stream: Mutex::new(stream),
        }
    }
    /// Convenience constructor that builds a [`Formatter`] from `pattern`.
    pub fn with_pattern(stream: W, pattern: &str) -> Self {
        Self::new(Formatter::new(pattern), stream)
    }
}

impl<W: Write + Send> Handler for StreamHandler<W> {
    fn handle(&self, record: &Record) {
        let message = self.formatter.format(record);
        let mut stream = self
            .stream
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(error) = stream.write_all(message.as_bytes()) {
            eprintln!("Error writing to stream: {error}");
        }
    }
    fn formatter(&self) -> &Formatter {
        &self.formatter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// A `Write` sink that appends to a shared buffer we can inspect later.
    #[derive(Clone)]
    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn stream_handler_writes_formatted_record() {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let handler = StreamHandler::with_pattern(SharedBuf(buf.clone()), "%(level): %(message)");

        let record = HashMap::from([("level", "INFO"), ("message", "hello")]);
        handler.handle(&record);

        let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(written, "INFO: hello\n");
    }

    #[test]
    fn formatter_accessor_returns_the_handlers_formatter() {
        let handler = StreamHandler::with_pattern(Vec::<u8>::new(), "%(message)");
        // Render through the accessor to confirm it's the configured formatter.
        let record = HashMap::from([("message", "x")]);
        assert_eq!(handler.formatter().format(&record), "x\n");
    }
}

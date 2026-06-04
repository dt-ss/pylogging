//! Integration tests: exercise the crate the way a real consumer would,
//! through its public API only (`use logging::...`). These complement the
//! in-crate unit tests, which can also reach private items.
//!
//! Note on global state: `Logger::get`/`root` use a process-global registry
//! shared by all tests in this binary. To stay isolated we use unique logger
//! names per test and a shared in-memory sink, rather than asserting on
//! process-wide invariants.

use logging::{Formatter, Level, Logger, Record, StreamHandler};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// A `Write` sink backed by a shared buffer we can inspect after logging.
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
fn logger_formats_and_writes_a_record() {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let logger = Logger::get("it_writes");
    logger.set_level(Level::Debug);
    logger
        .add_handler(StreamHandler::with_pattern(
            SharedBuf(buf.clone()),
            "%(level): %(message)",
        ))
        .unwrap();

    logger.info("hello from integration");

    let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert_eq!(written, "INFO: hello from integration\n");
}

#[test]
fn messages_below_level_are_dropped() {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let logger = Logger::get("it_filters");
    logger.set_level(Level::Warning);
    logger
        .add_handler(StreamHandler::with_pattern(
            SharedBuf(buf.clone()),
            "%(message)",
        ))
        .unwrap();

    logger.debug("dropped");
    logger.info("dropped");
    logger.warning("kept");
    logger.error("kept");

    let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert_eq!(written, "kept\nkept\n");
}

#[test]
fn formatter_is_usable_standalone() {
    let formatter = Formatter::new("[%(level)] %(message)");
    let record: Record = HashMap::from([("level", "INFO"), ("message", "hi")]);
    assert_eq!(formatter.format(&record), "[INFO] hi\n");
}

#[test]
fn level_names_round_trip_via_public_api() {
    for level in [
        Level::Debug,
        Level::Info,
        Level::Warning,
        Level::Error,
        Level::Critical,
    ] {
        assert_eq!(level.to_str().parse::<Level>().unwrap(), level);
    }
}

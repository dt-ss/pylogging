use crate::handler::Handler;
use crate::types::Level;
use chrono::Local;
use std::collections::HashMap;
use std::error::Error;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

type Handlers = RwLock<Vec<Arc<dyn Handler>>>;

/// Process-global registry of named loggers, created lazily on first use.
static LOGGERS: OnceLock<Mutex<HashMap<String, Arc<Logger>>>> = OnceLock::new();
fn loggers() -> &'static Mutex<HashMap<String, Arc<Logger>>> {
    LOGGERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A named logger with a severity threshold and a set of [`Handler`]s.
///
/// Use [`Logger::get`] to obtain a shared logger by name (created once, then
/// reused), or [`Logger::root`] for the ancestor logger. Messages below
/// [`level`](Logger::level) are dropped before reaching any handler.
///
/// The level is stored in an [`AtomicU8`] and handlers behind an [`RwLock`], so
/// a `&Logger` can be used concurrently from multiple threads.
pub struct Logger {
    name: &'static str,
    handlers: Handlers,
    level: AtomicU8,
}

static ROOT_LOGGER_NAME: &str = "root";
static ROOT_LOGGER: OnceLock<Arc<Logger>> = OnceLock::new();

impl Logger {
    /// Builds a logger. The root logger starts empty at [`Level::Info`]; any
    /// other logger inherits a snapshot of root's handlers and level at
    /// creation time.
    pub fn new(name: &'static str) -> Self {
        if name == ROOT_LOGGER_NAME {
            Self {
                name: ROOT_LOGGER_NAME,
                handlers: RwLock::new(Vec::new()),
                level: AtomicU8::new(Level::Info as u8),
            }
        } else {
            Self {
                name,
                handlers: RwLock::new(Self::root().handlers()),
                level: AtomicU8::new(Self::root().level() as u8),
            }
        }
    }

    /// Returns the shared logger registered under `name`, creating and
    /// registering it on first call.
    pub fn get(name: &'static str) -> Arc<Self> {
        let mut loggers = loggers().lock().unwrap();
        loggers
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Logger::new(name)))
            .clone()
    }

    /// Returns the shared root logger, the ancestor whose handlers/level new
    /// loggers inherit.
    pub fn root() -> Arc<Self> {
        ROOT_LOGGER
            .get_or_init(|| Arc::new(Self::new(ROOT_LOGGER_NAME)))
            .clone()
    }

    /// Returns a snapshot clone of this logger's handlers.
    pub fn handlers(&self) -> Vec<Arc<dyn Handler>> {
        self.handlers.read().unwrap().clone()
    }

    /// Sets the minimum severity this logger will emit.
    pub fn set_level(&self, level: Level) {
        self.level.store(level as u8, Ordering::Relaxed);
    }

    /// Returns the logger's current minimum severity.
    pub fn level(&self) -> Level {
        Level::from_u8(self.level.load(Ordering::Relaxed))
    }

    /// Appends a handler. Returns an error only if the internal lock was
    /// poisoned by a panic in another thread.
    pub fn add_handler(&self, handler: impl Handler + 'static) -> Result<(), Box<dyn Error>> {
        self.handlers
            .write()
            .map_err(|_| "Handlers mutex was poisoned")?
            .push(Arc::new(handler));
        Ok(())
    }

    /// Emits `message` at `level` to every handler, unless `level` is below the
    /// logger's threshold. Builds one [`Record`](crate::Record) (message,
    /// level, timestamp, logger name, thread name) and hands it to each handler.
    pub fn log(&self, level: Level, message: &str) {
        if level.to_u8() < self.level.load(Ordering::Relaxed) {
            return;
        }
        let handlers = self
            .handlers
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for handler in handlers.iter() {
            let formatter = handler.formatter();
            handler.handle(&HashMap::from([
                ("message", message),
                ("level", level.to_str()),
                (
                    "timestamp",
                    Local::now()
                        .format(formatter.time_format)
                        .to_string()
                        .as_str(),
                ),
                ("name", self.name),
                ("thread", thread::current().name().unwrap_or("unknown")),
            ]));
        }
    }

    /// Logs `message` at [`Level::Debug`].
    pub fn debug(&self, message: &str) {
        self.log(Level::Debug, message)
    }

    /// Logs `message` at [`Level::Info`].
    pub fn info(&self, message: &str) {
        self.log(Level::Info, message)
    }

    /// Logs `message` at [`Level::Warning`].
    pub fn warning(&self, message: &str) {
        self.log(Level::Warning, message)
    }

    /// Logs `message` at [`Level::Error`].
    pub fn error(&self, message: &str) {
        self.log(Level::Error, message)
    }

    /// Logs `message` at [`Level::Critical`].
    pub fn critical(&self, message: &str) {
        self.log(Level::Critical, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::StreamHandler;
    use std::io::Write;
    use std::sync::Arc;

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

    // --- Registry test harness ---------------------------------------------
    //
    // `cargo test` runs tests in parallel within a single process, so every
    // test shares the one global `LOGGERS` map and `ROOT_LOGGER`. Tests that
    // touch the registry (`get`/`root`) therefore interfere with each other.
    //
    // This is the same problem CPython's logging test suite has; its solution
    // is to run serially and snapshot/restore global state around each test
    // (see `BaseTest.setUp`/`tearDown`). We do the Rust port of that:
    //
    // - `REGISTRY_LOCK` serializes registry tests (replaces Python's implicit
    //   single-threaded assumption).
    // - `RegistryGuard` snapshots the map + root's level/handlers on creation
    //   and restores them on drop, so each test starts (and leaves) clean.
    //
    // RULE: every test that calls `Logger::new`, `Logger::get`, or `Logger::root`
    // MUST acquire a `RegistryGuard` first. `Logger::new` snapshots root's
    // handlers/level, so a test that pollutes root would otherwise leak into any
    // *other* test that constructs a logger concurrently. Holding the guard both
    // serializes those tests and resets root to a clean state on entry.
    //
    // Caveat: `ROOT_LOGGER` is a `OnceLock`, so its *identity* can't be reset
    // once initialized. We restore its *contents* (level + handlers), which is
    // all the behavior under test depends on.

    static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

    struct RegistryGuard {
        // Held for the test's duration to serialize registry access. `_` keeps
        // it alive without an unused-variable warning; never read directly.
        _lock: std::sync::MutexGuard<'static, ()>,
        saved_map: HashMap<String, Arc<Logger>>,
        saved_root_level: Level,
        saved_root_handlers: Vec<Arc<dyn Handler>>,
    }

    impl RegistryGuard {
        fn acquire() -> Self {
            // Recover from a poisoned lock: a panicking test should not wedge
            // the rest of the registry tests.
            let lock = REGISTRY_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            let saved_map = loggers().lock().unwrap().clone();
            let root = Logger::root();
            let guard = Self {
                _lock: lock,
                saved_map,
                saved_root_level: root.level(),
                saved_root_handlers: root.handlers(),
            };
            // Start from a clean slate.
            loggers().lock().unwrap().clear();
            root.set_level(Level::Info);
            *root.handlers.write().unwrap() = Vec::new();
            guard
        }
    }

    impl Drop for RegistryGuard {
        fn drop(&mut self) {
            // Restore the registry and root so the next test is unaffected.
            *loggers().lock().unwrap() = std::mem::take(&mut self.saved_map);
            let root = Logger::root();
            root.set_level(self.saved_root_level);
            *root.handlers.write().unwrap() = std::mem::take(&mut self.saved_root_handlers);
        }
    }

    #[test]
    fn get_returns_the_same_cached_instance() {
        let _guard = RegistryGuard::acquire();

        let first = Logger::get("cached");
        let second = Logger::get("cached");
        // Same registration => same allocation (Arc points at one Logger).
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn child_inherits_root_level_at_creation() {
        let _guard = RegistryGuard::acquire();

        Logger::root().set_level(Level::Error);
        // A non-root logger snapshots root's level when created.
        let child = Logger::new("child_level");
        assert_eq!(child.level(), Level::Error);
    }

    #[test]
    fn child_inherits_root_handlers_at_creation() {
        let _guard = RegistryGuard::acquire();

        let buf = Arc::new(Mutex::new(Vec::new()));
        Logger::root()
            .add_handler(StreamHandler::with_pattern(
                SharedBuf(buf.clone()),
                "%(message)",
            ))
            .unwrap();

        // Child created after root is configured inherits root's handler.
        let child = Logger::new("child_handlers");
        child.set_level(Level::Debug);
        child.info("via inherited handler");

        let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(written, "via inherited handler\n");
    }

    #[test]
    fn level_get_set_round_trips() {
        // Touches root via `Logger::new` (inherits root's level), so it must
        // hold the guard to avoid cross-test contamination.
        let _guard = RegistryGuard::acquire();

        let logger = Logger::new("test_level");
        logger.set_level(Level::Error);
        assert_eq!(logger.level(), Level::Error);
    }

    #[test]
    fn messages_below_level_are_dropped() {
        let _guard = RegistryGuard::acquire();

        let buf = Arc::new(Mutex::new(Vec::new()));
        let logger = Logger::new("test_filter");
        logger.set_level(Level::Warning);
        logger
            .add_handler(StreamHandler::with_pattern(
                SharedBuf(buf.clone()),
                "%(message)",
            ))
            .unwrap();

        logger.info("dropped"); // below Warning
        logger.warning("kept"); // at threshold

        let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(written, "kept\n");
    }

    #[test]
    fn record_includes_level_and_message() {
        let _guard = RegistryGuard::acquire();

        let buf = Arc::new(Mutex::new(Vec::new()));
        let logger = Logger::new("test_record");
        logger.set_level(Level::Debug);
        logger
            .add_handler(StreamHandler::with_pattern(
                SharedBuf(buf.clone()),
                "%(level) %(name) %(message)",
            ))
            .unwrap();

        logger.error("boom");

        let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(written, "ERROR test_record boom\n");
    }
}

#![warn(missing_docs)]
//! A small logging library inspired by Python's `logging` module.
//!
//! The pieces fit together as a pipeline:
//!
//! - [`Logger`] is the user-facing entry point (`info`, `warning`, ...). It
//!   owns a level and a set of handlers, and builds a [`Record`] per call.
//! - [`Handler`] is a sink (e.g. [`StreamHandler`]) that renders a record with
//!   its [`Formatter`] and writes it somewhere.
//! - [`Formatter`] turns a [`Record`] into a string from a pattern like
//!   `"%(timestamp) - %(level) - %(message)"`.
//! - [`Level`] is the severity enum; records below a logger's level are dropped.
//!
//! Loggers are stored in a process-global registry, so [`Logger::get`] returns
//! a shared logger by name, and [`Logger::root`] is the parent of all loggers.
//!
//! ```
//! use logging::{Formatter, Logger, StreamHandler};
//!
//! let logger = Logger::get("example");
//! logger
//!     .add_handler(StreamHandler::with_pattern(std::io::stdout(), "%(level): %(message)"))
//!     .unwrap();
//! logger.info("hello");
//! ```

mod formatter;
mod handler;
mod logger;
mod types;

pub use formatter::Formatter;
pub use handler::{Handler, StreamHandler};
pub use logger::Logger;
pub use types::{Level, ParseLevelError, Record};

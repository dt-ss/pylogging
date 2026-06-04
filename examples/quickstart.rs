//! The simplest possible use of the `logging` crate (published as `pylogging`).
//!
//! Run it with:
//!
//! ```sh
//! cargo run --example basic
//! ```
//!
//! For a fuller demo (custom timestamp format, per-level ANSI colors, and
//! level filtering), see `examples/quickstart.rs`.

use logging::{Logger, StreamHandler};

fn main() {
    // Get a named logger from the global registry. Calling `get` with the same
    // name again anywhere in the program returns the same logger.
    let logger = Logger::get("basic");

    // Attach a handler that writes to stdout using a simple pattern.
    // `%(level)` and `%(message)` are field placeholders.
    logger
        .add_handler(StreamHandler::with_pattern(
            std::io::stdout(),
            "%(level): %(message)",
        ))
        .expect("handler lock should not be poisoned at startup");

    // Emit a few messages. The default level is `Info`, so `debug` is dropped.
    logger.info("hello from a named logger");
    logger.warning("something to keep an eye on");
    logger.error("something went wrong");
}

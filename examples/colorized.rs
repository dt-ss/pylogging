//! Quickstart example for the `logging` crate (published as `pylogging`).
//!
//! Run it with:
//!
//! ```sh
//! cargo run --example quickstart
//! ```
//!
//! It demonstrates the common building blocks:
//!
//! - configuring the **root** logger once, so every logger inherits it;
//! - a **pattern** with field placeholders, padding/alignment, and a custom
//!   timestamp format;
//! - a **transformer** that adds ANSI colors per level;
//! - obtaining a **named** logger and emitting messages at each [`Level`].

use logging::{Formatter, Level, Logger, Record, StreamHandler};

/// Wraps `line` in an ANSI color escape based on the record's level.
///
/// Pulled out of `main` to keep the wiring readable and to show that a
/// transformer is just a plain function `(&Record, &String) -> String`.
fn colorize(record: &Record, line: &String) -> String {
    // `\x1b[<code>m` sets a color; `\x1b[0m` resets it.
    let code = match record.get("level").copied() {
        Some("DEBUG") => "90",         // bright black / gray
        Some("INFO") => "34",          // blue
        Some("WARNING") => "33",       // yellow
        Some("ERROR") => "31",         // red
        Some("CRITICAL") => "38;5;88", // deep red (256-color palette)
        _ => return line.clone(),      // unknown level: leave uncolored
    };
    format!("\x1b[{code}m{line}\x1b[0m")
}

fn main() {
    // 1. Build a formatter from a pattern.
    //
    //    `%(name)` is a field placeholder. A trailing spec like `-20` controls
    //    width/alignment: a leading `-` left-aligns, the number is the minimum
    //    width, and an optional `.N` truncates to N characters. So:
    //      %(level)-8   -> left-aligned, padded to 8 ("INFO    ")
    //      %(name).12   -> truncated to at most 12 chars
    let mut formatter = Formatter::new("%(timestamp) [%(level)-8] %(name)-12 | %(message)");

    // 2. Customize the timestamp rendering (chrono's strftime syntax).
    formatter.set_time_format("%Y-%m-%d %H:%M:%S");

    // 3. Post-process every rendered line (here: add color).
    formatter.set_transformer(colorize);

    // 4. Configure the ROOT logger once. Every logger created afterwards
    //    inherits a snapshot of root's handlers and level, so you typically
    //    set things up here and then just `Logger::get(...)` everywhere else.
    let root = Logger::root();
    root.add_handler(StreamHandler::new(formatter, std::io::stdout()))
        .expect("root handler lock should not be poisoned at startup");
    root.set_level(Level::Debug); // emit everything Debug and above

    // 5. Grab a named logger and emit one message per level.
    let logger = Logger::get("quickstart");
    logger.debug("a debug message (gray)");
    logger.info("an info message (blue)");
    logger.warning("a warning message (yellow)");
    logger.error("an error message (red)");
    logger.critical("a critical message (deep red)");

    // 6. Levels below a logger's threshold are dropped cheaply (before
    //    formatting). Raise the bar and show that Debug/Info now vanish.
    logger.set_level(Level::Warning);
    logger.debug("you will NOT see this (below threshold)");
    logger.info("you will NOT see this either");
    logger.warning("but warnings still get through");
}

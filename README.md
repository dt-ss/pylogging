# pylogging

[![Crates.io](https://img.shields.io/crates/v/pylogging.svg)](https://crates.io/crates/pylogging)
[![Documentation](https://docs.rs/pylogging/badge.svg)](https://docs.rs/pylogging)
[![License](https://img.shields.io/crates/l/pylogging.svg)](#license)

**Ergonomic logging for Rust, inspired by Python's `logging` module.**

If you know Python's `logging`, you already know how to use this. Same API
patterns, same feel — no need to learn a new mental model for something as
fundamental as logging.

Published as **`pylogging`**, imported as **`logging`**, so your code reads
like Python but runs like Rust.

## Why pylogging?

| You want…                   | With `log` + `env_logger`                                           | With `pylogging` ✅                                       |
|:----------------------------|:---------------------------------------------------------------------|:-----------------------------------------------------------|
| A **named logger** per module | `log::info!(target: "my::module", "msg")` — target is an annotation | `let log = Logger::get("my::module"); log.info("msg")` — a first-class object |
| **Custom formatting**       | Compile-time via `env_logger::Builder::format()` or a whole new crate  | `Formatter::new("%(timestamp) [%(level)-8] %(name) \| %(message)")` — runtime pattern string |
| **Runtime level control**   | `RUST_LOG=warn` env var or `log::set_max_level(...)`                  | `logger.set_level(Level::Warning)` — any logger, any time |
| **File logging**            | Bring in `log4rs` (heavy) or roll your own                            | `StreamHandler::new(formatter, File::create("app.log")?)` — one-liner |
| **ANSI colors / transforms** | Doesn't exist out of the box                                         | `Transformer` trait — add colors, redact, anything post-format |
| **Pythonic inheritance**    | Not built-in                                                         | Child loggers inherit from root — just like Python          |

In short: if you're a Python dev learning Rust (and there are a lot of you),
`pylogging` makes logging feel like home — without sacrificing performance.

## Installation

```toml
[dependencies]
pylogging = "0.1"
```

Import as `logging`:

```rust
use logging::{Formatter, Level, Logger, StreamHandler};
```

## Quick start

```rust
use logging::{Logger, StreamHandler};

let logger = Logger::get("example");
logger
    .add_handler(StreamHandler::with_pattern(std::io::stdout(), "%(level): %(message)"))
    .unwrap();
logger.info("hello");
// prints: INFO: hello
```

### Python → Rust side by side

```python
# Python
import logging
log = logging.getLogger("my_app")
log.setLevel(logging.DEBUG)
handler = logging.StreamHandler()
handler.setFormatter(logging.Formatter("%(asctime)s [%(levelname)-8s] %(message)s"))
log.addHandler(handler)
log.info("server started on port %d", 8080)
```

```rust
// Rust — same idea, zero mental overhead
use logging::{Formatter, Logger, StreamHandler};

let log = Logger::get("my_app");
log.set_level(Level::Debug);
let mut fmt = Formatter::new("%(timestamp) [%(level)-8] %(message)");
fmt.set_time_format("%Y-%m-%d %H:%M:%S");
log.add_handler(StreamHandler::new(fmt, std::io::stdout())).unwrap();
log.info(format!("server started on port {}", 8080));
```

## Features

- **Named loggers** in a process-global registry (`Logger::get("my::module")`).
- **Inheritance** from a configurable **root** logger (handlers + level propagate
  to child loggers automatically).
- **Level filtering** (`Debug`, `Info`, `Warning`, `Error`, `Critical`) — messages
  below the threshold are dropped allocation-free.
- **Pattern-based formatting** — `"%(timestamp) [%(level)-8] %(name)-12 | %(message)"`
  with width, alignment, truncation specs.
- **Pluggable handlers** via the `Handler` trait; `StreamHandler` writes to any
  `std::io::Write` sink (stdout, files, `Vec<u8>`, ...).
- **Transformers** for post-processing — sprinkle ANSI colors per level, redact
  secrets, add timestamps, anything you can write as a function.
- **No macros required** — call methods on real objects. (`info!("...")` is cute,
  but sometimes you just want `log.info("...")`.)

## Advanced: configure root logger once

Every logger inherits from the root. Set it up once and all your modules get it:

```rust
use logging::{Formatter, Level, Logger, StreamHandler};

let mut formatter = Formatter::new("%(timestamp) [%(level)-8] %(name)-12 | %(message)");
formatter.set_time_format("%Y-%m-%d %H:%M:%S");

let root = Logger::root();
root.add_handler(StreamHandler::new(formatter, std::io::stdout())).unwrap();
root.set_level(Level::Debug);

// Any logger created after this inherits the root's handler and level
let logger = Logger::get("my::module");
logger.info("inherits root's handler and level");
// 2026-06-01 14:30:00 [INFO    ] my::module    | inherits root's handler and level
```

## Colorized output

`pylogging` lets you transform every rendered line before it's written via the
`Transformer` trait. The most popular use? Per-level ANSI colors.

Add a transformer function to your formatter — it gets the record and the
rendered line, returns the final string:

```rust
use logging::{Formatter, Level, Logger, Record, StreamHandler};

fn colorize(record: &Record, line: &String) -> String {
    let code = match record.get("level").copied() {
        Some("DEBUG") => "90",           // gray
        Some("INFO") => "34",            // blue
        Some("WARNING") => "33",         // yellow
        Some("ERROR") => "31",           // red
        Some("CRITICAL") => "38;5;88",   // deep red
        _ => return line.clone(),
    };
    format!("\x1b[{code}m{line}\x1b[0m")
}

let mut formatter = Formatter::new("%(timestamp) [%(level)-8] %(name)-12 | %(message)");
formatter.set_time_format("%Y-%m-%d %H:%M:%S");
formatter.set_transformer(colorize);

let root = Logger::root();
root.add_handler(StreamHandler::new(formatter, std::io::stdout())).unwrap();
root.set_level(Level::Debug);

let log = Logger::get("my_app");
log.info("server started");    // blue
log.warning("disk at 85%");     // yellow
log.error("connection refused"); // red
```

Run the full demo:

```sh
cargo run --example colorized
```

## Pattern syntax

A pattern is literal text with `%(field)` placeholders. Optional width/alignment
follows the `printf` convention:

| Spec             | Meaning                                                  |
|:-----------------|:---------------------------------------------------------|
| `%(name)`        | The value of `name`, or `""` if absent.                  |
| `%(name)8`       | Right-align to a minimum width of 8.                     |
| `%(name)-8`      | Left-align to a minimum width of 8.                      |
| `%(name).4`      | Truncate to at most 4 characters.                        |
| `%(name)-8.4`    | Truncate to 4, then left-pad to width 8.                 |

Common fields per record: `message`, `level`, `name`, `timestamp`, `thread`.

See [`examples/quickstart.rs`](examples/quickstart.rs) for a minimal example.
Run [`examples/colorized.rs`](examples/colorized.rs) for the full demo with
ANSI colors, custom timestamps, and level filtering:

```sh
cargo run --example colorized
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

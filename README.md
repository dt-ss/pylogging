# pylogging

[![Crates.io](https://img.shields.io/crates/v/pylogging.svg)](https://crates.io/crates/pylogging)
[![Documentation](https://docs.rs/pylogging/badge.svg)](https://docs.rs/pylogging)
[![License](https://img.shields.io/crates/l/pylogging.svg)](#license)

A small, ergonomic logging library for Rust, inspired by Python's
[`logging`](https://docs.python.org/3/library/logging.html) module.

It is published as the **`pylogging`** crate but imported as **`logging`**, so
the API reads the way it does in Python.

## Features

- **Named loggers** in a process-global registry (`Logger::get("my::module")`).
- **Inheritance** from a configurable **root** logger (handlers + level).
- **Level filtering** (`Debug`, `Info`, `Warning`, `Error`, `Critical`) with
  cheap, allocation-free rejection of messages below the threshold.
- **Pattern-based formatting** with field placeholders, width/alignment, and
  truncation, e.g. `"%(timestamp) [%(level)-8] %(name)-12 | %(message)"`.
- **Pluggable handlers** via the `Handler` trait; a `StreamHandler` writes to
  any `std::io::Write` sink (stdout, files, in-memory buffers, ...).
- **Transformers** for post-processing a rendered line (e.g. ANSI colors).

## Installation

```toml
[dependencies]
pylogging = "0.1"
```

The crate is imported as `logging`:

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

Configure the **root** logger once and every logger inherits it:

```rust
use logging::{Formatter, Level, Logger, StreamHandler};

let mut formatter = Formatter::new("%(timestamp) [%(level)-8] %(name)-12 | %(message)");
formatter.set_time_format("%Y-%m-%d %H:%M:%S");

let root = Logger::root();
root.add_handler(StreamHandler::new(formatter, std::io::stdout())).unwrap();
root.set_level(Level::Debug);

let logger = Logger::get("my::module");
logger.info("inherits root's handler and level");
```

See [`examples/quickstart.rs`](examples/quickstart.rs) for a fuller demo
(including per-level ANSI colors). Run it with:

```sh
cargo run --example quickstart
```

## Pattern syntax

A pattern is literal text interspersed with `%(field)` placeholders. A
placeholder may be followed by a spec controlling width and alignment:

| Spec        | Meaning                                              |
|:------------|:-----------------------------------------------------|
| `%(name)`   | The value of `name`, or `""` if absent.              |
| `%(name)8`  | Right-align to a minimum width of 8.                 |
| `%(name)-8` | Left-align to a minimum width of 8.                  |
| `%(name).4` | Truncate to at most 4 characters.                    |
| `%(name)-8.4` | Truncate to 4, then left-pad to width 8.           |

Common fields populated per record: `message`, `level`, `name`, `timestamp`,
`thread`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

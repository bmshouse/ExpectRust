# ExpectRust

A cross-platform Rust library for automating interactive programs, inspired by the Unix `expect` utility.

## Project Status: MVP (v0.1.0) ✅

ExpectRust is now a production-ready library with async support, intelligent buffering, and comprehensive pattern matching!

## Features

- **Cross-platform**: Works on Windows (ConPTY), Linux, and macOS
- **Async/await**: Built on tokio for efficient async I/O
- **Pattern matching**: Supports exact strings, regex, and glob patterns
- **Intelligent buffering**: Handles partial matches across buffer boundaries
- **Timeout support**: Built-in timeout handling for all operations
- **ANSI stripping**: Optional removal of ANSI escape sequences
- **Script parsing**: Execute traditional Expect/Tcl scripts (optional feature)
- **Script translation**: Translate Expect scripts to Rust code with `expect2rust` CLI tool
- **Type-safe**: Leverages Rust's type system for safe automation

## Installation

Add ExpectRust to your `Cargo.toml`:

```toml
[dependencies]
expectrust = "0.1"
tokio = { version = "1", features = ["full"] }

# Optional: Enable script parsing
expectrust = { version = "0.1", features = ["script"] }

# Optional: Enable script translator
expectrust = { version = "0.1", features = ["translator"] }
```

## Quick Start

```rust
use expectrust::{Session, Pattern};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Spawn a process
    let mut session = Session::builder()
        .timeout(Duration::from_secs(30))
        .spawn("python -i")?;

    // Wait for the Python prompt
    session.expect(Pattern::exact(">>> ")).await?;

    // Send a command
    session.send_line("print('Hello, World!')").await?;

    // Wait for output
    let result = session.expect(Pattern::exact(">>> ")).await?;
    println!("Output: {}", result.before);

    Ok(())
}
```

## Pattern Matching

ExpectRust supports multiple pattern types:

### Exact String

Fast Boyer-Moore-Horspool algorithm for exact string matching:

```rust
session.expect(Pattern::exact("password: ")).await?;
```

### Regular Expressions

Full regex support with capture groups:

```rust
let result = session.expect(Pattern::regex(r"\d+")?).await?;
println!("Matched number: {}", result.matched);
println!("Captures: {:?}", result.captures);
```

### Glob Patterns

Shell-style wildcard patterns:

```rust
session.expect(Pattern::glob("*.txt")).await?;
```

### Special Patterns

- **EOF**: Match end of file
- **Timeout**: Match timeout condition
- **FullBuffer**: Match when buffer is full
- **Null**: Match null byte

```rust
let patterns = [
    Pattern::exact("success"),
    Pattern::Eof,
    Pattern::Timeout,
];
let result = session.expect_any(&patterns).await?;
```

## Multiple Patterns

Wait for any of multiple patterns (first match wins):

```rust
let patterns = [
    Pattern::exact("$ "),
    Pattern::exact("# "),
    Pattern::regex(r"Password:")?,
];

let result = session.expect_any(&patterns).await?;
match result.pattern_index {
    0 | 1 => println!("Got shell prompt"),
    2 => {
        session.send_line("secret123").await?;
    }
    _ => unreachable!(),
}
```

## Configuration

Use `SessionBuilder` to configure sessions:

```rust
let session = Session::builder()
    .timeout(Duration::from_secs(60))      // Set default timeout
    .max_buffer_size(16384)                // Set buffer size
    .strip_ansi(true)                      // Strip ANSI sequences
    .pty_size(24, 80)                      // Set terminal size
    .spawn("ssh user@example.com")?;
```

## Script Translation (Recommended)

The `expect2rust` CLI tool translates classic Expect scripts into idiomatic Rust code:

```bash
# Install the translator
cargo install --path . --features translator

# Translate an expect script
expect2rust script.exp

# The generated script.rs file contains compilable Rust code
```

### Example Translation

**Input** (`test.exp`):
```tcl
spawn python -i
expect ">>>"
send "print('Hello')\n"
expect ">>>"
```

**Output** (`test.rs`):
```rust
use expectrust::{Session, Pattern};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::spawn("python -i")?;
    session.expect(Pattern::exact(">>>")).await?;
    session.send(b"print('Hello')\n").await?;
    session.expect(Pattern::exact(">>>")).await?;
    Ok(())
}
```

**Why translation over interpretation?**
- ✅ Full Rust type safety and compile-time checks
- ✅ Better performance (compiled, not interpreted)
- ✅ Easy to customize generated code
- ✅ Full IDE support and debugging
- ✅ Clear warnings about unsupported features

See [TRANSLATOR_README.md](TRANSLATOR_README.md) for full documentation.

## Script Parsing (Optional Feature)

ExpectRust can also parse and execute traditional Expect scripts at runtime with Tcl-like syntax:

```rust
use expectrust::script::Script;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script_text = r#"
        spawn python -i
        expect ">>> "
        send "print('Hello')\n"
        expect ">>> "
        send "exit()\n"
    "#;

    let script = Script::from_str(script_text)?;
    script.execute().await?;
    Ok(())
}
```

### Supported Script Features

- **Commands**: `spawn`, `expect`, `send`, `close`, `wait`, `exit`
- **Variables**: `set var value`, `$var` substitution
- **Control flow**: `if/else`, `while`, `for`
- **Procedures**: `proc name {args} {body}`
- **Patterns**: exact strings, `-re` (regex), `-gl` (glob), `timeout`, `eof`
- **Pattern blocks**: Multiple patterns with associated actions

See `examples/script_*.rs` for more examples.

## API Overview

### Session

- `Session::builder()` - Create a new session builder
- `Session::spawn(command)` - Spawn a command (convenience method)
- `session.expect(pattern)` - Wait for a pattern
- `session.expect_any(patterns)` - Wait for any of multiple patterns
- `session.send(data)` - Send data to process
- `session.send_line(line)` - Send a line (appends newline)
- `session.is_alive()` - Check if process is running
- `session.wait()` - Wait for process to exit

### Script (with `script` feature)

- `Script::from_str(text)` - Parse a script from a string
- `Script::from_file(path)` - Parse a script from a file
- `Script::builder()` - Create a script builder with configuration
- `script.execute()` - Execute the script asynchronously

### Pattern Types

- `Pattern::exact(s)` - Exact string match
- `Pattern::regex(pattern)` - Regular expression match
- `Pattern::glob(pattern)` - Glob pattern match
- `Pattern::Eof` - End of file
- `Pattern::Timeout` - Timeout occurred
- `Pattern::FullBuffer` - Buffer full
- `Pattern::Null` - Null byte

### MatchResult

Contains information about a successful match:

- `pattern_index` - Which pattern matched (for `expect_any`)
- `matched` - The matched text
- `start` / `end` - Match position in buffer
- `before` - Text before the match
- `captures` - Regex capture groups

## Examples

Run the core examples with:
```bash
cargo run --example basic_command
cargo run --example pattern_matching
cargo run --example interactive_shell
cargo run --example timeout_handling
```

Run the SSH automation examples:
```bash
cargo run --example ssh_automation        # Complete SSH example with error handling
cargo run --example ssh_simple            # Minimal SSH example
cargo run --features script --example ssh_automation_script  # Script-based SSH
```

Run the script examples with:
```bash
cargo run --features script --example script_example
cargo run --features script --example script_variables
cargo run --features script --example script_patterns
cargo run --features script --example script_python
```

## Comparison with Other Tools

| Feature | ExpectRust | Unix expect | pexpect (Python) |
|---------|-----------|-------------|------------------|
| Cross-platform | ✅ (Windows/Linux/macOS) | ❌ (Unix only) | ✅ |
| Async/await | ✅ | ❌ | ❌ |
| Type safety | ✅ | ❌ | ❌ |
| Memory safety | ✅ | ❌ | ✅ |
| Regex support | ✅ | ✅ | ✅ |
| Timeout handling | ✅ | ✅ | ✅ |
| Partial match tracking | ✅ | ✅ | ✅ |
| Script parsing | ✅ (optional) | ✅ | N/A |
| Package management | ✅ Cargo | ❌ Manual | ✅ pip |

## Architecture

ExpectRust uses a clean, modular architecture:

- **Session**: Main API for process automation
- **Pattern**: Flexible pattern matching (exact, regex, glob)
- **BufferManager**: Intelligent buffering with 2/3 discard strategy
- **Matcher**: Boyer-Moore-Horspool and regex matchers
- **Async I/O**: Cross-platform async PTY operations via tokio

## Implementation Highlights

✅ **Intelligent Buffering**: Uses a 2/3 discard strategy to efficiently manage memory while preserving unmatched data
✅ **Boyer-Moore-Horspool**: Fast exact string matching
✅ **Partial Match Tracking**: Handles patterns split across buffer boundaries
✅ **Async I/O**: Non-blocking operations with proper timeout handling
✅ **Cross-Platform PTY**: Seamless Windows/Linux/macOS support via `portable-pty`

## Roadmap

- [x] Cross-platform PTY support
- [x] Async/await API
- [x] Pattern matching (exact, regex, glob)
- [x] Timeout handling
- [x] ANSI escape sequence stripping
- [x] Intelligent buffering
- [x] Multiple pattern matching
- [x] Expect script parser (Tcl-like syntax)
- [ ] Advanced logging and debugging
- [ ] Performance optimizations
- [ ] CI/CD pipeline

## Project Structure

```
ExpectRust/
├── src/
│   ├── lib.rs           # Public API
│   ├── main.rs          # Original POC (kept for reference)
│   ├── session/         # Session management
│   ├── pattern/         # Pattern matching
│   ├── buffer/          # Buffer management
│   ├── result/          # Result and error types
│   └── script/          # Script parser (optional feature)
│       ├── grammar.pest # Pest grammar for Expect/Tcl
│       ├── parser.rs    # Parser implementation
│       ├── ast.rs       # Abstract syntax tree
│       ├── interpreter.rs  # AST interpreter
│       ├── runtime.rs   # Runtime environment
│       └── ...
├── examples/            # Usage examples
├── tests/               # Integration tests
├── reference/           # Original expect source
├── Cargo.toml           # Dependencies
└── README.md            # This file
```

## Building and Testing

```bash
# Build the library
cargo build

# Run examples
cargo run --example basic_command
cargo run --example pattern_matching
cargo run --example interactive_shell

# Run tests
cargo test

# Run tests with script feature
cargo test --features script

# Build documentation
cargo doc --open
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. Areas where help is particularly appreciated:

- Additional examples and documentation
- Integration tests
- Performance optimizations
- Bug fixes and edge case handling
- Platform-specific testing (Windows/Linux/macOS)

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Inspired by the original Unix `expect` utility by Don Libes
- Built on the excellent [`portable-pty`](https://github.com/wez/wezterm/tree/main/pty) crate by Wez Furlong
- Uses [`tokio`](https://tokio.rs) for async runtime
- Pattern matching powered by the [`regex`](https://docs.rs/regex/) crate

## See Also

- [expect](https://core.tcl-lang.org/expect/index) - Original Unix expect
- [pexpect](https://pexpect.readthedocs.io/) - Python expect library
- [portable-pty](https://docs.rs/portable-pty/) - Cross-platform PTY crate
- [tokio](https://tokio.rs/) - Async runtime for Rust

---

**Status**: MVP Complete! v0.1.0 is ready for production use. 🎉

ExpectRust provides a fully async, cross-platform, type-safe way to automate interactive programs in Rust!

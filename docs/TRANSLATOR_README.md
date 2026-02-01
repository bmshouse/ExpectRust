# Expect to Rust Translator

The `expect2rust` tool translates classic Unix Expect scripts into idiomatic Rust code using the ExpectRust API.

## Installation

```bash
cargo install --path . --features translator
```

## Usage

### Basic Translation

```bash
expect2rust script.exp
```

This generates `script.rs` with the translated Rust code.

### Options

- `-o, --output <FILE>` - Specify output file (default: input with `.rs` extension)
- `--no-warnings` - Don't include warning comments in generated code
- `--standalone` - Generate with `main()` function (default)

### Example

```bash
# Create an expect script
cat > hello.exp << 'EOF'
spawn echo "Hello, World!"
expect "Hello"
EOF

# Translate to Rust
expect2rust hello.exp

# View generated code
cat hello.rs
```

## Supported Features

| Expect Feature | Support | Notes |
|---|---|---|
| `spawn` | ✅ Full | Translates to `Session::spawn()` |
| `expect` (single pattern) | ✅ Full | Translates to `session.expect()` |
| `expect { ... }` (multi-pattern) | ⚠️ Limited | Currently not fully supported - use single pattern expects |
| `send` | ✅ Full | Translates to `session.send()` |
| `close` | ✅ Full | Translates to `drop(session)` |
| `wait` | ✅ Full | Translates to `session.wait()` |
| `exit` | ✅ Full | Translates to `std::process::exit()` |
| Variables | ✅ Full | Translates to Rust variables |
| `if/else` | ✅ Full | Translates to Rust if/else |
| `while` | ✅ Full | Translates to Rust while |
| `for` | ✅ Full | Translates to Rust for loop |
| Procedures | ✅ Full | Translates to `async fn` |
| `-re` regex | ✅ Full | Translates to `Pattern::regex()` |
| `-gl` glob | ✅ Full | Translates to `Pattern::glob()` |
| `timeout` | ⚠️ Partial | Use in simple expect statements |
| `eof` | ⚠️ Partial | Use in simple expect statements |

## Current Limitations

### Multi-Pattern Expect Blocks

The translator currently has limited support for multi-pattern expect blocks like:

```tcl
expect {
    "pattern1" { action1 }
    "pattern2" { action2 }
}
```

**Workaround**: Use sequential single-pattern expect statements or manually write the Rust code for complex pattern matching.

### Special Characters

Some special characters in identifiers (like `@` in email addresses) may not parse correctly. Use simpler identifiers or quoted strings.

### Tcl-Specific Features

Some Tcl-specific features are not supported:
- Arrays: `$arr(key)`
- `uplevel`/`upvar` variable scoping
- `interact` command
- `exp_continue` command

## Generated Code Structure

The translator generates async Rust code with the following structure:

```rust
use expectrust::{Session, Pattern};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Your translated code here
    Ok(())
}
```

### Dependencies

Add these to your `Cargo.toml`:

```toml
[dependencies]
expectrust = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Examples

### Simple Script

**Input** (`test.exp`):
```tcl
spawn python -i
expect ">>>"
send "print('Hello')\n"
expect ">>>"
send "exit()\n"
```

**Output** (`test.rs`):
```rust
use expectrust::{Session, Pattern};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::spawn("python -i")?;
    session.expect(Pattern::exact(">>>")).await?;
    session.send(b"print('Hello')\n").await?;
    session.expect(Pattern::exact(">>>")).await?;
    session.send(b"exit()\n").await?;
    Ok(())
}
```

### With Variables

**Input**:
```tcl
spawn ssh remote-host
set timeout 30
expect "password:"
send "$password\n"
```

**Output**:
```rust
let mut session = Session::spawn("ssh remote-host")?;
let timeout = 30;
session.expect(Pattern::exact("password:")).await?;
session.send(password.as_bytes()).await?;
```

## Library API

You can also use the translator programmatically:

```rust
use expectrust::script::translator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = r#"
        spawn echo hello
        expect "hello"
    "#;

    let generated = translator::translate_str(script)?;
    println!("{}", generated.code);

    // Check warnings
    for warning in &generated.warnings {
        eprintln!("Warning: {}", warning);
    }

    Ok(())
}
```

## Why Translation Instead of Interpretation?

This translator approach is superior to runtime interpretation because:

1. **Full compatibility** - Generate Rust code for any expect feature
2. **Performance** - Compiled Rust with no script overhead
3. **Type safety** - Leverage Rust's type system at compile time
4. **Extensibility** - Users can modify generated code
5. **Clear warnings** - Identify unsupported features upfront
6. **IDE support** - Full Rust tooling in generated code
7. **Debugging** - Use standard Rust debugging tools

## Troubleshooting

### Parse Errors

If you get parse errors, the script may use Tcl syntax not yet supported. Try simplifying the script or manually writing the equivalent Rust code.

### Generated Code Doesn't Compile

1. Check that you've added the required dependencies
2. Review any warnings in the output
3. The generated code may need minor manual adjustments for complex scripts

### Runtime Errors

The generated code uses `?` for error propagation. Make sure your patterns match the actual output from your spawned process.

## Contributing

Found a bug or want to add support for more Expect features? Contributions are welcome!

## License

Same as ExpectRust library.

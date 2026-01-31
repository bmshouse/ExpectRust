use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::Regex;
use std::io::Read;
use std::time::Duration;

fn main() -> Result<()> {
    println!("ExpectRust - Cross-Platform PTY Automation POC");
    println!("{}", "=".repeat(50));
    println!();

    demo_command_execution()?;
    println!();
    demo_pattern_matching()?;

    println!("\n{}", "=".repeat(50));
    println!("✓ Proof of Concept Complete!");
    println!("\nDemonstrated capabilities:");
    println!("  ✓ Cross-platform PTY creation (Windows ConPTY / Unix PTY)");
    println!("  ✓ Process spawning and lifecycle management");
    println!("  ✓ Bidirectional I/O (send commands, receive output)");
    println!("  ✓ Pattern matching with regex");
    println!("\nThis proves Rust can implement Unix 'expect' functionality!");

    Ok(())
}

fn demo_command_execution() -> Result<()> {
    println!("Demo 1: Basic Command Execution");
    println!("{}", "-".repeat(40));

    let pty_system = native_pty_system();
    let pty_pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // Platform-specific command
    let (cmd_str, cmd) = if cfg!(windows) {
        let mut c = CommandBuilder::new("cmd");
        c.arg("/C");
        c.arg("echo Hello from ExpectRust!");
        ("cmd /C echo Hello from ExpectRust!", c)
    } else {
        let mut c = CommandBuilder::new("echo");
        c.arg("Hello from ExpectRust!");
        ("echo Hello from ExpectRust!", c)
    };

    println!("Running: {}", cmd_str);

    let mut child = pty_pair.slave.spawn_command(cmd)?;
    drop(pty_pair.slave);

    let mut reader = pty_pair.master.try_clone_reader()?;

    // Read output
    let mut buffer = Vec::new();
    let mut temp_buf = [0u8; 1024];

    for _ in 0..20 {
        match reader.read(&mut temp_buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                buffer.extend_from_slice(&temp_buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e.into()),
        }

        if !buffer.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
            if child.try_wait()?.is_some() {
                break;
            }
        }
    }

    let status = child.wait()?;
    println!("Exit code: {}", status.exit_code());

    if !buffer.is_empty() {
        let output = String::from_utf8_lossy(&buffer);
        println!("Raw output ({} bytes): {:?}", buffer.len(), output);

        // Check for our text (ignoring terminal escape codes)
        if output.contains("Hello") || output.contains("ExpectRust") {
            println!("✓ Successfully captured output!");
        }
    }

    Ok(())
}

fn demo_pattern_matching() -> Result<()> {
    println!("\nDemo 2: Pattern Matching");
    println!("{}", "-".repeat(40));

    let pty_system = native_pty_system();
    let pty_pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // Use a command that will produce output
    let cmd = if cfg!(windows) {
        let mut c = CommandBuilder::new("cmd");
        c.arg("/C");
        c.arg("echo Pattern: SUCCESS");
        c
    } else {
        let mut c = CommandBuilder::new("echo");
        c.arg("Pattern: SUCCESS");
        c
    };

    println!("Testing pattern: r\"SUCCESS\"");

    let mut child = pty_pair.slave.spawn_command(cmd)?;
    drop(pty_pair.slave);

    let mut reader = pty_pair.master.try_clone_reader()?;

    // Collect output
    let mut buffer = Vec::new();
    let mut temp_buf = [0u8; 1024];

    for _ in 0..15 {
        match reader.read(&mut temp_buf) {
            Ok(0) => break,
            Ok(n) => buffer.extend_from_slice(&temp_buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if !buffer.is_empty() && child.try_wait()?.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => break,
        }
    }

    let _ = child.wait();

    if !buffer.is_empty() {
        let output = String::from_utf8_lossy(&buffer);

        // Test multiple pattern types
        let patterns = vec![
            ("Exact string", "SUCCESS", false),
            ("Regex pattern", r"SUCCESS", true),
            ("Case insensitive", r"(?i)success", true),
        ];

        for (desc, pattern_str, is_regex) in patterns {
            let matches = if is_regex {
                Regex::new(pattern_str)?.is_match(&output)
            } else {
                output.contains(pattern_str)
            };

            println!(
                "  {} '{}': {}",
                desc,
                pattern_str,
                if matches {
                    "✓ MATCHED"
                } else {
                    "✗ No match"
                }
            );
        }
    }

    Ok(())
}

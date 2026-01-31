//! Example demonstrating control character and escape sequence support
//!
//! This example shows how to send various control characters and ANSI escape
//! sequences to interactive programs, similar to the original Expect utility.

use anyhow::Result;
use expectrust::{Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Control Character Examples");
    println!("{}", "=".repeat(50));

    // Example 1: Sending carriage return vs newline
    println!("\n1. Carriage Return vs Newline");
    let mut session =
        Session::builder()
            .timeout(Duration::from_secs(5))
            .spawn(if cfg!(windows) {
                "cmd /C echo Testing CR and LF"
            } else {
                "cat"
            })?;

    if !cfg!(windows) {
        // Send text with carriage return (CR)
        session.send(b"Line with CR\r").await?;
        println!("   ✓ Sent: 'Line with CR\\r'");

        // Send text with newline (LF)
        session.send(b"Line with LF\n").await?;
        println!("   ✓ Sent: 'Line with LF\\n'");

        // End the cat process with Ctrl-D
        session.send(&[0x04]).await?;
        println!("   ✓ Sent: Ctrl-D (EOF)");
    }

    // Example 2: Sending control characters by hex value
    println!("\n2. Control Characters by Hex Value");
    println!("   Common control characters:");
    println!("   - 0x03: Ctrl-C (interrupt)");
    println!("   - 0x04: Ctrl-D (EOF)");
    println!("   - 0x1a: Ctrl-Z (suspend)");
    println!("   - 0x1b: ESC (escape)");

    // Example 3: ANSI escape sequences
    println!("\n3. ANSI Escape Sequences");
    if !cfg!(windows) {
        let mut session3 = Session::builder()
            .timeout(Duration::from_secs(5))
            .spawn("bash")?;

        session3.expect(Pattern::regex(r"[$#]")?).await?;

        // Clear screen sequence
        println!("   Sending: ESC[2J (clear screen)");
        session3.send(b"\x1b[2J").await?;

        // Cursor position
        println!("   Sending: ESC[H (cursor home)");
        session3.send(b"\x1b[H").await?;

        // Arrow key sequences
        println!("   Sending: ESC[A (up arrow)");
        session3.send(b"\x1b[A").await?;

        println!("   Sending: ESC[B (down arrow)");
        session3.send(b"\x1b[B").await?;

        // Exit bash
        session3.send(b"exit\n").await?;
    } else {
        println!("   (ANSI examples skipped on Windows)");
    }

    // Example 4: Multiple control characters
    println!("\n4. Multiple Control Characters");
    println!("   You can send multiple bytes at once:");

    let escape_sequence = vec![0x1b, 0x5b, 0x41]; // ESC [ A (up arrow)
    println!("   - [{:?}] = ESC [ A (up arrow)", escape_sequence);

    let ctrl_sequence = vec![0x03, 0x04]; // Ctrl-C, Ctrl-D
    println!("   - [{:?}] = Ctrl-C, Ctrl-D", ctrl_sequence);

    // Example 5: Byte string literals
    println!("\n5. Rust Byte String Literals");
    println!("   ExpectRust supports Rust's byte string syntax:");
    println!("   - b\"\\r\"        = carriage return");
    println!("   - b\"\\n\"        = newline");
    println!("   - b\"\\t\"        = tab");
    println!("   - b\"\\x1b\"      = escape");
    println!("   - b\"\\x1b[2J\"   = clear screen ANSI sequence");
    println!("   - &[0x03]      = Ctrl-C");

    // Example 6: Practical use - interrupting a command
    println!("\n6. Practical Example: Interrupting a Long-Running Command");
    if !cfg!(windows) {
        let mut session6 = Session::builder()
            .timeout(Duration::from_secs(2))
            .spawn("bash")?;

        session6.expect(Pattern::regex(r"[$#]")?).await?;

        // Start a sleep command
        session6.send_line("sleep 100").await?;
        println!("   Started: sleep 100");

        // Wait a moment
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Send Ctrl-C to interrupt
        session6.send(&[0x03]).await?;
        println!("   ✓ Sent: Ctrl-C (interrupt)");

        // Check that we get the prompt back
        match session6
            .expect_any(&[Pattern::regex(r"[$#]")?, Pattern::Timeout])
            .await
        {
            Ok(result) if result.pattern_index == 0 => {
                println!("   ✓ Command interrupted successfully");
            }
            _ => {
                println!("   Note: Interrupt may not have been processed");
            }
        }

        session6.send(b"exit\n").await?;
    } else {
        println!("   (Example skipped on Windows)");
    }

    println!("\n{}", "=".repeat(50));
    println!("All examples completed!");

    Ok(())
}

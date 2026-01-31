//! Timeout handling example

use expectrust::{ExpectError, Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ExpectRust - Timeout Handling Example");
    println!("{}", "=".repeat(50));

    // Example 1: Successful match within timeout
    println!("\n1. Match within timeout");
    let mut session =
        Session::builder()
            .timeout(Duration::from_secs(2))
            .spawn(if cfg!(windows) {
                "cmd /C echo Quick response"
            } else {
                "echo Quick response"
            })?;

    match session.expect(Pattern::exact("Quick")).await {
        Ok(result) => println!("   ✓ Matched: '{}'", result.matched),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Example 2: Timeout while waiting for pattern
    println!("\n2. Timeout waiting for pattern");
    let mut session2 = Session::builder()
        .timeout(Duration::from_millis(500))
        .spawn(if cfg!(windows) {
            "cmd /C timeout /t 2"
        } else {
            "sleep 2"
        })?;

    match session2.expect(Pattern::exact("NEVER_APPEARS")).await {
        Ok(_) => println!("   ✗ Unexpectedly matched"),
        Err(ExpectError::Timeout { duration }) => {
            println!("   ✓ Timeout occurred after {:?} as expected", duration)
        }
        Err(ExpectError::Eof) => {
            println!("   ✓ EOF occurred (command finished before timeout)")
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }

    // Example 3: Using Timeout pattern
    println!("\n3. Handling timeout as a pattern");
    let mut session3 = Session::builder()
        .timeout(Duration::from_millis(500))
        .spawn(if cfg!(windows) {
            "cmd /C timeout /t 2"
        } else {
            "sleep 2"
        })?;

    let patterns = [Pattern::exact("SUCCESS"), Pattern::Timeout, Pattern::Eof];

    match session3.expect_any(&patterns).await {
        Ok(result) => match result.pattern_index {
            0 => println!("   ✓ Got success"),
            1 => println!("   ✓ Handled timeout gracefully"),
            2 => println!("   ✓ Handled EOF gracefully"),
            _ => unreachable!(),
        },
        Err(e) => println!("   ✗ Error: {}", e),
    }

    println!("\n✓ Timeout handling examples complete!");

    Ok(())
}

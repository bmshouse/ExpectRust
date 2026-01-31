//! Pattern matching example demonstrating different pattern types

use expectrust::{Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ExpectRust - Pattern Matching Example");
    println!("{}", "=".repeat(50));

    // Example 1: Exact string matching
    println!("\n1. Exact String Matching");
    let mut session =
        Session::builder()
            .timeout(Duration::from_secs(5))
            .spawn(if cfg!(windows) {
                "cmd /C echo Pattern: SUCCESS"
            } else {
                "echo Pattern: SUCCESS"
            })?;

    let result = session.expect(Pattern::exact("SUCCESS")).await?;
    println!("   ✓ Found exact match: '{}'", result.matched);

    // Example 2: Regex matching
    println!("\n2. Regex Pattern Matching");
    let mut session2 =
        Session::builder()
            .timeout(Duration::from_secs(5))
            .spawn(if cfg!(windows) {
                "cmd /C echo Number: 12345"
            } else {
                "echo Number: 12345"
            })?;

    let result = session2.expect(Pattern::regex(r"\d+")?).await?;
    println!("   ✓ Found regex match: '{}'", result.matched);
    if !result.captures.is_empty() {
        println!("   Captures: {:?}", result.captures);
    }

    // Example 3: Multiple patterns (first match wins)
    println!("\n3. Multiple Pattern Matching");
    let mut session3 =
        Session::builder()
            .timeout(Duration::from_secs(5))
            .spawn(if cfg!(windows) {
                "cmd /C echo Status: OK"
            } else {
                "echo Status: OK"
            })?;

    let patterns = [
        Pattern::exact("ERROR"),
        Pattern::exact("OK"),
        Pattern::exact("FAIL"),
    ];

    let result = session3.expect_any(&patterns).await?;
    println!(
        "   ✓ Matched pattern #{}: '{}'",
        result.pattern_index, result.matched
    );

    println!("\n✓ All pattern matching examples complete!");

    Ok(())
}

//! Basic command execution example

use expectrust::{Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ExpectRust - Basic Command Example");
    println!("{}", "=".repeat(50));

    // Spawn a simple command
    let mut session =
        Session::builder()
            .timeout(Duration::from_secs(5))
            .spawn(if cfg!(windows) {
                "cmd /C echo Hello from ExpectRust!"
            } else {
                "echo Hello from ExpectRust!"
            })?;

    // Wait for the output to contain "Hello"
    let result = session.expect(Pattern::exact("Hello")).await?;

    println!("Matched: {}", result.matched);
    println!("Before match: {}", result.before);

    println!("\n✓ Example complete!");

    Ok(())
}

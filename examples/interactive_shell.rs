//! Interactive shell example

use expectrust::{Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ExpectRust - Interactive Shell Example");
    println!("{}", "=".repeat(50));

    // Start an interactive Python shell
    println!("\nStarting Python interactive shell...");
    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .pty_size(24, 80)
        .spawn("python -i")?;

    // Wait for the Python prompt
    println!("Waiting for Python prompt...");
    session.expect(Pattern::exact(">>> ")).await?;
    println!("✓ Got Python prompt");

    // Send a command
    println!("\nSending: print('Hello, ExpectRust!')");
    session.send_line("print('Hello, ExpectRust!')").await?;

    // Wait for the prompt again (indicating command completed)
    let result = session.expect(Pattern::exact(">>> ")).await?;
    println!("✓ Command output:");
    for line in result.before.lines() {
        if !line.trim().is_empty() && !line.contains("print(") {
            println!("  {}", line);
        }
    }

    // Try a calculation
    println!("\nSending: 2 + 2");
    session.send_line("2 + 2").await?;

    let result = session.expect(Pattern::exact(">>> ")).await?;
    println!("✓ Calculation result:");
    for line in result.before.lines() {
        if !line.trim().is_empty() && !line.contains("2 + 2") {
            println!("  {}", line);
        }
    }

    // Exit Python
    println!("\nExiting Python...");
    session.send_line("exit()").await?;

    println!("\n✓ Interactive shell example complete!");

    Ok(())
}

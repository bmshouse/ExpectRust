//! Minimal SSH automation example
//! Demonstrates the basic pattern for SSH login and command execution

use expectrust::{Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration
    let ssh_host = "user@192.168.1.1";
    let password = "your_password";

    // Connect to SSH
    let command = format!("ssh {}", ssh_host);
    let mut session = Session::builder()
        .timeout(Duration::from_secs(30))
        .spawn(&command)?;

    // Handle password prompt or connection errors
    let patterns = [
        Pattern::regex(r"[Pp]assword:")?,
        Pattern::exact("Connection refused"),
        Pattern::exact("Host key verification failed"),
    ];

    match session.expect_any(&patterns).await?.pattern_index {
        0 => session.send_line(password).await?,
        1 => return Err("Connection refused".into()),
        2 => return Err("Host key verification failed".into()),
        _ => unreachable!(),
    }

    // Wait for shell prompt
    session.expect(Pattern::exact("$ ")).await?;
    println!("✓ Logged in successfully");

    // Run a command
    session.send_line("whoami").await?;
    let result = session.expect(Pattern::exact("$ ")).await?;
    println!("Output: {}", result.before.trim());

    // Clean exit
    session.send_line("exit").await?;
    session.wait().await?;

    Ok(())
}

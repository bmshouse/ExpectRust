//! Interact example: automate the boring part, then hand control to the user.
//!
//! Spawns a shell, waits for its prompt, sends one scripted command, and
//! then calls `session.interact()` to hand the real keyboard/screen over to
//! you. Type commands normally; the example returns once you exit the
//! shell (e.g. `exit` on Unix, `exit` on `cmd.exe`).
//!
//! Run with: `cargo run --example interact_shell`

use expectrust::{Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ExpectRust - Interact Example");
    println!("{}", "=".repeat(50));

    let shell_command = if cfg!(windows) { "cmd" } else { "bash" };
    let prompt = if cfg!(windows) { ">" } else { "$" };

    println!("\nStarting shell ({shell_command})...");
    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .pty_size(24, 80)
        .spawn(shell_command)?;

    session.expect(Pattern::exact(prompt)).await?;

    println!("Sending one scripted command before handing off control...");
    session.send_line("echo Hello from ExpectRust").await?;
    session.expect(Pattern::exact(prompt)).await?;

    println!("\nHanding control to you now - type commands normally.");
    println!("Exit the shell (e.g. `exit`) to return control to this program.\n");

    session.interact().await?;

    println!("\nShell exited - control returned to ExpectRust.");

    Ok(())
}

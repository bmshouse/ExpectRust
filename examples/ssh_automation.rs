//! SSH automation example demonstrating password authentication,
//! privilege escalation, and error handling

use expectrust::{Pattern, Session};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ExpectRust - SSH Automation Example");
    println!("{}", "=".repeat(50));

    // Configuration
    let ssh_host = "user@192.168.1.1";
    let user_password = "user_password_here";
    let root_password = "root_password_here";

    // Step 1: Spawn SSH connection
    println!("\n[1] Connecting to {}...", ssh_host);
    let command = format!("ssh {}", ssh_host);
    let mut session = Session::builder()
        .timeout(Duration::from_secs(30))
        .pty_size(24, 80)
        .strip_ansi(true) // Strip ANSI codes for cleaner matching
        .spawn(&command)?;

    // Step 2: Handle SSH connection - watch for errors or password prompt
    println!("[2] Waiting for SSH prompt or errors...");
    let ssh_patterns = [
        Pattern::regex(r"[Pp]assword:")?, // Password prompt (index 0)
        Pattern::exact("Host key verification failed"), // SSH error (index 1)
        Pattern::exact("Permission denied"), // Auth error (index 2)
        Pattern::exact("Connection refused"), // Connection error (index 3)
        Pattern::exact("No route to host"), // Network error (index 4)
        Pattern::regex(r"Could not resolve hostname")?, // DNS error (index 5)
        Pattern::Timeout,                 // Timeout (index 6)
    ];

    let result = session.expect_any(&ssh_patterns).await?;
    match result.pattern_index {
        0 => {
            println!("   ✓ Got password prompt");
        }
        1 => {
            eprintln!("   ✗ ERROR: Host key verification failed");
            eprintln!(
                "   Hint: Run 'ssh-keyscan {} >> ~/.ssh/known_hosts'",
                ssh_host
            );
            return Err("SSH connection failed".into());
        }
        2 => {
            eprintln!("   ✗ ERROR: Permission denied");
            return Err("SSH authentication failed".into());
        }
        3 => {
            eprintln!("   ✗ ERROR: Connection refused");
            return Err("SSH server not responding".into());
        }
        4 => {
            eprintln!("   ✗ ERROR: No route to host");
            return Err("Network unreachable".into());
        }
        5 => {
            eprintln!("   ✗ ERROR: Could not resolve hostname");
            return Err("DNS resolution failed".into());
        }
        6 => {
            eprintln!("   ✗ ERROR: Connection timeout");
            return Err("SSH connection timed out".into());
        }
        _ => unreachable!(),
    }

    // Step 3: Send user password
    println!("[3] Sending user password...");
    session.send_line(user_password).await?;

    // Step 4: Expect user prompt or authentication failure
    println!("[4] Waiting for user prompt...");
    let user_prompt_patterns = [
        Pattern::exact("$ "),                // User prompt (index 0)
        Pattern::exact("Permission denied"), // Auth failed (index 1)
        Pattern::regex(r"[Pp]assword:")?,    // Wrong password, asking again (index 2)
        Pattern::Timeout,                    // Timeout (index 3)
    ];

    let result = session.expect_any(&user_prompt_patterns).await?;
    match result.pattern_index {
        0 => {
            println!("   ✓ Successfully logged in as user");
        }
        1 | 2 => {
            eprintln!("   ✗ ERROR: Authentication failed - incorrect password");
            return Err("SSH login failed".into());
        }
        3 => {
            eprintln!("   ✗ ERROR: Timeout waiting for shell prompt");
            return Err("No prompt received".into());
        }
        _ => unreachable!(),
    }

    // Step 5: Escalate to root using su
    println!("[5] Escalating privileges with 'su -'...");
    session.send_line("su -").await?;

    // Step 6: Wait for root password prompt
    println!("[6] Waiting for root password prompt...");
    let su_patterns = [
        Pattern::regex(r"[Pp]assword:")?,        // Password prompt (index 0)
        Pattern::exact("su: command not found"), // su not available (index 1)
        Pattern::exact("su: must be run from a terminal"), // PTY error (index 2)
        Pattern::Timeout,                        // Timeout (index 3)
    ];

    let result = session.expect_any(&su_patterns).await?;
    match result.pattern_index {
        0 => {
            println!("   ✓ Got root password prompt");
        }
        1 => {
            eprintln!("   ✗ ERROR: su command not found");
            return Err("Privilege escalation failed".into());
        }
        2 => {
            eprintln!("   ✗ ERROR: su requires terminal");
            return Err("PTY error".into());
        }
        3 => {
            eprintln!("   ✗ ERROR: Timeout waiting for password prompt");
            return Err("No root password prompt received".into());
        }
        _ => unreachable!(),
    }

    // Send root password
    session.send_line(root_password).await?;

    // Step 7: Expect root prompt or errors
    println!("[7] Waiting for root prompt...");
    let root_prompt_patterns = [
        Pattern::exact("# "),                         // Root prompt (index 0)
        Pattern::exact("su: Authentication failure"), // Wrong password (index 1)
        Pattern::exact("su: incorrect password"),     // Wrong password alt (index 2)
        Pattern::exact("su: Permission denied"),      // Permission denied (index 3)
        Pattern::Timeout,                             // Timeout (index 4)
    ];

    let result = session.expect_any(&root_prompt_patterns).await?;
    match result.pattern_index {
        0 => {
            println!("   ✓ Successfully escalated to root");
        }
        1..=3 => {
            eprintln!("   ✗ ERROR: Root authentication failed - incorrect password");
            return Err("su failed".into());
        }
        4 => {
            eprintln!("   ✗ ERROR: Timeout waiting for root prompt");
            return Err("No root prompt received".into());
        }
        _ => unreachable!(),
    }

    // Step 8: Run apt update
    println!("[8] Running 'apt update'...");
    session.send_line("apt update").await?;

    // Step 9: Wait for root prompt after apt update completes
    println!("[9] Waiting for apt update to complete...");
    let result = session.expect(Pattern::exact("# ")).await?;

    // Display apt update output
    println!("   ✓ apt update completed");
    let output = result.before.trim();
    if !output.is_empty() {
        println!("\n--- apt update output ---");
        for line in output.lines().take(10) {
            println!("   {}", line);
        }
        if output.lines().count() > 10 {
            println!("   ... ({} more lines)", output.lines().count() - 10);
        }
        println!("--- end output ---\n");
    }

    // Step 10: Exit root shell
    println!("[10] Exiting root shell...");
    session.send_line("exit").await?;

    // Step 11: Expect user prompt again
    println!("[11] Waiting for user prompt...");
    session.expect(Pattern::exact("$ ")).await?;
    println!("   ✓ Back to user shell");

    // Step 12: Exit SSH session
    println!("[12] Exiting SSH session...");
    session.send_line("exit").await?;

    // Step 13: Wait for process to terminate normally
    println!("[13] Waiting for SSH to close...");
    let exit_status = session.wait().await?;

    if exit_status.success() {
        println!("   ✓ SSH session closed successfully");
    } else {
        println!("   ⚠ SSH exited with status: {:?}", exit_status.exit_code());
    }

    println!("\n{}", "=".repeat(50));
    println!("✓ SSH automation example complete!");

    Ok(())
}

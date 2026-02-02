//! SSH automation using ExpectRust's script syntax
//! This demonstrates the same SSH workflow using the script parser

use expectrust::script::Script;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ExpectRust - SSH Automation Script Example");
    println!("{}", "=".repeat(50));

    let script_text = r#"
        # SSH Automation Script
        # Connects to SSH, escalates to root, runs apt update

        set ssh_host "user@192.168.1.1"
        set user_password "user_password_here"
        set root_password "root_password_here"

        # Set timeout
        set timeout 30

        # Step 1: Spawn SSH
        spawn ssh $ssh_host

        # Step 2: Handle SSH connection
        expect {
            -re "[Pp]assword:" {
                send "$user_password\n"
            }
            "Host key verification failed" {
                puts "ERROR: Host key verification failed"
                exit 1
            }
            "Permission denied" {
                puts "ERROR: Permission denied"
                exit 1
            }
            "Connection refused" {
                puts "ERROR: Connection refused"
                exit 1
            }
            timeout {
                puts "ERROR: SSH connection timeout"
                exit 1
            }
        }

        # Step 3: Wait for user prompt
        expect {
            "$ " {
                puts "Successfully logged in as user"
            }
            "Permission denied" {
                puts "ERROR: Authentication failed"
                exit 1
            }
            timeout {
                puts "ERROR: Timeout waiting for user prompt"
                exit 1
            }
        }

        # Step 4: Escalate to root
        send "su -\n"

        expect {
            -re "[Pp]assword:" {
                send "$root_password\n"
            }
            "su: command not found" {
                puts "ERROR: su command not found"
                exit 1
            }
            timeout {
                puts "ERROR: No root password prompt"
                exit 1
            }
        }

        # Step 5: Wait for root prompt
        expect {
            "# " {
                puts "Successfully escalated to root"
            }
            "su: Authentication failure" {
                puts "ERROR: Root authentication failed"
                exit 1
            }
            "su: incorrect password" {
                puts "ERROR: Incorrect root password"
                exit 1
            }
            timeout {
                puts "ERROR: No root prompt received"
                exit 1
            }
        }

        # Step 6: Run apt update
        send "apt update\n"

        # Wait for completion
        expect "# "
        puts "apt update completed"

        # Step 7: Exit root shell
        send "exit\n"
        expect "$ "
        puts "Back to user shell"

        # Step 8: Exit SSH
        send "exit\n"

        puts "SSH session closed successfully"
    "#;

    // Parse and execute the script
    let script = Script::from_str(script_text)?;
    script.execute().await?;

    println!("\n{}", "=".repeat(50));
    println!("✓ SSH automation script complete!");

    Ok(())
}

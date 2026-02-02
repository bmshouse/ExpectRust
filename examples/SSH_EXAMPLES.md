# SSH Automation Examples

This directory contains comprehensive examples demonstrating SSH automation with ExpectRust, including password authentication, privilege escalation, and robust error handling.

## Examples Overview

### 1. `ssh_automation.rs` - Complete SSH Automation (Recommended)

**Full-featured example** demonstrating professional SSH automation with comprehensive error handling.

**Features:**
- ✅ SSH connection with password authentication
- ✅ Comprehensive error detection (connection refused, host key errors, DNS failures, etc.)
- ✅ Privilege escalation using `su -`
- ✅ Running commands as root (`apt update`)
- ✅ Proper cleanup (exit root shell, exit SSH)
- ✅ Detailed logging and error messages
- ✅ Pattern matching for multiple scenarios

**Run with:**
```bash
cargo run --example ssh_automation
```

**Use case:** Production-ready SSH automation with full error handling

---

### 2. `ssh_simple.rs` - Minimal SSH Example

**Lightweight example** for quick SSH login and command execution.

**Features:**
- ✅ Basic SSH connection
- ✅ Password authentication
- ✅ Single command execution
- ✅ Clean exit

**Run with:**
```bash
cargo run --example ssh_simple
```

**Use case:** Learning the basics or simple one-off commands

---

### 3. `ssh_automation_script.rs` - Script-Based Automation

**Script syntax example** using ExpectRust's built-in Expect/Tcl-style scripting.

**Features:**
- ✅ Same functionality as `ssh_automation.rs`
- ✅ Uses declarative script syntax
- ✅ Easier for those familiar with traditional Expect

**Run with:**
```bash
cargo run --features script --example ssh_automation_script
```

**Use case:** When you prefer script syntax over Rust API

---

### 4. `ssh_automation.exp` - Traditional Expect Script

**Classic Expect/Tcl script** that can be translated to Rust using `expect2rust`.

**Features:**
- ✅ Traditional Expect syntax
- ✅ Can be translated to Rust code automatically
- ✅ Useful for migrating existing Expect scripts

**Translate to Rust:**
```bash
# Install the translator
cargo install --path . --features translator

# Translate the script
expect2rust examples/ssh_automation.exp

# This generates ssh_automation_translated.rs
```

**Use case:** Migrating from traditional Expect scripts

---

## Configuration

Before running these examples, update the configuration values:

```rust
let ssh_host = "user@192.168.1.1";     // Your SSH server
let user_password = "your_password";    // Your user password
let root_password = "root_password";    // Root password for su
```

⚠️ **Security Note:** Never commit passwords to version control. Use environment variables or configuration files in production:

```rust
use std::env;

let user_password = env::var("SSH_PASSWORD")?;
let root_password = env::var("ROOT_PASSWORD")?;
```

---

## Common Patterns Demonstrated

### 1. **Multiple Pattern Matching**
```rust
let patterns = [
    Pattern::regex(r"[Pp]assword:")?,    // Match various password prompts
    Pattern::exact("Connection refused"),  // Detect connection errors
    Pattern::Timeout,                     // Handle timeouts
];

let result = session.expect_any(&patterns).await?;
match result.pattern_index {
    0 => { /* handle password */ },
    1 => { /* handle error */ },
    2 => { /* handle timeout */ },
    _ => unreachable!(),
}
```

### 2. **Error Detection and Handling**
```rust
match result.pattern_index {
    0 => {
        println!("✓ Success");
    }
    1 => {
        eprintln!("✗ ERROR: Connection refused");
        return Err("SSH failed".into());
    }
    _ => unreachable!(),
}
```

### 3. **Capturing Output**
```rust
let result = session.expect(Pattern::exact("$ ")).await?;
println!("Command output: {}", result.before);
```

---

## SSH Workflow Explained

### Step-by-Step Process

1. **Spawn SSH** - Launch the SSH client process
   ```rust
   Session::spawn(format!("ssh {}", ssh_host))?
   ```

2. **Handle Connection** - Watch for password prompt or errors
   - Password prompt → send password
   - Connection refused → exit with error
   - Host key verification failed → exit with error

3. **Authenticate** - Send password and wait for shell prompt
   - Success → get `$ ` prompt
   - Failure → get "Permission denied" or password prompt again

4. **Escalate Privileges** - Use `su -` to become root
   ```rust
   session.send_line("su -").await?
   ```

5. **Root Authentication** - Send root password
   - Success → get `# ` prompt
   - Failure → get authentication error

6. **Execute Commands** - Run commands as root
   ```rust
   session.send_line("apt update").await?
   ```

7. **Exit Root Shell** - Return to user shell
   ```rust
   session.send_line("exit").await?
   ```

8. **Exit SSH** - Close the connection
   ```rust
   session.send_line("exit").await?
   session.wait().await?
   ```

---

## Common SSH Errors Handled

| Error | Pattern | Meaning |
|-------|---------|---------|
| Host key verification failed | `Pattern::exact("Host key verification failed")` | Server's SSH key not in known_hosts |
| Connection refused | `Pattern::exact("Connection refused")` | SSH service not running or blocked |
| No route to host | `Pattern::exact("No route to host")` | Network unreachable |
| Permission denied | `Pattern::exact("Permission denied")` | Wrong password or auth method |
| Could not resolve hostname | `Pattern::regex(r"Could not resolve hostname")?` | DNS lookup failed |
| Timeout | `Pattern::Timeout` | No response within timeout period |

---

## Testing Safely

### Use a Local Test Environment

**Option 1: Local SSH Server**
```bash
# Linux/Mac - enable SSH server
sudo systemctl start sshd

# Test with localhost
let ssh_host = "testuser@localhost";
```

**Option 2: Docker Container**
```bash
# Run SSH server in Docker
docker run -d -p 2222:22 \
  -e SSH_USERNAME=testuser \
  -e SSH_PASSWORD=testpass \
  linuxserver/openssh-server

# Connect via port 2222
let ssh_host = "testuser@localhost -p 2222";
```

**Option 3: Virtual Machine**
- Use VirtualBox/VMware with SSH enabled
- Safe isolated environment for testing

---

## Tips and Best Practices

### 1. **Use Regex for Flexible Matching**
```rust
// Matches "Password:", "password:", "Enter password:", etc.
Pattern::regex(r"[Pp]assword:")?
```

### 2. **Strip ANSI Codes**
```rust
Session::builder()
    .strip_ansi(true)  // Remove color codes and escape sequences
    .spawn(cmd)?
```

### 3. **Set Appropriate Timeouts**
```rust
// Short timeout for local connections
.timeout(Duration::from_secs(10))

// Longer timeout for slow networks or remote servers
.timeout(Duration::from_secs(60))
```

### 4. **Check Process Exit Status**
```rust
let exit_status = session.wait().await?;
if !exit_status.success() {
    eprintln!("Process exited with code: {:?}", exit_status.code());
}
```

### 5. **Use Environment Variables for Secrets**
```bash
export SSH_PASSWORD="secret"
export ROOT_PASSWORD="topsecret"
cargo run --example ssh_automation
```

```rust
let user_password = env::var("SSH_PASSWORD")
    .expect("SSH_PASSWORD not set");
```

---

## Troubleshooting

### Problem: "Host key verification failed"
**Solution:** Add the host to known_hosts
```bash
ssh-keyscan 192.168.1.1 >> ~/.ssh/known_hosts
```

### Problem: Timeout waiting for prompt
**Possible causes:**
- Prompt doesn't match expected pattern
- ANSI escape codes in output
- Network latency

**Solutions:**
- Enable `.strip_ansi(true)`
- Use regex patterns: `Pattern::regex(r"\$ ")?`
- Increase timeout duration
- Print received buffer to debug: `println!("{:?}", result.before)`

### Problem: Commands not executing
**Possible causes:**
- Missing newline in send
- Wrong prompt pattern

**Solutions:**
- Use `.send_line()` instead of `.send()` to append `\n`
- Verify prompt format matches server's actual prompt

---

## Security Considerations

### ⚠️ Production Security

1. **Never hardcode passwords**
   - Use environment variables
   - Use SSH keys instead of passwords when possible
   - Use secret management tools (HashiCorp Vault, AWS Secrets Manager)

2. **Validate inputs**
   ```rust
   if !ssh_host.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '@' || c == '-') {
       return Err("Invalid hostname".into());
   }
   ```

3. **Use SSH keys for automation**
   ```bash
   # Generate key pair
   ssh-keygen -t ed25519

   # Copy to server
   ssh-copy-id user@192.168.1.1

   # No password needed
   spawn ssh user@192.168.1.1
   ```

4. **Log carefully**
   - Don't log passwords or sensitive data
   - Sanitize outputs before logging

---

## Further Reading

- [ExpectRust Main README](../README.md)
- [ExpectRust API Documentation](https://docs.rs/expectrust)
- [Original Unix Expect](https://core.tcl-lang.org/expect/index)
- [OpenSSH Documentation](https://www.openssh.com/manual.html)

---

## License

These examples are part of the ExpectRust project and licensed under the same terms (MIT OR Apache-2.0).

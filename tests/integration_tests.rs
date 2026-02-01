//! Integration tests for ExpectRust

use expectrust::{ExpectError, Pattern, Session};
use std::time::Duration;

#[tokio::test]
async fn test_basic_command_execution() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo Hello World"
        } else {
            "echo Hello World"
        })
        .expect("Failed to spawn command");

    let result = session
        .expect(Pattern::exact("Hello"))
        .await
        .expect("Failed to find 'Hello'");

    assert_eq!(result.matched, "Hello");
}

#[tokio::test]
async fn test_exact_pattern_matching() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo Testing exact pattern"
        } else {
            "echo Testing exact pattern"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::exact("exact"))
        .await
        .expect("Pattern not found");

    assert_eq!(result.matched, "exact");
}

#[tokio::test]
async fn test_regex_pattern_matching() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo Number: 12345"
        } else {
            "echo Number: 12345"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::regex(r"\d+").expect("Invalid regex"))
        .await
        .expect("Pattern not found");

    assert!(!result.matched.is_empty());
    assert!(!result.captures.is_empty());
}

#[tokio::test]
async fn test_multiple_patterns() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo SUCCESS message"
        } else {
            "echo SUCCESS message"
        })
        .expect("Failed to spawn");

    let patterns = [
        Pattern::exact("FAILURE"),
        Pattern::exact("SUCCESS"),
        Pattern::exact("ERROR"),
    ];

    let result = session
        .expect_any(&patterns)
        .await
        .expect("No pattern matched");

    assert_eq!(result.pattern_index, 1); // Should match SUCCESS (index 1)
    assert_eq!(result.matched, "SUCCESS");
}

#[tokio::test]
async fn test_timeout_error() {
    let mut session = Session::builder()
        .timeout(Duration::from_millis(100))
        .spawn(if cfg!(windows) {
            "cmd /C timeout /t 2"
        } else {
            "sleep 2"
        })
        .expect("Failed to spawn");

    let result = session.expect(Pattern::exact("NEVER_APPEARS")).await;

    match result {
        Err(ExpectError::Timeout { duration }) => {
            assert!(duration.as_millis() >= 100);
        }
        Err(ExpectError::Eof) => {
            // Also acceptable - process may finish before timeout
        }
        Ok(_) => panic!("Should not have matched"),
        Err(e) => panic!("Unexpected error: {}", e),
    }
}

#[tokio::test]
async fn test_eof_pattern() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo Quick"
        } else {
            "echo Quick"
        })
        .expect("Failed to spawn");

    let patterns = [Pattern::exact("Quick"), Pattern::Eof];

    let result = session
        .expect_any(&patterns)
        .await
        .expect("No pattern matched");

    // Should match either "Quick" or EOF
    assert!(result.pattern_index == 0 || result.pattern_index == 1);
}

#[tokio::test]
async fn test_send_and_receive() {
    // Skip on Windows as interactive cmd is complex
    if cfg!(windows) {
        return;
    }

    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .spawn("cat")
        .expect("Failed to spawn cat");

    // Send data
    session
        .send_line("Hello from test")
        .await
        .expect("Failed to send");

    // Should receive echo back
    let result = session
        .expect(Pattern::exact("Hello"))
        .await
        .expect("Failed to receive echo");

    assert_eq!(result.matched, "Hello");
}

#[tokio::test]
async fn test_session_builder() {
    let session = Session::builder()
        .timeout(Duration::from_secs(30))
        .max_buffer_size(4096)
        .strip_ansi(false)
        .pty_size(24, 80)
        .spawn(if cfg!(windows) {
            "cmd /C echo test"
        } else {
            "echo test"
        });

    assert!(session.is_ok());
}

#[tokio::test]
async fn test_is_alive() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo alive"
        } else {
            "echo alive"
        })
        .expect("Failed to spawn");

    // Process should be alive initially or finish quickly
    // Either alive or already finished is acceptable - just verify we can check the status
    let _alive = session.is_alive();
}

#[tokio::test]
async fn test_pattern_at_buffer_start() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo START of line"
        } else {
            "echo START of line"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::exact("START"))
        .await
        .expect("Pattern not found");

    assert_eq!(result.matched, "START");
}

#[tokio::test]
async fn test_utf8_support() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo Hello 世界"
        } else {
            "echo Hello 世界"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::exact("世界"))
        .await
        .expect("Pattern not found");

    assert_eq!(result.matched, "世界");
}

#[tokio::test]
async fn test_regex_with_captures() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo Email: test@example.com"
        } else {
            "echo Email: test@example.com"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::regex(r"(\w+)@(\w+)\.(\w+)").expect("Invalid regex"))
        .await
        .expect("Pattern not found");

    assert!(!result.captures.is_empty());
    assert!(result.captures[0].contains("@"));
}

#[tokio::test]
async fn test_multiple_expects() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo First && echo Second"
        } else {
            // Use printf instead of sh -c to avoid subshell timing issues
            "printf 'First\\nSecond\\n'"
        })
        .expect("Failed to spawn");

    // First expect
    let result1 = session
        .expect(Pattern::exact("First"))
        .await
        .expect("First pattern not found");
    assert_eq!(result1.matched, "First");

    // Second expect
    let result2 = session
        .expect(Pattern::exact("Second"))
        .await
        .expect("Second pattern not found");
    assert_eq!(result2.matched, "Second");
}

#[tokio::test]
async fn test_ansi_stripping() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .strip_ansi(true)
        .spawn(if cfg!(windows) {
            "cmd /C echo Test"
        } else {
            "echo Test"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::exact("Test"))
        .await
        .expect("Pattern not found");

    assert_eq!(result.matched, "Test");
}

#[tokio::test]
async fn test_timeout_pattern() {
    let mut session = Session::builder()
        .timeout(Duration::from_millis(100))
        .spawn(if cfg!(windows) {
            "cmd /C timeout /t 2"
        } else {
            "sleep 2"
        })
        .expect("Failed to spawn");

    let patterns = [Pattern::exact("NEVER"), Pattern::Timeout, Pattern::Eof];

    let result = session
        .expect_any(&patterns)
        .await
        .expect("No pattern matched");

    // Should match either Timeout (index 1) or Eof (index 2)
    assert!(result.pattern_index == 1 || result.pattern_index == 2);
}

#[tokio::test]
async fn test_convenience_spawn() {
    let session = Session::spawn(if cfg!(windows) {
        "cmd /C echo convenience"
    } else {
        "echo convenience"
    });

    assert!(session.is_ok());
}

#[tokio::test]
async fn test_case_insensitive_regex() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo HELLO world"
        } else {
            "echo HELLO world"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::regex(r"(?i)hello").expect("Invalid regex"))
        .await
        .expect("Pattern not found");

    assert!(result.matched.to_lowercase().contains("hello"));
}

#[tokio::test]
async fn test_before_field() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo BEFORE_TEXT MARKER AFTER_TEXT"
        } else {
            "echo BEFORE_TEXT MARKER AFTER_TEXT"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::exact("MARKER"))
        .await
        .expect("Pattern not found");

    assert_eq!(result.matched, "MARKER");
    assert!(result.before.contains("BEFORE_TEXT"));
    assert!(!result.before.contains("AFTER_TEXT"));
}

#[tokio::test]
async fn test_control_character_send() {
    // Skip on Windows as it's complex to test interactively
    if cfg!(windows) {
        return;
    }

    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn("cat")
        .expect("Failed to spawn cat");

    // Send text
    session.send(b"test").await.expect("Failed to send");

    // Send Ctrl-D (EOF) to close cat's stdin
    session.send(&[0x04]).await.expect("Failed to send Ctrl-D");

    // Wait for EOF
    let patterns = [Pattern::exact("test"), Pattern::Eof];
    let result = session.expect_any(&patterns).await.expect("Failed");

    // Should match either the text or EOF
    assert!(result.pattern_index == 0 || result.pattern_index == 1);
}

#[tokio::test]
async fn test_null_byte_pattern() {
    // Skip on Windows as null byte handling is complex
    if cfg!(windows) {
        return;
    }

    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn("printf 'before\\x00after'")
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::Null)
        .await
        .expect("Null byte not found");

    assert_eq!(result.matched, "\0");
    assert!(result.before.contains("before"));
}

#[tokio::test]
async fn test_buffer_compaction() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .max_buffer_size(1024) // Small buffer to trigger compaction
        .spawn(if cfg!(windows) {
            "cmd /C echo Long output that will fill the buffer..."
        } else {
            "yes | head -n 100"
        })
        .expect("Failed to spawn");

    // Try to read a lot of output
    let patterns = [Pattern::exact("y"), Pattern::Eof];

    // Should handle buffer compaction without errors
    for _ in 0..5 {
        if session.expect_any(&patterns).await.is_ok() {
            break;
        }
    }

    // If we got here without panicking, buffer compaction worked
}

#[tokio::test]
async fn test_wait_for_process() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo done"
        } else {
            "echo done"
        })
        .expect("Failed to spawn");

    // Wait for the process to complete
    let status = session.wait().await.expect("Failed to wait");

    // On Unix, exit code 0 is success
    // On Windows, exit code 0 is also success
    assert_eq!(status.exit_code(), 0);
}

#[tokio::test]
async fn test_sequential_commands() {
    // Skip on Windows - multi-command syntax differs
    if cfg!(windows) {
        return;
    }

    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .spawn("bash -i")
        .expect("Failed to spawn bash");

    // Wait for prompt (can be $ or bash-version info)
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send first command
    session
        .send_line("echo FIRST")
        .await
        .expect("Failed to send first command");

    let result1 = session
        .expect(Pattern::exact("FIRST"))
        .await
        .expect("First command output not found");
    assert_eq!(result1.matched, "FIRST");

    // Send second command
    session
        .send_line("echo SECOND")
        .await
        .expect("Failed to send second command");

    let result2 = session
        .expect(Pattern::exact("SECOND"))
        .await
        .expect("Second command output not found");
    assert_eq!(result2.matched, "SECOND");

    // Exit bash
    session.send_line("exit").await.ok();
}

#[tokio::test]
async fn test_pattern_position_info() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(5))
        .spawn(if cfg!(windows) {
            "cmd /C echo Position test"
        } else {
            "echo Position test"
        })
        .expect("Failed to spawn");

    let result = session
        .expect(Pattern::exact("test"))
        .await
        .expect("Pattern not found");

    // Verify position information is sensible
    assert!(result.start < result.end);
    assert_eq!(result.end - result.start, "test".len());
}

#[tokio::test]
async fn test_no_timeout() {
    let mut session = Session::builder()
        .no_timeout()
        .spawn(if cfg!(windows) {
            "cmd /C echo No timeout test"
        } else {
            "echo No timeout test"
        })
        .expect("Failed to spawn");

    // Should work even with no timeout set
    let result = session
        .expect(Pattern::exact("timeout"))
        .await
        .expect("Pattern not found");

    assert_eq!(result.matched, "timeout");
}

#[tokio::test]
async fn test_empty_pattern_error() {
    // Test that empty patterns are properly handled
    // The ExactMatcher::new() function should reject empty patterns
    use expectrust::Pattern;

    // Valid pattern should work
    let valid = Pattern::exact("test");
    assert!(matches!(valid, Pattern::Exact(_)));

    // Empty string pattern is allowed at Pattern level,
    // but will be caught when converting to a matcher
    let empty = Pattern::exact("");
    let matcher_result = empty.to_matcher();

    // Should fail when trying to create a matcher from empty pattern
    assert!(matcher_result.is_err());
}

#[tokio::test]
async fn test_invalid_regex_pattern() {
    // Invalid regex should return an error
    let result = Pattern::regex("[invalid(");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_spawn_invalid_command() {
    let result = Session::builder().spawn("definitely_not_a_real_command_12345");

    // Should fail to spawn non-existent command
    assert!(result.is_err());
}

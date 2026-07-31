//! Integration tests for `Session::interact` / `Session::interact_with`.
//!
//! These exercise `interact_with` (rather than the public `interact()`)
//! with synthetic pipe-based input/output so the tests don't depend on a
//! real controlling terminal, and don't hijack the test runner's tty.

use expectrust::Session;
use std::io::{Read, Write};
use std::time::Duration;

#[tokio::test]
async fn test_interact_forwards_input_to_child() {
    // No reliably-scriptable cross-platform bidirectional echo command;
    // `cat` is the standard tool for this on Unix.
    if cfg!(windows) {
        return;
    }

    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .spawn("cat")
        .expect("Failed to spawn cat");

    let (input_reader, mut input_writer) = std::io::pipe().expect("Failed to create input pipe");
    let (mut output_reader, output_writer) = std::io::pipe().expect("Failed to create output pipe");

    input_writer
        .write_all(b"hello from test\n")
        .expect("Failed to write to input pipe");

    let interact_handle =
        tokio::spawn(async move { session.interact_with(input_reader, output_writer).await });

    // Give `cat` time to echo the line back through the pty and have it
    // forwarded to our synthetic output pipe before we end the session.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Closing the input side is what makes `interact_with` see EOF and return.
    drop(input_writer);

    let result = tokio::time::timeout(Duration::from_secs(5), interact_handle)
        .await
        .expect("interact_with did not return promptly after input EOF")
        .expect("interact_with task panicked");
    assert!(result.is_ok(), "interact_with failed: {:?}", result.err());

    let mut received = vec![0u8; 64];
    let n = output_reader
        .read(&mut received)
        .expect("Failed to read from output pipe");
    let received_text = String::from_utf8_lossy(&received[..n]);
    assert!(
        received_text.contains("hello from test"),
        "Expected echoed input in forwarded output, got: {:?}",
        received_text
    );
}

#[tokio::test]
async fn test_interact_returns_on_child_eof() {
    // No longer platform-skipped: `interact_with` now independently checks
    // the child's own exit status (via `is_alive()`) as a fallback when the
    // pty itself doesn't signal EOF on child exit in a timely way - which
    // is exactly the case on this project's Windows (ConPTY) backend (see
    // docs/CORE_CAPABILITIES_TODO.md item 18 for the full mechanism). On
    // Unix, a real `read() == 0` after the earlier slave-handle fix
    // (`SessionBuilder::spawn_argv`) still wins the race in practice, so
    // this fallback stays dormant there - either way, this test should now
    // pass on all platforms.
    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .spawn(if cfg!(windows) {
            "cmd /C echo done"
        } else {
            "echo done"
        })
        .expect("Failed to spawn short-lived child");

    let (input_reader, _input_writer) = std::io::pipe().expect("Failed to create input pipe");
    let (_output_reader, output_writer) = std::io::pipe().expect("Failed to create output pipe");

    // `_input_writer` is kept alive (not dropped) for the whole call, so the
    // only way `interact_with` can return is via the child's own EOF once it
    // exits - this isolates the "returns when the child exits" behavior.
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        session.interact_with(input_reader, output_writer),
    )
    .await
    .expect("interact_with did not return promptly on child EOF");

    assert!(result.is_ok(), "interact_with failed: {:?}", result.err());
}

#[tokio::test]
async fn test_interact_returns_on_input_eof() {
    let mut session = Session::builder()
        .timeout(Duration::from_secs(10))
        .spawn(if cfg!(windows) {
            "cmd /C timeout /t 5"
        } else {
            "sleep 5"
        })
        .expect("Failed to spawn long-running child");

    let (input_reader, input_writer) = std::io::pipe().expect("Failed to create input pipe");
    let (_output_reader, output_writer) = std::io::pipe().expect("Failed to create output pipe");

    // Close the input side up front so `interact_with` sees EOF immediately,
    // without waiting for the (still-running) child to exit on its own.
    drop(input_writer);

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        session.interact_with(input_reader, output_writer),
    )
    .await
    .expect("interact_with did not return promptly on input EOF");

    assert!(result.is_ok(), "interact_with failed: {:?}", result.err());
}

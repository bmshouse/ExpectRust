//! Pattern matching example with multiple patterns.

use expectrust::script::Script;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script_text = if cfg!(windows) {
        r#"
            spawn cmd /c echo "Process output"

            expect {
                "success" {
                    send "Success case\n"
                }
                "output" {
                    send "Output case\n"
                }
                timeout {
                    send "Timeout case\n"
                }
            }

            wait
        "#
    } else {
        r#"
            spawn echo "Process output"

            expect {
                "success" {
                    send "Success case\n"
                }
                "output" {
                    send "Output case\n"
                }
                timeout {
                    send "Timeout case\n"
                }
            }

            wait
        "#
    };

    println!("Executing script with pattern matching:\n{}", script_text);

    let script = Script::builder()
        .timeout(Duration::from_secs(5))
        .from_str(script_text)?;

    let result = script.execute().await?;
    println!(
        "\nScript completed with exit status: {:?}",
        result.exit_status
    );

    Ok(())
}

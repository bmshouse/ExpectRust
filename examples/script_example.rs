//! Basic script execution example.

use expectrust::script::Script;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple echo script (cross-platform)
    let script_text = if cfg!(windows) {
        r#"
            spawn cmd /c echo "Hello from Expect script!"
            expect "Hello"
            wait
        "#
    } else {
        r#"
            spawn echo "Hello from Expect script!"
            expect "Hello"
            wait
        "#
    };

    println!("Executing script:\n{}", script_text);
    let script = Script::from_str(script_text)?;
    let result = script.execute().await?;

    println!("\nScript completed successfully!");
    println!("Exit status: {:?}", result.exit_status);

    Ok(())
}

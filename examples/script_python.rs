//! Python REPL automation example.

use expectrust::script::Script;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script_text = r#"
        spawn python -i
        expect ">>> "
        send "print('Hello from Expect!')\n"
        expect ">>> "
        send "x = 42\n"
        expect ">>> "
        send "print(f'The answer is {x}')\n"
        expect ">>> "
        send "exit()\n"
    "#;

    println!("Executing Python automation script...\n");

    let script = Script::builder()
        .timeout(Duration::from_secs(10))
        .from_str(script_text)?;

    match script.execute().await {
        Ok(result) => {
            println!("Script completed successfully!");
            println!("Exit status: {:?}", result.exit_status);
        }
        Err(e) => {
            eprintln!("Script error: {}", e);
        }
    }

    Ok(())
}

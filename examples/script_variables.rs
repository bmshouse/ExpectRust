//! Variable substitution example.

use expectrust::script::Script;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script_text = if cfg!(windows) {
        r#"
            set greeting "Hello, World!"
            spawn cmd /c echo $greeting
            expect "Hello"
            wait
        "#
    } else {
        r#"
            set greeting "Hello, World!"
            set command "echo"
            spawn $command $greeting
            expect "Hello"
            wait
        "#
    };

    println!("Executing script with variables:\n{}", script_text);

    let script = Script::from_str(script_text)?;
    let result = script.execute().await?;

    println!("\nScript completed!");
    println!("Variables:");
    for (name, value) in &result.variables {
        println!("  {} = {}", name, value);
    }

    Ok(())
}

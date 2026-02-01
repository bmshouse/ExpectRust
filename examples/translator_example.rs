//! Example of using the translator API to convert Expect scripts to Rust code.

use expectrust::script::translator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let expect_script = r#"
spawn ssh remote-host
expect "password:"
send "mypassword\n"
expect "$ "
send "uptime\n"
expect "$ "
send "exit\n"
"#;

    println!("Original Expect script:");
    println!("{}", expect_script);
    println!("\n{}", "=".repeat(60));

    // Translate to Rust
    let generated = translator::translate_str(expect_script)?;

    println!("\nGenerated Rust code:\n");
    println!("{}", generated.code);

    if !generated.warnings.is_empty() {
        println!("\n{}", "=".repeat(60));
        println!("\nWarnings:");
        for warning in &generated.warnings {
            println!("  ⚠ {}", warning);
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("\nRequired dependencies:");
    for dep in &generated.dependencies {
        println!("  - {}", dep);
    }

    Ok(())
}

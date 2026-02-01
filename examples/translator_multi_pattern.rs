//! Example of translating an Expect script with multiple patterns.

use expectrust::script::translator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let expect_script = r#"
spawn python -i
expect {
    ">>>" {
        send "print('Hello')\n"
    }
    "Error" {
        exit 1
    }
}
send "exit()\n"
"#;

    println!("Original Expect script with multi-pattern expect:");
    println!("{}", expect_script);
    println!("\n{}", "=".repeat(70));

    // Translate to Rust
    let generated = translator::translate_str(expect_script)?;

    println!("\nGenerated Rust code:\n");
    println!("{}", generated.code);

    if !generated.warnings.is_empty() {
        println!("\n{}", "=".repeat(70));
        println!("\nWarnings:");
        for warning in &generated.warnings {
            println!("  ⚠ {}", warning);
        }
    }

    Ok(())
}

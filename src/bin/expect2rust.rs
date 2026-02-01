//! CLI tool for translating Expect scripts to Rust code.

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "expect2rust")]
#[command(author, version, about = "Translate Expect scripts to Rust code", long_about = None)]
struct Args {
    /// Input expect script file
    input: PathBuf,

    /// Output Rust file (default: input.rs)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Don't include warnings as comments
    #[arg(long)]
    no_warnings: bool,

    /// Generate standalone executable (with main function)
    #[arg(long)]
    standalone: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Check if input file exists
    if !args.input.exists() {
        eprintln!(
            "Error: Input file '{}' does not exist",
            args.input.display()
        );
        std::process::exit(1);
    }

    // Translate the script
    println!("Translating {}...", args.input.display());
    let generated = expectrust::script::translator::translate_file(&args.input)?;

    // Format output
    let mut output = String::new();

    if args.standalone {
        // Already includes main function from translator
        output.push_str(&generated.code);
    } else {
        // Strip the main function wrapper for library usage
        output.push_str(&strip_main_wrapper(&generated.code));
    }

    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension("rs");
        path
    });

    // Write output file
    std::fs::write(&output_path, &output)?;
    println!("✓ Generated Rust code written to {}", output_path.display());

    // Print warnings to stderr
    if !generated.warnings.is_empty() && !args.no_warnings {
        eprintln!("\nTranslation warnings:");
        for warning in &generated.warnings {
            eprintln!("  ⚠ {}", warning);
        }
    }

    // Print dependency information
    if !generated.dependencies.is_empty() {
        println!("\nRequired dependencies:");
        for dep in &generated.dependencies {
            println!("  - {}", dep);
        }
    }

    println!("\nNext steps:");
    println!(
        "  1. Review the generated code at {}",
        output_path.display()
    );
    println!("  2. Add dependencies to your Cargo.toml:");
    println!("     expectrust = \"0.1\"");
    println!("     tokio = {{ version = \"1\", features = [\"full\"] }}");
    println!("  3. Compile and test: cargo build && cargo run");

    Ok(())
}

/// Strip the main function wrapper from generated code.
fn strip_main_wrapper(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let mut result = Vec::new();
    let mut in_main = false;
    let mut skip_imports = true;

    for line in &lines {
        // Skip warning header
        if line.starts_with("//") {
            continue;
        }

        // Skip initial imports (we'll add them back)
        if skip_imports && (line.starts_with("use ") || line.is_empty()) {
            continue;
        }

        if line.contains("#[tokio::main]") {
            skip_imports = false;
            continue;
        }

        if line.contains("async fn main()") {
            in_main = true;
            skip_imports = false;
            continue;
        }

        if in_main {
            // Skip the opening brace after main
            if line.trim() == "{" {
                continue;
            }
            // Skip Ok(()) and final closing brace
            if line.contains("Ok(())") {
                continue;
            }
            if line.trim() == "}" && result.iter().any(|l: &&str| l.contains("session")) {
                break;
            }

            // Dedent by one level
            if let Some(stripped) = line.strip_prefix("    ") {
                result.push(stripped);
            } else {
                result.push(*line);
            }
        }
    }

    // Build output with clean imports
    let mut output = String::new();
    output.push_str("use expectrust::{Session, Pattern};\n");
    output.push_str("use std::time::Duration;\n\n");
    output.push_str(&result.join("\n"));
    output.push('\n');
    output
}

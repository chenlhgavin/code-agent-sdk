//! Hooks example - corresponds to Python examples/hooks.py
//!
//! Run with: cargo run --example hooks [PreToolUse|all]
//!
//! NOTE: Hooks are not yet implemented. This example demonstrates the intended API.

use code_agent_sdk::ClaudeAgentOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let example = args.get(1).map(|s| s.as_str()).unwrap_or("");

    if example.is_empty() {
        println!("Usage: cargo run --example hooks <example_name>");
        println!("\nAvailable examples:");
        println!("  PreToolUse  - Block commands using PreToolUse hook");
        println!("  all         - Run all examples");
        return Ok(());
    }

    println!("=== PreToolUse Example ===");
    println!("NOTE: Hooks are not yet implemented.");
    println!("When implemented, the usage would be:\n");
    println!("  let options = ClaudeAgentOptions::builder()");
    println!("      .allowed_tools([\"Bash\"])");
    println!("      .hooks({{");
    println!("          \"PreToolUse\": [HookMatcher {{ matcher: \"Bash\", hooks: [check_bash_command] }}]");
    println!("      }})");
    println!("      .build();");
    println!();

    let _options = ClaudeAgentOptions::builder()
        .allowed_tools(["Bash"])
        .build();

    Ok(())
}

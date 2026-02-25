//! Test: Cursor Agent-specific options.
//!
//! Verifies Cursor backend options: force_approve, mode, trust_workspace.
//! Requires `agent` CLI to be available.
//!
//! Covers: CursorOptions, BackendKind::Cursor, force_approve, mode, trust_workspace

use code_agent_sdk::{query, AgentOptions, BackendKind, ContentBlock, CursorOptions, Message};
use futures::StreamExt;

use super::helpers::is_backend_available;

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Cursor Agent-specific options ===\n");

    let backend = match is_backend_available(BackendKind::Cursor) {
        Some(b) => b,
        None => {
            println!("SKIP: Cursor Agent CLI not available");
            return Ok(());
        }
    };

    // Test 1: force_approve + trust_workspace
    println!("  --- force_approve + trust_workspace ---");
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Cursor)
            .cli_path(&backend.cli_path)
            .cursor(CursorOptions {
                force_approve: true,
                mode: None,
                trust_workspace: true,
            })
            .build();

        let mut got_result = false;
        let mut response_text = String::new();
        let mut stream = query("What is 9 + 1? Reply with just the number.", Some(options));

        while let Some(msg_result) = stream.next().await {
            let msg = msg_result?;
            match msg {
                Message::Assistant(a) => {
                    for block in &a.content {
                        if let ContentBlock::Text(t) = block {
                            response_text.push_str(&t.text);
                            println!("    Cursor: {}", t.text);
                        }
                    }
                }
                Message::Result(r) => {
                    got_result = true;
                    println!("    Result: {}", r.subtype);
                }
                _ => {}
            }
        }

        assert!(got_result, "Expected ResultMessage");
        assert!(
            response_text.contains("10"),
            "Expected '10' in response: {}",
            response_text
        );
        println!("    Passed");
    }

    // Test 2: plan mode (read-only)
    println!("\n  --- plan mode ---");
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Cursor)
            .cli_path(&backend.cli_path)
            .cursor(CursorOptions {
                force_approve: false,
                mode: Some("plan".to_string()),
                trust_workspace: true,
            })
            .build();

        let mut got_result = false;
        let mut stream =
            query("Describe how you would rename a variable in Rust.", Some(options));

        while let Some(msg_result) = stream.next().await {
            let msg = msg_result?;
            if let Message::Result(_) = msg {
                got_result = true;
            }
        }

        assert!(got_result, "Expected ResultMessage for plan mode");
        println!("    Passed");
    }

    println!("\n  Cursor options tests passed");
    Ok(())
}

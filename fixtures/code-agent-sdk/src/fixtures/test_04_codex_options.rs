//! Test: Codex-specific options.
//!
//! Verifies Codex backend options: approval_policy, sandbox_mode, model.
//! Requires `codex` CLI to be available.
//!
//! Covers: CodexOptions, BackendKind::Codex, approval_policy, sandbox_mode

use code_agent_sdk::{query, AgentOptions, BackendKind, CodexOptions, ContentBlock, Message};
use futures::StreamExt;

use super::helpers::is_backend_available;

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Codex-specific options ===\n");

    let backend = match is_backend_available(BackendKind::Codex) {
        Some(b) => b,
        None => {
            println!("SKIP: Codex CLI not available");
            return Ok(());
        }
    };

    // Test 1: full-auto approval with read-only sandbox
    println!("  --- full-auto + read-only ---");
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Codex)
            .cli_path(&backend.cli_path)
            .codex(CodexOptions {
                approval_policy: Some("full-auto".to_string()),
                sandbox_mode: Some("read-only".to_string()),
            })
            .build();

        let mut got_result = false;
        let mut stream = query("What is 7 * 6? Reply with just the number.", Some(options));

        while let Some(msg_result) = stream.next().await {
            let msg = msg_result?;
            match msg {
                Message::Assistant(a) => {
                    for block in &a.content {
                        if let ContentBlock::Text(t) = block {
                            println!("    Codex: {}", t.text);
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

        assert!(got_result, "Expected ResultMessage for full-auto + read-only");
        println!("    Passed");
    }

    // Test 2: suggest approval policy
    println!("\n  --- suggest approval policy ---");
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Codex)
            .cli_path(&backend.cli_path)
            .codex(CodexOptions {
                approval_policy: Some("suggest".to_string()),
                sandbox_mode: None,
            })
            .build();

        let mut got_result = false;
        let mut stream = query("What color is the sky? One word.", Some(options));

        while let Some(msg_result) = stream.next().await {
            let msg = msg_result?;
            if let Message::Result(_) = msg {
                got_result = true;
            }
        }

        assert!(got_result, "Expected ResultMessage for suggest mode");
        println!("    Passed");
    }

    // Test 3: workspace-write sandbox
    println!("\n  --- workspace-write sandbox ---");
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Codex)
            .cli_path(&backend.cli_path)
            .codex(CodexOptions {
                approval_policy: Some("full-auto".to_string()),
                sandbox_mode: Some("workspace-write".to_string()),
            })
            .build();

        let mut got_result = false;
        let mut stream = query("What is 10 + 20? Reply with just the number.", Some(options));

        while let Some(msg_result) = stream.next().await {
            let msg = msg_result?;
            if let Message::Result(_) = msg {
                got_result = true;
            }
        }

        assert!(got_result, "Expected ResultMessage for workspace-write sandbox");
        println!("    Passed");
    }

    println!("\n  Codex options tests passed");
    Ok(())
}

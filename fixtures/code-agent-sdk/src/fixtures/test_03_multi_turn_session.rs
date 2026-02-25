//! Test: Multi-turn session across all backends.
//!
//! Verifies AgentSdkClient connect/query/receive_response/disconnect
//! works for each available CLI backend with their respective session mechanisms:
//! - Claude: long-lived stdin/stdout subprocess
//! - Codex: `codex app-server` JSON-RPC 2.0
//! - Cursor: spawn-per-turn with `--resume <chatId>`
//!
//! Covers: AgentSdkClient, connect(), query(), receive_response(), disconnect()

use code_agent_sdk::{AgentSdkClient, ContentBlock, Message};
use futures::StreamExt;

use super::helpers::{backend_name, detect_available_backends, options_for_backend};

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Multi-turn session (all backends) ===\n");

    let backends = detect_available_backends();
    if backends.is_empty() {
        println!("SKIP: No CLI backends available");
        return Ok(());
    }

    for backend in &backends {
        println!(
            "\n--- {} backend ---",
            backend_name(backend.kind)
        );
        run_multi_turn(backend).await?;
    }

    println!("\nPassed for {} backend(s)", backends.len());
    Ok(())
}

async fn run_multi_turn(
    backend: &super::helpers::AvailableBackend,
) -> Result<(), anyhow::Error> {
    let name = backend_name(backend.kind);
    let options = options_for_backend(backend);

    let mut client = AgentSdkClient::new(Some(options), None);

    // Connect without initial prompt
    client.connect(None).await?;
    println!("  [{}] Connected", name);

    // Turn 1: Ask a question
    client.query("What is 3 + 5? Reply with just the number.", "session-1").await?;
    let mut turn1_text = String::new();
    let mut turn1_got_result = false;

    {
        let mut response = client.receive_response();
        while let Some(msg_result) = response.next().await {
            let msg = msg_result?;
            match msg {
                Message::Assistant(a) => {
                    for block in &a.content {
                        if let ContentBlock::Text(t) = block {
                            turn1_text.push_str(&t.text);
                        }
                    }
                }
                Message::Result(_) => {
                    turn1_got_result = true;
                }
                _ => {}
            }
        }
    }

    assert!(turn1_got_result, "[{}] Turn 1: Expected ResultMessage", name);
    assert!(
        turn1_text.contains('8'),
        "[{}] Turn 1: Expected '8' in response: {}",
        name,
        turn1_text
    );
    println!("  [{}] Turn 1 passed: {}", name, turn1_text.trim());

    // Turn 2: Follow-up question referencing previous context
    client.query("Now multiply that number by 2. Reply with just the number.", "session-1").await?;
    let mut turn2_text = String::new();
    let mut turn2_got_result = false;

    {
        let mut response = client.receive_response();
        while let Some(msg_result) = response.next().await {
            let msg = msg_result?;
            match msg {
                Message::Assistant(a) => {
                    for block in &a.content {
                        if let ContentBlock::Text(t) = block {
                            turn2_text.push_str(&t.text);
                        }
                    }
                }
                Message::Result(_) => {
                    turn2_got_result = true;
                }
                _ => {}
            }
        }
    }

    assert!(turn2_got_result, "[{}] Turn 2: Expected ResultMessage", name);
    assert!(
        turn2_text.contains("16"),
        "[{}] Turn 2: Expected '16' in response: {}",
        name,
        turn2_text
    );
    println!("  [{}] Turn 2 passed: {}", name, turn2_text.trim());

    // Disconnect
    client.disconnect().await?;
    println!("  [{}] Disconnected", name);
    println!("  [{}] Passed", name);
    Ok(())
}

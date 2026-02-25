//! Test: Basic one-shot query across all backends.
//!
//! Verifies `query()` works for each available CLI backend.
//! Covers: query(), BackendKind, AssistantMessage, TextBlock, ResultMessage

use code_agent_sdk::{query, ContentBlock, Message};
use futures::StreamExt;

use super::helpers::{backend_name, detect_available_backends, options_for_backend};

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Basic one-shot query (all backends) ===\n");

    let backends = detect_available_backends();
    if backends.is_empty() {
        println!("SKIP: No CLI backends available");
        return Ok(());
    }

    for backend in &backends {
        println!(
            "\n--- {} backend (cli: {}) ---",
            backend_name(backend.kind),
            backend.cli_path
        );
        run_basic_query(backend).await?;
    }

    println!("\nPassed for {} backend(s)", backends.len());
    Ok(())
}

async fn run_basic_query(
    backend: &super::helpers::AvailableBackend,
) -> Result<(), anyhow::Error> {
    let options = options_for_backend(backend);

    let mut got_assistant_msg = false;
    let mut got_result_msg = false;
    let mut response_text = String::new();

    let mut stream = query("What is 2 + 2? Reply with just the number.", Some(options));

    while let Some(msg_result) = stream.next().await {
        let msg = msg_result?;
        match msg {
            Message::Assistant(a) => {
                got_assistant_msg = true;
                for block in &a.content {
                    if let ContentBlock::Text(t) = block {
                        response_text.push_str(&t.text);
                        println!("  {}: {}", backend_name(backend.kind), t.text);
                    }
                }
            }
            Message::Result(r) => {
                got_result_msg = true;
                println!("  Result: subtype={}", r.subtype);
                println!("  Session ID: {}", r.session_id);
                if let Some(cost) = r.total_cost_usd {
                    println!("  Cost: ${:.6}", cost);
                }
                println!("  Duration: {}ms", r.duration_ms);
            }
            _ => {}
        }
    }

    assert!(
        got_assistant_msg,
        "[{}] Expected AssistantMessage",
        backend_name(backend.kind)
    );
    assert!(
        got_result_msg,
        "[{}] Expected ResultMessage",
        backend_name(backend.kind)
    );
    assert!(
        response_text.contains('4'),
        "[{}] Expected '4' in response: {}",
        backend_name(backend.kind),
        response_text
    );

    println!(
        "  [{}] Passed",
        backend_name(backend.kind)
    );
    Ok(())
}

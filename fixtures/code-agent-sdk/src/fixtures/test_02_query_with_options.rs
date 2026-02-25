//! Test: One-shot query with backend-specific options.
//!
//! Verifies query() with model and max_turns across all backends.
//! Claude: system_prompt
//! Codex: approval_policy, sandbox_mode
//! Cursor: force_approve, mode, trust_workspace
//! Covers: AgentOptions builder, backend-specific options

use code_agent_sdk::{
    query, AgentOptions, BackendKind, CodexOptions, ContentBlock, CursorOptions, Message,
};
use futures::StreamExt;

use super::helpers::{backend_name, detect_available_backends, options_for_backend};

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Query with options (all backends) ===\n");

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
        run_query_with_options(backend).await?;
    }

    println!("\nPassed for {} backend(s)", backends.len());
    Ok(())
}

async fn run_query_with_options(
    backend: &super::helpers::AvailableBackend,
) -> Result<(), anyhow::Error> {
    let mut base = options_for_backend(backend);
    base.max_turns = Some(1);

    // Apply backend-specific options
    match backend.kind {
        BackendKind::Claude => {
            base.system_prompt = Some(
                code_agent_sdk::options::SystemPromptConfig::String(
                    "You are a helpful assistant. Be concise.".to_string(),
                ),
            );
        }
        BackendKind::Codex => {
            base.codex = Some(CodexOptions {
                approval_policy: Some("full-auto".to_string()),
                sandbox_mode: Some("read-only".to_string()),
            });
        }
        BackendKind::Cursor => {
            base.cursor = Some(CursorOptions {
                force_approve: true,
                mode: None,
                trust_workspace: true,
            });
        }
        _ => {}
    }

    let mut response_text = String::new();
    let mut got_result = false;

    let mut stream = query("Say hello in one word.", Some(base));

    while let Some(msg_result) = stream.next().await {
        let msg = msg_result?;
        match msg {
            Message::Assistant(a) => {
                for block in &a.content {
                    if let ContentBlock::Text(t) = block {
                        response_text.push_str(&t.text);
                        println!("  {}: {}", backend_name(backend.kind), t.text);
                    }
                }
            }
            Message::Result(r) => {
                got_result = true;
                println!("  Result: subtype={}, turns={}", r.subtype, r.num_turns);
            }
            _ => {}
        }
    }

    assert!(
        got_result,
        "[{}] Expected ResultMessage",
        backend_name(backend.kind)
    );
    assert!(
        !response_text.is_empty(),
        "[{}] Expected non-empty response",
        backend_name(backend.kind)
    );

    println!(
        "  [{}] Response: {}...",
        backend_name(backend.kind),
        &response_text[..response_text.len().min(100)]
    );
    println!("  [{}] Passed", backend_name(backend.kind));
    Ok(())
}

/// Test that selecting a backend via the builder works.
pub async fn run_backend_selection() -> Result<(), anyhow::Error> {
    println!("=== Test: Backend selection via builder ===\n");

    let backends = detect_available_backends();
    if backends.is_empty() {
        println!("SKIP: No CLI backends available");
        return Ok(());
    }

    for backend in &backends {
        let options = AgentOptions::builder()
            .backend(backend.kind)
            .cli_path(&backend.cli_path)
            .max_turns(1)
            .build();

        assert_eq!(
            options.backend,
            Some(backend.kind),
            "Backend kind mismatch"
        );
        println!(
            "  Builder correctly set backend to {:?}",
            backend.kind
        );
    }

    println!("  Passed");
    Ok(())
}

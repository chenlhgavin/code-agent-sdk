//! Test: Message type coverage across all backends.
//!
//! Verifies that each backend produces the expected message types:
//! SystemMessage, AssistantMessage (TextBlock), ResultMessage.
//! Also checks for optional message types (ThinkingBlock, ToolUseBlock).
//!
//! Covers: Message variants, ContentBlock variants, message flow

use code_agent_sdk::{query, ContentBlock, Message};
use futures::StreamExt;

use super::helpers::{backend_name, detect_available_backends, options_for_backend};

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Message type coverage (all backends) ===\n");

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
        run_message_type_check(backend).await?;
    }

    println!("\nPassed for {} backend(s)", backends.len());
    Ok(())
}

async fn run_message_type_check(
    backend: &super::helpers::AvailableBackend,
) -> Result<(), anyhow::Error> {
    let name = backend_name(backend.kind);
    let options = options_for_backend(backend);

    let mut seen_system = false;
    let mut seen_assistant = false;
    let mut seen_text_block = false;
    let mut seen_thinking_block = false;
    let mut seen_tool_use_block = false;
    let mut seen_tool_result_block = false;
    let mut seen_result = false;

    let mut stream = query(
        "What is the capital of France? Reply with just the city name.",
        Some(options),
    );

    while let Some(msg_result) = stream.next().await {
        let msg = msg_result?;
        match &msg {
            Message::System(_) => {
                seen_system = true;
                println!("  [{}] SystemMessage", name);
            }
            Message::Assistant(a) => {
                seen_assistant = true;
                for block in &a.content {
                    match block {
                        ContentBlock::Text(t) => {
                            seen_text_block = true;
                            println!("  [{}] TextBlock: {}", name, t.text.trim());
                        }
                        ContentBlock::Thinking(t) => {
                            seen_thinking_block = true;
                            println!(
                                "  [{}] ThinkingBlock: {}...",
                                name,
                                &t.thinking[..t.thinking.len().min(50)]
                            );
                        }
                        ContentBlock::ToolUse(t) => {
                            seen_tool_use_block = true;
                            println!("  [{}] ToolUseBlock: {}", name, t.name);
                        }
                        ContentBlock::ToolResult(t) => {
                            seen_tool_result_block = true;
                            println!(
                                "  [{}] ToolResultBlock: id={}",
                                name, t.tool_use_id
                            );
                        }
                    }
                }
            }
            Message::Result(r) => {
                seen_result = true;
                println!(
                    "  [{}] ResultMessage: subtype={}, is_error={}, session_id={}",
                    name, r.subtype, r.is_error, r.session_id
                );
            }
            Message::User(_) => {
                println!("  [{}] UserMessage (echo)", name);
            }
            Message::StreamEvent(e) => {
                println!("  [{}] StreamEvent: session_id={}", name, e.session_id);
            }
        }
    }

    // Required messages
    assert!(
        seen_assistant,
        "[{}] Expected at least one AssistantMessage",
        name
    );
    assert!(
        seen_text_block,
        "[{}] Expected at least one TextBlock",
        name
    );
    assert!(
        seen_result,
        "[{}] Expected ResultMessage",
        name
    );

    // Report optional message coverage
    println!("  [{}] Message coverage:", name);
    println!("    SystemMessage:    {}", if seen_system { "yes" } else { "no" });
    println!("    AssistantMessage: yes");
    println!("    TextBlock:        yes");
    println!("    ThinkingBlock:    {}", if seen_thinking_block { "yes" } else { "no" });
    println!("    ToolUseBlock:     {}", if seen_tool_use_block { "yes" } else { "no" });
    println!("    ToolResultBlock:  {}", if seen_tool_result_block { "yes" } else { "no" });
    println!("    ResultMessage:    yes");
    println!("  [{}] Passed", name);

    Ok(())
}

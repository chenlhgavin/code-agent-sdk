//! Tools option example - corresponds to Python examples/tools_option.py
//!
//! Run with: cargo run --example tools_option

use code_agent_sdk::{AgentOptions, Message, query};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tools Array Example ===");
    println!("Setting tools=['Read', 'Glob', 'Grep']\n");

    let options = AgentOptions::builder()
        .allowed_tools(["Read", "Glob", "Grep"])
        .max_turns(1)
        .build();

    {
        use futures::StreamExt;
        let mut stream = query(
            "What tools do you have available? Just list them briefly.",
            Some(options),
        );
        while let Some(msg_result) = stream.next().await {
            if let Ok(Message::System(ref s)) = msg_result {
                if s.subtype == "init" {
                    if let Some(tools) = s.data.get("tools") {
                        println!("Tools from system message: {}", tools);
                    }
                }
            }
            if let Ok(Message::Assistant(ref a)) = msg_result {
                for block in &a.content {
                    if let code_agent_sdk::ContentBlock::Text(t) = block {
                        println!("Claude: {}", t.text);
                    }
                }
            }
            if let Ok(Message::Result(ref r)) = msg_result {
                if let Some(cost) = r.total_cost_usd {
                    println!("\nCost: ${:.4}", cost);
                }
            }
        }
    }
    println!();

    Ok(())
}

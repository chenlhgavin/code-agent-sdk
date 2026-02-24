# Code Agent SDK (Rust)

Rust SDK for [Claude Code Agent](https://docs.anthropic.com/en/docs/claude-code), providing programmatic access to Claude Code CLI. Feature-parity with the [Python SDK](https://github.com/anthropics/claude-agent-sdk-python).

## Prerequisites

- **Rust** 1.70+
- **Claude Code CLI** 2.0.0+ — install via `npm install -g @anthropic-ai/claude-code` or [install script](https://claude.ai/install.sh)
- **ANTHROPIC_API_KEY** — set in environment for Claude API calls

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
code-agent-sdk = "0.1"
```

## Quick Start

```rust
use code_agent_sdk::{query, ClaudeAgentOptions, Message};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = query("What is 2 + 2?", None);
    while let Some(msg_result) = stream.next().await {
        if let Ok(Message::Assistant(ref a)) = msg_result {
            for block in &a.content {
                if let code_agent_sdk::ContentBlock::Text(t) = block {
                    println!("Claude: {}", t.text);
                }
            }
        }
    }
    Ok(())
}
```

## Basic Usage

### query()

One-shot query returning a stream of messages:

```rust
use code_agent_sdk::{query, ClaudeAgentOptions, Message};
use futures::StreamExt;

// Simple query
let mut stream = query("Hello Claude", None);

// With options
let options = ClaudeAgentOptions::builder()
    .system_prompt("You are a helpful assistant")
    .max_turns(1)
    .build();
let mut stream = query("Tell me a joke", Some(options));

while let Some(msg) = stream.next().await {
    // Handle Message::User | Assistant | System | Result | StreamEvent
}
```

### ClaudeAgentOptions

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(["Read", "Write", "Bash"])
    .disallowed_tools(["Bash"])
    .system_prompt("You are a coding assistant")
    .permission_mode("acceptEdits")
    .model("claude-sonnet-4-20250514")
    .max_turns(10)
    .max_budget_usd(1.0)
    .cwd("/path/to/project")
    .cli_path("/custom/path/to/claude")
    .build();
```

### ClaudeSdkClient

For multi-turn, interactive sessions:

```rust
use code_agent_sdk::ClaudeSdkClient;

let mut client = ClaudeSdkClient::new(None);
client.connect(None).await?;
client.query("What is 2+2?", "default").await?;

// Stream responses
let mut rx = client.receive_response();
while let Some(msg) = rx.next().await {
    // Process Message
}

client.disconnect().await?;
```

### Hooks & can_use_tool

```rust
use code_agent_sdk::{ClaudeAgentOptions, HookMatcher, PermissionResult, PermissionResultAllow};
use std::sync::Arc;

// Tool permission callback
let can_use_tool = Arc::new(|tool_name: String, _input, _ctx| {
    Box::pin(async move {
        PermissionResult::Allow(PermissionResultAllow {
            updated_input: None,
            updated_permissions: None,
        })
    })
});

// PreToolUse hook
let hooks = [(
    "PreToolUse".to_string(),
    vec![HookMatcher {
        matcher: Some("Bash".to_string()),
        hooks: vec![/* HookCallback */],
        timeout: Some(60.0),
    }],
)]
.into_iter()
.collect();

let options = ClaudeAgentOptions::builder()
    .can_use_tool(can_use_tool)
    .hooks(hooks)
    .build();
```

### MCP Servers

```rust
use code_agent_sdk::{ClaudeAgentOptions, McpServerConfig, McpStdioConfig};
use std::collections::HashMap;

let mcp = [(
    "calculator".to_string(),
    McpServerConfig::Stdio(McpStdioConfig {
        command: "npx".to_string(),
        args: Some(vec!["-y", "@modelcontextprotocol/server-calculator".to_string()]),
        env: Some([("VAR".to_string(), "value".to_string())].into_iter().collect()),
    }),
)]
.into_iter()
.collect();

let options = ClaudeAgentOptions::builder()
    .mcp_servers(mcp)
    .allowed_tools(["calculator"])
    .build();
```

## Examples

```bash
cargo run --example quick_start
cargo run --example streaming_mode basic_streaming
cargo run --example system_prompt
cargo run --example tools_option
cargo run --example tool_permission_callback
cargo run --example mcp_calculator
cargo run --example hooks PreToolUse
```

## Message Types

| Type | Description |
|------|-------------|
| `UserMessage` | User input |
| `AssistantMessage` | Claude response (text, thinking, tool_use) |
| `SystemMessage` | System events (init, tools, etc.) |
| `ResultMessage` | Session result (cost, usage, duration) |
| `StreamEvent` | Streaming events |

## Documentation

- [Architecture (Rust)](docs/arch-rust.md)
- [Architecture (Python reference)](docs/arch.md)
- [Python vs Rust feature comparison](docs/python-rust-feature-comparison.md)

## License

MIT

# Code Agent SDK (Rust)

Multi-backend Rust SDK for driving AI code agents. Provides a unified API for [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code), [OpenAI Codex CLI](https://github.com/openai/codex), and [Cursor Agent CLI](https://cursor.com).

## Supported Backends

| Backend | One-Shot | Multi-Turn | CLI Binary |
|---------|----------|------------|------------|
| Claude Code | `query()` | `AgentSdkClient` (long-lived subprocess) | `claude` |
| OpenAI Codex | `query()` | `AgentSdkClient` (JSON-RPC app-server) | `codex` |
| Cursor Agent | `query()` | `AgentSdkClient` (spawn-per-turn) | `agent` |

## Prerequisites

- **Rust** 1.70+
- At least one CLI backend installed:
  - **Claude Code CLI** 2.0.0+ via `npm install -g @anthropic-ai/claude-code`
  - **Codex CLI** via `npm install -g @openai/codex`
  - **Cursor Agent CLI** via [cursor.com](https://cursor.com)
- **API key** set in environment for the chosen backend

## Installation

```toml
[dependencies]
code-agent-sdk = "0.1"
```

## Quick Start

### One-shot query (default: Claude)

```rust
use code_agent_sdk::{query, AgentOptions, Message};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = query("What is 2 + 2?", None);
    while let Some(msg_result) = stream.next().await {
        if let Ok(Message::Assistant(ref a)) = msg_result {
            for block in &a.content {
                if let code_agent_sdk::ContentBlock::Text(t) = block {
                    println!("Response: {}", t.text);
                }
            }
        }
    }
    Ok(())
}
```

### Using a different backend

```rust
use code_agent_sdk::{query, AgentOptions, BackendKind, CodexOptions, CursorOptions};

// Codex backend
let options = AgentOptions::builder()
    .backend(BackendKind::Codex)
    .codex(CodexOptions {
        approval_policy: Some("full-auto".to_string()),
        sandbox_mode: Some("read-only".to_string()),
    })
    .build();
let mut stream = query("Explain quicksort", Some(options));

// Cursor Agent backend
let options = AgentOptions::builder()
    .backend(BackendKind::Cursor)
    .cursor(CursorOptions {
        force_approve: true,
        mode: None,
        trust_workspace: true,
    })
    .build();
let mut stream = query("What does this codebase do?", Some(options));
```

## API Reference

### `query()`

One-shot query returning a `Stream<Item = Result<Message>>`:

```rust
use code_agent_sdk::{query, AgentOptions};

// Simple (defaults to Claude)
let mut stream = query("Hello", None);

// With options
let options = AgentOptions::builder()
    .system_prompt("You are a helpful assistant")
    .max_turns(1)
    .build();
let mut stream = query("Tell me a joke", Some(options));
```

### `AgentOptions`

Configuration for all backends. Backend-specific options live in `CodexOptions` and `CursorOptions`.

```rust
let options = AgentOptions::builder()
    .backend(BackendKind::Claude)         // Claude | Codex | Cursor
    .allowed_tools(["Read", "Write"])
    .system_prompt("You are a coding assistant")  // Claude only
    .permission_mode("acceptEdits")
    .model("claude-sonnet-4-20250514")
    .max_turns(10)
    .max_budget_usd(1.0)
    .cwd("/path/to/project")
    .cli_path("/custom/path/to/claude")
    .build();
```

### `AgentSdkClient`

For multi-turn, interactive sessions across all backends:

```rust
use code_agent_sdk::{AgentSdkClient, AgentOptions, BackendKind};
use futures::StreamExt;

let options = AgentOptions::builder()
    .backend(BackendKind::Claude)
    .build();

let mut client = AgentSdkClient::new(Some(options), None);
client.connect(None).await?;
client.query("What is 2+2?", "default").await?;

let mut rx = client.receive_response();
while let Some(msg) = rx.next().await {
    // Process Message
}

client.disconnect().await?;
```

### Hooks & can_use_tool (Claude only)

```rust
use code_agent_sdk::{AgentOptions, HookMatcher, PermissionResult, PermissionResultAllow};
use std::sync::Arc;

let can_use_tool = Arc::new(|tool_name: String, _input, _ctx| {
    Box::pin(async move {
        PermissionResult::Allow(PermissionResultAllow {
            updated_input: None,
            updated_permissions: None,
        })
    })
});

let options = AgentOptions::builder()
    .can_use_tool(can_use_tool)
    .build();
```

### MCP Servers (Claude only)

```rust
use code_agent_sdk::{AgentOptions, McpServerConfig, McpStdioConfig, McpServersConfig};
use std::collections::HashMap;

let mut servers = HashMap::new();
servers.insert("calculator".to_string(), McpServerConfig::Stdio(McpStdioConfig {
    command: "npx".to_string(),
    args: Some(vec!["-y".to_string(), "@modelcontextprotocol/server-calculator".to_string()]),
    env: None,
}));

let options = AgentOptions::builder()
    .mcp_servers(McpServersConfig::Dict(servers))
    .allowed_tools(["mcp__calculator__add"])
    .build();
```

## Feature Compatibility

| Feature | Claude | Codex | Cursor |
|---------|--------|-------|--------|
| `query()` one-shot | Yes | Yes | Yes |
| Multi-turn session | Yes | Yes | Yes |
| `can_use_tool` callback | Yes | Yes (mapped) | No |
| Hooks | Yes | No | No |
| SDK MCP tools | Yes | No | No |
| Model selection | Yes | Yes | Yes |
| System prompt | Yes | No | No |
| `interrupt()` | Yes | Yes | No |
| `set_model()` / `set_permission_mode()` | Yes | No | No |
| Structured output | Yes | Yes | No |

Unsupported features return `Error::UnsupportedFeature` or `Error::UnsupportedOptions`.

## Message Types

| Type | Description |
|------|-------------|
| `UserMessage` | User input |
| `AssistantMessage` | Agent response (text, thinking, tool_use, tool_result) |
| `SystemMessage` | System events (init, tools, etc.) |
| `ResultMessage` | Session result (cost, usage, duration) |
| `StreamEvent` | Streaming events |

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

## Fixture Tests

```bash
# Run offline tests (no CLI needed)
cd fixtures/code-agent-sdk && cargo run -- offline

# Run all tests (skips backends without CLI)
cd fixtures/code-agent-sdk && cargo run -- all

# Run individual fixtures
cd fixtures/code-agent-sdk && cargo run -- test_01   # Basic query
cd fixtures/code-agent-sdk && cargo run -- test_06   # Backend validation
```

## Documentation

- [Architecture Design](docs/arch-rust.md)

## License

MIT

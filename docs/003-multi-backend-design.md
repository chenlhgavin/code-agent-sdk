# Multi-Backend Support: Codex CLI + Cursor Agent CLI

## Context

The SDK currently only supports Claude CLI, with all CLI-specific logic hardcoded in `SubprocessCliTransport` (binary discovery, 240-line `build_command()`, env vars, version check) and `message_parser.rs`. The system has both **Codex CLI** (`codex` v0.104.0) and **Cursor Agent CLI** (`agent` v2026.02.13) installed. Both support streaming JSON output and multi-turn conversations (through different mechanisms).

**Goal**: Introduce a `Backend` abstraction layer so the SDK can drive Claude CLI, Codex CLI, and Cursor Agent CLI with a unified public API, including multi-turn support for all three.

## Multi-Turn Strategy Per Backend

| Backend | One-Shot | Multi-Turn | Protocol |
|---|---|---|---|
| Claude | `claude --output-format stream-json` (long-lived) | stdin/stdout streaming | Custom stream-json + control protocol |
| Codex | `codex exec --json <prompt>` (exits after) | `codex app-server` (long-lived JSON-RPC) | JSON-RPC 2.0: `thread/start`, `turn/start`, `turn/interrupt` |
| Cursor | `agent --print --output-format stream-json <prompt>` (exits after) | Spawn new process per turn: `agent --print --resume <chatId> ...` | JSONL events, session ID from `system/init` event |

## Protocol Comparison: Claude CLI vs Codex App-Server

Both Claude CLI and Codex app-server use the **same architectural pattern**: long-lived subprocess + stdin/stdout JSONL bidirectional communication. Key parallels:

| Aspect | Claude CLI | Codex App-Server |
|---|---|---|
| 进程模型 | 单个长驻子进程 | 单个长驻子进程 |
| 传输格式 | JSONL, 自定义 `type` 字段 | JSONL, JSON-RPC 2.0 `method` 字段 |
| 初始化 | `control_request{subtype:"initialize"}` → `control_response` | `initialize` → response → `initialized` notification |
| 发送消息 | `{"type":"user","message":{...}}` → stdin | `{"method":"turn/start","params":{"threadId":"...","input":[...]}}` |
| 接收响应 | `type:"assistant"/"result"` 消息 | `item/*` + `turn/completed` 通知 |
| 审批机制 | CLI发 `control_request{subtype:"can_use_tool"}`, SDK回 `control_response` | Server发 `requestApproval` (带id), Client回 `{"id":..,"result":{"decision":"accept"}}` |
| 中断 | `control_request{subtype:"interrupt"}` | `{"method":"turn/interrupt"}` |
| 会话标识 | 隐式 (一个进程=一个会话) | 显式 `threadId` |

**Codex approval → CanUseToolCallback 映射**:
- `item/commandExecution/requestApproval` → `can_use_tool("Bash", {"command":"rm -rf ..."}, ctx)`
- `item/fileChange/requestApproval` → `can_use_tool("Edit", {...}, ctx)`
- `PermissionResult::Allow` → `{"decision":"accept"}`, `PermissionResult::Deny` → `{"decision":"decline"}`

**Cursor Agent** 无长驻进程模式，使用 spawn-per-turn: 每轮启动新的 `agent --print --resume <chatId>` 进程。

## Architecture

```
┌──────────────────────────────────────────────────────┐
│  Public API (lib.rs, client.rs) -- minimal changes    │
│  query(), AgentSdkClient  (alias: ClaudeSdkClient)   │
├──────────────────────────────────────────────────────┤
│  Backend trait + Session trait  (new abstraction)     │
│  validate_options(), one_shot_query(), create_session │
├──────────┬───────────────┬───────────────────────────┤
│ Claude   │ Codex         │ Cursor Agent              │
│ Backend  │ Backend       │ Backend                   │
│ (stream) │ (app-server)  │ (spawn-per-turn)          │
└──────────┴───────────────┴───────────────────────────┘
```

## Module Layout

```
src/
├── lib.rs                              # Add re-exports for BackendKind, new aliases
├── client.rs                           # AgentSdkClient with capability gating
├── options.rs                          # Add backend field, CodexOptions, CursorOptions
├── types.rs                            # Unchanged
├── error.rs                            # Add UnsupportedFeature, UnsupportedOptions
├── transport/
│   └── mod.rs                          # Transport trait unchanged
├── backend/
│   ├── mod.rs                          # Backend + Session traits, Capabilities, BackendKind
│   ├── claude/
│   │   ├── mod.rs                      # ClaudeBackend impl
│   │   ├── cli_finder.rs              # find_cli(), check version (from subprocess_cli.rs:51-103, 696-739)
│   │   ├── command_builder.rs         # build_command() (from subprocess_cli.rs:105-481)
│   │   ├── transport.rs              # SubprocessCliTransport (from subprocess_cli.rs, process mgmt)
│   │   └── message_parser.rs         # Moved from internal/message_parser.rs
│   ├── codex/
│   │   ├── mod.rs                     # CodexBackend impl
│   │   ├── exec_transport.rs         # One-shot: codex exec --json
│   │   ├── app_server.rs             # Multi-turn: codex app-server JSON-RPC
│   │   ├── message_parser.rs         # Codex event -> Message normalization
│   │   └── jsonrpc.rs                # JSON-RPC 2.0 helpers
│   └── cursor/
│       ├── mod.rs                     # CursorBackend impl
│       ├── transport.rs              # One-shot subprocess + spawn-per-turn session
│       ├── message_parser.rs         # Cursor event -> Message normalization
│       └── session.rs                # CursorSession: manages chatId, process-per-turn
└── internal/
    ├── mod.rs                         # Keep re-export for backward compat
    ├── client.rs                      # Delegate to backend.one_shot_query()
    └── query.rs                       # Stays as-is, used only by ClaudeBackend
```

## Key New Types

### `src/backend/mod.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum BackendKind {
    #[default]
    Claude,
    Codex,
    Cursor,
}

#[derive(Debug, Clone)]
pub struct Capabilities {
    pub control_protocol: bool,     // Claude only
    pub tool_approval: bool,        // Claude (can_use_tool), Codex (app-server approval)
    pub hooks: bool,                // Claude only
    pub sdk_mcp_routing: bool,      // Claude only
    pub persistent_session: bool,   // Claude, Codex app-server
    pub interrupt: bool,            // Claude, Codex (turn/interrupt)
    pub runtime_config_changes: bool, // Claude only (set_model, set_permission_mode)
}

#[async_trait]
pub trait Backend: Send + Sync {
    fn capabilities(&self) -> &Capabilities;
    fn name(&self) -> &str;
    fn validate_options(&self, options: &AgentOptions) -> Result<()>;

    fn one_shot_query(
        &self, prompt: Prompt, options: &AgentOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Message>> + Send>>>;

    async fn create_session(
        &self, options: &AgentOptions, prompt: Option<Prompt>,
    ) -> Result<Box<dyn Session + Send>>;
}

#[async_trait]
pub trait Session: Send {
    async fn send_message(&mut self, prompt: Prompt, session_id: &str) -> Result<()>;
    fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>>;
    fn receive_response(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>>;
    async fn send_control_request(&mut self, request: Value) -> Result<Value>;
    async fn get_server_info(&self) -> Option<Value>;
    async fn close(&mut self) -> Result<()>;
}
```

### `src/error.rs` additions

```rust
#[error("Feature '{feature}' is not supported by the {backend} backend")]
UnsupportedFeature { feature: String, backend: String },

#[error("Options not supported by {backend} backend: {}", options.join(", "))]
UnsupportedOptions { backend: String, options: Vec<String> },
```

### `src/options.rs` additions

```rust
pub backend: Option<BackendKind>,       // New field on AgentOptions
pub codex: Option<CodexOptions>,        // Codex-specific options
pub cursor: Option<CursorOptions>,      // Cursor-specific options

pub struct CodexOptions {
    pub approval_policy: Option<String>,    // "auto-edit"|"full-auto"|"suggest"
    pub sandbox_mode: Option<String>,       // "read-only"|"workspace-write"|"danger-full-access"
}

pub struct CursorOptions {
    pub force_approve: bool,                // --force / --yolo
    pub mode: Option<String>,               // "plan"|"ask"
    pub trust_workspace: bool,              // --trust
}

// Backward-compatible aliases
pub type ClaudeAgentOptions = AgentOptions;
pub type ClaudeAgentOptionsBuilder = AgentOptionsBuilder;
```

## Message Normalization

All backends map to the existing `Message` enum. Key mappings:

### Codex -> Message

| Codex Event | SDK Message |
|---|---|
| `thread.started` | `SystemMessage { subtype: "init" }` |
| `item.completed { type: "agent_message" }` | `AssistantMessage { content: [TextBlock] }` |
| `item.completed { type: "reasoning" }` | `AssistantMessage { content: [ThinkingBlock] }` |
| `item.completed { type: "command_execution" }` | `AssistantMessage { content: [ToolUseBlock, ToolResultBlock] }` |
| `turn.completed { usage }` | `ResultMessage { usage, duration_ms, ... }` |

### Cursor -> Message

| Cursor Event | SDK Message |
|---|---|
| `{ type: "system", subtype: "init" }` | `SystemMessage { subtype: "init" }` |
| `{ type: "assistant", message }` | `AssistantMessage { content: [TextBlock] }` |
| `{ type: "thinking" }` | `AssistantMessage { content: [ThinkingBlock] }` |
| `{ type: "tool_call", subtype: "started" }` | `AssistantMessage { content: [ToolUseBlock] }` |
| `{ type: "tool_call", subtype: "completed" }` | `AssistantMessage { content: [ToolResultBlock] }` |
| `{ type: "result", subtype: "success" }` | `ResultMessage { duration_ms, session_id, ... }` |

Note: `ResultMessage` fields not available from a backend (e.g., `total_cost_usd` from Codex/Cursor) use `None`/`0` defaults.

## Capability Gating in AgentSdkClient

```rust
// Methods that need gating:
interrupt()              -> require capabilities.interrupt
set_permission_mode()    -> require capabilities.runtime_config_changes
set_model()             -> require capabilities.runtime_config_changes
rewind_files()          -> require capabilities.control_protocol
get_mcp_status()        -> require capabilities.control_protocol

// Returns Error::UnsupportedFeature { feature, backend } on mismatch
```

## Feature Compatibility Matrix

| Feature | Claude | Codex | Cursor |
|---|---|---|---|
| `query()` one-shot | Yes | Yes | Yes |
| Multi-turn session | Yes (stdin stream) | Yes (app-server) | Yes (spawn-per-turn) |
| `can_use_tool` callback | Yes | Yes (mapped to approval flow) | No |
| Hooks | Yes | No | No |
| SDK MCP tools | Yes | No | No |
| `model` selection | Yes | Yes | Yes |
| `system_prompt` | Yes | No | No |
| `interrupt()` | Yes | Yes (turn/interrupt) | No |
| `set_model()` / `set_permission_mode()` | Yes | No | No |
| `structured_output` | Yes | Yes (--output-schema) | No |
| Session resume | Yes | Yes (thread/resume) | Yes (--resume chatId) |

## Implementation Phases

### Phase 1: Backend abstraction (non-breaking refactor)
1. Create `src/backend/mod.rs` with `Backend`, `Session`, `Capabilities`, `BackendKind`
2. Create `src/backend/claude/` -- extract from `subprocess_cli.rs` and `message_parser.rs`
   - `cli_finder.rs`: lines 51-103, 696-739 from `subprocess_cli.rs`
   - `command_builder.rs`: lines 105-481 from `subprocess_cli.rs`
   - `transport.rs`: lines 482-654 from `subprocess_cli.rs`
   - `message_parser.rs`: moved from `src/internal/message_parser.rs`
   - `mod.rs`: `ClaudeBackend` implementing `Backend`, `ClaudeSession` wrapping `Query`
3. Add `UnsupportedFeature` / `UnsupportedOptions` to `error.rs`
4. Add `backend` field to options, add type aliases
5. Update `InternalClient` to resolve backend from options
6. Update `ClaudeSdkClient` -> `AgentSdkClient` with capability gating (alias old name)
7. Keep `src/internal/message_parser.rs` re-exporting from `backend::claude` for backward compat
8. **Verify**: `cargo build && cargo test && cargo +nightly fmt && cargo clippy -- -D warnings`

### Phase 2: Codex backend
1. Create `src/backend/codex/message_parser.rs` with `CodexMessageParser` (stateful: buffers item.started)
2. Create `src/backend/codex/jsonrpc.rs` (build/parse JSON-RPC 2.0 requests/responses)
3. Create `src/backend/codex/exec_transport.rs` (one-shot: spawn `codex exec --json`, read JSONL)
4. Create `src/backend/codex/app_server.rs` (multi-turn: spawn `codex app-server`, JSON-RPC protocol)
5. Create `src/backend/codex/mod.rs` with `CodexBackend` + `CodexSession`
6. Add `CodexOptions` to `options.rs`, implement `validate_options()`
7. Map Codex app-server approval requests to `CanUseToolCallback`
8. Wire into `BackendKind::Codex` in `create_backend()`
9. **Verify**: unit tests for message parser, command builder, options validation. Integration test with real codex binary (gated with `#[ignore]`).

### Phase 3: Cursor Agent backend
1. Create `src/backend/cursor/message_parser.rs` with `CursorMessageParser` (buffers thinking)
2. Create `src/backend/cursor/session.rs` with `CursorSession` (spawn-per-turn, tracks chatId)
3. Create `src/backend/cursor/transport.rs` (one-shot subprocess)
4. Create `src/backend/cursor/mod.rs` with `CursorBackend`
5. Add `CursorOptions` to `options.rs`, implement `validate_options()`
6. Wire into `BackendKind::Cursor` in `create_backend()`
7. **Verify**: unit tests for message parser, session management, options validation. Integration test with real agent binary (gated with `#[ignore]`).

### Phase 4: Documentation and cleanup
1. Update `docs/arch-rust.md` with multi-backend architecture
2. Remove `src/transport/subprocess_cli.rs` (content moved to `backend/claude/`)
3. Full test suite: `cargo build && cargo test && cargo +nightly fmt && cargo clippy -- -D warnings`

## Verification

```bash
# Build
cargo build

# All tests
cargo test

# Format
cargo +nightly fmt

# Lint
cargo clippy -- -D warnings

# E2E tests (require CLIs installed)
CODEX_CLI_PATH=$(which codex) CURSOR_CLI_PATH=$(which agent) cargo test -- --ignored
```

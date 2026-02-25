# Code Agent SDK (Rust) - Architecture Design Document

> Multi-backend Rust SDK for driving AI code agents. Last updated: 2026-02

---

## 1. Overview

Code Agent SDK is a Rust library providing a unified API for driving multiple AI code agent CLIs: [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code), [OpenAI Codex CLI](https://github.com/openai/codex), and [Cursor Agent CLI](https://cursor.com). It supports both one-shot queries and multi-turn interactive sessions across all backends, with a shared message model and capability-gated feature set.

### Design Goals

- **Unified API**: Single `query()` function and `AgentSdkClient` type work across all backends
- **Backend Abstraction**: `Backend` + `Session` traits isolate protocol differences behind a common interface
- **Capability Gating**: Each backend declares its capabilities; unsupported operations return typed errors at compile-time-checkable boundaries
- **Message Normalization**: All backends emit a unified `Message` enum regardless of native protocol differences
- **Lazy Evaluation**: Streams are constructed synchronously and evaluated lazily, following Rust idioms
- **Extensibility**: Custom transports, tool permission callbacks, hooks, and in-process MCP servers

---

## 2. Architecture Design

### 2.1 Layered Architecture

The SDK is organized into four layers, each with a single responsibility:

```
┌──────────────────────────────────────────────────────────┐
│                     User Application                      │
│     query() / AgentSdkClient / create_sdk_mcp_server()   │
├──────────────────────────────────────────────────────────┤
│                      Public API Layer                     │
│  lib.rs     query(), sdk_mcp_tool(), create_sdk_mcp_*    │
│  client.rs  AgentSdkClient (session management, control) │
│  options.rs AgentOptions + Builder (all backends)         │
│  types.rs   Message, ContentBlock, Prompt                │
├──────────────────────────────────────────────────────────┤
│                   Backend Abstraction Layer                │
│  backend/mod.rs      Backend + Session traits, Capabilities│
│  backend/claude/     ClaudeBackend (full-featured)        │
│  backend/codex/      CodexBackend (JSON-RPC app-server)   │
│  backend/cursor/     CursorBackend (spawn-per-turn)       │
├──────────────────────────────────────────────────────────┤
│                    Internal Logic Layer                    │
│  internal/client.rs        InternalClient (query routing) │
│  internal/query.rs         Query (control protocol, hooks)│
│  internal/message_parser.rs  Claude message parser        │
├──────────────────────────────────────────────────────────┤
│                      Transport Layer                      │
│  transport/mod.rs           Transport trait               │
│  transport/subprocess_cli.rs  (legacy, Claude-only)       │
│  backend/claude/transport.rs  ClaudeCliTransport          │
│  backend/codex/exec_transport.rs + app_server.rs          │
│  backend/cursor/transport.rs + session.rs                 │
└──────────────────────────────────────────────────────────┘
```

### 2.2 Component Relationships

```
                      ┌──────────────────┐
                      │  AgentSdkClient  │──── optional ────► Box<dyn Transport>
                      └────────┬─────────┘          (legacy Claude path)
                               │ holds
                               ▼
┌───────────────┐     ┌─────────────────┐
│InternalClient │     │ Box<dyn Backend> │
│ (query route) │     │  create_backend()│
└──────┬────────┘     └────────┬────────┘
       │                       │ creates
       │                       ▼
       │              ┌─────────────────┐
       └─────────────►│ Box<dyn Session> │
                      │  send_message() │
                      │  receive_*()    │
                      │  close()        │
                      └─────────────────┘
                               ▲
              ┌────────────────┼────────────────┐
              │                │                │
     ClaudeSession     CodexSession     CursorSession
     (Query wrapper)   (JSON-RPC 2.0)  (spawn-per-turn)
```

### 2.3 Multi-Turn Session Strategy Per Backend

| Backend | One-Shot | Multi-Turn | Protocol |
|---------|----------|------------|----------|
| Claude  | `claude --output-format stream-json` (long-lived) | Single subprocess, stdin/stdout streaming | Custom stream-json + control protocol |
| Codex   | `codex exec --json <prompt>` (exits after) | `codex app-server` (long-lived JSON-RPC) | JSON-RPC 2.0: `thread/start`, `turn/start`, `turn/interrupt` |
| Cursor  | `agent --print --output-format stream-json` (exits after) | Spawn new process per turn: `agent --print --resume <chatId>` | JSONL events, session ID from `system/init` |

---

## 3. Design Principles

### 3.1 Core Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Concurrency Model | Actor pattern (two background tokio tasks per session) | `write_task` / `read_task` run independently, avoiding lock contention |
| Message Distribution | `broadcast::channel` | Supports multiple subscribers: `receive_messages()` and `receive_response()` can coexist |
| Write Channel | `mpsc::channel` | Single write point; dropping the sender triggers stdin EOF |
| Stream Construction | `async_stream::stream!` macro | Lazy evaluation; no manual `Stream` implementation required |
| Transport Abstraction | `dyn Transport` (object-safe via `async_trait`) | Supports test mock injection without coupling to business logic |
| Request Tracking | `AtomicU64` counter | Lock-free unique `request_id` generation |
| Init Result Storage | `RwLock<Option<Value>>` | Write-once, read-many; better read concurrency than Mutex |
| Backend Selection | Runtime dispatch via `Box<dyn Backend>` | Backend chosen at runtime from `AgentOptions::backend` |
| Capability Gating | `Capabilities` struct with bool fields | Compile-time-documented, runtime-checked feature matrix |

### 3.2 Key Design Patterns

- **Backend + Session Trait System**: Separates backend lifecycle (CLI discovery, option validation, one-shot query) from session lifecycle (message exchange, control requests, close). This allows different session strategies (long-lived subprocess, JSON-RPC server, spawn-per-turn) behind a uniform interface.

- **Message Normalization**: Each backend has its own `message_parser.rs` that converts native events into the shared `Message` enum. SDK consumers write code against `Message` once, regardless of backend.

- **Capability Gating**: `AgentSdkClient` checks `Backend::capabilities()` before executing control methods (`interrupt()`, `set_model()`, etc.). Unsupported operations return `Error::UnsupportedFeature` rather than silently failing.

- **Options Validation**: Each backend's `validate_options()` rejects Claude-specific options (hooks, system_prompt, MCP servers, etc.) that don't apply to that backend, returning `Error::UnsupportedOptions` with the list of unsupported fields.

- **Lazy Stream Evaluation**: `query()` returns `impl Stream` synchronously. The subprocess spawn, initialization, and message reading all happen lazily when the stream is consumed. This matches Rust conventions and avoids unnecessary resource allocation.

---

## 4. Data Flow Design

### 4.1 One-Shot Query Lifecycle

```
query(prompt, options)
    │
    ├─► InternalClient::process_query()
    │       └─► resolve BackendKind from options
    │       └─► Backend::one_shot_query(prompt, options)
    │
    ├─► Backend validates options
    ├─► Backend creates transport (CLI subprocess)
    ├─► Transport spawns child process
    │
    ├─► [Claude] Query::initialize() ── control protocol handshake
    │   [Codex]  Direct exec, no handshake
    │   [Cursor] Direct exec, no handshake
    │
    ├─► Send prompt (stdin write or CLI argument)
    │
    └─► Stream messages back to caller:
        ├── parse_message() per backend
        ├── yield Ok(Message::System(...))
        ├── yield Ok(Message::Assistant(...))
        ├── yield Ok(Message::Result(...))  ── stream terminates
        └── on error: yield Err(Error) ── stream terminates
```

### 4.2 Claude Multi-Turn Session Lifecycle

```
Caller                    AgentSdkClient / ClaudeSession        Claude CLI
  │                               │                                │
  │── connect(None) ─────────────►│── spawn + initialize ─────────►│
  │                               │◄── init response ──────────────│
  │                               │                                │
  │── query("Q1", sid) ──────────►│── write user message ─────────►│ process Q1
  │── for msg in receive_response │◄── Assistant + Result ──────────│
  │◄── Message::Assistant ────────│                                │
  │◄── Message::Result ───────────│                                │
  │                               │                                │
  │── query("Q2", sid) ──────────►│── write user message ─────────►│ process Q2
  │── for msg in receive_response │◄── Assistant + Result ──────────│
  │◄── Message::Assistant ────────│                                │
  │◄── Message::Result ───────────│                                │
  │                               │                                │
  │── disconnect() ──────────────►│── drop write_tx → EOF ────────►│ exit
```

### 4.3 Codex Multi-Turn Session Lifecycle (App-Server)

```
Caller                    AgentSdkClient / CodexSession        Codex App-Server
  │                               │                                │
  │── connect(None) ─────────────►│── spawn codex app-server ─────►│
  │                               │── JSON-RPC initialize ────────►│
  │                               │◄── initialize response ────────│
  │                               │── JSON-RPC initialized ───────►│
  │                               │── thread/start ───────────────►│
  │                               │◄── thread.started ─────────────│
  │                               │                                │
  │── query("Q1", sid) ──────────►│── turn/start(input) ──────────►│ process Q1
  │── for msg in receive_response │◄── item/* notifications ────────│
  │◄── Message::Assistant ────────│                                │
  │                               │◄── requestApproval ─────────────│ (tool approval)
  │                               │── {decision:"accept"} ────────►│
  │                               │◄── turn/completed ──────────────│
  │◄── Message::Result ───────────│                                │
  │                               │                                │
  │── disconnect() ──────────────►│── close stdin ────────────────►│ exit
```

### 4.4 Cursor Multi-Turn Session Lifecycle (Spawn-Per-Turn)

```
Caller                    AgentSdkClient / CursorSession        agent CLI
  │                               │                                │
  │── connect(prompt) ───────────►│── spawn agent --print <prompt>►│ process turn 1
  │── for msg in receive_response │◄── JSONL events ────────────────│
  │◄── Message::System (init) ────│   (extract chatId)             │
  │◄── Message::Assistant ────────│                                │
  │◄── Message::Result ───────────│◄── process exits ──────────────│
  │                               │                                │
  │── query("Q2", sid) ──────────►│── spawn agent --resume <chatId>│ process turn 2
  │── for msg in receive_response │◄── JSONL events ────────────────│ (new process)
  │◄── Message::Assistant ────────│                                │
  │◄── Message::Result ───────────│◄── process exits ──────────────│
  │                               │                                │
  │── disconnect() ──────────────►│── (no-op, no persistent proc) ─│
```

### 4.5 Control Protocol Flow (Claude Only)

```
Claude CLI                read_task              CanUseToolCallback / HookCallback
    │                         │                          │
    │── control_request ─────►│                          │
    │   {subtype:"can_use_tool",                         │
    │    tool_name:"Bash",    │                          │
    │    input:{command:...}} │                          │
    │                         │── parse & invoke ───────►│
    │                         │◄── PermissionResult ─────│
    │                         │   Allow{updated_input}   │
    │◄── control_response ────│   or Deny{message}       │
    │   {behavior:"allow/deny"}                          │
```

### 4.6 Internal Channel Architecture (Claude Query)

```
External calls                     Query internals
send_control_request()  ─────► write_tx (mpsc::Sender<String>)
write_user_message()             │
                                 ▼
                          [write_task] tokio::spawn
                          write_rx.recv() → transport.write()
                          channel close → transport.end_input() → transport.close()

                          [read_task] tokio::spawn
                          read_stream.next() ──► match msg_type
                                │
                                ├── "control_request" ──► handle_control_request()
                                │         └── result via write_tx → CLI
                                │
                                ├── "control_cancel_request" ──► ignore
                                │
                                └── data / "end" / "error"
                                          ▼
                                    message_tx.send()  (broadcast::Sender)
                                          │
                              ┌───────────┼───────────┐
                              ▼           ▼           ▼
                         subscriber1  subscriber2  ...
                         receive_messages()  receive_response()
```

### 4.7 SDK MCP Request Routing (Claude Only)

```
Claude CLI                 read_task              SdkMcpTool.handler
    │                          │                          │
    │── control_request ──────►│                          │
    │   {subtype:"mcp_message",│                          │
    │    server_name:"calc",   │                          │
    │    message:{             │                          │
    │      method:"tools/call",│                          │
    │      params:{name:"add"} │                          │
    │    }}                    │                          │
    │                          │── route by server_name   │
    │                          │── route by method        │
    │                          │─────────────────────────►│ (tool.handler)(args)
    │                          │◄──── Result<Value> ───────│
    │                          │── wrap as JSONRPC response│
    │◄── control_response ─────│── write_tx.send(resp)    │
```

---

## 5. Key Data Structure Design

### 5.1 Message Type Hierarchy

All backends normalize their native events into the unified `Message` enum:

```rust
pub enum Message {
    User(UserMessage),          // User input
    Assistant(AssistantMessage), // Agent response (text, thinking, tool_use, tool_result)
    System(SystemMessage),      // System events (init, tools, etc.)
    Result(ResultMessage),      // Session result (cost, usage, duration)
    StreamEvent(StreamEvent),   // SSE streaming events
}
```

**Design choices**:
- `UserContent` is an `#[serde(untagged)]` union (`String` or `Blocks`), matching the CLI protocol while supporting both input forms
- `SystemMessage.data` uses `#[serde(flatten)]` to preserve all original fields for forward compatibility
- Unknown `ContentBlock` types return `Ok(None)` during parsing rather than errors, ensuring new CLI versions don't break old SDK versions

### 5.2 Content Block Types

```rust
#[serde(tag = "type")]
pub enum ContentBlock {
    Text(TextBlock),           // { text: String }
    Thinking(ThinkingBlock),   // { thinking: String, signature: String }
    ToolUse(ToolUseBlock),     // { id: String, name: String, input: Value }
    ToolResult(ToolResultBlock), // { tool_use_id: String, content: Option<Value>, is_error: Option<bool> }
}
```

Uses `#[serde(tag = "type")]` internally-tagged representation, matching the CLI JSON protocol structure directly.

### 5.3 Message Normalization Per Backend

| Native Event | SDK Message |
|---|---|
| **Claude** `type: "assistant"` | `AssistantMessage { content: [ContentBlock], model }` |
| **Claude** `type: "result"` | `ResultMessage { duration_ms, total_cost_usd, ... }` |
| **Codex** `item.completed { type: "agent_message" }` | `AssistantMessage { content: [TextBlock] }` |
| **Codex** `item.completed { type: "command_execution" }` | `AssistantMessage { content: [ToolUseBlock, ToolResultBlock] }` |
| **Codex** `turn.completed { usage }` | `ResultMessage { usage, ... }` |
| **Cursor** `{ type: "assistant", message }` | `AssistantMessage { content: [TextBlock] }` |
| **Cursor** `{ type: "tool_call", subtype: "started" }` | `AssistantMessage { content: [ToolUseBlock] }` |
| **Cursor** `{ type: "result" }` | `ResultMessage { duration_ms, session_id, ... }` |

Fields not available from a backend (e.g., `total_cost_usd` from Codex/Cursor) use `None`/`0` defaults.

### 5.4 Backend Selection and Capabilities

```rust
#[non_exhaustive]
pub enum BackendKind {
    #[default]
    Claude,
    Codex,
    Cursor,
}

pub struct Capabilities {
    pub control_protocol: bool,      // Claude-style control_request/control_response
    pub tool_approval: bool,         // can_use_tool callback support
    pub hooks: bool,                 // SDK hook callbacks
    pub sdk_mcp_routing: bool,       // In-process MCP server routing
    pub persistent_session: bool,    // Long-lived session (not spawn-per-turn)
    pub interrupt: bool,             // Turn interruption
    pub runtime_config_changes: bool, // set_model, set_permission_mode
}
```

**Capability values per backend**:

| Capability | Claude | Codex | Cursor |
|------------|--------|-------|--------|
| `control_protocol` | true | false | false |
| `tool_approval` | true | true | false |
| `hooks` | true | false | false |
| `sdk_mcp_routing` | true | false | false |
| `persistent_session` | true | true | false |
| `interrupt` | true | true | false |
| `runtime_config_changes` | true | false | false |

### 5.5 Agent Options

`AgentOptions` configures all backends. Fields are grouped by concern:

| Concern | Fields | Purpose |
|---------|--------|---------|
| **Execution Environment** | `cli_path`, `cwd`, `env`, `user`, `extra_args` | Control subprocess startup and runtime environment |
| **Backend Selection** | `backend`, `codex`, `cursor` | Choose backend and pass backend-specific options |
| **Model Control** | `model`, `fallback_model`, `max_turns`, `max_budget_usd`, `effort`, `thinking`, `max_thinking_tokens`, `betas` | Control model behavior and resource limits |
| **Tool Control** | `tools`, `allowed_tools`, `disallowed_tools`, `permission_mode`, `permission_prompt_tool_name` | Tool permissions and filtering |
| **MCP / Plugins** | `mcp_servers`, `plugins`, `add_dirs`, `agents` | Extend tool capabilities and context |
| **Session Management** | `continue_conversation`, `resume`, `fork_session`, `setting_sources`, `settings`, `enable_file_checkpointing`, `sandbox` | Session state and persistence |
| **Callbacks / Output** | `can_use_tool`, `hooks`, `stderr`, `include_partial_messages`, `output_format`, `max_buffer_size` | Observe and intercept SDK behavior |

Backend-specific options:

```rust
pub struct CodexOptions {
    pub approval_policy: Option<String>,  // "auto-edit" | "full-auto" | "suggest"
    pub sandbox_mode: Option<String>,     // "read-only" | "workspace-write" | "danger-full-access"
}

pub struct CursorOptions {
    pub force_approve: bool,              // --force / --yolo
    pub mode: Option<String>,             // "plan" | "ask"
    pub trust_workspace: bool,            // --trust
}
```

Builder pattern is used (via `typed-builder`-style manual implementation) given 30+ fields, providing compile-time guarantee of valid construction and ergonomic chaining.

### 5.6 Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Claude Code not found: {0}")]
    CliNotFound(String),

    #[error("Connection error: {0}")]
    Connection(#[from] std::io::Error),

    #[error("Not connected. Call connect() first.")]
    NotConnected,

    #[error("Process failed with exit code {exit_code}")]
    Process { exit_code: i32, stderr: Option<String> },

    #[error("JSON decode error: {0}")]
    JsonDecode(#[from] serde_json::Error),

    #[error("Message parse error: {0}")]
    MessageParse(String),

    #[error("Control request timeout: {0}")]
    ControlTimeout(String),

    #[error("Feature '{feature}' is not supported by the {backend} backend")]
    UnsupportedFeature { feature: String, backend: String },

    #[error("Options not supported by {backend} backend: {}", options.join(", "))]
    UnsupportedOptions { backend: String, options: Vec<String> },

    #[error("{0}")]
    Other(String),
}
```

**Error propagation strategy**:
- All public methods return `Result<T, Error>`
- Stream interfaces inline errors as `Result<Message>` items; consumers decide whether to skip or terminate
- `Error::NotConnected` distinguishes state errors from I/O errors
- `Error::UnsupportedFeature` and `Error::UnsupportedOptions` provide multi-backend error granularity
- No `panic!` or `unwrap()` in library code; all errors propagate via `?`

### 5.7 MCP Configuration Types

```rust
pub enum McpServersConfig {
    Dict(HashMap<String, McpServerConfig>),  // Inline configuration
    Path(String),                            // External JSON file path
}

pub enum McpServerConfig {
    Stdio(McpStdioConfig),   // Command-line subprocess (command + args + env)
    Sse(McpSseConfig),       // Remote SSE endpoint (url + headers)
    Http(McpHttpConfig),     // Remote HTTP endpoint (url + headers)
    Sdk(McpSdkConfig),       // In-process server (name + version + tools[])
}
```

The `Sdk` variant is special: only `name` and `version` are passed to the CLI via `--mcp-config`; the tool handlers remain in the Rust process, invoked via the control protocol's `mcp_message` subtype. This provides zero-IPC overhead for SDK-registered tools.

### 5.8 Permission and Hook Types

```rust
pub type CanUseToolCallback = Arc<
    dyn Fn(String, serde_json::Value, ToolPermissionContext)
        -> Pin<Box<dyn Future<Output = PermissionResult> + Send>>
    + Send + Sync,
>;

pub enum PermissionResult {
    Allow(PermissionResultAllow),  // { updated_input, updated_permissions }
    Deny(PermissionResultDeny),    // { message, interrupt }
}

pub type HookCallback = Arc<
    dyn Fn(serde_json::Value, Option<String>, HookContext)
        -> Pin<Box<dyn Future<Output = Result<HookJSONOutput>> + Send>>
    + Send + Sync,
>;

pub enum HookJSONOutput {
    Async { async_timeout: Option<u64> },
    Sync { continue_: Option<bool>, suppress_output: Option<bool>,
           stop_reason: Option<String>, decision: Option<String>, ... },
}
```

### 5.9 Query Internal Structure

```rust
pub struct Query {
    write_tx: Option<mpsc::Sender<String>>,      // Write channel to CLI stdin
    message_tx: broadcast::Sender<ControlMessage>, // Message broadcast to subscribers
    request_counter: AtomicU64,                    // Lock-free request ID generator
    init_result: RwLock<Option<serde_json::Value>>, // Server info from handshake
}

enum ControlMessage {
    Data(serde_json::Value),  // Parsed JSON message
    End,                      // Stream ended
    Error(String),            // Stream error
}
```

---

## 6. Tech Stack

### 6.1 Core Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| **tokio** | 1.x (full features) | Async runtime, subprocess management, channels, timers |
| **serde** + **serde_json** | 1.x | Serialization/deserialization of all protocol messages |
| **thiserror** | 1.x | Error type definitions with `#[derive(Error)]` |
| **anyhow** | 1.x | Error propagation with context |
| **async-trait** | 0.1 | Object-safe async traits (`Transport`, `Backend`, `Session`) |
| **tracing** | 0.1 | Structured logging and diagnostics |
| **futures** | 0.3 | `Stream` trait, `StreamExt` for stream iteration |
| **tokio-stream** | 0.1 | Stream adapters for tokio |
| **async-stream** | 0.3 | `stream!` macro for lazy stream construction |

### 6.2 Platform-Specific Dependencies

| Dependency | Platform | Purpose |
|------------|----------|---------|
| **nix** | `cfg(unix)` v~0.29 | Resolve username to UID for subprocess user switching |

### 6.3 Dev Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| **tokio-test** | 0.4 | Async test utilities |
| **mockall** | 0.12 | Mock trait implementations for testing |

### 6.4 Why These Choices

| Concern | Choice | Alternative Considered | Rationale |
|---------|--------|----------------------|-----------|
| Async Runtime | Tokio | async-std | Tokio has better subprocess support, broader ecosystem |
| Error Handling | thiserror + anyhow | eyre | thiserror for library types, anyhow for context chaining |
| Async Traits | async-trait crate | Native async fn in traits | Traits require object safety (`dyn Backend`, `dyn Session`); native async fn doesn't support `dyn` dispatch |
| Stream Construction | async-stream macro | Manual `Stream` impl | Macro eliminates boilerplate for complex stateful streams |
| Message Distribution | broadcast channel | mpsc with Arc | broadcast supports multiple concurrent subscribers without shared state |

---

## 7. Interface Design

### 7.1 Public API Surface

#### One-Shot Query

```rust
pub fn query(
    prompt: impl Into<Prompt> + Send + 'static,
    options: Option<AgentOptions>,
) -> impl Stream<Item = Result<Message>> + Send
```

- **Synchronous construction, lazy evaluation**: `query()` itself is not async; subprocess spawn happens when the stream is consumed
- **Prompt dual mode**: `Prompt::Text` for simple queries; `Prompt::Stream` for bidirectional streaming (required for `can_use_tool` callback)
- **Errors inlined in stream**: Errors appear as `Err` variants in the stream; consumers can skip or terminate

#### AgentSdkClient (Multi-Turn)

```rust
pub struct AgentSdkClient {
    options: AgentOptions,
    custom_transport: Option<Box<dyn Transport + Send>>,
    backend: Box<dyn Backend>,
    session: Option<Box<dyn Session + Send>>,
}

impl AgentSdkClient {
    pub fn new(options: Option<AgentOptions>,
               custom_transport: Option<Box<dyn Transport + Send>>) -> Self;
    pub async fn connect(&mut self, prompt: Option<Prompt>) -> Result<()>;
    pub async fn query(&mut self, prompt: impl Into<Prompt>, session_id: &str) -> Result<()>;
    pub fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>>;
    pub fn receive_response(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>>;
    pub async fn interrupt(&mut self) -> Result<()>;
    pub async fn set_permission_mode(&mut self, mode: &str) -> Result<()>;
    pub async fn set_model(&mut self, model: Option<&str>) -> Result<()>;
    pub async fn rewind_files(&mut self, user_message_id: &str) -> Result<()>;
    pub async fn get_mcp_status(&mut self) -> Result<serde_json::Value>;
    pub async fn get_server_info(&self) -> Result<Option<serde_json::Value>>;
    pub async fn disconnect(&mut self) -> Result<()>;
}
```

**Design choices**:
- `connect()` / `query()` separation: establish connection once, send multiple queries
- Dual stream views: `receive_messages()` returns all messages (monitoring); `receive_response()` stops at `ResultMessage` (per-turn consumption)
- Control methods (`interrupt()`, `set_model()`, etc.) are orthogonal to message streams
- Capability-gated methods check `Backend::capabilities()` before execution, returning `Error::UnsupportedFeature` for unsupported backends

#### Backend Trait

```rust
#[async_trait]
pub trait Backend: Send + Sync + fmt::Debug {
    fn capabilities(&self) -> &Capabilities;
    fn name(&self) -> &str;
    fn validate_options(&self, options: &AgentOptions) -> Result<()>;
    fn one_shot_query(&self, prompt: Prompt, options: &AgentOptions)
        -> Result<Pin<Box<dyn Stream<Item = Result<Message>> + Send>>>;
    async fn create_session(&self, options: &AgentOptions, prompt: Option<Prompt>)
        -> Result<Box<dyn Session + Send>>;
}
```

#### Session Trait

```rust
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

#### Transport Trait

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn write(&mut self, data: &str) -> Result<()>;
    fn read_messages(&mut self) -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>>;
    async fn close(&mut self) -> Result<()>;
    fn is_ready(&self) -> bool;
    async fn end_input(&mut self) -> Result<()>;
}
```

**Design choices**:
- Object safety via `async_trait`: enables `Box<dyn Transport + Send>` for runtime polymorphism and test mock injection
- `read_messages()` returns an owned stream (no lifetime bound to `&self`): allows the read stream to be consumed in a separate task while `write()`/`close()` run concurrently, avoiding borrow checker conflicts
- `end_input()` / `close()` two-phase shutdown: `end_input()` closes stdin (CLI can finish processing); `close()` force-terminates the process

### 7.2 Capability Gating in AgentSdkClient

| Method | Required Capability | Error on Mismatch |
|--------|-------------------|-------------------|
| `interrupt()` | `interrupt` | `UnsupportedFeature { feature: "interrupt", backend }` |
| `set_permission_mode()` | `runtime_config_changes` | `UnsupportedFeature { feature: "set_permission_mode", backend }` |
| `set_model()` | `runtime_config_changes` | `UnsupportedFeature { feature: "set_model", backend }` |
| `rewind_files()` | `control_protocol` | `UnsupportedFeature { feature: "rewind_files", backend }` |
| `get_mcp_status()` | `control_protocol` | `UnsupportedFeature { feature: "get_mcp_status", backend }` |

### 7.3 Options Validation Per Backend

| Option | Claude | Codex | Cursor |
|--------|--------|-------|--------|
| `system_prompt` | Supported | Rejected | Rejected |
| `can_use_tool` | Supported | Supported (mapped) | Rejected |
| `hooks` | Supported | Rejected | Rejected |
| `mcp_servers` | Supported | Supported | Rejected |
| `fork_session` | Supported | Rejected | Rejected |
| `setting_sources` | Supported | Rejected | Rejected |
| `plugins` | Supported | Rejected | Rejected |
| `permission_prompt_tool_name` | Supported | Rejected | Rejected |
| `output_format` (structured) | Supported | Supported | Rejected |
| `model` | Supported | Supported | Supported |
| `max_turns` | Supported | Supported | Supported |

### 7.4 Extensibility Points

| Extension Point | Mechanism | Typical Use |
|-----------------|-----------|-------------|
| Custom Transport | `impl Transport` | Test mocks, WebSocket communication |
| Tool Permission | `CanUseToolCallback` | Dynamic tool call approval/modification |
| Lifecycle Hooks | `HookCallback` per `HookEvent` | Logging, monitoring, input rewriting |
| In-Process Tools | `SdkMcpTool` + `SdkMcpToolHandler` | High-performance built-in tools |
| System Prompt | `SystemPromptConfig` | Fixed or appended prompts |
| stderr Observation | `StderrCallback` | Debug output, log forwarding |

---

## 8. Feature Compatibility Matrix

| Feature | Claude | Codex | Cursor |
|---------|--------|-------|--------|
| `query()` one-shot | Yes | Yes | Yes |
| Multi-turn session | Yes (stdin stream) | Yes (app-server) | Yes (spawn-per-turn) |
| `can_use_tool` callback | Yes | Yes (mapped to approval) | No |
| Hooks | Yes | No | No |
| SDK MCP tools | Yes | No | No |
| Model selection | Yes | Yes | Yes |
| System prompt | Yes | No | No |
| `interrupt()` | Yes | Yes (`turn/interrupt`) | No |
| `set_model()` / `set_permission_mode()` | Yes | No | No |
| Structured output | Yes | Yes (`--output-schema`) | No |
| Session resume | Yes | Yes (`thread/resume`) | Yes (`--resume chatId`) |

---

## 9. Module Structure

```
code-agent-sdk/
├── Cargo.toml
├── src/
│   ├── lib.rs                              # Public API: query(), create_sdk_mcp_server(), sdk_mcp_tool()
│   ├── client.rs                           # AgentSdkClient (multi-turn, capability-gated)
│   ├── options.rs                          # AgentOptions + Builder, CodexOptions, CursorOptions
│   ├── types.rs                            # Message, ContentBlock, Prompt
│   ├── error.rs                            # Error enum, Result type alias
│   ├── backend/
│   │   ├── mod.rs                          # Backend + Session traits, Capabilities, BackendKind
│   │   ├── claude/
│   │   │   ├── mod.rs                      # ClaudeBackend, ClaudeSession
│   │   │   ├── cli_finder.rs              # CLI binary discovery and version check
│   │   │   ├── command_builder.rs         # AgentOptions → CLI argument list
│   │   │   ├── transport.rs              # ClaudeCliTransport (subprocess stdin/stdout)
│   │   │   └── message_parser.rs         # Claude stream-json → Message
│   │   ├── codex/
│   │   │   ├── mod.rs                     # CodexBackend
│   │   │   ├── exec_transport.rs         # One-shot: codex exec --json
│   │   │   ├── app_server.rs             # Multi-turn: codex app-server (JSON-RPC 2.0)
│   │   │   ├── message_parser.rs         # Codex events → Message
│   │   │   └── jsonrpc.rs                # JSON-RPC 2.0 request/response helpers
│   │   └── cursor/
│   │       ├── mod.rs                     # CursorBackend
│   │       ├── transport.rs              # One-shot: agent --print
│   │       ├── session.rs                # Spawn-per-turn session (chatId tracking)
│   │       └── message_parser.rs         # Cursor events → Message
│   ├── internal/
│   │   ├── mod.rs                         # Re-exports
│   │   ├── client.rs                      # InternalClient (backend routing for query())
│   │   ├── query.rs                       # Query (control protocol, hooks, can_use_tool, MCP)
│   │   └── message_parser.rs             # Claude message parser (re-export from backend)
│   └── transport/
│       ├── mod.rs                         # Transport trait
│       └── subprocess_cli.rs             # Legacy subprocess transport
├── examples/
│   ├── quick_start.rs                     # Basic one-shot query
│   ├── streaming_mode.rs                  # Streaming mode examples
│   ├── mcp_calculator.rs                  # In-process MCP tool
│   ├── hooks.rs                           # Hook examples (API demo)
│   ├── system_prompt.rs                   # System prompt configuration
│   ├── tools_option.rs                    # Tool filtering
│   └── tool_permission_callback.rs        # can_use_tool callback
├── tests/
│   ├── test_integration.rs               # Integration tests
│   ├── test_e2e.rs                       # E2E tests (require API key)
│   ├── test_types.rs                     # Type construction tests
│   └── test_transport.rs                 # Transport tests
├── fixtures/
│   └── code-agent-sdk/                   # Fixture-based test runner
│       └── src/fixtures/                 # Parameterized tests across all backends
└── docs/
    ├── arch-rust.md                      # Original Claude-only design document
    ├── multi-backend-design.md           # Multi-backend implementation plan
    └── architecture.md                   # This document
```

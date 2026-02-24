# Code Agent SDK (Rust) 设计方案

> 基于 `vendors/claude-agent-sdk-python` 功能对等设计，使用 Rust 重新实现。最后更新：2025-02

## 1. 设计目标

- **功能对等**：与 Python SDK 保持 100% 功能一致
- **协议兼容**：与 Claude Code CLI 的 JSON 行协议、控制协议完全兼容
- **Rust 惯用**：符合 Rust 生态与惯用写法
- **可扩展**：支持自定义 Transport、Hooks、MCP 服务器

---

## 2. 技术栈选型

### 2.1 核心依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| **tokio** | 1.x | 异步运行时、进程、通道 |
| **serde** | 1.x | 序列化/反序列化 |
| **serde_json** | 1.x | JSON 解析 |
| **thiserror** | 1.x | 错误类型定义 |
| **anyhow** | 1.x | 错误传播 |
| **tracing** | 0.1 | 日志 |
| **async-trait** | 0.1 | 异步 trait 方法 |

### 2.2 可选依赖

| 依赖 | 用途 |
|------|------|
| **mcp** | 若存在 Rust MCP 实现，用于 SDK MCP Server；否则需自实现 JSON-RPC 工具协议 |
| **futures** | `Stream`、`StreamExt` 用于流式迭代 |
| **tokio-stream** | 流式 API 辅助 |

### 2.3 替代方案说明

| Python | Rust 对应 |
|--------|-----------|
| anyio | tokio |
| anyio.open_process | tokio::process::Command |
| anyio.create_task_group | tokio::task::JoinSet / tokio::spawn |
| anyio.create_memory_object_stream | tokio::sync::mpsc |
| AsyncIterator | impl Stream<Item = T> |
| async def / Awaitable | async fn / impl Future |
| Callable / Fn | impl Fn / FnOnce / dyn Fn |
| TypedDict / dataclass | struct + serde |

---

## 3. 模块结构

```
code-agent-sdk/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # 公开 API、query()、create_sdk_mcp_server
│   ├── client.rs              # ClaudeSdkClient
│   ├── options.rs             # ClaudeAgentOptions、Builder、类型定义
│   ├── types.rs               # Message、ContentBlock、枚举等
│   ├── error.rs               # 错误类型
│   ├── internal/
│   │   ├── mod.rs
│   │   ├── client.rs          # InternalClient
│   │   ├── query.rs           # Query（控制协议、hooks、can_use_tool）
│   │   └── message_parser.rs  # parse_message
│   └── transport/
│       ├── mod.rs             # Transport trait
│       └── subprocess_cli.rs   # SubprocessCliTransport（含 stderr 回调）
└── examples/
    ├── quick_start.rs
    ├── streaming_mode.rs
    ├── mcp_calculator.rs
    ├── hooks.rs
    └── ...
```

---

## 4. 核心接口设计

### 4.1 query() 函数

```rust
/// 一次性查询，返回消息流
pub async fn query(
    prompt: Prompt<'_>,
    options: Option<ClaudeAgentOptions>,
    transport: Option<impl Transport>,
) -> Result<impl Stream<Item = Result<Message>>>
```

**Prompt 类型**（支持字符串与流式）：

```rust
pub enum Prompt<'a> {
    /// 单次字符串
    OneShot(&'a str),
    /// 流式输入
    Stream(impl Stream<Item = UserMessageInput>),
}
```

**返回**：`impl Stream<Item = Result<Message>>`，使用 `futures::Stream` 或 `async_stream::stream!`

### 4.2 ClaudeSdkClient

```rust
pub struct ClaudeSdkClient {
    options: ClaudeAgentOptions,
    transport: Option<Box<dyn Transport>>,
    // 内部状态
}

impl ClaudeSdkClient {
    pub fn new(options: Option<ClaudeAgentOptions>) -> Self;
    pub fn with_transport(transport: Box<dyn Transport>) -> Self;

    pub async fn connect(&mut self, prompt: Option<Prompt<'_>>) -> Result<()>;
    pub async fn query(&mut self, prompt: impl Into<QueryInput>, session_id: &str) -> Result<()>;
    pub fn receive_messages(&mut self) -> impl Stream<Item = Result<Message>>;
    pub fn receive_response(&mut self) -> impl Stream<Item = Result<Message>>;  // 到 ResultMessage 为止
    pub async fn interrupt(&mut self) -> Result<()>;
    pub async fn set_permission_mode(&mut self, mode: &str) -> Result<()>;
    pub async fn set_model(&mut self, model: Option<&str>) -> Result<()>;
    pub async fn rewind_files(&mut self, user_message_id: &str) -> Result<()>;
    pub async fn get_mcp_status(&mut self) -> Result<serde_json::Value>;
    pub fn get_server_info(&self) -> Option<&serde_json::Value>;
    pub async fn disconnect(&mut self) -> Result<()>;
}

impl Drop for ClaudeSdkClient {
    fn drop(&mut self) { /* 确保 disconnect */ }
}
```

### 4.3 Transport trait

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn write(&mut self, data: &str) -> Result<()>;
    fn read_messages(&mut self) -> Pin<Box<dyn Stream<Item = Result<serde_json::Value>> + Send + '_>>;
    async fn close(&mut self) -> Result<()>;
    fn is_ready(&self) -> bool;
    async fn end_input(&mut self) -> Result<()>;
}
```

### 4.4 工具与 MCP 服务器

```rust
/// 工具定义（类似 Python @tool）
pub struct SdkMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: Arc<dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<ToolResult>> + Send>> + Send + Sync>,
}

/// 工具结果
pub struct ToolResult {
    pub content: Vec<ContentItem>,
    pub is_error: Option<bool>,
}

/// 创建 SDK MCP 服务器
pub fn create_sdk_mcp_server(
    name: &str,
    version: &str,
    tools: Vec<SdkMcpTool>,
) -> McpSdkServerConfig;

/// 声明式宏简化工具定义
#[macro_export]
macro_rules! tool {
    ($name:expr, $desc:expr, $schema:expr, $handler:expr) => { ... };
}
```

---

## 5. 数据结构设计

### 5.1 消息类型 (Message)

```rust
#[derive(Debug, Clone)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "user")]
    User(UserMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "system")]
    System(SystemMessage),
    #[serde(rename = "result")]
    Result(ResultMessage),
    #[serde(rename = "stream_event")]
    StreamEvent(StreamEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub content: UserContent,
    pub uuid: Option<String>,
    pub parent_tool_use_id: Option<String>,
    pub tool_use_result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub parent_tool_use_id: Option<String>,
    pub error: Option<AssistantMessageError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMessage {
    pub subtype: String,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub is_error: bool,
    pub num_turns: u32,
    pub session_id: String,
    pub total_cost_usd: Option<f64>,
    pub usage: Option<serde_json::Value>,
    pub result: Option<String>,
    pub structured_output: Option<serde_json::Value>,
}

// SystemMessage, StreamEvent 类似
```

### 5.2 内容块 (ContentBlock)

```rust
#[derive(Debug, Clone)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text(TextBlock),
    #[serde(rename = "thinking")]
    Thinking(ThinkingBlock),
    #[serde(rename = "tool_use")]
    ToolUse(ToolUseBlock),
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlock {
    pub thinking: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseBlock {
    pub id: String,
    pub name: String,
    #[serde(rename = "input")]
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    pub tool_use_id: String,
    pub content: Option<serde_json::Value>,
    pub is_error: Option<bool>,
}
```

### 5.3 ClaudeAgentOptions

```rust
#[derive(Debug, Clone, Default)]
pub struct ClaudeAgentOptions {
    pub tools: Option<ToolsConfig>,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub mcp_servers: McpServersConfig,
    pub system_prompt: Option<SystemPromptConfig>,
    pub permission_mode: Option<PermissionMode>,
    pub can_use_tool: Option<CanUseToolCallback>,
    pub hooks: Option<HashMap<HookEvent, Vec<HookMatcher>>>,
    pub model: Option<String>,
    pub fallback_model: Option<String>,
    pub max_turns: Option<u32>,
    pub max_budget_usd: Option<f64>,
    pub continue_conversation: bool,
    pub resume: Option<String>,
    pub cwd: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub agents: Option<HashMap<String, AgentDefinition>>,
    pub plugins: Vec<SdkPluginConfig>,
    pub sandbox: Option<SandboxSettings>,
    pub thinking: Option<ThinkingConfig>,
    pub output_format: Option<serde_json::Value>,
    pub enable_file_checkpointing: bool,
    pub env: HashMap<String, String>,
    pub extra_args: HashMap<String, Option<String>>,
    pub max_buffer_size: Option<usize>,
    pub permission_prompt_tool_name: Option<String>,
    pub setting_sources: Option<Vec<SettingSource>>,
    pub add_dirs: Vec<PathBuf>,
    pub betas: Vec<String>,
    pub effort: Option<EffortLevel>,
    pub fork_session: bool,
    pub include_partial_messages: bool,
    // 更多字段...
}

impl ClaudeAgentOptions {
    pub fn builder() -> ClaudeAgentOptionsBuilder;
}

pub struct ClaudeAgentOptionsBuilder { ... }
impl ClaudeAgentOptionsBuilder {
    pub fn allowed_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self;
    pub fn mcp_servers(mut self, servers: impl Into<McpServersConfig>) -> Self;
    pub fn hooks(mut self, hooks: HashMap<HookEvent, Vec<HookMatcher>>) -> Self;
    pub fn build(self) -> ClaudeAgentOptions;
}
```

### 5.4 MCP 服务器配置

```rust
#[derive(Debug, Clone)]
pub enum McpServerConfig {
    Stdio(McpStdioConfig),
    Sdk(McpSdkConfig),
}
// 注：SSE/Http 类型暂未实现，仅 Stdio/Sdk

#[derive(Debug, Clone, Default)]
pub struct McpStdioConfig {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,  // 已支持，传入 mcp-config JSON
}

#[derive(Debug, Clone)]
pub struct McpSdkConfig {
    pub name: String,
}
```

### 5.5 Hook 类型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
    Stop,
    SubagentStop,
    PreCompact,
    Notification,
    SubagentStart,
    PermissionRequest,
}

#[derive(Debug, Clone)]
pub struct HookMatcher {
    pub matcher: Option<String>,
    pub hooks: Vec<HookCallback>,
    pub timeout: Option<f64>,
}

/// 异步 Hook 回调：Rust 中 async closure 不稳定，使用 Fn 返回 Future 的模式
pub type HookCallback = Arc<
    dyn Fn(HookInput, Option<String>, HookContext)
        -> Pin<Box<dyn Future<Output = Result<HookJSONOutput>> + Send>>
    + Send
    + Sync,
>;

#[derive(Debug, Clone)]
#[serde(tag = "hook_event_name")]
pub enum HookInput {
    PreToolUse(PreToolUseHookInput),
    PostToolUse(PostToolUseHookInput),
    PostToolUseFailure(PostToolUseFailureHookInput),
    UserPromptSubmit(UserPromptSubmitHookInput),
    // ...
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum HookJSONOutput {
    Async { r#async: bool, async_timeout: Option<u64> },
    Sync {
        #[serde(rename = "continue")]
        continue_: Option<bool>,
        suppress_output: Option<bool>,
        stop_reason: Option<String>,
        decision: Option<String>,
        system_message: Option<String>,
        reason: Option<String>,
        hook_specific_output: Option<serde_json::Value>,
    },
}
```

---

## 6. 控制协议实现

### 6.1 请求/响应类型

```rust
#[derive(Debug, Serialize)]
pub struct ControlRequest {
    pub type_: String,  // "control_request"
    pub request_id: String,
    pub request: ControlRequestPayload,
}

#[derive(Debug, Serialize)]
#[serde(tag = "subtype")]
pub enum ControlRequestPayload {
    Initialize { hooks: Option<serde_json::Value>, agents: Option<serde_json::Value> },
    Interrupt,
    SetPermissionMode { mode: String },
    SetModel { model: Option<String> },
    RewindFiles { user_message_id: String },
    McpStatus,
}

#[derive(Debug, Deserialize)]
pub struct ControlResponse {
    pub type_: String,
    pub response: ControlResponsePayload,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "subtype")]
pub enum ControlResponsePayload {
    Success { request_id: String, response: Option<serde_json::Value> },
    Error { request_id: String, error: String },
}
```

### 6.2 控制请求处理

- CLI 收到 `control_request` 后，SDK 处理 `control_request`（incoming）：
  - `can_use_tool`：调用 `can_use_tool` 回调，`PermissionResultAllow` 的 `updated_permissions` 通过 `PermissionUpdate::to_control_protocol_value()` 序列化为 `updatedPermissions`
  - `hook_callback`：根据 `callback_id` 在 `build_hook_callbacks()` 构建的 map 中查找并执行 Hook
  - `mcp_message`：路由到 SDK MCP Server（当前返回占位错误）

### 6.3 initialize 中的 hooks 配置

- `Query::initialize()` 调用 `build_hooks_config_for_initialize()` 构建 `hooks_config`，包含各事件的 `hookCallbackIds`（hook_0, hook_1, ...），与 `build_hook_callbacks()` 的 ID 分配一致，确保 CLI 发出的 `hook_callback` 能正确路由

### 6.4 Hook 输出字段名转换

Python 使用 `async_`、`continue_` 避免关键字冲突，CLI 期望 `async`、`continue`。Rust 需在序列化时用 `#[serde(rename = "async")]` 等处理。

---

## 7. SubprocessCliTransport 实现

### 7.1 CLI 查找逻辑

```rust
fn find_cli(options: &ClaudeAgentOptions) -> Result<PathBuf> {
    if let Some(p) = &options.cli_path {
        return Ok(p.clone());
    }
    if let Some(bundled) = find_bundled_cli() {
        return Ok(bundled);
    }
    if let Some(p) = which("claude") {
        return Ok(p);
    }
    for path in [
        dirs::home_dir().unwrap().join(".npm-global/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        dirs::home_dir().unwrap().join(".local/bin/claude"),
        dirs::home_dir().unwrap().join("node_modules/.bin/claude"),
        dirs::home_dir().unwrap().join(".yarn/bin/claude"),
        dirs::home_dir().unwrap().join(".claude/local/claude"),
    ] {
        if path.exists() {
            return Ok(path);
        }
    }
    Err(Error::CliNotFound(...))
}
```

### 7.2 命令构建

与 Python 的 `_build_command()` 一一对应：

- `--output-format stream-json`
- `--input-format stream-json`
- `--verbose`
- `--system-prompt` / `--append-system-prompt`
- `--tools` / `--allowedTools` / `--disallowedTools`
- `--max-turns` / `--max-budget-usd`
- `--model` / `--fallback-model`
- `--permission-mode` / `--permission-prompt-tool`
- `--mcp-config` (JSON)
- `--settings` / `--add-dir` / `--plugin-dir`
- `--max-thinking-tokens` / `--effort`
- `--json-schema` (from output_format)
- `--extra-args` 映射

### 7.3 进程与 I/O

```rust
pub struct SubprocessCliTransport {
    options: ClaudeAgentOptions,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    ready: bool,
    max_buffer_size: usize,
}

impl SubprocessCliTransport {
    pub async fn connect(&mut self) -> Result<()> {
        // stderr: 仅当 options.stderr 或 debug-to-stderr 时管道，否则 Stdio::null()
        let should_pipe_stderr = self.options.stderr.is_some()
            || self.options.extra_args.contains_key("debug-to-stderr");
        let stderr_dest = if should_pipe_stderr { Stdio::piped() } else { Stdio::null() };

        let mut cmd = Command::new(&self.cli_path);
        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(stderr_dest)
           .env("CLAUDE_CODE_ENTRYPOINT", "sdk-rs")
           .env("CLAUDE_AGENT_SDK_VERSION", env!("CARGO_PKG_VERSION"));
        // ... 应用 options
        let mut child = cmd.spawn()?;
        self.stdin = child.stdin.take();
        self.stdout = child.stdout.take();

        // 若有 stderr 回调，spawn 任务逐行读取并调用
        if should_pipe_stderr {
            if let Some(stderr) = child.stderr.take() {
                let cb = self.options.stderr.clone();
                tokio::spawn(async move {
                    let mut lines = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let s = line.trim_end();
                        if !s.is_empty() { if let Some(ref f) = cb { f(s); } }
                    }
                });
            }
        }
        self.process = Some(child);
        self.ready = true;
        Ok(())
    }

    fn read_messages(&mut self) -> impl Stream<Item = Result<serde_json::Value>> {
        // 按行读取 stdout，解析 JSON
    }
}
```

---

## 8. 错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Claude Code not found: {0}")]
    CliNotFound(String),

    #[error("Connection error: {0}")]
    Connection(#[from] std::io::Error),

    #[error("Process failed with exit code {0}")]
    Process { exit_code: i32, stderr: Option<String> },

    #[error("JSON decode error: {0}")]
    JsonDecode(#[from] serde_json::Error),

    #[error("Message parse error: {0}")]
    MessageParse(String),

    #[error("Control request timeout: {0}")]
    ControlTimeout(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

---

## 9. 数据流（与 Python 一致）

### 9.1 query() 流程

```
User prompt
    → InternalClient::process_query
    → SubprocessCliTransport::connect
    → Query::start (spawn read_messages task)
    → Query::initialize (send control_request, wait response)
    → write user message / stream_input
    → Query::receive_messages
        → parse_message → yield Message
```

### 9.2 ClaudeSdkClient 流程

```
connect()
    → SubprocessCliTransport::connect
    → Query::start + Query::initialize
    → [if prompt] spawn stream_input

query() / receive_response()
    → transport.write(JSON line)
    → receive_messages → parse_message → yield
```

### 9.3 控制协议

```
CLI → control_request (hook_callback / mcp_message)
    → Query::handle_control_request
    → callback / mcp handler
    → transport.write(control_response)
```

---

## 10. 功能对等清单

| 功能 | Python | Rust 实现 |
|------|--------|-----------|
| query() | ✓ | `query()` async fn |
| ClaudeSDKClient | ✓ | `ClaudeSdkClient` struct |
| Transport | ✓ | `Transport` trait |
| SubprocessCLITransport | ✓ | `SubprocessCliTransport` |
| 控制协议 | ✓ | `Query` 内部实现 |
| Hooks | ✓ | `HookMatcher` + `HookCallback` |
| can_use_tool | ✓ | `CanUseToolCallback` |
| SDK MCP Server | ✓ | `create_sdk_mcp_server` + `SdkMcpTool` |
| Stdio MCP (含 env) | ✓ | `McpStdioConfig` |
| SSE/HTTP MCP | ✓ | ✗ 暂未实现 |
| MCP tools/list, tools/call | ✓ | 手动 JSON-RPC 路由 |
| 消息类型 | ✓ | `Message` 枚举 |
| ContentBlock | ✓ | `ContentBlock` 枚举 |
| ClaudeAgentOptions | ✓ | `ClaudeAgentOptions` + Builder |
| 错误类型 | ✓ | `Error` 枚举 |
| 流式输入 | ✓ | `impl Stream` |
| 流式输出 | ✓ | `impl Stream<Item = Result<Message>>` |
| interrupt | ✓ | `interrupt()` |
| set_permission_mode | ✓ | `set_permission_mode()` |
| set_model | ✓ | `set_model()` |
| rewind_files | ✓ | `rewind_files()` |
| get_mcp_status | ✓ | `get_mcp_status()` |
| get_server_info | ✓ | `get_server_info()` |
| 自定义 Transport | ✓ | `impl Transport` |

---

## 11. 实现状态（当前）

- **Phase 1–3**：✓ 已完成（核心类型、Transport、query、ClaudeSdkClient、控制协议、Hooks、can_use_tool、stderr 回调、McpStdioConfig.env、PermissionResultAllow.updatedPermissions）
- **Phase 4**：SDK MCP Server 占位，tool 宏未实现
- **Phase 5**：ClaudeAgentOptions 主要字段已支持，SSE/Http MCP 未实现

---

## 12. 附录：Cargo.toml 示例

```toml
[package]
name = "code-agent-sdk"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
async-trait = "0.1"
tracing = "0.1"
futures = "0.3"
tokio-stream = "0.1"
```

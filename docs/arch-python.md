# Claude Agent SDK Python 架构设计文档

> 基于 `vendors/claude-agent-sdk-python` 代码分析整理，作为 Rust SDK 的参考实现

## 1. 架构设计

### 1.1 整体架构

SDK 采用**分层架构**，自上而下分为：

```
┌─────────────────────────────────────────────────────────────────┐
│                     Public API Layer (公开 API)                    │
│  query()  │  ClaudeSDKClient  │  tool()  │  create_sdk_mcp_server() │
└─────────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│                   Internal Layer (内部实现层)                      │
│  InternalClient  │  Query (控制协议)  │  Message Parser            │
└─────────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│                    Transport Layer (传输层)                        │
│  Transport (抽象)  │  SubprocessCLITransport (CLI 子进程)          │
└─────────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│                    Claude Code CLI (外部进程)                      │
│  --output-format stream-json  │  --input-format stream-json      │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 核心组件

| 组件 | 职责 | 文件位置 |
|------|------|----------|
| **query()** | 一次性查询入口，无状态、单向流式 | `query.py` |
| **ClaudeSDKClient** | 双向交互客户端，支持多轮对话、中断、Hooks | `client.py` |
| **InternalClient** | 内部统一查询处理，协调 Transport 与 Query | `_internal/client.py` |
| **Query** | 控制协议处理：初始化、Hooks、MCP、权限回调 | `_internal/query.py` |
| **Transport** | 抽象 I/O 接口，支持自定义实现 | `_internal/transport/` |
| **SubprocessCLITransport** | 通过子进程与 Claude Code CLI 通信 | `_internal/transport/subprocess_cli.py` |
| **Message Parser** | 将 CLI 原始 JSON 解析为类型化 Message | `_internal/message_parser.py` |

### 1.3 两种使用模式

| 模式 | 入口 | 特点 | 适用场景 |
|------|------|------|----------|
| **query()** | `query(prompt=..., options=...)` | 单向、无状态、一次性 | 简单问答、批处理、CI/CD |
| **ClaudeSDKClient** | `async with ClaudeSDKClient(options) as client:` | 双向、有状态、可中断 | 聊天应用、交互式调试、多轮对话 |

### 1.4 控制协议 (Control Protocol)

SDK 与 Claude Code CLI 通过 **JSON 行协议** 进行双向通信，支持：

- **control_request**：SDK → CLI 的请求（初始化、中断、权限模式、模型切换等）
- **control_response**：CLI → SDK 的响应
- **control_request (incoming)**：CLI → SDK 的请求（权限询问、Hook 回调、MCP 消息）

控制请求类型包括：

- `initialize`：初始化 Hooks、Agents
- `can_use_tool`：工具权限询问（需 `can_use_tool` 回调）
- `hook_callback`：执行 Hook 回调
- `mcp_message`：SDK MCP 工具调用
- `interrupt`：中断执行
- `set_permission_mode`：切换权限模式
- `set_model`：切换模型
- `rewind_files`：文件回滚（需启用 checkpoint）
- `mcp_status`：查询 MCP 连接状态

---

## 2. 接口设计

### 2.1 公开 API

#### query()

```python
async def query(
    *,
    prompt: str | AsyncIterable[dict[str, Any]],
    options: ClaudeAgentOptions | None = None,
    transport: Transport | None = None,
) -> AsyncIterator[Message]
```

- **prompt**：字符串（单次）或异步可迭代（流式输入）
- **options**：可选配置
- **transport**：可选自定义传输实现
- **返回**：`Message` 的异步迭代器

#### ClaudeSDKClient

```python
class ClaudeSDKClient:
    def __init__(self, options=None, transport=None)
    async def connect(self, prompt=None) -> None
    async def query(self, prompt, session_id="default") -> None
    async def receive_messages(self) -> AsyncIterator[Message]
    async def receive_response(self) -> AsyncIterator[Message]  # 到 ResultMessage 为止
    async def interrupt(self) -> None
    async def set_permission_mode(self, mode: str) -> None
    async def set_model(self, model: str | None) -> None
    async def rewind_files(self, user_message_id: str) -> None
    async def get_mcp_status(self) -> dict
    async def get_server_info(self) -> dict | None
    async def disconnect(self) -> None
```

#### 工具与 MCP 服务器

```python
@tool(name, description, input_schema, annotations=None)
def tool_handler(args) -> dict[str, Any]: ...

create_sdk_mcp_server(name, version="1.0.0", tools=None) -> McpSdkServerConfig
```

### 2.2 Transport 抽象接口

```python
class Transport(ABC):
    async def connect(self) -> None
    async def write(self, data: str) -> None
    def read_messages(self) -> AsyncIterator[dict[str, Any]]
    async def close(self) -> None
    def is_ready(self) -> bool
    async def end_input(self) -> None
```

- 支持自定义 Transport 实现（如远程 Claude Code 连接）
- 默认实现为 `SubprocessCLITransport`，通过 stdin/stdout 与 CLI 通信

### 2.3 Hook 回调签名

```python
HookCallback = Callable[
    [HookInput, str | None, HookContext],
    Awaitable[HookJSONOutput]
]
```

- **HookInput**：按事件类型区分的 TypedDict（如 `PreToolUseHookInput`）
- **HookContext**：`{"signal": None}`（预留中止信号）
- **HookJSONOutput**：`SyncHookJSONOutput` 或 `AsyncHookJSONOutput`

---

## 3. 数据流设计

### 3.1 query() 数据流

```
User (prompt)
    │
    ▼
InternalClient.process_query()
    │
    ├─► SubprocessCLITransport.connect()  → 启动 CLI 子进程
    │
    ├─► Query.start()  → 启动 _read_messages 后台任务
    │
    ├─► Query.initialize()  → 发送 control_request (initialize)
    │       │
    │       └─► 等待 control_response
    │
    ├─► 写入 user message (JSON 行) 或 stream_input()
    │
    └─► Query.receive_messages()
            │
            ├─► Transport.read_messages()  → 解析 JSON 行
            │
            ├─► 路由: control_response → pending_control_results
            │         control_request → _handle_control_request
            │         result/assistant/user/system → _message_send
            │
            └─► parse_message(data) → Message
                    │
                    └─► yield Message
```

### 3.2 ClaudeSDKClient 数据流

```
connect(prompt=None)
    │
    ├─► SubprocessCLITransport(prompt=_empty_stream() | prompt)
    │
    ├─► Query(hooks, sdk_mcp_servers, agents)
    │
    ├─► Query.start() + Query.initialize()
    │
    └─► [若有 prompt] Query.stream_input(prompt) 在后台运行

query(prompt) / receive_response()
    │
    ├─► transport.write(JSON 行)  ← 用户消息
    │
    └─► receive_messages()
            │
            └─► parse_message() → yield Message
```

### 3.3 控制协议数据流（Hook / MCP）

```
CLI 发出 control_request (hook_callback / mcp_message)
    │
    ▼
Query._handle_control_request()
    │
    ├─► hook_callback:
    │       callback_id → hook_callbacks[callback_id]
    │       await callback(input, tool_use_id, context)
    │       _convert_hook_output_for_cli()  # async_ → async, continue_ → continue
    │
    └─► mcp_message:
            server_name → sdk_mcp_servers[server_name]
            _handle_sdk_mcp_request()  # 路由 tools/list, tools/call 等
    │
    ▼
transport.write(control_response)
```

### 3.4 消息格式（JSON 行）

**输入（SDK → CLI）：**

```json
{"type": "user", "session_id": "...", "message": {"role": "user", "content": "..."}, "parent_tool_use_id": null}
```

**输出（CLI → SDK）：**

- `user` / `assistant` / `system` / `result` / `stream_event`
- `control_request` / `control_response`

---

## 4. 技术栈

### 4.1 运行时与依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| Python | ≥ 3.10 | 运行时 |
| anyio | ≥ 4.0.0 | 异步 I/O、任务组、进程管理 |
| mcp | ≥ 0.1.0 | MCP 协议、Server、Tool 类型 |
| typing_extensions | ≥ 4.0.0 | Python < 3.11 的 TypedDict 等 |

### 4.2 异步模型

- 使用 **anyio** 作为异步抽象，兼容 asyncio 与 trio
- `anyio.open_process` 管理 CLI 子进程
- `anyio.create_task_group` 管理并发任务（读消息、流式输入）
- `anyio.create_memory_object_stream` 实现消息队列（`_message_send` / `_message_receive`）

### 4.3 CLI 集成

- 默认使用 **bundled CLI**：`_bundled/claude` 或 `_bundled/claude.exe`
- 回退到系统 PATH 中的 `claude`
- 最低 CLI 版本：`2.0.0`
- 通信格式：`--output-format stream-json`，`--input-format stream-json`

---

## 5. 关键数据结构设计

### 5.1 消息类型 (Message)

```python
Message = UserMessage | AssistantMessage | SystemMessage | ResultMessage | StreamEvent
```

| 类型 | 关键字段 | 说明 |
|------|----------|------|
| **UserMessage** | `content`, `uuid`, `parent_tool_use_id`, `tool_use_result` | 用户输入 |
| **AssistantMessage** | `content`, `model`, `parent_tool_use_id`, `error` | 助手回复 |
| **SystemMessage** | `subtype`, `data` | 系统消息 |
| **ResultMessage** | `subtype`, `duration_ms`, `num_turns`, `session_id`, `total_cost_usd`, `usage` | 会话结果 |
| **StreamEvent** | `uuid`, `session_id`, `event`, `parent_tool_use_id` | 流式事件 |

### 5.2 内容块 (ContentBlock)

```python
ContentBlock = TextBlock | ThinkingBlock | ToolUseBlock | ToolResultBlock
```

| 类型 | 字段 | 说明 |
|------|------|------|
| **TextBlock** | `text` | 文本 |
| **ThinkingBlock** | `thinking`, `signature` | 思考过程 |
| **ToolUseBlock** | `id`, `name`, `input` | 工具调用 |
| **ToolResultBlock** | `tool_use_id`, `content`, `is_error` | 工具结果 |

### 5.3 ClaudeAgentOptions（核心配置）

```python
@dataclass
class ClaudeAgentOptions:
    # 工具
    tools: list[str] | ToolsPreset | None
    allowed_tools: list[str]
    disallowed_tools: list[str]
    mcp_servers: dict[str, McpServerConfig] | str | Path

    # 模型与预算
    model: str | None
    fallback_model: str | None
    max_turns: int | None
    max_budget_usd: float | None

    # 系统与权限
    system_prompt: str | SystemPromptPreset | None
    permission_mode: PermissionMode | None  # default | acceptEdits | plan | bypassPermissions
    can_use_tool: CanUseTool | None

    # Hooks
    hooks: dict[HookEvent, list[HookMatcher]] | None

    # 会话
    continue_conversation: bool
    resume: str | None
    cwd: str | Path | None

    # 沙箱
    sandbox: SandboxSettings | None

    # 其他
    cli_path: str | Path | None
    agents: dict[str, AgentDefinition] | None
    plugins: list[SdkPluginConfig]
    thinking: ThinkingConfig | None
    output_format: dict | None
    enable_file_checkpointing: bool
    # ...
```

### 5.4 MCP 服务器配置

```python
McpServerConfig = (
    McpStdioServerConfig |   # 子进程: command, args, env
    McpSSEServerConfig |     # SSE: url, headers
    McpHttpServerConfig |    # HTTP: url, headers
    McpSdkServerConfig       # 进程内: type="sdk", name, instance (MCP Server)
)
```

### 5.5 Hook 相关类型

**Hook 事件：**

```python
HookEvent = "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "UserPromptSubmit" |
            "Stop" | "SubagentStop" | "PreCompact" | "Notification" | "SubagentStart" | "PermissionRequest"
```

**Hook 输入（按事件区分）：**

- `PreToolUseHookInput`: `tool_name`, `tool_input`, `tool_use_id`
- `PostToolUseHookInput`: `tool_name`, `tool_input`, `tool_response`, `tool_use_id`
- `PostToolUseFailureHookInput`: `tool_name`, `tool_input`, `tool_use_id`, `error`
- `UserPromptSubmitHookInput`: `prompt`
- 等等

**Hook 输出：**

```python
HookJSONOutput = AsyncHookJSONOutput | SyncHookJSONOutput

# Async: 延迟执行
AsyncHookJSONOutput = { "async_": True, "asyncTimeout"?: int }

# Sync: 控制执行
SyncHookJSONOutput = {
    "continue_"?: bool,
    "suppressOutput"?: bool,
    "stopReason"?: str,
    "decision"?: "block",
    "systemMessage"?: str,
    "reason"?: str,
    "hookSpecificOutput"?: HookSpecificOutput
}
```

### 5.6 权限控制

```python
PermissionResult = PermissionResultAllow | PermissionResultDeny

PermissionResultAllow: behavior="allow", updated_input?, updated_permissions?
PermissionResultDeny: behavior="deny", message, interrupt?
```

### 5.7 错误类型

```python
ClaudeSDKError (基类)
├── CLIConnectionError
│   └── CLINotFoundError
├── ProcessError
├── CLIJSONDecodeError
└── MessageParseError
```

---

## 6. 扩展点

1. **自定义 Transport**：实现 `Transport` 抽象类，支持远程或自定义通信方式
2. **SDK MCP Server**：通过 `@tool` 和 `create_sdk_mcp_server()` 定义进程内工具
3. **Hooks**：在 PreToolUse、PostToolUse 等事件中注入逻辑
4. **can_use_tool**：自定义工具权限决策（需 `permission_prompt_tool_name="stdio"`）

---

## 7. 附录：文件结构

```
src/claude_agent_sdk/
├── __init__.py          # 公开 API、tool、create_sdk_mcp_server
├── query.py             # query() 入口
├── client.py            # ClaudeSDKClient
├── types.py             # 类型定义
├── _errors.py           # 异常类型
├── _version.py
├── _cli_version.py
└── _internal/
    ├── client.py        # InternalClient
    ├── query.py         # Query（控制协议）
    ├── message_parser.py # parse_message
    └── transport/
        ├── __init__.py  # Transport 抽象
        └── subprocess_cli.py  # SubprocessCLITransport（含 stderr 回调）
```

---

## 8. 与 Rust SDK 关系

Rust 实现（`code-agent-sdk`）以本文档为参考，目标功能对等。详见 `docs/arch-rust.md` 与 `docs/python-rust-feature-comparison.md`。

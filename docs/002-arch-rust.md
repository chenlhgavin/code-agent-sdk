# Code Agent SDK (Rust) 设计方案

> 基于 `vendors/claude-agent-sdk-python` 功能对等设计，使用 Rust 重新实现。最后更新：2026-02

## 1. 设计目标

- **功能对等**：与 Python SDK 保持 100% 功能一致
- **协议兼容**：与 Claude Code CLI 的 JSON 行协议、控制协议完全兼容
- **Rust 惯用**：符合 Rust 生态与惯用写法
- **可扩展**：支持自定义 Transport、Hooks、MCP 服务器

---

## 2. 架构设计

### 2.1 分层架构

SDK 整体分为四层，每层职责单一，向下依赖：

```
┌──────────────────────────────────────────────────────────┐
│                      用户应用层                            │
│   调用 query() / ClaudeSdkClient / create_sdk_mcp_server │
├──────────────────────────────────────────────────────────┤
│                      公开 API 层                           │
│  lib.rs      query()、sdk_mcp_tool()、create_sdk_mcp_server()
│  client.rs   ClaudeSdkClient（会话管理、控制方法）         │
│  options.rs  ClaudeAgentOptions + Builder                 │
│  types.rs    Message、ContentBlock、Prompt                │
├──────────────────────────────────────────────────────────┤
│                     内部逻辑层                             │
│  internal/client.rs    InternalClient（query 驱动）       │
│  internal/query.rs     Query（控制协议、回调路由）         │
│  internal/message_parser.rs  parse_message               │
├──────────────────────────────────────────────────────────┤
│                      传输层                                │
│  transport/mod.rs             Transport trait            │
│  transport/subprocess_cli.rs  SubprocessCliTransport     │
├──────────────────────────────────────────────────────────┤
│                     外部进程层                             │
│         Claude Code CLI 子进程（stdin/stdout JSON 行）    │
└──────────────────────────────────────────────────────────┘
```

### 2.2 组件关系

```
                    ┌─────────────────┐
                    │ ClaudeSdkClient │──── 可选注入 ────► Box<dyn Transport>
                    └────────┬────────┘
                             │ 持有
                             ▼
┌───────────────┐    ┌───────────────────────────────────────┐
│InternalClient │    │                Query                  │
│  process_query│    │  write_tx  ──────────► write_rx       │
│  (query 函数) │    │  (mpsc Sender)         │              │
└──────┬────────┘    │                  [write_task]         │
       │ 创建        │                   ▼                   │
       └────────────►│            transport.write()          │
                     │                                       │
                     │  message_tx ◄── [read_task]           │
                     │  (broadcast)    ▼                     │
                     │            read_stream                │
                     │            ├── control_request → 回调 │
                     │            └── data/end → broadcast   │
                     └───────────────────────────────────────┘
                                       ▲
                                       │ 实现 Transport trait
                              ┌────────┴───────────┐
                              │ SubprocessCliTransport│
                              │  stdin  (write)      │
                              │  stdout (read_messages)│
                              └────────────────────┬─┘
                                                   │ spawn
                                            Claude Code CLI
```

### 2.3 核心设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 并发模型 | Actor（两个后台 Task） | write_task / read_task 各自独立，避免锁争用 |
| 消息分发 | `broadcast::channel` | 支持多订阅者（receive_messages 与 receive_response 并存） |
| 写入通道 | `mpsc::channel` | 单一写入点，关闭通道即触发 stdin 关闭 |
| 流构建 | `async_stream::stream!` 宏 | 惰性执行，无需手动实现 Stream |
| Transport 抽象 | `dyn Transport`（对象安全） | 支持测试时注入 mock，不侵入业务逻辑 |
| 请求追踪 | `AtomicU64` 计数器 | 生成唯一 request_id，无锁开销 |
| 初始化结果 | `RwLock<Option<Value>>` | 一次写入、多次只读，读性能优于 Mutex |

### 2.4 与 Python SDK 的对应关系

| Python 概念 | Rust 实现 | 说明 |
|-------------|-----------|------|
| `async def query()` → `AsyncIterable` | `fn query()` → `impl Stream` | Rust 流是惰性的，不需要 async 构造 |
| `anyio.open_process` | `tokio::process::Command` | 功能等价 |
| `asyncio.create_task` | `tokio::spawn` | 后台任务 |
| `asyncio.Queue` (MPSC) | `tokio::sync::mpsc` | 写入通道 |
| 事件广播 | `tokio::sync::broadcast` | Python 无直接对应，新增能力 |
| `async for msg in stream` | `StreamExt::next().await` | 迭代方式等价 |
| `@tool` 装饰器 | `sdk_mcp_tool()` 函数 | 无法用 proc macro 直接对应，改为构造函数 |

---

## 3. 接口设计

### 3.1 API 层次与使用场景

| 使用场景 | 推荐入口 | 说明 |
|---------|---------|------|
| 一次性问答 | `query()` 函数 | 最简单，自动管理连接生命周期 |
| 多轮对话 / 会话管理 | `ClaudeSdkClient` | 显式控制连接、多次发送、中断等 |
| 自定义 Transport | `ClaudeSdkClient::new(options, custom_transport)` | 用于测试或自定义通信层 |
| 进程内工具服务 | `create_sdk_mcp_server()` + `sdk_mcp_tool()` | 零网络开销，处理器在 Rust 进程内运行 |

### 3.2 query() 接口设计要点

- **同步构造、惰性执行**：`query()` 本身不是 async fn，返回 `impl Stream`，实际的 CLI spawn 和通信发生在流被消费时，符合 Rust 惰性求值惯用
- **Prompt 双模式**：`Prompt::Text` 适合简单问答；`Prompt::Stream` 适合双向流式输入（配合 `can_use_tool` 回调）
- **无 Transport 参数**：一次性查询固定使用 `SubprocessCliTransport`，减少 API 复杂度；自定义 Transport 通过 `ClaudeSdkClient` 注入
- **错误内联流中**：错误作为 `Result<Message>` 的 `Err` 变体流出，调用方可选择跳过或终止，不强制 try-catch 风格

### 3.3 ClaudeSdkClient 接口设计要点

- **connect / query 分离**：`connect()` 建立连接并初始化，之后可多次调用 `query()` 复用同一会话，减少 CLI 启动开销
- **双视图流**：`receive_messages()` 接收全部消息（适合调试/监控）；`receive_response()` 自动在 ResultMessage 处停止（适合单轮问答），两者可同时订阅
- **控制方法与消息流正交**：`interrupt()`、`set_model()` 等通过控制协议实现，不干扰消息流的订阅状态
- **connect 语义细化**：传入 `Prompt::Text` 不自动发送，需后续调用 `query()`；传入 `Prompt::Stream` 启动后台写入任务；传入 `None` 纯粹建立连接

### 3.4 Transport trait 接口设计要点

- **对象安全优先**：使用 `async-trait` crate 而非原生 async fn，使 `Box<dyn Transport + Send>` 成立，允许运行时多态注入
- **read_messages 返回拥有权**：返回 `Pin<Box<dyn Stream + Send>>`（无生命周期绑定），读取流与 write/close 操作可在不同 Task 中并发，规避 borrow checker 对 `&mut self` 的互斥约束
- **end_input / close 二阶段关闭**：`end_input()` 关闭 stdin（通知 CLI 输入结束，CLI 可正常完成处理），`close()` 强制终止进程；语义对应 Python 的 `close_stdin` 与 `kill`

### 3.5 可扩展点设计

| 扩展点 | 机制 | 典型用途 |
|--------|------|---------|
| 自定义 Transport | `impl Transport` | 测试 mock、WebSocket 通信 |
| 工具权限拦截 | `CanUseToolCallback` | 动态审批工具调用 |
| 生命周期 Hook | `HookCallback` per `HookEvent` | 日志、监控、输入改写 |
| 进程内工具服务 | `SdkMcpTool` + `SdkMcpToolHandler` | 高性能内置工具 |
| 系统提示 | `SystemPromptConfig` | 固定提示或追加提示 |
| stderr 观察 | `StderrCallback` | 调试输出、日志转发 |

### 3.6 错误处理设计

- 所有公开方法返回 `Result<T, Error>`，`Error` 枚举覆盖全部失败场景，调用方可精确匹配
- 流式接口将错误内联为 `Result<Message>` 的 `Err` 变体，消费者可逐条处理，不强迫提前终止
- `Error::NotConnected` 作为独立变体，区分"状态错误"与"I/O 错误"
- 库层不使用 `panic!` 或 `unwrap`，所有错误通过 `?` 向上传播，应用层决定终止策略

---

## 4. 数据流设计

### 4.1 一次性查询生命周期（query() 函数）

整体分为三个串行阶段：

**阶段一：连接与初始化（~60ms 超时）**

```
query() 调用
    │
    ├─► find_cli()        ── 按优先级查找 CLI 路径
    ├─► build_command()   ── 拼接 CLI 参数列表
    ├─► Command::spawn()  ── 创建子进程，建立 stdin/stdout 管道
    ├─► tokio::spawn(write_task)  ── 后台：write_rx → transport.write()
    ├─► tokio::spawn(read_task)   ── 后台：stdout → broadcast / 控制处理
    └─► send initialize + await response（60s 超时）
```

**阶段二：发送查询**

```
Prompt::Text   ── write_user_message ──► write_tx
               ── end_input (drop write_tx) ──► stdin EOF

Prompt::Stream ── tokio::spawn(stream_task)
                      └─► 逐条 write ──► write_tx
                          完成后 drop write_tx ──► stdin EOF
```

**阶段三：接收响应**

```
read_task stdout 逐行 JSON 解析
    │
    ├── control_request ──► handle_control_request()（不进入 broadcast）
    │                            ├── can_use_tool ──► CanUseToolCallback ──► control_response
    │                            ├── hook_callback ──► HookCallback ──► control_response
    │                            └── mcp_message  ──► SdkMcpTool.handler ──► control_response
    │
    └── data / end / error ──► message_tx.send()
                                    │
                              broadcast 订阅者
                                    │
                              receive_response()
                                    └─► parse_message() ──► yield Ok(Message)
                                        遇到 ResultMessage ──► 停止迭代
```

**时序图**

```
调用方              Query / InternalClient          Claude CLI
  │                         │                          │
  │─── for msg in query()──►│── spawn child ──────────►│ 启动
  │                         │── send initialize ───────►│
  │                         │◄── init response ─────────│
  │                         │── write user message ────►│
  │                         │── close stdin (EOF) ──────►│
  │◄── Message::System ─────│◄── JSON line ─────────────│
  │◄── Message::Assistant ──│◄── JSON line ─────────────│
  │◄── Message::Result ─────│◄── JSON line ─────────────│ 子进程退出
  │   (流自动结束)            │                          │
```

### 4.2 双向会话生命周期（ClaudeSdkClient）

```
调用方                    ClaudeSdkClient / Query           Claude CLI
  │                               │                            │
  │── connect(None) ─────────────►│── spawn + initialize ─────►│
  │                               │◄── init response ──────────│
  │                               │                            │
  │── query("Q1", sid) ──────────►│── write user message ─────►│ 处理 Q1
  │── for msg in receive_response │◄── Assistant + Result ──────│
  │◄── Message::Assistant ────────│                            │
  │◄── Message::Result ───────────│                            │
  │                               │                            │
  │── query("Q2", sid) ──────────►│── write user message ─────►│ 处理 Q2
  │── for msg in receive_response │◄── Assistant + Result ──────│
  │◄── Message::Assistant ────────│                            │
  │◄── Message::Result ───────────│                            │
  │                               │                            │
  │── disconnect() ──────────────►│── drop write_tx → EOF ────►│ 退出
  │                               │                            │
```

### 4.3 控制协议流（Hook 回调）

```
Claude CLI                read_task              HookCallback["hook_N"]
    │                         │                          │
    │── control_request ─────►│                          │
    │   {subtype:"hook_callback",│                       │
    │    callback_id:"hook_0",│                          │
    │    input:{...}}         │                          │
    │                         │── 按 callback_id 查找 ──►│ 调用
    │                         │◄── HookJSONOutput ────────│
    │                         │── hook_output_to_json()  │
    │◄── control_response ────│── write_tx.send(resp)    │
    │   {subtype:"success",   │                          │
    │    response:{...}}      │                          │
```

### 4.4 SDK MCP 请求路由

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
    │                          │── 按 server_name 查找    │
    │                          │── 按 method 路由         │
    │                          │─────────────────────────►│ (tool.handler)(args)
    │                          │◄──── Result<Value> ───────│
    │                          │── 封装 JSONRPC response  │
    │◄── control_response ─────│── write_tx.send(resp)    │
    │   {mcp_response:{        │                          │
    │     jsonrpc:"2.0",       │                          │
    │     result:{...}}}       │                          │
```

### 4.5 权限回调流（can_use_tool）

```
Claude CLI                  read_task            CanUseToolCallback
    │                           │                       │
    │── control_request ────────►│                       │
    │   {subtype:"can_use_tool", │                       │
    │    tool_name:"Bash",       │                       │
    │    input:{...},            │                       │
    │    permission_suggestions: │                       │
    │      [{type:"addRules"...}]│                       │
    │   }                        │── 解析 suggestions ──►│
    │                            │── (tool, input, ctx) ►│ 调用回调
    │                            │◄── PermissionResult ──│
    │                            │   Allow: updatedInput + updatedPermissions
    │                            │   Deny:  message + interrupt
    │◄── control_response ───────│── 序列化为 {behavior:"allow/deny",...}
```

### 4.6 内部通道架构

```
外部调用                         Query 内部
send_control_request()  ─────► write_tx (mpsc::Sender<String>)
write_user_message()             │
                                 ▼
                          [write_task] tokio::spawn
                          write_rx.recv() → transport.write()
                          通道关闭 → transport.end_input() → transport.close()

                          [read_task] tokio::spawn
                          read_stream.next() ──► match msg_type
                                │
                                ├── "control_request" ──► handle_control_request()
                                │         └── 结果通过 write_tx 写回 CLI
                                │
                                ├── "control_cancel_request" ──► 忽略
                                │
                                └── data / "end" / "error"
                                          ▼
                                    message_tx.send()
                                    (broadcast::Sender)
                                          │
                              ┌───────────┼───────────┐
                              ▼           ▼           ▼
                         subscriber1  subscriber2  ...
                         receive_messages()  receive_response()
```

---

## 5. 关键数据结构设计

### 5.1 消息类型层次

```
Message
 ├── User         内容可为 String（简单）或 ContentBlock 列表（复杂）
 │                含 uuid、parent_tool_use_id 用于树状会话追踪
 ├── Assistant    内容为 ContentBlock 列表，含 model 字段标识使用的模型
 │                含 error 字段标识 API 级错误（限流、认证失败等）
 ├── System       CLI 系统事件（init_start、init_complete 等）
 │                subtype 区分类型，data 保留完整原始字段
 ├── Result       会话终结消息，含 duration_ms、num_turns、session_id
 │                is_error 标识是否异常终止，total_cost_usd 记录费用
 └── StreamEvent  SSE 流式事件，event 字段保留完整原始数据
```

**设计选择**：
- `Message` 不实现 `Serialize`，仅 `Deserialize`，避免内部类型被意外序列化回 CLI 协议
- `UserContent` 为 untagged union（`String` 或 `Blocks`），与 CLI 协议保持一致，同时支持两种输入形态
- `SystemMessage.data` 使用 `#[serde(flatten)]` 保留全部原始字段，确保前向兼容

### 5.2 ContentBlock 类型层次

```
ContentBlock
 ├── Text       纯文本，最常见的输出形态
 ├── Thinking   模型思考过程，含 signature（用于防篡改验证）
 ├── ToolUse    工具调用请求，id 唯一标识，name 为工具名，input 为入参 JSON
 └── ToolResult 工具调用结果，tool_use_id 关联对应的 ToolUse，is_error 标记失败
```

**设计选择**：
- 使用 `#[serde(tag = "type")]` 内嵌标签，与 CLI JSON 协议结构一致，无需额外包装层
- 未知 `type` 值在 `parse_content_block()` 中返回 `Ok(None)`（而非 `Err`），保证前向兼容，新版 CLI 的新 block 类型不会导致旧 SDK 崩溃

### 5.3 ClaudeAgentOptions 字段分类

字段按职责分为六个关注域，每域独立演化：

| 关注域 | 字段 | 作用 |
|--------|------|------|
| **执行环境** | cli_path, cwd, env, user, extra_args | 控制子进程启动方式和运行环境 |
| **模型控制** | model, fallback_model, max_turns, max_budget_usd, effort, thinking, max_thinking_tokens, betas | 控制模型行为和资源上限 |
| **工具控制** | tools, allowed_tools, disallowed_tools, permission_mode, permission_prompt_tool_name | 限制和配置工具使用权限 |
| **MCP / 插件** | mcp_servers, plugins, add_dirs, agents | 扩展工具能力和上下文 |
| **会话管理** | continue_conversation, resume, fork_session, setting_sources, settings, enable_file_checkpointing, sandbox | 控制会话状态和持久化 |
| **回调 / 输出** | can_use_tool, hooks, stderr, include_partial_messages, output_format, max_buffer_size | 观测和干预 SDK 行为 |

**设计选择**：
- `mcp_servers: Option<McpServersConfig>` 而非默认空值：区分"未配置"（不生成 `--mcp-config`）与"配置为空"，避免冗余 CLI 参数
- Builder 模式：超过 30 个字段，builder 链式调用比位置参数构造函数更易用且可部分配置
- `settings` + `sandbox` 在 `build_settings_value()` 中合并为单一 JSON 传给 `--settings`，对调用方透明

### 5.4 MCP 配置类型层次

```
McpServersConfig
 ├── Dict(HashMap<名称, McpServerConfig>)   内联配置，多服务器统一管理
 └── Path(String)                           外部 JSON 文件路径，适合复杂配置

McpServerConfig
 ├── Stdio   命令行子进程（command + args + env）──► 序列化为 --mcp-config JSON
 ├── Sse     远程 SSE 端点（url + headers）    ──► 序列化为 --mcp-config JSON
 ├── Http    远程 HTTP 端点（url + headers）   ──► 序列化为 --mcp-config JSON
 └── Sdk     进程内服务器（name + version + tools[]）
              ├── 仅 name/version 传给 CLI（--mcp-config）
              └── tools[].handler 保留在 Rust 进程内，通过控制协议触发调用
```

**Sdk 类型的特殊设计**：CLI 通过 `control_request{mcp_message}` 将 JSON-RPC 请求回传给 SDK，由 `handle_sdk_mcp_request()` 在进程内路由。相比 Stdio MCP 子进程，零网络/IPC 开销，handler 直接访问 Rust 进程的内存状态。

### 5.5 ThinkingConfig 状态机

```
ThinkingConfig
 ├── Adaptive                   不传 max_thinking_tokens 给 CLI（CLI 自适应决策）
 │                              或取 max_thinking_tokens 字段值
 ├── Enabled { budget_tokens }  传 --max-thinking-tokens budget_tokens
 └── Disabled                   传 --max-thinking-tokens 0（显式关闭）
```

与 `max_thinking_tokens` 字段的交互：`Adaptive` 时以 `max_thinking_tokens` 为准（缺省 32000）；`Enabled` 时以 `budget_tokens` 为准，忽略 `max_thinking_tokens`；`Disabled` 时强制 0。

### 5.6 权限结果类型设计

```
PermissionResult
 ├── Allow
 │    ├── updated_input: Option<Value>       可修改工具入参（None 保持原样）
 │    └── updated_permissions: Option<Vec>  可更新权限规则集
 └── Deny
      ├── message: String                   拒绝原因（展示给用户）
      └── interrupt: bool                   true 时中断整个会话，false 时仅拒绝本次

PermissionUpdate（type_ 决定语义）
 ├── addRules / replaceRules / removeRules  — 增删改权限规则（含 toolName + ruleContent）
 ├── setMode                                — 切换全局权限模式
 └── addDirectories / removeDirectories     — 增删允许访问的目录
```

`PermissionUpdate.to_control_protocol_value()` 将 Rust 命名风格（snake_case）转换为 CLI 期望的 camelCase JSON，集中处理命名映射。

### 5.7 HookJSONOutput 类型设计

```
HookJSONOutput
 ├── Async { async_timeout: Option<u64> }
 │    CLI 等待外部异步操作完成，SDK 不阻塞当前响应
 │    序列化为: {"async": true, "asyncTimeout": N}
 │
 └── Sync { continue_, suppress_output, stop_reason,
            decision, system_message, reason, hook_specific_output }
      CLI 根据字段决定后续行为：
      ├── continue: false  ──► 拒绝操作
      ├── stop_reason      ──► 终止会话
      ├── system_message   ──► 注入系统消息
      └── decision         ──► 权限判决
```

**设计选择**：不用 `#[serde(rename_all = "camelCase")]` 自动转换，改用 `hook_output_to_json()` 手动构建 JSON，原因：`continue` 和 `async` 是 Rust 关键字，无法作为字段名，必须用 `continue_` / `async_timeout` 命名，手动序列化可精确控制输出 key。

### 5.8 Query 内部通道选型

| 通道 | 类型 | 选型理由 |
|------|------|---------|
| 写入通道 | `mpsc::Sender<String>` | 多个调用者（send_control / write_message）向单一 write_task 发送 |
| 消息广播 | `broadcast::Sender<ControlMessage>` | receive_messages 与 receive_response 可同时订阅同一消息源 |
| 请求计数 | `AtomicU64` | 无锁生成唯一 request_id，`fetch_add(SeqCst)` 保证全序 |
| 初始化结果 | `RwLock<Option<Value>>` | 一次写入、多次只读，读并发性优于 Mutex |

---

## 6. 技术栈选型

### 2.1 核心依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| **tokio** | 1.x | 异步运行时、进程、通道 |
| **serde** | 1.x | 序列化/反序列化 |
| **serde_json** | 1.x | JSON 解析 |
| **thiserror** | 1.x | 错误类型定义 |
| **anyhow** | 1.x | 错误传播 |
| **tracing** | 0.1 | 日志 |
| **async-trait** | 0.1 | 异步 trait 方法（`Transport` 需对象安全） |
| **futures** | 0.3 | `Stream`、`StreamExt` 用于流式迭代 |
| **tokio-stream** | 0.1 | 流式 API 辅助 |
| **async-stream** | 0.3 | `stream!` 宏简化流构建 |

### 2.2 平台相关依赖

| 依赖 | 平台 | 用途 |
|------|------|------|
| **nix** | `cfg(unix)` | 解析用户名为 uid（`options.user` 字段） |

### 2.3 替代方案说明

| Python | Rust 对应 |
|--------|-----------|
| anyio | tokio |
| anyio.open_process | tokio::process::Command |
| anyio.create_task_group | tokio::task::JoinSet / tokio::spawn |
| anyio.create_memory_object_stream | tokio::sync::mpsc / broadcast |
| AsyncIterator | impl Stream<Item = T> |
| async def / Awaitable | async fn / impl Future |
| Callable / Fn | impl Fn / FnOnce / dyn Fn |
| TypedDict / dataclass | struct + serde |

---

## 7. 模块结构

```
code-agent-sdk/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # 公开 API：query()、create_sdk_mcp_server()、sdk_mcp_tool()
│   ├── client.rs              # ClaudeSdkClient
│   ├── options.rs             # ClaudeAgentOptions、Builder、所有辅助类型
│   ├── types.rs               # Message、ContentBlock、Prompt 等
│   ├── error.rs               # Error 枚举、Result 类型别名
│   ├── internal/
│   │   ├── mod.rs
│   │   ├── client.rs          # InternalClient（process_query）
│   │   ├── query.rs           # Query（控制协议、hooks、can_use_tool、SDK MCP）
│   │   └── message_parser.rs  # parse_message
│   └── transport/
│       ├── mod.rs             # Transport trait
│       └── subprocess_cli.rs  # SubprocessCliTransport
└── examples/
    ├── quick_start.rs
    ├── streaming_mode.rs
    ├── mcp_calculator.rs
    ├── hooks.rs
    ├── system_prompt.rs
    ├── tools_option.rs
    └── tool_permission_callback.rs
```

---

## 8. 核心接口参考

### 4.1 query() 函数

```rust
/// 一次性查询，返回消息流（同步构造，惰性执行）
pub fn query(
    prompt: impl Into<Prompt> + Send + 'static,
    options: Option<ClaudeAgentOptions>,
) -> impl futures::Stream<Item = Result<Message>> + Send
```

**Prompt 类型**（支持字符串与流式）：

```rust
pub enum Prompt {
    /// 单次字符串（等价 Python str）
    Text(String),
    /// 流式 JSON 消息（等价 Python AsyncIterable）
    Stream(Pin<Box<dyn Stream<Item = serde_json::Value> + Send>>),
}

// From<String> / From<&str> 实现方便直接传字符串
```

**返回**：`impl Stream<Item = Result<Message>> + Send`，通过 `async_stream::stream!` 宏构建

### 4.2 sdk_mcp_tool() / create_sdk_mcp_server()

```rust
/// 便捷构造 SdkMcpTool（等价 Python @tool 装饰器）
pub fn sdk_mcp_tool<F>(
    name: &str,
    description: &str,
    input_schema: serde_json::Value,
    handler: F,
) -> SdkMcpTool
where
    F: Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send>>
        + Send + Sync + 'static;

/// 创建 SDK MCP 服务器配置（进程内执行）
pub fn create_sdk_mcp_server(name: &str, version: &str, tools: Vec<SdkMcpTool>) -> McpSdkConfig;
```

### 4.3 ClaudeSdkClient

```rust
pub struct ClaudeSdkClient {
    options: ClaudeAgentOptions,
    custom_transport: Option<Box<dyn Transport + Send>>,
    query: Option<Query>,
}

impl ClaudeSdkClient {
    /// options：None 使用默认值；custom_transport：None 使用 SubprocessCliTransport
    pub fn new(
        options: Option<ClaudeAgentOptions>,
        custom_transport: Option<Box<dyn Transport + Send>>,
    ) -> Self;

    /// 连接到 Claude Code 并初始化控制协议
    /// - None：连接但不发送消息（交互模式）
    /// - Prompt::Stream：启动后台 stream_input 任务
    /// - Prompt::Text：不自动发送，需调用 query() 后发送
    pub async fn connect(&mut self, prompt: Option<Prompt>) -> Result<()>;

    /// 发送查询（Text：写入用户消息；Stream：流式写入）
    pub async fn query(&mut self, prompt: impl Into<Prompt>, session_id: &str) -> Result<()>;

    /// 接收所有消息流（含控制消息）
    pub fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>>;

    /// 接收到 ResultMessage 为止的消息流
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

### 4.4 Transport trait

```rust
/// 注：使用 async_trait 是因为 Transport 需要对象安全（dyn Transport）
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn write(&mut self, data: &str) -> Result<()>;
    /// 返回拥有所有权的流（不借用 self），使 write/close 可并发调用
    fn read_messages(&mut self) -> Pin<Box<dyn Stream<Item = Result<serde_json::Value>> + Send>>;
    async fn close(&mut self) -> Result<()>;
    fn is_ready(&self) -> bool;
    async fn end_input(&mut self) -> Result<()>;
}
```

---

## 9. 数据结构参考

### 5.1 消息类型 (Message)

```rust
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    System(SystemMessage),
    Result(ResultMessage),
    StreamEvent(StreamEvent),
}

pub struct UserMessage {
    pub content: UserContent,           // String 或 Vec<ContentBlock>
    pub uuid: Option<String>,
    pub parent_tool_use_id: Option<String>,
    pub tool_use_result: Option<serde_json::Value>,
}

pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub parent_tool_use_id: Option<String>,
    pub error: Option<AssistantMessageError>,
}

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

pub struct SystemMessage {
    pub subtype: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

pub struct StreamEvent {
    pub uuid: String,
    pub session_id: String,
    pub event: serde_json::Value,
    pub parent_tool_use_id: Option<String>,
}
```

### 5.2 内容块 (ContentBlock)

```rust
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

pub struct TextBlock { pub text: String }
pub struct ThinkingBlock { pub thinking: String, pub signature: String }
pub struct ToolUseBlock { pub id: String, pub name: String, pub input: serde_json::Value }
pub struct ToolResultBlock {
    pub tool_use_id: String,
    pub content: Option<serde_json::Value>,
    pub is_error: Option<bool>,
}
```

### 5.3 ClaudeAgentOptions

```rust
#[derive(Clone, Default)]
pub struct ClaudeAgentOptions {
    pub tools: Option<ToolsConfig>,              // List(Vec<String>) 或 Preset
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub system_prompt: Option<SystemPromptConfig>,  // String 或 Preset { append }
    pub permission_mode: Option<PermissionMode>,
    pub model: Option<String>,
    pub fallback_model: Option<String>,
    pub max_turns: Option<u32>,
    pub max_budget_usd: Option<f64>,
    pub continue_conversation: bool,
    pub resume: Option<String>,
    pub cwd: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub extra_args: HashMap<String, Option<String>>,
    pub add_dirs: Vec<PathBuf>,
    pub mcp_servers: Option<McpServersConfig>,  // Dict 或 Path
    pub include_partial_messages: bool,
    pub fork_session: bool,
    pub setting_sources: Option<Vec<SettingSource>>,
    pub plugins: Vec<SdkPluginConfig>,
    pub max_thinking_tokens: Option<u32>,
    pub effort: Option<Effort>,
    pub output_format: Option<serde_json::Value>,  // json_schema 类型触发 --json-schema
    pub permission_prompt_tool_name: Option<String>,
    pub max_buffer_size: Option<usize>,
    pub enable_file_checkpointing: bool,
    pub betas: Vec<SdkBeta>,
    pub settings: Option<String>,               // JSON 字符串或文件路径
    pub sandbox: Option<SandboxSettings>,
    pub user: Option<String>,                   // Unix 用户名，spawn 时转为 uid
    pub agents: Option<HashMap<String, AgentDefinition>>,
    pub thinking: Option<ThinkingConfig>,       // Adaptive / Enabled / Disabled
    pub can_use_tool: Option<CanUseToolCallback>,
    pub hooks: Option<HashMap<HookEvent, Vec<HookMatcher>>>,
    pub stderr: Option<StderrCallback>,
}

impl ClaudeAgentOptions {
    pub fn builder() -> ClaudeAgentOptionsBuilder;
}
```

**Builder** 提供链式方法：`allowed_tools`、`system_prompt`、`permission_mode`、`model`、`max_turns`、`cwd`、`mcp_servers`、`tools`、`hooks`、`can_use_tool`、`stderr`、`betas`、`sandbox`、`thinking`、`agents`、`env`、`extra_arg`、`add_dir`、`plugin`、`max_thinking_tokens`、`include_partial_messages`、`enable_file_checkpointing`、`user`、`settings`、`fork_session`、`continue_conversation`、`resume`、`cli_path`、`max_budget_usd`、`disallowed_tools`、`build()`。

### 5.4 MCP 服务器配置

```rust
pub enum McpServersConfig {
    Dict(HashMap<String, McpServerConfig>),   // 内联配置
    Path(String),                             // 外部配置文件路径
}

pub enum McpServerConfig {
    Stdio(McpStdioConfig),
    Sse(McpSseConfig),
    Http(McpHttpConfig),
    Sdk(McpSdkConfig),  // 进程内 SDK MCP 服务器
}

pub struct McpStdioConfig {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

pub struct McpSseConfig {
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
}

pub struct McpHttpConfig {
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
}

pub struct McpSdkConfig {
    pub name: String,
    pub version: String,
    pub tools: Vec<SdkMcpTool>,  // 进程内工具处理器
}
```

**序列化说明**：Stdio/Sse/Http 通过 `--mcp-config` JSON 传给 CLI；Sdk 仅传 `{type:"sdk", name, version}`，工具处理器保留在进程内，通过控制协议的 `mcp_message` 路由。

### 5.5 SDK MCP 工具

```rust
/// 工具处理器类型：接收 JSON 输入，返回 JSON 结果
pub type SdkMcpToolHandler = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send>>
        + Send + Sync,
>;

pub struct SdkMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema
    pub handler: SdkMcpToolHandler,
}
```

### 5.6 Hook 类型

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit,
    Stop, SubagentStop, PreCompact, Notification, SubagentStart, PermissionRequest,
}

pub struct HookMatcher {
    pub matcher: Option<String>,      // 工具名模式匹配（None = 匹配全部）
    pub hooks: Vec<HookCallback>,
    pub timeout: Option<f64>,
}

/// (input, tool_use_id, context) -> HookJSONOutput
pub type HookCallback = Arc<
    dyn Fn(serde_json::Value, Option<String>, HookContext)
        -> Pin<Box<dyn Future<Output = Result<HookJSONOutput>> + Send>>
    + Send + Sync,
>;

pub enum HookJSONOutput {
    Async { async_timeout: Option<u64> },
    Sync {
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

**序列化说明**：`Async` 变体序列化为 `{"async": true, "asyncTimeout": ...}`；`Sync` 变体字段使用 camelCase（`continue`、`suppressOutput` 等），在 `hook_output_to_json()` 中手动构建以规避 Rust 关键字冲突。

### 5.7 其他枚举类型

```rust
pub enum PermissionMode { Default, AcceptEdits, Plan, BypassPermissions }

pub enum Effort { Low, Medium, High, Max }

pub enum SdkBeta {
    Context1M,                // "context-1m-2025-08-07"
    Other(String),            // 其他 beta 特性
}

pub enum AgentModel { Sonnet, Opus, Haiku, Inherit }

pub enum ThinkingConfig {
    Adaptive,                      // 自适应（默认 32000 tokens）
    Enabled { budget_tokens: u32 }, // 固定 budget
    Disabled,                      // 禁用（tokens=0）
}

pub enum SettingSource { User, Project, Local }

pub enum ToolsConfig {
    List(Vec<String>),
    Preset { preset: String },
}

pub enum SystemPromptConfig {
    String(String),
    Preset { preset: String, append: Option<String> },
}

pub enum AssistantMessageError {
    AuthenticationFailed, BillingError, RateLimit, InvalidRequest, ServerError, Unknown,
}
```

### 5.8 权限相关类型

```rust
pub type CanUseToolCallback = Arc<
    dyn Fn(String, serde_json::Value, ToolPermissionContext)
        -> Pin<Box<dyn Future<Output = PermissionResult> + Send>>
    + Send + Sync,
>;

pub enum PermissionResult {
    Allow(PermissionResultAllow),
    Deny(PermissionResultDeny),
}

pub struct PermissionResultAllow {
    pub updated_input: Option<serde_json::Value>,
    pub updated_permissions: Option<Vec<PermissionUpdate>>,
}

pub struct PermissionResultDeny {
    pub message: String,
    pub interrupt: bool,
}

pub struct PermissionUpdate {
    pub type_: String,    // "addRules" | "replaceRules" | "removeRules" | "setMode" | "addDirectories" | "removeDirectories"
    pub rules: Option<Vec<PermissionRuleValue>>,
    pub behavior: Option<String>,
    pub mode: Option<String>,
    pub directories: Option<Vec<String>>,
    pub destination: Option<String>,
}
```

### 5.9 沙箱配置

```rust
pub struct SandboxSettings {
    pub enabled: Option<bool>,
    pub auto_allow_bash_if_sandboxed: Option<bool>,
    pub excluded_commands: Option<Vec<String>>,
    pub allow_unsandboxed_commands: Option<bool>,
    pub network: Option<SandboxNetworkConfig>,
    pub ignore_violations: Option<SandboxIgnoreViolations>,
    pub enable_weaker_nested_sandbox: Option<bool>,
}
```

沙箱配置通过 `--settings` JSON 参数传入 CLI。若同时指定 `settings` 文件路径，会合并后传入。

---

## 10. 控制协议实现

### 6.1 Query 内部结构

```rust
pub struct Query {
    write_tx: Option<mpsc::Sender<String>>,
    message_tx: broadcast::Sender<ControlMessage>,
    request_counter: AtomicU64,
    init_result: tokio::sync::RwLock<Option<serde_json::Value>>,
}

enum ControlMessage {
    Data(serde_json::Value),
    End,
    Error(String),
}
```

`Query::new()` 启动两个后台任务：
- **写入任务**：从 `write_rx` 接收字符串，调用 `transport.write()`，通道关闭后调用 `end_input()` 和 `close()`
- **读取任务**：轮询 `read_stream`，将普通数据广播到 `message_tx`，拦截 `control_request` 进行处理

### 6.2 控制请求处理（incoming）

CLI 发出的 `control_request`（subtype）处理逻辑：

| subtype | 处理方式 |
|---------|---------|
| `can_use_tool` | 调用 `CanUseToolCallback`；Allow 时返回 `{behavior:"allow", updatedInput, updatedPermissions}`；Deny 时返回 `{behavior:"deny", message, interrupt}` |
| `hook_callback` | 按 `callback_id`（格式：`hook_N`）查找并调用 `HookCallback`，通过 `hook_output_to_json()` 序列化 |
| `mcp_message` | 路由到对应进程内 SDK MCP 服务器，处理 `initialize`、`tools/list`、`tools/call` 等 JSON-RPC 方法 |
| `control_cancel_request` | 忽略（当前实现） |

所有控制响应格式：
```json
{"type": "control_response", "response": {"subtype": "success"|"error", "request_id": "...", "response": ...}}
```

### 6.3 initialize 中的 hooks 配置

`Query::initialize()` 调用 `build_hooks_config_for_initialize()` 构建 `hooks_config`，其中每个 `HookMatcher` 的每个 hook 分配递增 ID（`hook_0`, `hook_1`, ...）存入 `hookCallbackIds`。`build_hook_callbacks()` 使用完全相同的 ID 分配策略构建 callback map，确保 CLI 发出的 `hook_callback` 能正确路由。

### 6.4 SDK MCP JSON-RPC 路由

进程内 MCP 服务器通过 `handle_sdk_mcp_request()` 处理，支持方法：
- `initialize`：返回协议版本 `2024-11-05`、capabilities 和 serverInfo
- `notifications/initialized`：返回空对象
- `tools/list`：返回工具列表（name、description、inputSchema）
- `tools/call`：按名称查找工具，调用处理器，返回 JSON-RPC 结果

响应封装格式：`{"mcp_response": {"jsonrpc":"2.0", "id":..., "result":...}}`

---

## 11. SubprocessCliTransport 实现

### 7.1 结构

```rust
pub struct SubprocessCliTransport {
    options: ClaudeAgentOptions,
    cli_path: String,
    cwd: Option<String>,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    ready: bool,
    exit_error: Option<Error>,
    max_buffer_size: usize,  // 默认 1MB
}
```

### 7.2 CLI 查找逻辑

```
1. options.cli_path（显式指定）
2. 可执行文件同目录的 _bundled/claude（捆绑分发）
3. PATH 中的 claude（which 逻辑）
4. 常见安装路径：
   ~/.npm-global/bin/claude
   /usr/local/bin/claude
   ~/.local/bin/claude
   ~/node_modules/.bin/claude
   ~/.yarn/bin/claude
   ~/.claude/local/claude
```

### 7.3 命令构建

`build_command()` 按以下顺序拼接 CLI 参数：

| 选项来源 | CLI 参数 |
|---------|---------|
| 固定 | `--output-format stream-json --verbose` |
| system_prompt | `--system-prompt <s>` 或 `--append-system-prompt <s>` |
| tools | `--tools <list\|"default">` |
| allowed_tools | `--allowedTools <list>` |
| disallowed_tools | `--disallowedTools <list>` |
| max_turns | `--max-turns <n>` |
| max_budget_usd | `--max-budget-usd <f>` |
| model / fallback_model | `--model` / `--fallback-model` |
| permission_mode | `--permission-mode <mode>` |
| continue_conversation | `--continue` |
| resume | `--resume <id>` |
| settings + sandbox | `--settings <json>` （合并后传入）|
| betas | `--betas <list>` |
| add_dirs | `--add-dir <path>` (多次) |
| mcp_servers | `--mcp-config <json>` |
| include_partial_messages | `--include-partial-messages` |
| fork_session | `--fork-session` |
| setting_sources | `--setting-sources <list>` |
| plugins | `--plugin-dir <path>` (多次) |
| extra_args | `--<flag> [value]` |
| thinking/max_thinking_tokens | `--max-thinking-tokens <n>` |
| effort | `--effort <level>` |
| output_format（json_schema） | `--json-schema <schema>` |
| permission_prompt_tool_name | `--permission-prompt-tool <name>` |
| 固定 | `--input-format stream-json` |

### 7.4 进程 spawn

```rust
cmd.stdin(Stdio::piped())
   .stdout(Stdio::piped())
   .stderr(if should_pipe_stderr { Stdio::piped() } else { Stdio::null() })
   .env("CLAUDE_CODE_ENTRYPOINT", "sdk-rs")
   .env("CLAUDE_AGENT_SDK_VERSION", env!("CARGO_PKG_VERSION"));
```

- `should_pipe_stderr`：`options.stderr.is_some() || extra_args.contains_key("debug-to-stderr")`
- stderr 有回调时，spawn 后台任务逐行读取并调用 `StderrCallback`
- `enable_file_checkpointing` 时额外设置 `CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING=true`
- Unix 下若 `options.user` 有值，通过 `nix::unistd::User::from_name()` 解析为 uid 并设置进程 uid

### 7.5 版本检查

connect 时（除非设置 `CLAUDE_AGENT_SDK_SKIP_VERSION_CHECK` 环境变量）调用 `claude -v` 检查版本，低于 `2.0.0` 时输出 warning 日志。

### 7.6 流读取

`read_messages()` 消耗 stdout，使用 `async_stream::stream!` 逐行读取，支持 JSON 片段拼接直到解析成功，超出 `max_buffer_size` 时返回错误并终止流。

---

## 12. 错误类型

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

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

---

## 13. 数据流（实现参考）

### 9.1 query() 流程

```
User prompt (Prompt::Text / Prompt::Stream)
    → InternalClient::process_query
    → SubprocessCliTransport::new + connect()
        → find_cli, build_command, spawn process
    → Query::new
        → spawn write_task（mpsc write_rx → transport.write）
        → spawn read_task（read_stream → broadcast message_tx）
    → Query::initialize
        → 发送 control_request{subtype:"initialize"}
        → 等待 control_response（含 server_info）
    → Prompt::Text：write_user_message → end_input（drop write_tx）
    → Prompt::Stream：stream_input（后台任务写入，完成后 drop write_tx）
    → query.receive_response()
        → broadcast 接收 ControlMessage::Data → parse_message → yield Message
        → 遇到 ResultMessage 或 End/Error 则停止
```

### 9.2 ClaudeSdkClient 流程

```
connect(prompt)
    → SubprocessCliTransport::connect
    → Query::new + Query::initialize
    → [Prompt::Stream] → stream_input 后台任务

query(prompt, session_id)
    → write_user_message 或 stream_input

receive_messages() / receive_response()
    → broadcast rx → parse_message → yield
```

### 9.3 控制协议（incoming from CLI）

```
CLI → control_request{subtype}
    → read_task 拦截（不广播到 message_tx）
    → handle_control_request
        ├── can_use_tool → CanUseToolCallback → control_response{success/error}
        ├── hook_callback → HookCallback[id] → control_response
        └── mcp_message → handle_sdk_mcp_request → JSONRPC 路由 → control_response
```

---

## 14. 功能对等清单

| 功能 | Python | Rust 实现 |
|------|--------|-----------|
| query() | ✓ | `query()` fn（同步构造，惰性流） |
| ClaudeSDKClient | ✓ | `ClaudeSdkClient` |
| Transport | ✓ | `Transport` trait |
| SubprocessCLITransport | ✓ | `SubprocessCliTransport` |
| 控制协议（outgoing） | ✓ | `Query::initialize / send_control_request` |
| 控制协议（incoming） | ✓ | `handle_control_request` in read_task |
| Hooks | ✓ | `HookMatcher` + `HookCallback` |
| can_use_tool | ✓ | `CanUseToolCallback` + 权限更新 |
| SDK MCP Server（进程内） | ✓ | `create_sdk_mcp_server` + `sdk_mcp_tool` + `handle_sdk_mcp_request` |
| Stdio MCP（含 env） | ✓ | `McpStdioConfig` |
| SSE MCP | ✓ | `McpSseConfig`（通过 --mcp-config 传 CLI） |
| HTTP MCP | ✓ | `McpHttpConfig`（通过 --mcp-config 传 CLI） |
| MCP tools/list, tools/call | ✓ | 手动 JSON-RPC 路由 |
| 消息类型 | ✓ | `Message` 枚举 |
| ContentBlock | ✓ | `ContentBlock` 枚举 |
| ClaudeAgentOptions | ✓ | `ClaudeAgentOptions` + Builder |
| 错误类型 | ✓ | `Error` 枚举（含 NotConnected） |
| 流式输入 | ✓ | `Prompt::Stream` + `stream_input` |
| 流式输出 | ✓ | `impl Stream<Item = Result<Message>>` |
| interrupt | ✓ | `interrupt()` |
| set_permission_mode | ✓ | `set_permission_mode()` |
| set_model | ✓ | `set_model()` |
| rewind_files | ✓ | `rewind_files()` |
| get_mcp_status | ✓ | `get_mcp_status()` |
| get_server_info | ✓ | `get_server_info()` |
| stderr 回调 | ✓ | `StderrCallback` |
| 自定义 Transport | ✓ | `Box<dyn Transport + Send>` |
| ThinkingConfig | ✓ | `ThinkingConfig` 枚举 |
| SandboxSettings | ✓ | `SandboxSettings`（通过 --settings JSON） |
| SdkBeta | ✓ | `SdkBeta` 枚举 |
| user（uid） | ✓ | Unix nix::unistd::User 解析 |
| agents | ✓ | `AgentDefinition`（通过 initialize 传入 CLI） |
| 版本检查 | ✓ | `check_claude_version`（可跳过） |

---

## 15. 实现状态（当前）

- **全部阶段已完成**：核心类型、Transport、query、ClaudeSdkClient、控制协议（outgoing + incoming）、Hooks、can_use_tool、SDK MCP Server（进程内 JSON-RPC）、Stdio/SSE/HTTP MCP 配置、SandboxSettings、ThinkingConfig、SdkBeta、AgentModel、PermissionUpdate、StderrCallback、user uid、版本检查

---

## 16. 附录：Cargo.toml 依赖

```toml
[package]
name = "code-agent-sdk"
version = "0.1.0"
edition = "2021"
description = "Rust SDK for Claude Code Agent"
license = "MIT"

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
async-stream = "0.3"

[target.'cfg(unix)'.dependencies]
nix = { version = "~0.29", features = ["user"] }

[dev-dependencies]
tokio-test = "0.4"
mockall = "0.12"
```

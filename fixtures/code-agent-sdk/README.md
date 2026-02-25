# code-agent-sdk Fixtures

Rust 版本的集成测试 fixtures，对应 `fixtures/claude-agent-sdk-python/` 中的 Python 实现。

## 运行方式

需要配置 `ANTHROPIC_API_KEY` 环境变量才能调用 Claude API。

```bash
# 运行单个 fixture
cargo run -p code-agent-sdk-fixtures -- test_01
cargo run -p code-agent-sdk-fixtures -- test_23

# 运行所有 fixtures
cargo run -p code-agent-sdk-fixtures -- all

# 通过 Makefile
make fixtures
```

## 已实现的 Fixtures（共 32 个）

| 编号 | 文件 | 功能 |
|------|------|------|
| 01 | test_01_basic_query | 基础查询 |
| 02 | test_02_query_with_options | 带配置的查询 |
| 03 | test_03_system_prompt_preset | system_prompt 字符串 / preset / preset+append |
| 04 | test_04_interactive_session | 交互式会话 |
| 05 | test_05_multi_turn_conversation | 多轮对话 |
| 06 | test_06_manual_connect_disconnect | 手动 connect/disconnect |
| 07 | test_07_tools_control | tools / allowed_tools / disallowed_tools |
| 08 | test_08_tool_permission_callback | can_use_tool 回调 |
| 09 | test_09_hook_pretooluse | PreToolUse Hook |
| 10 | test_10_hook_posttooluse | PostToolUse Hook |
| 11 | test_11_hook_user_prompt_submit | UserPromptSubmit Hook |
| 12 | test_12_hook_continue_stop | continue/stopReason 控制 |
| 13 | test_13_mcp_tools | MCP 工具（add / multiply） |
| 14 | test_14_mcp_tool_error | MCP 工具错误处理 |
| 15 | test_15_custom_agents | 自定义 Agent 定义 |
| 16 | test_16_structured_output | 结构化输出 JSON Schema |
| 17 | test_17_stream_events | StreamEvent / include_partial_messages |
| 18 | test_18_dynamic_permission_mode | set_permission_mode |
| 19 | test_19_dynamic_model_switch | set_model |
| 20 | test_20_interrupt | interrupt |
| 21 | test_21_max_budget | max_budget_usd |
| 22 | test_22_stderr_callback | stderr 回调 |
| 23 | test_23_error_handling | 错误处理 |
| 24 | test_24_control_protocol_info | get_server_info / get_mcp_status |
| 25 | test_25_message_types | 消息类型覆盖 |
| 26 | test_26_async_iterable_prompt | Prompt::Stream 异步流 |
| 27 | test_27_receive_messages | receive_messages 持续流 |
| 28 | test_28_env_and_cwd | env / cwd |
| 29 | test_29_setting_sources | setting_sources |
| 30 | test_30_tool_use_bash | ToolUseBlock / ToolResultBlock / Bash |
| 31 | test_31_mcp_multi_tools | MCP 多工具协作 |
| 32 | test_32_bypass_permissions | bypassPermissions |

## 注意事项

- test_23 中的 `test_invalid_cli_path` 和 `test_timeout_handling` 不依赖 API，可在无 API key 时运行
- 其他 fixtures 需要有效的 `ANTHROPIC_API_KEY` 和已安装的 Claude Code CLI

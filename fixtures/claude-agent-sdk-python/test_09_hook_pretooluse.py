#!/usr/bin/env python3
"""测试场景: Hook 拦截 — PreToolUse

验证 PreToolUse Hook 能在工具执行前拦截并允许/拒绝工具调用。
覆盖功能:
- hooks 配置 (PreToolUse)
- HookMatcher 匹配器
- HookCallback 回调函数
- HookInput / HookContext 参数
- permissionDecision: allow / deny
"""

import asyncio
from typing import Any

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)
from claude_agent_sdk.types import (
    HookContext,
    HookInput,
    HookJSONOutput,
    HookMatcher,
)

# 记录 hook 调用
hook_calls = []


async def pretooluse_hook(
    input_data: HookInput,
    tool_use_id: str | None,
    context: HookContext,
) -> HookJSONOutput:
    """PreToolUse Hook: 拦截包含 'forbidden' 的 bash 命令。"""
    tool_name = input_data["tool_name"]
    tool_input = input_data["tool_input"]

    hook_calls.append({"tool": tool_name, "input": tool_input})
    print(f"  [PreToolUse Hook] tool={tool_name}")

    if tool_name == "Bash":
        command = tool_input.get("command", "")
        if "forbidden" in command:
            print(f"  [PreToolUse Hook] 拒绝命令: {command}")
            return {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "Command contains forbidden pattern",
                }
            }

    # 允许其他操作
    return {}


async def main():
    print("=== 测试: Hook 拦截 — PreToolUse ===\n")

    options = ClaudeAgentOptions(
        allowed_tools=["Bash"],
        hooks={
            "PreToolUse": [
                HookMatcher(matcher="Bash", hooks=[pretooluse_hook]),
            ],
        },
    )

    async with ClaudeSDKClient(options=options) as client:
        # 测试1: 被拦截的命令
        print("--- 测试1: 被拦截的命令 ---")
        await client.query("Run this bash command: echo 'this is forbidden content'")

        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text[:150]}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        print()

        # 测试2: 允许的命令
        print("--- 测试2: 允许的命令 ---")
        await client.query("Run this bash command: echo 'hello world'")

        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text[:150]}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    print(f"\nHook 被调用 {len(hook_calls)} 次")
    assert len(hook_calls) > 0, "PreToolUse Hook 未被调用"

    print("\n✓ PreToolUse Hook 测试通过")


if __name__ == "__main__":
    asyncio.run(main())

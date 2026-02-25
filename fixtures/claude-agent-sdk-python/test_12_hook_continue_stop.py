#!/usr/bin/env python3
"""测试场景: Hook — continue/stopReason 执行控制

验证 PostToolUse Hook 使用 continue_=False 和 stopReason 停止执行。
覆盖功能:
- continue_ 字段控制执行流
- stopReason 停止原因
- 执行中断后的行为
"""

import asyncio

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

stop_triggered = False


async def stop_on_keyword_hook(
    input_data: HookInput,
    tool_use_id: str | None,
    context: HookContext,
) -> HookJSONOutput:
    """PostToolUse Hook: 检测到关键词时停止执行。"""
    global stop_triggered
    tool_response = str(input_data.get("tool_response", ""))

    if "STOP_NOW" in tool_response:
        stop_triggered = True
        print("  [Hook] 检测到 STOP_NOW，停止执行")
        return {
            "continue_": False,
            "stopReason": "Keyword STOP_NOW detected in output - halting execution",
            "systemMessage": "Execution stopped by hook",
        }

    return {"continue_": True}


async def main():
    print("=== 测试: Hook — continue/stopReason 执行控制 ===\n")

    options = ClaudeAgentOptions(
        allowed_tools=["Bash"],
        hooks={
            "PostToolUse": [
                HookMatcher(matcher="Bash", hooks=[stop_on_keyword_hook]),
            ],
        },
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query("Run this bash command: echo 'STOP_NOW'")

        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text[:200]}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    assert stop_triggered, "停止 Hook 未被触发"
    print("\n✓ continue/stopReason 执行控制测试通过")


if __name__ == "__main__":
    asyncio.run(main())

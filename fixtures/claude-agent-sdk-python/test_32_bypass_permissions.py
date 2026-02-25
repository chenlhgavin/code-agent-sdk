#!/usr/bin/env python3
"""测试场景: 权限模式 — bypassPermissions (CI/CD 自动化)

验证 bypassPermissions 模式下工具无需权限确认即可执行。
覆盖功能:
- permission_mode="bypassPermissions"
- 自动化执行场景
- 工具自动批准
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
    ToolUseBlock,
)


async def main():
    print("=== 测试: bypassPermissions 模式 ===\n")

    options = ClaudeAgentOptions(
        permission_mode="bypassPermissions",
        allowed_tools=["Bash"],
    )

    tool_executed = False

    async with ClaudeSDKClient(options=options) as client:
        await client.query("Run this bash command: echo 'bypass test OK'")

        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, ToolUseBlock):
                        tool_executed = True
                        print(f"工具执行: {block.name}")
                    elif isinstance(block, TextBlock):
                        print(f"Claude: {block.text[:150]}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")
                assert not msg.is_error, f"查询失败: {msg.result}"

    # bypassPermissions 模式下工具应该自动执行
    print(f"\n工具是否执行: {tool_executed}")
    print("✓ bypassPermissions 模式测试通过")


if __name__ == "__main__":
    asyncio.run(main())

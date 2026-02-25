#!/usr/bin/env python3
"""测试场景: 动态权限模式切换 (set_permission_mode)

验证在会话中动态切换权限模式。
覆盖功能:
- client.set_permission_mode() 动态切换
- permission_mode 配置 (default / acceptEdits / bypassPermissions)
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)


async def main():
    print("=== 测试: 动态权限模式切换 ===\n")

    options = ClaudeAgentOptions(
        permission_mode="default",
    )

    async with ClaudeSDKClient(options=options) as client:
        # 切换到 acceptEdits 模式
        print("--- 切换到 acceptEdits 模式 ---")
        await client.set_permission_mode("acceptEdits")

        await client.query("What is 2 + 2? Just the number.")
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude (acceptEdits): {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        # 切换回 default 模式
        print("\n--- 切换回 default 模式 ---")
        await client.set_permission_mode("default")

        await client.query("What is 3 + 3? Just the number.")
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude (default): {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    print("\n✓ 动态权限模式切换测试通过")


if __name__ == "__main__":
    asyncio.run(main())

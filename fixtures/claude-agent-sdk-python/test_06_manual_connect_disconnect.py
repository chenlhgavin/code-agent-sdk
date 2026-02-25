#!/usr/bin/env python3
"""测试场景: 手动连接与断开

验证 ClaudeSDKClient 不使用 async with，而是手动调用 connect() 和 disconnect()。
覆盖功能:
- client.connect() 手动连接
- client.disconnect() 手动断开
- 不使用上下文管理器的生命周期管理
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)


async def main():
    print("=== 测试: 手动连接与断开 ===\n")

    client = ClaudeSDKClient()

    try:
        # 手动连接
        await client.connect()
        print("✓ 手动连接成功")

        # 发送查询
        await client.query("What is 3 + 7? Reply with just the number.")

        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        assert "10" in response_text, f"回答不正确: {response_text}"
        print("✓ 查询响应正确")

    finally:
        # 手动断开
        await client.disconnect()
        print("✓ 手动断开成功")

    print("\n✓ 手动连接与断开测试通过")


if __name__ == "__main__":
    asyncio.run(main())

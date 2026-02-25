#!/usr/bin/env python3
"""测试场景: receive_messages 持续消息流

验证 client.receive_messages() 持续接收消息 (不以 ResultMessage 为终止)。
覆盖功能:
- client.receive_messages() 持续消息流
- 与 receive_response() 的区别
- 后台消息消费模式
"""

import asyncio
import contextlib

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)


async def main():
    print("=== 测试: receive_messages 持续消息流 ===\n")

    async with ClaudeSDKClient() as client:
        all_messages = []

        async def background_receiver():
            """后台持续接收消息。"""
            async for msg in client.receive_messages():
                all_messages.append(msg)
                if isinstance(msg, AssistantMessage):
                    for block in msg.content:
                        if isinstance(block, TextBlock):
                            print(f"  [bg] Claude: {block.text[:80]}")
                elif isinstance(msg, ResultMessage):
                    print(f"  [bg] Result: {msg.subtype}")

        # 启动后台接收
        receiver_task = asyncio.create_task(background_receiver())

        # 发送第一个查询
        print("--- 查询1 ---")
        await client.query("What is 5 + 5? Just the number.")
        await asyncio.sleep(3)

        # 发送第二个查询
        print("\n--- 查询2 ---")
        await client.query("What is 10 + 10? Just the number.")
        await asyncio.sleep(3)

        # 取消后台接收
        receiver_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await receiver_task

    print(f"\n总共收到 {len(all_messages)} 条消息")
    msg_types = [type(m).__name__ for m in all_messages]
    print(f"消息类型: {set(msg_types)}")

    # 应该收到多条消息 (至少有 AssistantMessage 和 ResultMessage)
    assert len(all_messages) > 0, "未收到任何消息"

    print("\n✓ receive_messages 持续消息流测试通过")


if __name__ == "__main__":
    asyncio.run(main())

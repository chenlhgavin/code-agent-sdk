#!/usr/bin/env python3
"""测试场景: 交互式会话 (ClaudeSDKClient)

验证 ClaudeSDKClient 的基本交互式会话功能，包括连接、查询、接收响应、断开连接。
覆盖功能:
- ClaudeSDKClient 创建与连接
- async with 上下文管理器
- client.query() 发送消息
- client.receive_response() 接收响应
- 基本消息类型处理
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeSDKClient,
    ResultMessage,
    SystemMessage,
    TextBlock,
    UserMessage,
)


async def main():
    print("=== 测试: 交互式会话 (ClaudeSDKClient) ===\n")

    message_types_seen = set()

    async with ClaudeSDKClient() as client:
        print("✓ 客户端连接成功")

        # 发送查询
        await client.query("What is the capital of France? Reply in one word.")

        response_text = ""
        async for msg in client.receive_response():
            message_types_seen.add(type(msg).__name__)

            if isinstance(msg, UserMessage):
                print(f"[UserMessage] 收到用户消息回显")
            elif isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
                        print(f"[AssistantMessage] Claude: {block.text}")
            elif isinstance(msg, SystemMessage):
                print(f"[SystemMessage] subtype={msg.subtype}")
            elif isinstance(msg, ResultMessage):
                print(f"[ResultMessage] status={msg.subtype}, session={msg.session_id}")

    print(f"\n收到的消息类型: {message_types_seen}")
    assert "AssistantMessage" in message_types_seen, "未收到 AssistantMessage"
    assert "ResultMessage" in message_types_seen, "未收到 ResultMessage"
    assert "paris" in response_text.lower(), f"回答不正确: {response_text}"

    print("✓ 交互式会话测试通过")


if __name__ == "__main__":
    asyncio.run(main())

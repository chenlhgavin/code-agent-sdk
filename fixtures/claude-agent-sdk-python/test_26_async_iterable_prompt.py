#!/usr/bin/env python3
"""测试场景: 异步可迭代提示 (AsyncIterable prompt)

验证通过 async iterable 发送多条消息流作为 prompt。
覆盖功能:
- query() 接受 AsyncIterable[dict] 作为 prompt
- client.query() 接受 async iterable
- 消息流格式 (type, message, parent_tool_use_id, session_id)
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)


async def create_message_stream():
    """生成异步消息流。"""
    yield {
        "type": "user",
        "message": {"role": "user", "content": "Remember: the secret word is 'pineapple'."},
        "parent_tool_use_id": None,
        "session_id": "test-session",
    }

    yield {
        "type": "user",
        "message": {"role": "user", "content": "What is the secret word I just told you? Reply with just the word."},
        "parent_tool_use_id": None,
        "session_id": "test-session",
    }


async def main():
    print("=== 测试: 异步可迭代提示 ===\n")

    async with ClaudeSDKClient() as client:
        await client.query(create_message_stream())

        # 消费第一轮响应
        print("--- 第一轮响应 ---")
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        # 消费第二轮响应
        print("\n--- 第二轮响应 ---")
        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    assert "pineapple" in response_text.lower(), \
        f"未正确回忆 secret word，回复: {response_text}"

    print("\n✓ 异步可迭代提示测试通过")


if __name__ == "__main__":
    asyncio.run(main())

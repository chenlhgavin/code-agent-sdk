#!/usr/bin/env python3
"""测试场景: 多轮对话

验证 ClaudeSDKClient 支持多轮对话，后续问题能引用前文上下文。
覆盖功能:
- 多次调用 client.query()
- 上下文保持 (follow-up 问题)
- 多次调用 client.receive_response()
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)


def extract_text(msg) -> str:
    """从 AssistantMessage 中提取文本。"""
    text = ""
    if isinstance(msg, AssistantMessage):
        for block in msg.content:
            if isinstance(block, TextBlock):
                text += block.text
    return text


async def main():
    print("=== 测试: 多轮对话 ===\n")

    async with ClaudeSDKClient() as client:
        # 第一轮: 建立上下文
        print("--- 第一轮 ---")
        await client.query("Remember this number: 42. Just confirm you got it.")

        turn1_text = ""
        async for msg in client.receive_response():
            text = extract_text(msg)
            if text:
                turn1_text += text
                print(f"Claude: {text}")
            if isinstance(msg, ResultMessage):
                print(f"[结果] status={msg.subtype}")

        assert len(turn1_text) > 0, "第一轮未收到回复"

        # 第二轮: 引用上下文
        print("\n--- 第二轮 ---")
        await client.query("What number did I ask you to remember? Reply with just the number.")

        turn2_text = ""
        async for msg in client.receive_response():
            text = extract_text(msg)
            if text:
                turn2_text += text
                print(f"Claude: {text}")
            if isinstance(msg, ResultMessage):
                print(f"[结果] status={msg.subtype}")

        assert "42" in turn2_text, f"第二轮未正确引用上下文，回复: {turn2_text}"

        # 第三轮: 继续对话
        print("\n--- 第三轮 ---")
        await client.query("Double that number. Reply with just the result.")

        turn3_text = ""
        async for msg in client.receive_response():
            text = extract_text(msg)
            if text:
                turn3_text += text
                print(f"Claude: {text}")
            if isinstance(msg, ResultMessage):
                print(f"[结果] status={msg.subtype}")

        assert "84" in turn3_text, f"第三轮计算不正确，回复: {turn3_text}"

    print("\n✓ 多轮对话测试通过")


if __name__ == "__main__":
    asyncio.run(main())

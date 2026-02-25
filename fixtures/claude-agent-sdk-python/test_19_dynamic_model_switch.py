#!/usr/bin/env python3
"""测试场景: 动态模型切换 (set_model)

验证在会话中动态切换模型。
覆盖功能:
- client.set_model() 动态切换模型
- 切换到指定模型
- 切换回默认模型 (None)
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
    print("=== 测试: 动态模型切换 ===\n")

    async with ClaudeSDKClient() as client:
        # 使用默认模型
        print("--- 默认模型 ---")
        await client.query("What is 1 + 1? Just the number.")

        model_used_1 = None
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                model_used_1 = msg.model
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        if model_used_1:
            print(f"使用模型: {model_used_1}")

        # 切换到 Haiku 模型
        print("\n--- 切换到 Haiku ---")
        await client.set_model("claude-3-5-haiku-20241022")

        await client.query("What is 2 + 2? Just the number.")

        model_used_2 = None
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                model_used_2 = msg.model
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        if model_used_2:
            print(f"使用模型: {model_used_2}")

        # 切换回默认模型
        print("\n--- 切换回默认模型 ---")
        await client.set_model(None)

        await client.query("What is 3 + 3? Just the number.")

        model_used_3 = None
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                model_used_3 = msg.model
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        if model_used_3:
            print(f"使用模型: {model_used_3}")

    print("\n✓ 动态模型切换测试通过")


if __name__ == "__main__":
    asyncio.run(main())

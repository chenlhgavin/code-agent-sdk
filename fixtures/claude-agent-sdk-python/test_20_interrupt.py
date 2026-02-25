#!/usr/bin/env python3
"""测试场景: 中断 (interrupt)

验证在会话中发送中断信号，停止当前响应并发送新查询。
覆盖功能:
- client.interrupt() 中断信号
- 中断后继续对话
- 消息消费与中断的配合
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
    print("=== 测试: 中断 (interrupt) ===\n")

    async with ClaudeSDKClient() as client:
        # 发送一个长任务
        print("--- 发送长任务 ---")
        await client.query("Count from 1 to 100 slowly, one number per line.")

        # 后台消费消息
        messages_before_interrupt = []

        async def consume():
            async for msg in client.receive_response():
                messages_before_interrupt.append(msg)
                if isinstance(msg, AssistantMessage):
                    for block in msg.content:
                        if isinstance(block, TextBlock):
                            preview = block.text[:80].replace("\n", " ")
                            print(f"  Claude: {preview}...")

        consume_task = asyncio.create_task(consume())

        # 等待一段时间后中断
        await asyncio.sleep(2)
        print("\n[发送中断信号...]")

        try:
            await client.interrupt()
            print("✓ 中断信号已发送")
        except Exception as e:
            print(f"中断异常: {e}")

        # 等待消费任务完成
        with contextlib.suppress(asyncio.CancelledError):
            await consume_task

        print(f"中断前收到 {len(messages_before_interrupt)} 条消息")

        # 中断后发送新查询
        print("\n--- 中断后发送新查询 ---")
        await client.query("Just say 'Hello after interrupt!'")

        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        assert len(response_text) > 0, "中断后未收到新回复"

    print("\n✓ 中断测试通过")


if __name__ == "__main__":
    asyncio.run(main())

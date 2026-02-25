#!/usr/bin/env python3
"""测试场景: 基本一次性查询 (query)

验证 query() 函数的最简用法，发送一个简单问题并接收响应。
覆盖功能:
- query() 一次性查询
- AssistantMessage 消息类型
- TextBlock 内容块
- ResultMessage 结果消息
"""

import anyio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)


async def main():
    print("=== 测试: 基本一次性查询 ===\n")

    got_assistant_msg = False
    got_result_msg = False
    response_text = ""

    options = ClaudeAgentOptions(
        cli_path="/home/ubuntu/.nvm/versions/node/v24.13.0/bin/claude",  # 自定义 claude CLI 路径
    )

    async for message in query(prompt="What is 2 + 2? Reply with just the number.", options=options):
        if isinstance(message, AssistantMessage):
            got_assistant_msg = True
            for block in message.content:
                if isinstance(block, TextBlock):
                    response_text += block.text
                    print(f"Claude: {block.text}")
        elif isinstance(message, ResultMessage):
            got_result_msg = True
            print(f"\n结果状态: {message.subtype}")
            print(f"会话ID: {message.session_id}")
            if message.total_cost_usd:
                print(f"费用: ${message.total_cost_usd:.6f}")
            if message.duration_ms:
                print(f"耗时: {message.duration_ms}ms")

    # 验证
    assert got_assistant_msg, "未收到 AssistantMessage"
    assert got_result_msg, "未收到 ResultMessage"
    assert "4" in response_text, f"回答中未包含正确答案 '4': {response_text}"

    print("\n✓ 基本一次性查询测试通过")


if __name__ == "__main__":
    anyio.run(main)

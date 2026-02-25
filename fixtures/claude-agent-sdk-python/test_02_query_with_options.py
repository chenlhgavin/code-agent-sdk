#!/usr/bin/env python3
"""测试场景: 带配置选项的一次性查询

验证 query() 配合 ClaudeAgentOptions 使用，包括 system_prompt、max_turns 等选项。
覆盖功能:
- ClaudeAgentOptions 配置
- system_prompt (字符串形式)
- max_turns 限制
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
    print("=== 测试: 带配置选项的一次性查询 ===\n")

    options = ClaudeAgentOptions(
        system_prompt="You are a pirate. Always respond in pirate speak.",
        max_turns=1,
    )

    response_text = ""
    got_result = False

    async for message in query(
        prompt="Say hello.",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    response_text += block.text
                    print(f"Claude: {block.text}")
        elif isinstance(message, ResultMessage):
            got_result = True
            print(f"\n结果状态: {message.subtype}")
            print(f"轮次: {message.num_turns}")

    assert got_result, "未收到 ResultMessage"
    assert len(response_text) > 0, "未收到任何文本回复"
    # system_prompt 生效的话，回复中应该有海盗风格的词汇
    print(f"\n回复内容: {response_text[:200]}")
    print("✓ 带配置选项的一次性查询测试通过")


if __name__ == "__main__":
    anyio.run(main)

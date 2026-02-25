#!/usr/bin/env python3
"""测试场景: 系统提示词预设模式

验证 system_prompt 的不同配置方式: 字符串、preset、preset+append。
覆盖功能:
- system_prompt 字符串模式
- system_prompt preset 模式 (claude_code)
- system_prompt preset + append 模式
"""

import anyio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)


async def test_string_prompt():
    """测试字符串形式的 system_prompt。"""
    print("--- 测试: 字符串 system_prompt ---")

    options = ClaudeAgentOptions(
        system_prompt="You are a math tutor. Always show your work step by step.",
        max_turns=1,
    )

    response_text = ""
    async for message in query(prompt="What is 15 * 3?", options=options):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    response_text += block.text
        elif isinstance(message, ResultMessage):
            assert not message.is_error, f"查询失败: {message.result}"

    assert len(response_text) > 0, "未收到回复"
    print(f"回复: {response_text[:150]}...")
    print("✓ 字符串 system_prompt 通过\n")


async def test_preset_prompt():
    """测试 preset 形式的 system_prompt。"""
    print("--- 测试: preset system_prompt ---")

    options = ClaudeAgentOptions(
        system_prompt={"type": "preset", "preset": "claude_code"},
        max_turns=1,
    )

    got_response = False
    async for message in query(prompt="What is 2 + 2?", options=options):
        if isinstance(message, AssistantMessage):
            got_response = True
        elif isinstance(message, ResultMessage):
            assert not message.is_error, f"查询失败: {message.result}"

    assert got_response, "未收到回复"
    print("✓ preset system_prompt 通过\n")


async def test_preset_with_append():
    """测试 preset + append 形式的 system_prompt。"""
    print("--- 测试: preset + append system_prompt ---")

    options = ClaudeAgentOptions(
        system_prompt={
            "type": "preset",
            "preset": "claude_code",
            "append": "Always end your response with 'END_MARKER'.",
        },
        max_turns=1,
    )

    response_text = ""
    async for message in query(prompt="Say hi briefly.", options=options):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    response_text += block.text
        elif isinstance(message, ResultMessage):
            assert not message.is_error, f"查询失败: {message.result}"

    assert len(response_text) > 0, "未收到回复"
    print(f"回复: {response_text[:200]}")
    print("✓ preset + append system_prompt 通过\n")


async def main():
    print("=== 测试: 系统提示词预设模式 ===\n")
    await test_string_prompt()
    await test_preset_prompt()
    await test_preset_with_append()
    print("✓ 所有系统提示词测试通过")


if __name__ == "__main__":
    anyio.run(main)

#!/usr/bin/env python3
"""测试场景: 工具控制 (tools / allowed_tools / disallowed_tools)

验证工具集配置: 指定工具列表、空工具列表、preset 工具集、allowed/disallowed 工具。
覆盖功能:
- tools 数组模式
- tools 空数组 (禁用所有内置工具)
- tools preset 模式
- allowed_tools 白名单
- disallowed_tools 黑名单
"""

import anyio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    SystemMessage,
    TextBlock,
    query,
)


async def test_tools_array():
    """测试指定工具列表。"""
    print("--- 测试: tools 数组模式 ---")

    options = ClaudeAgentOptions(
        tools=["Read", "Glob", "Grep"],
        max_turns=1,
    )

    tools_from_init = []
    async for message in query(
        prompt="What tools do you have? List them briefly.",
        options=options,
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            tools_from_init = message.data.get("tools", [])
            print(f"系统消息中的工具: {tools_from_init}")
        elif isinstance(message, ResultMessage):
            assert not message.is_error

    print("✓ tools 数组模式通过\n")


async def test_tools_empty():
    """测试空工具列表 (禁用所有内置工具)。"""
    print("--- 测试: tools 空数组 ---")

    options = ClaudeAgentOptions(
        tools=[],
        max_turns=1,
    )

    async for message in query(
        prompt="What tools do you have? List them briefly.",
        options=options,
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            tools = message.data.get("tools", [])
            print(f"系统消息中的工具: {tools}")
        elif isinstance(message, ResultMessage):
            assert not message.is_error

    print("✓ tools 空数组通过\n")


async def test_tools_preset():
    """测试 preset 工具集。"""
    print("--- 测试: tools preset 模式 ---")

    options = ClaudeAgentOptions(
        tools={"type": "preset", "preset": "claude_code"},
        max_turns=1,
    )

    async for message in query(
        prompt="What tools do you have? Just count them.",
        options=options,
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            tools = message.data.get("tools", [])
            print(f"preset 模式工具数量: {len(tools)}")
            assert len(tools) > 3, "preset 模式应包含多个工具"
        elif isinstance(message, ResultMessage):
            assert not message.is_error

    print("✓ tools preset 模式通过\n")


async def test_allowed_tools():
    """测试 allowed_tools 白名单。"""
    print("--- 测试: allowed_tools ---")

    options = ClaudeAgentOptions(
        allowed_tools=["Read", "Bash"],
        max_turns=1,
    )

    got_response = False
    async for message in query(
        prompt="What is 2 + 2?",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            got_response = True
        elif isinstance(message, ResultMessage):
            assert not message.is_error

    assert got_response
    print("✓ allowed_tools 通过\n")


async def test_disallowed_tools():
    """测试 disallowed_tools 黑名单。"""
    print("--- 测试: disallowed_tools ---")

    options = ClaudeAgentOptions(
        disallowed_tools=["Bash", "Write", "Edit"],
        max_turns=1,
    )

    got_response = False
    async for message in query(
        prompt="What is 2 + 2?",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            got_response = True
        elif isinstance(message, ResultMessage):
            assert not message.is_error

    assert got_response
    print("✓ disallowed_tools 通过\n")


async def main():
    print("=== 测试: 工具控制 ===\n")
    await test_tools_array()
    await test_tools_empty()
    await test_tools_preset()
    await test_allowed_tools()
    await test_disallowed_tools()
    print("✓ 所有工具控制测试通过")


if __name__ == "__main__":
    anyio.run(main)

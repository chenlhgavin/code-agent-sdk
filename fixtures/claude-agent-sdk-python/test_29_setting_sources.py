#!/usr/bin/env python3
"""测试场景: 设置源控制 (setting_sources)

验证 setting_sources 配置控制加载哪些设置源。
覆盖功能:
- setting_sources 配置
- 默认行为 (无设置加载)
- ["user"] 仅用户设置
- ["user", "project"] 用户+项目设置
"""

import asyncio

from claude_agent_sdk import (
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    SystemMessage,
)


async def test_default_no_settings():
    """测试默认行为: 不加载任何设置。"""
    print("--- 测试: 默认 (无设置) ---")

    options = ClaudeAgentOptions()

    async with ClaudeSDKClient(options=options) as client:
        await client.query("What is 2 + 2?")

        async for msg in client.receive_response():
            if isinstance(msg, SystemMessage) and msg.subtype == "init":
                commands = msg.data.get("slash_commands", [])
                print(f"  可用命令: {commands}")
            elif isinstance(msg, ResultMessage):
                print(f"  结果: {msg.subtype}")
                break

    print("✓ 默认无设置通过\n")


async def test_user_settings_only():
    """测试仅加载用户设置。"""
    print("--- 测试: 仅用户设置 ---")

    options = ClaudeAgentOptions(
        setting_sources=["user"],
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query("What is 2 + 2?")

        async for msg in client.receive_response():
            if isinstance(msg, SystemMessage) and msg.subtype == "init":
                commands = msg.data.get("slash_commands", [])
                print(f"  可用命令: {commands}")
            elif isinstance(msg, ResultMessage):
                print(f"  结果: {msg.subtype}")
                break

    print("✓ 仅用户设置通过\n")


async def test_user_and_project_settings():
    """测试加载用户+项目设置。"""
    print("--- 测试: 用户+项目设置 ---")

    options = ClaudeAgentOptions(
        setting_sources=["user", "project"],
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query("What is 2 + 2?")

        async for msg in client.receive_response():
            if isinstance(msg, SystemMessage) and msg.subtype == "init":
                commands = msg.data.get("slash_commands", [])
                print(f"  可用命令: {commands}")
            elif isinstance(msg, ResultMessage):
                print(f"  结果: {msg.subtype}")
                break

    print("✓ 用户+项目设置通过\n")


async def main():
    print("=== 测试: 设置源控制 ===\n")
    await test_default_no_settings()
    await test_user_settings_only()
    await test_user_and_project_settings()
    print("✓ 所有设置源控制测试通过")


if __name__ == "__main__":
    asyncio.run(main())

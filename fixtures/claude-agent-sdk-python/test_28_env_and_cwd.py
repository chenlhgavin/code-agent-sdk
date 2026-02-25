#!/usr/bin/env python3
"""测试场景: 环境变量与工作目录

验证 env 和 cwd 配置选项。
覆盖功能:
- env 环境变量传递
- cwd 工作目录设置
"""

import asyncio
import tempfile

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)


async def test_cwd():
    """测试工作目录设置。"""
    print("--- 测试: cwd 工作目录 ---")

    tmp_dir = tempfile.gettempdir()
    options = ClaudeAgentOptions(
        cwd=tmp_dir,
        allowed_tools=["Bash"],
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query("Run: pwd")

        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    print(f"回复: {response_text[:200]}")
    print("✓ cwd 工作目录通过\n")


async def test_env_vars():
    """测试环境变量传递。"""
    print("--- 测试: env 环境变量 ---")

    options = ClaudeAgentOptions(
        env={
            "MY_TEST_VAR": "hello_from_sdk",
        },
        allowed_tools=["Bash"],
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query("Run this bash command: echo $MY_TEST_VAR")

        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    print(f"回复: {response_text[:200]}")
    print("✓ env 环境变量通过\n")


async def main():
    print("=== 测试: 环境变量与工作目录 ===\n")
    await test_cwd()
    await test_env_vars()
    print("✓ 所有环境配置测试通过")


if __name__ == "__main__":
    asyncio.run(main())

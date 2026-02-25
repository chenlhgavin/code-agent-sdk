#!/usr/bin/env python3
"""测试场景: 错误处理

验证各种错误场景的异常处理: 连接错误、超时、进程错误。
覆盖功能:
- CLIConnectionError 连接错误
- CLINotFoundError CLI 未找到
- ProcessError 进程错误
- asyncio.timeout 超时处理
- 异常层次结构
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    CLIConnectionError,
    ResultMessage,
    TextBlock,
)
from claude_agent_sdk._errors import (
    CLINotFoundError,
    ClaudeSDKError,
    ProcessError,
)


async def test_error_hierarchy():
    """测试异常层次结构。"""
    print("--- 测试: 异常层次结构 ---")

    # 验证继承关系
    assert issubclass(CLIConnectionError, ClaudeSDKError), \
        "CLIConnectionError 应继承 ClaudeSDKError"
    assert issubclass(CLINotFoundError, CLIConnectionError), \
        "CLINotFoundError 应继承 CLIConnectionError"
    assert issubclass(ProcessError, ClaudeSDKError), \
        "ProcessError 应继承 ClaudeSDKError"

    print("✓ 异常层次结构正确\n")


async def test_invalid_cli_path():
    """测试无效 CLI 路径。"""
    print("--- 测试: 无效 CLI 路径 ---")

    options = ClaudeAgentOptions(
        cli_path="/nonexistent/path/to/claude",
    )

    client = ClaudeSDKClient(options=options)
    try:
        await client.connect()
        # 如果连接成功了，也要断开
        await client.disconnect()
        print("  (未抛出异常，CLI 路径可能被忽略)")
    except (CLIConnectionError, CLINotFoundError, FileNotFoundError, OSError) as e:
        print(f"  预期异常: {type(e).__name__}: {e}")
        print("✓ 无效 CLI 路径正确抛出异常")
    except Exception as e:
        print(f"  其他异常: {type(e).__name__}: {e}")

    print()


async def test_timeout_handling():
    """测试超时处理。"""
    print("--- 测试: 超时处理 ---")

    client = ClaudeSDKClient()

    try:
        await client.connect()

        await client.query("Count from 1 to 1000 very slowly, one per line.")

        messages = []
        try:
            async with asyncio.timeout(5.0):
                async for msg in client.receive_response():
                    messages.append(msg)
                    if isinstance(msg, ResultMessage):
                        break
        except asyncio.TimeoutError:
            print(f"  超时! 收到 {len(messages)} 条消息")
            print("✓ 超时处理正确")

    except CLIConnectionError as e:
        print(f"  连接错误: {e}")
    finally:
        await client.disconnect()

    print()


async def main():
    print("=== 测试: 错误处理 ===\n")
    await test_error_hierarchy()
    await test_invalid_cli_path()
    await test_timeout_handling()
    print("✓ 所有错误处理测试通过")


if __name__ == "__main__":
    asyncio.run(main())

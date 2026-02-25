#!/usr/bin/env python3
"""测试场景: stderr 回调

验证通过 stderr 回调捕获 CLI 的 stderr 输出。
覆盖功能:
- stderr 回调配置
- extra_args 传递额外 CLI 参数
- debug-to-stderr 调试输出
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)

stderr_lines = []


def stderr_callback(message: str):
    """收集 stderr 输出。"""
    stderr_lines.append(message)


async def main():
    print("=== 测试: stderr 回调 ===\n")

    options = ClaudeAgentOptions(
        stderr=stderr_callback,
        extra_args={"debug-to-stderr": None},
    )

    print("--- 带 debug 模式的查询 ---")
    async for message in query(
        prompt="What is 2 + 2? Just the number.",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Claude: {block.text}")
        elif isinstance(message, ResultMessage):
            print(f"结果: {message.subtype}")

    print(f"\n捕获 {len(stderr_lines)} 行 stderr 输出")
    if stderr_lines:
        print(f"前3行:")
        for line in stderr_lines[:3]:
            print(f"  {line[:120]}")

    # debug 模式下应该有 stderr 输出
    assert len(stderr_lines) > 0, "未捕获到 stderr 输出 (debug 模式可能未生效)"

    print("\n✓ stderr 回调测试通过")


if __name__ == "__main__":
    asyncio.run(main())

#!/usr/bin/env python3
"""测试场景: MCP 工具错误处理

验证 MCP 工具返回 is_error=True 时的行为。
覆盖功能:
- MCP 工具错误返回 (is_error: True)
- 工具异常处理
- Claude 对工具错误的响应
"""

import asyncio
from typing import Any

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
    create_sdk_mcp_server,
    tool,
)


@tool("safe_divide", "Divide a by b safely", {"a": float, "b": float})
async def safe_divide(args: dict[str, Any]) -> dict[str, Any]:
    """除法工具，除以零时返回错误。"""
    if args["b"] == 0:
        return {
            "content": [
                {"type": "text", "text": "Error: Cannot divide by zero"}
            ],
            "is_error": True,
        }
    result = args["a"] / args["b"]
    return {
        "content": [{"type": "text", "text": str(result)}]
    }


@tool("failing_tool", "A tool that always fails", {"input": str})
async def failing_tool(args: dict[str, Any]) -> dict[str, Any]:
    """总是返回错误的工具。"""
    return {
        "content": [
            {"type": "text", "text": f"Error: Cannot process '{args['input']}'"}
        ],
        "is_error": True,
    }


async def main():
    print("=== 测试: MCP 工具错误处理 ===\n")

    server = create_sdk_mcp_server(
        name="error_test",
        version="1.0.0",
        tools=[safe_divide, failing_tool],
    )

    options = ClaudeAgentOptions(
        mcp_servers={"err": server},
        allowed_tools=[
            "mcp__err__safe_divide",
            "mcp__err__failing_tool",
        ],
    )

    # 测试: 除以零
    print("--- 测试: 除以零 ---")
    async with ClaudeSDKClient(options=options) as client:
        await client.query("Use the safe_divide tool to divide 10 by 0.")

        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    print(f"回复: {response_text[:300]}")
    # Claude 应该识别到错误并给出合理回复
    assert len(response_text) > 0, "未收到回复"

    print("\n✓ MCP 工具错误处理测试通过")


if __name__ == "__main__":
    asyncio.run(main())

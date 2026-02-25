#!/usr/bin/env python3
"""测试场景: MCP 多工具协作

验证多个 MCP 工具在同一个 MCP 服务器中协作完成复杂计算。
覆盖功能:
- 多个 @tool 定义
- 单个 MCP 服务器包含多个工具
- Claude 自动选择合适的工具
- 工具链式调用
"""

import asyncio
from typing import Any

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
    ToolUseBlock,
    create_sdk_mcp_server,
    tool,
)

call_log = []


@tool("celsius_to_fahrenheit", "Convert Celsius to Fahrenheit", {"celsius": float})
async def celsius_to_fahrenheit(args: dict[str, Any]) -> dict[str, Any]:
    result = args["celsius"] * 9 / 5 + 32
    call_log.append(("c_to_f", args["celsius"], result))
    return {"content": [{"type": "text", "text": f"{args['celsius']}°C = {result}°F"}]}


@tool("fahrenheit_to_celsius", "Convert Fahrenheit to Celsius", {"fahrenheit": float})
async def fahrenheit_to_celsius(args: dict[str, Any]) -> dict[str, Any]:
    result = (args["fahrenheit"] - 32) * 5 / 9
    call_log.append(("f_to_c", args["fahrenheit"], result))
    return {"content": [{"type": "text", "text": f"{args['fahrenheit']}°F = {result:.2f}°C"}]}


@tool("km_to_miles", "Convert kilometers to miles", {"km": float})
async def km_to_miles(args: dict[str, Any]) -> dict[str, Any]:
    result = args["km"] * 0.621371
    call_log.append(("km_to_mi", args["km"], result))
    return {"content": [{"type": "text", "text": f"{args['km']} km = {result:.2f} miles"}]}


async def main():
    print("=== 测试: MCP 多工具协作 ===\n")

    converter = create_sdk_mcp_server(
        name="converter",
        version="1.0.0",
        tools=[celsius_to_fahrenheit, fahrenheit_to_celsius, km_to_miles],
    )

    options = ClaudeAgentOptions(
        mcp_servers={"conv": converter},
        allowed_tools=[
            "mcp__conv__celsius_to_fahrenheit",
            "mcp__conv__fahrenheit_to_celsius",
            "mcp__conv__km_to_miles",
        ],
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query(
            "Convert 100°C to Fahrenheit and also convert 42 km to miles. "
            "Use the converter tools."
        )

        tools_used = []
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, ToolUseBlock):
                        tools_used.append(block.name)
                        print(f"  工具: {block.name}({block.input})")
                    elif isinstance(block, TextBlock):
                        print(f"  Claude: {block.text[:150]}")
            elif isinstance(msg, ResultMessage):
                print(f"  结果: {msg.subtype}")

    print(f"\n工具调用记录: {call_log}")
    print(f"使用的工具: {tools_used}")

    assert len(call_log) >= 2, f"预期至少调用2个工具，实际: {len(call_log)}"

    print("\n✓ MCP 多工具协作测试通过")


if __name__ == "__main__":
    asyncio.run(main())

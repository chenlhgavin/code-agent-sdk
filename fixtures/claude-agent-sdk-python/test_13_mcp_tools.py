#!/usr/bin/env python3
"""测试场景: 进程内 MCP 工具 (@tool + create_sdk_mcp_server)

验证通过 @tool 装饰器定义工具并通过 create_sdk_mcp_server 注册为进程内 MCP 服务器。
覆盖功能:
- @tool 装饰器定义工具
- create_sdk_mcp_server 创建 MCP 服务器
- mcp_servers 配置
- allowed_tools 中的 mcp__<server>__<tool> 格式
- 工具调用与返回结果
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

# 记录工具调用
tool_calls = []


@tool("add", "Add two numbers together", {"a": float, "b": float})
async def add_numbers(args: dict[str, Any]) -> dict[str, Any]:
    """加法工具。"""
    result = args["a"] + args["b"]
    tool_calls.append({"name": "add", "args": args, "result": result})
    return {
        "content": [{"type": "text", "text": f"{args['a']} + {args['b']} = {result}"}]
    }


@tool("multiply", "Multiply two numbers", {"a": float, "b": float})
async def multiply_numbers(args: dict[str, Any]) -> dict[str, Any]:
    """乘法工具。"""
    result = args["a"] * args["b"]
    tool_calls.append({"name": "multiply", "args": args, "result": result})
    return {
        "content": [{"type": "text", "text": f"{args['a']} * {args['b']} = {result}"}]
    }


@tool("divide", "Divide a by b", {"a": float, "b": float})
async def divide_numbers(args: dict[str, Any]) -> dict[str, Any]:
    """除法工具 (含错误处理)。"""
    if args["b"] == 0:
        return {
            "content": [{"type": "text", "text": "Error: Division by zero"}],
            "is_error": True,
        }
    result = args["a"] / args["b"]
    tool_calls.append({"name": "divide", "args": args, "result": result})
    return {
        "content": [{"type": "text", "text": f"{args['a']} / {args['b']} = {result}"}]
    }


async def main():
    print("=== 测试: 进程内 MCP 工具 ===\n")

    # 创建 MCP 服务器
    calculator = create_sdk_mcp_server(
        name="calculator",
        version="1.0.0",
        tools=[add_numbers, multiply_numbers, divide_numbers],
    )

    options = ClaudeAgentOptions(
        mcp_servers={"calc": calculator},
        allowed_tools=[
            "mcp__calc__add",
            "mcp__calc__multiply",
            "mcp__calc__divide",
        ],
    )

    # 测试: 使用加法工具
    print("--- 测试: 加法 ---")
    async with ClaudeSDKClient(options=options) as client:
        await client.query("Calculate 15 + 27 using the add tool.")

        tool_used = False
        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
                    elif isinstance(block, ToolUseBlock):
                        tool_used = True
                        print(f"  工具调用: {block.name}({block.input})")
            elif isinstance(msg, ResultMessage):
                print(f"  结果: {msg.subtype}")

    print(f"  回复: {response_text[:200]}")
    assert len(tool_calls) > 0, "MCP 工具未被调用"
    assert any(c["name"] == "add" for c in tool_calls), "add 工具未被调用"
    print(f"\n工具调用记录: {tool_calls}")
    print("\n✓ 进程内 MCP 工具测试通过")


if __name__ == "__main__":
    asyncio.run(main())

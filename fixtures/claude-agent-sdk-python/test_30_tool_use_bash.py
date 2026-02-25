#!/usr/bin/env python3
"""测试场景: 工具使用与 Bash 命令

验证 Claude 使用内置工具 (Bash) 执行命令，并正确返回 ToolUseBlock 和 ToolResultBlock。
覆盖功能:
- ToolUseBlock 工具调用块
- ToolResultBlock 工具结果块
- Bash 工具执行
- 工具调用链路追踪
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
    ToolResultBlock,
    ToolUseBlock,
    UserMessage,
)


async def main():
    print("=== 测试: 工具使用与 Bash 命令 ===\n")

    options = ClaudeAgentOptions(
        allowed_tools=["Bash"],
    )

    tool_uses = []
    tool_results = []

    async with ClaudeSDKClient(options=options) as client:
        await client.query("Run this bash command: echo 'tool use test 123'")

        async for msg in client.receive_messages():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, ToolUseBlock):
                        tool_uses.append({
                            "name": block.name,
                            "id": block.id,
                            "input": block.input,
                        })
                        print(f"[ToolUse] name={block.name}, id={block.id}")
                        print(f"  input: {block.input}")
                    elif isinstance(block, TextBlock):
                        print(f"[Text] {block.text[:100]}")

            elif isinstance(msg, UserMessage):
                for block in msg.content:
                    if isinstance(block, ToolResultBlock):
                        tool_results.append({
                            "tool_use_id": block.tool_use_id,
                            "content": block.content,
                            "is_error": block.is_error,
                        })
                        content_preview = str(block.content)[:100] if block.content else "None"
                        print(f"[ToolResult] tool_use_id={block.tool_use_id}")
                        print(f"  content: {content_preview}")
                        print(f"  is_error: {block.is_error}")

            elif isinstance(msg, ResultMessage):
                print(f"\n[Result] {msg.subtype}")
                break

    print(f"\n工具调用: {len(tool_uses)} 次")
    print(f"工具结果: {len(tool_results)} 次")

    assert len(tool_uses) > 0, "未检测到 ToolUseBlock"
    assert any(t["name"] == "Bash" for t in tool_uses), "未使用 Bash 工具"

    # 验证 ToolUseBlock 结构
    for tu in tool_uses:
        assert tu["name"], "ToolUseBlock 缺少 name"
        assert tu["id"], "ToolUseBlock 缺少 id"

    print("\n✓ 工具使用与 Bash 命令测试通过")


if __name__ == "__main__":
    asyncio.run(main())

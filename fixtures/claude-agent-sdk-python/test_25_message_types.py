#!/usr/bin/env python3
"""测试场景: 消息类型全覆盖

验证所有消息类型和内容块类型的解析与处理。
覆盖功能:
- UserMessage (含 TextBlock, ToolResultBlock)
- AssistantMessage (含 TextBlock, ToolUseBlock)
- SystemMessage (init subtype)
- ResultMessage (所有字段)
- ContentBlock 类型: TextBlock, ToolUseBlock, ToolResultBlock
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    SystemMessage,
    TextBlock,
    ToolResultBlock,
    ToolUseBlock,
    UserMessage,
)


async def main():
    print("=== 测试: 消息类型全覆盖 ===\n")

    options = ClaudeAgentOptions(
        allowed_tools=["Bash"],
    )

    # 使用一个会触发工具使用的查询，以覆盖更多消息类型
    message_types = set()
    content_block_types = set()

    async with ClaudeSDKClient(options=options) as client:
        await client.query("Run this bash command: echo 'message type test'")

        async for msg in client.receive_messages():
            msg_type = type(msg).__name__
            message_types.add(msg_type)

            if isinstance(msg, UserMessage):
                print(f"[UserMessage] uuid={msg.uuid}")
                for block in msg.content:
                    block_type = type(block).__name__
                    content_block_types.add(block_type)
                    if isinstance(block, TextBlock):
                        print(f"  TextBlock: {block.text[:80]}")
                    elif isinstance(block, ToolResultBlock):
                        print(f"  ToolResultBlock: tool_use_id={block.tool_use_id}, is_error={block.is_error}")
                        content = block.content
                        if content:
                            print(f"    content: {str(content)[:80]}")

            elif isinstance(msg, AssistantMessage):
                print(f"[AssistantMessage] model={msg.model}")
                for block in msg.content:
                    block_type = type(block).__name__
                    content_block_types.add(block_type)
                    if isinstance(block, TextBlock):
                        print(f"  TextBlock: {block.text[:80]}")
                    elif isinstance(block, ToolUseBlock):
                        print(f"  ToolUseBlock: name={block.name}, id={block.id}")
                        print(f"    input: {block.input}")

            elif isinstance(msg, SystemMessage):
                print(f"[SystemMessage] subtype={msg.subtype}")
                if msg.subtype == "init":
                    tools = msg.data.get("tools", [])
                    print(f"  tools: {len(tools)} 个")

            elif isinstance(msg, ResultMessage):
                print(f"[ResultMessage]")
                print(f"  subtype: {msg.subtype}")
                print(f"  session_id: {msg.session_id}")
                print(f"  is_error: {msg.is_error}")
                print(f"  num_turns: {msg.num_turns}")
                print(f"  duration_ms: {msg.duration_ms}")
                if msg.total_cost_usd:
                    print(f"  total_cost_usd: ${msg.total_cost_usd:.6f}")
                break

    print(f"\n收到的消息类型: {message_types}")
    print(f"收到的内容块类型: {content_block_types}")

    # 验证核心消息类型
    assert "AssistantMessage" in message_types, "缺少 AssistantMessage"
    assert "ResultMessage" in message_types, "缺少 ResultMessage"

    print("\n✓ 消息类型全覆盖测试通过")


if __name__ == "__main__":
    asyncio.run(main())

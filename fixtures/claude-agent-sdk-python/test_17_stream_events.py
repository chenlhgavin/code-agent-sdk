#!/usr/bin/env python3
"""测试场景: 流式部分消息 (StreamEvent)

验证 include_partial_messages=True 时能接收 StreamEvent 流式事件。
覆盖功能:
- include_partial_messages 配置
- StreamEvent 消息类型
- 流式增量更新
"""

import asyncio

from claude_agent_sdk import (
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
)
from claude_agent_sdk.types import (
    AssistantMessage,
    StreamEvent,
    SystemMessage,
    TextBlock,
)


async def main():
    print("=== 测试: 流式部分消息 (StreamEvent) ===\n")

    options = ClaudeAgentOptions(
        include_partial_messages=True,
        max_turns=1,
    )

    stream_events = []
    assistant_messages = []
    result_message = None

    async with ClaudeSDKClient(options=options) as client:
        await client.query("Write a haiku about coding.")

        async for msg in client.receive_response():
            if isinstance(msg, StreamEvent):
                stream_events.append(msg)
                # StreamEvent 包含 event 字段 (原始 API 流事件)
                event_type = msg.event.get("type", "unknown") if msg.event else "unknown"
                if len(stream_events) <= 5:
                    print(f"  [StreamEvent] type={event_type}, session={msg.session_id}")
            elif isinstance(msg, AssistantMessage):
                assistant_messages.append(msg)
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"  [AssistantMessage] {block.text[:100]}")
            elif isinstance(msg, ResultMessage):
                result_message = msg
                print(f"  [ResultMessage] status={msg.subtype}")

    print(f"\n统计:")
    print(f"  StreamEvent 数量: {len(stream_events)}")
    print(f"  AssistantMessage 数量: {len(assistant_messages)}")

    assert result_message is not None, "未收到 ResultMessage"
    assert len(stream_events) > 0, "未收到任何 StreamEvent (include_partial_messages 未生效)"

    # 验证 StreamEvent 结构
    first_event = stream_events[0]
    assert hasattr(first_event, "event"), "StreamEvent 缺少 event 字段"
    assert hasattr(first_event, "session_id"), "StreamEvent 缺少 session_id 字段"

    print("\n✓ 流式部分消息测试通过")


if __name__ == "__main__":
    asyncio.run(main())

#!/usr/bin/env python3
"""测试场景: Hook 拦截 — PostToolUse

验证 PostToolUse Hook 能在工具执行后审查输出并提供反馈。
覆盖功能:
- hooks 配置 (PostToolUse)
- PostToolUse 回调
- reason / systemMessage 反馈字段
- additionalContext 附加上下文
"""

import asyncio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
)
from claude_agent_sdk.types import (
    HookContext,
    HookInput,
    HookJSONOutput,
    HookMatcher,
)

post_hook_calls = []


async def posttooluse_hook(
    input_data: HookInput,
    tool_use_id: str | None,
    context: HookContext,
) -> HookJSONOutput:
    """PostToolUse Hook: 审查工具输出。"""
    tool_response = input_data.get("tool_response", "")

    post_hook_calls.append({
        "tool_name": input_data.get("tool_name"),
        "response_preview": str(tool_response)[:100],
    })

    print(f"  [PostToolUse Hook] tool={input_data.get('tool_name')}")

    # 如果输出包含 error，添加额外上下文
    if "error" in str(tool_response).lower():
        return {
            "reason": "Tool execution encountered an error",
            "systemMessage": "Warning: tool output contains error",
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": "The command encountered an error. Consider trying a different approach.",
            },
        }

    return {}


async def main():
    print("=== 测试: Hook 拦截 — PostToolUse ===\n")

    options = ClaudeAgentOptions(
        allowed_tools=["Bash"],
        hooks={
            "PostToolUse": [
                HookMatcher(matcher="Bash", hooks=[posttooluse_hook]),
            ],
        },
    )

    async with ClaudeSDKClient(options=options) as client:
        # 执行一个会产生输出的命令
        print("--- 执行 bash 命令 ---")
        await client.query("Run this bash command: echo 'PostToolUse hook test'")

        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(f"Claude: {block.text[:200]}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    print(f"\nPostToolUse Hook 被调用 {len(post_hook_calls)} 次")
    for call in post_hook_calls:
        print(f"  - tool={call['tool_name']}, response={call['response_preview'][:60]}")

    assert len(post_hook_calls) > 0, "PostToolUse Hook 未被调用"
    print("\n✓ PostToolUse Hook 测试通过")


if __name__ == "__main__":
    asyncio.run(main())

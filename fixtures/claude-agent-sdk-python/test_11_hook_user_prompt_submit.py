#!/usr/bin/env python3
"""测试场景: Hook — UserPromptSubmit

验证 UserPromptSubmit Hook 能在用户提交提示时注入额外上下文。
覆盖功能:
- hooks 配置 (UserPromptSubmit)
- additionalContext 注入
- Hook 对 Claude 行为的影响
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

hook_invoked = False


async def user_prompt_hook(
    input_data: HookInput,
    tool_use_id: str | None,
    context: HookContext,
) -> HookJSONOutput:
    """UserPromptSubmit Hook: 注入额外上下文。"""
    global hook_invoked
    hook_invoked = True
    print("  [UserPromptSubmit Hook] 注入额外上下文")

    return {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": "The user's favorite programming language is Rust.",
        }
    }


async def main():
    print("=== 测试: Hook — UserPromptSubmit ===\n")

    options = ClaudeAgentOptions(
        hooks={
            "UserPromptSubmit": [
                HookMatcher(matcher=None, hooks=[user_prompt_hook]),
            ],
        },
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query("What is my favorite programming language? Just name it.")

        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
                        print(f"Claude: {block.text}")
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

    assert hook_invoked, "UserPromptSubmit Hook 未被调用"
    # Hook 注入了 "Rust" 作为上下文，Claude 应该回答 Rust
    assert "rust" in response_text.lower(), f"Hook 注入的上下文未生效，回复: {response_text}"

    print("\n✓ UserPromptSubmit Hook 测试通过")


if __name__ == "__main__":
    asyncio.run(main())

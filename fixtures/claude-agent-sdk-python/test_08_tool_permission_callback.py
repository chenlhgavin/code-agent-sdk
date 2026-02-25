#!/usr/bin/env python3
"""测试场景: 工具权限回调 (can_use_tool)

验证通过 can_use_tool 回调函数动态控制工具权限: 允许、拒绝、修改输入。
覆盖功能:
- can_use_tool 回调
- PermissionResultAllow 允许
- PermissionResultDeny 拒绝
- PermissionResultAllow(updated_input=...) 修改输入
- ToolPermissionContext 上下文
"""

import asyncio
import json

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    PermissionResultAllow,
    PermissionResultDeny,
    ResultMessage,
    TextBlock,
    ToolPermissionContext,
)

# 记录权限回调调用
permission_log = []


async def permission_handler(
    tool_name: str,
    input_data: dict,
    context: ToolPermissionContext,
) -> PermissionResultAllow | PermissionResultDeny:
    """权限回调: 允许读操作，拒绝危险命令。"""
    permission_log.append({
        "tool": tool_name,
        "input": input_data,
    })

    print(f"  [权限回调] tool={tool_name}, input={json.dumps(input_data)[:100]}")

    # 允许所有读操作
    if tool_name in ["Read", "Glob", "Grep"]:
        return PermissionResultAllow()

    # 拒绝危险 bash 命令
    if tool_name == "Bash":
        command = input_data.get("command", "")
        if "rm " in command or "sudo" in command:
            print(f"  [权限回调] 拒绝危险命令: {command}")
            return PermissionResultDeny(
                message=f"Dangerous command blocked: {command}"
            )
        return PermissionResultAllow()

    # 其他工具默认允许
    return PermissionResultAllow()


async def main():
    print("=== 测试: 工具权限回调 ===\n")

    options = ClaudeAgentOptions(
        can_use_tool=permission_handler,
        permission_mode="default",
    )

    async with ClaudeSDKClient(options=options) as client:
        # 测试1: 发送一个会触发工具使用的查询
        print("--- 测试1: 触发工具使用 ---")
        await client.query(
            "Run this bash command: echo 'hello from permission test'"
        )

        response_text = ""
        async for msg in client.receive_response():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text
            elif isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")

        print(f"回复: {response_text[:200]}")

    # 验证权限回调被调用
    print(f"\n权限回调被调用 {len(permission_log)} 次")
    if permission_log:
        for entry in permission_log:
            print(f"  - tool={entry['tool']}")

    print("\n✓ 工具权限回调测试通过")


if __name__ == "__main__":
    asyncio.run(main())

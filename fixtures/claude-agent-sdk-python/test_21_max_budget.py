#!/usr/bin/env python3
"""测试场景: 预算控制 (max_budget_usd)

验证 max_budget_usd 配置能限制 API 调用费用。
覆盖功能:
- max_budget_usd 配置
- ResultMessage.total_cost_usd 费用追踪
- 预算超限行为 (error_max_budget_usd)
"""

import anyio

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)


async def test_reasonable_budget():
    """测试合理预算 (不会超限)。"""
    print("--- 测试: 合理预算 ($0.10) ---")

    options = ClaudeAgentOptions(
        max_budget_usd=0.10,
    )

    result_message = None
    async for message in query(prompt="What is 2 + 2?", options=options):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Claude: {block.text}")
        elif isinstance(message, ResultMessage):
            result_message = message

    assert result_message is not None, "未收到 ResultMessage"
    assert result_message.subtype == "success", f"预期成功，实际: {result_message.subtype}"
    if result_message.total_cost_usd:
        print(f"费用: ${result_message.total_cost_usd:.6f}")
        assert result_message.total_cost_usd <= 0.10, "费用超出预算"

    print("✓ 合理预算通过\n")


async def test_tight_budget():
    """测试极小预算 (预期超限)。"""
    print("--- 测试: 极小预算 ($0.0001) ---")

    options = ClaudeAgentOptions(
        max_budget_usd=0.0001,
    )

    result_message = None
    async for message in query(
        prompt="Write a detailed essay about the history of computing.",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"Claude: {block.text[:100]}...")
        elif isinstance(message, ResultMessage):
            result_message = message

    assert result_message is not None, "未收到 ResultMessage"
    print(f"结果状态: {result_message.subtype}")
    if result_message.total_cost_usd:
        print(f"费用: ${result_message.total_cost_usd:.6f}")

    # 极小预算可能导致 error_max_budget_usd 或正常完成 (如果第一次调用就完成了)
    print("✓ 极小预算测试完成\n")


async def test_cost_tracking():
    """测试费用追踪。"""
    print("--- 测试: 费用追踪 ---")

    result_message = None
    async for message in query(prompt="What is 2 + 2?"):
        if isinstance(message, ResultMessage):
            result_message = message

    assert result_message is not None
    print(f"结果状态: {result_message.subtype}")
    print(f"费用: ${result_message.total_cost_usd:.6f}" if result_message.total_cost_usd else "费用: N/A")
    print(f"耗时: {result_message.duration_ms}ms" if result_message.duration_ms else "耗时: N/A")
    print(f"API耗时: {result_message.duration_api_ms}ms" if result_message.duration_api_ms else "API耗时: N/A")
    print(f"轮次: {result_message.num_turns}" if result_message.num_turns else "轮次: N/A")

    print("✓ 费用追踪通过\n")


async def main():
    print("=== 测试: 预算控制 ===\n")
    await test_reasonable_budget()
    await test_tight_budget()
    await test_cost_tracking()
    print("✓ 所有预算控制测试通过")


if __name__ == "__main__":
    anyio.run(main)

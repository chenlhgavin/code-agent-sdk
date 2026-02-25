#!/usr/bin/env python3
"""测试场景: 自定义 Agent (AgentDefinition)

验证通过 AgentDefinition 定义自定义子 Agent，包括指定工具、模型、提示词。
覆盖功能:
- agents 配置
- AgentDefinition 定义
- description / prompt / tools / model 字段
- 多 Agent 定义
"""

import anyio

from claude_agent_sdk import (
    AgentDefinition,
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)


async def test_single_agent():
    """测试单个自定义 Agent。"""
    print("--- 测试: 单个自定义 Agent ---")

    options = ClaudeAgentOptions(
        agents={
            "math-helper": AgentDefinition(
                description="A math helper that solves arithmetic problems",
                prompt="You are a math helper. Solve arithmetic problems step by step. Be concise.",
                tools=["Read"],
                model="sonnet",
            ),
        },
    )

    response_text = ""
    async for message in query(
        prompt="Use the math-helper agent to calculate 25 * 4.",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    response_text += block.text
        elif isinstance(message, ResultMessage):
            print(f"结果: {message.subtype}")
            if message.total_cost_usd:
                print(f"费用: ${message.total_cost_usd:.6f}")

    print(f"回复: {response_text[:300]}")
    assert len(response_text) > 0, "未收到回复"
    print("✓ 单个自定义 Agent 通过\n")


async def test_multiple_agents():
    """测试多个自定义 Agent。"""
    print("--- 测试: 多个自定义 Agent ---")

    options = ClaudeAgentOptions(
        agents={
            "analyzer": AgentDefinition(
                description="Analyzes text and provides insights",
                prompt="You are a text analyzer. Provide brief analysis of given text.",
                tools=["Read"],
            ),
            "summarizer": AgentDefinition(
                description="Summarizes content concisely",
                prompt="You are a summarizer. Provide very brief summaries.",
                tools=["Read"],
                model="sonnet",
            ),
        },
    )

    response_text = ""
    async for message in query(
        prompt="Use the summarizer agent to summarize this: 'Python is a high-level programming language known for its simplicity and readability.'",
        options=options,
    ):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    response_text += block.text
        elif isinstance(message, ResultMessage):
            print(f"结果: {message.subtype}")

    print(f"回复: {response_text[:300]}")
    assert len(response_text) > 0, "未收到回复"
    print("✓ 多个自定义 Agent 通过\n")


async def main():
    print("=== 测试: 自定义 Agent ===\n")
    await test_single_agent()
    await test_multiple_agents()
    print("✓ 所有自定义 Agent 测试通过")


if __name__ == "__main__":
    anyio.run(main)

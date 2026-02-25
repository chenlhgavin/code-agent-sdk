#!/usr/bin/env python3
"""测试场景: 结构化输出 (output_format + JSON Schema)

验证通过 output_format 配置 JSON Schema 约束 Claude 的输出格式。
覆盖功能:
- output_format 配置
- JSON Schema 约束
- ResultMessage.structured_output 字段
- 嵌套对象 / 数组 / 枚举类型
"""

import anyio

from claude_agent_sdk import (
    ClaudeAgentOptions,
    ResultMessage,
    query,
)


async def test_simple_schema():
    """测试简单 JSON Schema 输出。"""
    print("--- 测试: 简单 JSON Schema ---")

    schema = {
        "type": "object",
        "properties": {
            "answer": {"type": "number"},
            "explanation": {"type": "string"},
        },
        "required": ["answer", "explanation"],
    }

    options = ClaudeAgentOptions(
        output_format={"type": "json_schema", "schema": schema},
    )

    result_message = None
    async for message in query(
        prompt="What is 7 * 8? Provide the answer and a brief explanation.",
        options=options,
    ):
        if isinstance(message, ResultMessage):
            result_message = message

    assert result_message is not None, "未收到 ResultMessage"
    assert not result_message.is_error, f"查询失败: {result_message.result}"
    assert result_message.structured_output is not None, "未收到结构化输出"

    output = result_message.structured_output
    print(f"结构化输出: {output}")
    assert "answer" in output, "缺少 answer 字段"
    assert "explanation" in output, "缺少 explanation 字段"
    assert output["answer"] == 56, f"答案不正确: {output['answer']}"

    print("✓ 简单 JSON Schema 通过\n")


async def test_nested_schema():
    """测试嵌套 JSON Schema 输出。"""
    print("--- 测试: 嵌套 JSON Schema ---")

    schema = {
        "type": "object",
        "properties": {
            "analysis": {
                "type": "object",
                "properties": {
                    "word_count": {"type": "number"},
                    "language": {"type": "string"},
                },
                "required": ["word_count", "language"],
            },
            "keywords": {
                "type": "array",
                "items": {"type": "string"},
            },
        },
        "required": ["analysis", "keywords"],
    }

    options = ClaudeAgentOptions(
        output_format={"type": "json_schema", "schema": schema},
    )

    result_message = None
    async for message in query(
        prompt="Analyze this text: 'Python is great for data science'. Provide word count, language, and keywords.",
        options=options,
    ):
        if isinstance(message, ResultMessage):
            result_message = message

    assert result_message is not None, "未收到 ResultMessage"
    assert result_message.structured_output is not None, "未收到结构化输出"

    output = result_message.structured_output
    print(f"结构化输出: {output}")
    assert "analysis" in output, "缺少 analysis 字段"
    assert "keywords" in output, "缺少 keywords 字段"
    assert isinstance(output["analysis"], dict), "analysis 应为对象"
    assert isinstance(output["keywords"], list), "keywords 应为数组"

    print("✓ 嵌套 JSON Schema 通过\n")


async def test_enum_schema():
    """测试带枚举约束的 JSON Schema。"""
    print("--- 测试: 枚举 JSON Schema ---")

    schema = {
        "type": "object",
        "properties": {
            "sentiment": {
                "type": "string",
                "enum": ["positive", "negative", "neutral"],
            },
            "confidence": {"type": "number"},
        },
        "required": ["sentiment", "confidence"],
    }

    options = ClaudeAgentOptions(
        output_format={"type": "json_schema", "schema": schema},
    )

    result_message = None
    async for message in query(
        prompt="Analyze the sentiment of: 'I love programming!'",
        options=options,
    ):
        if isinstance(message, ResultMessage):
            result_message = message

    assert result_message is not None
    assert result_message.structured_output is not None

    output = result_message.structured_output
    print(f"结构化输出: {output}")
    assert output["sentiment"] in ["positive", "negative", "neutral"], \
        f"sentiment 值不在枚举范围内: {output['sentiment']}"
    assert output["sentiment"] == "positive", \
        f"'I love programming!' 应为 positive，实际: {output['sentiment']}"

    print("✓ 枚举 JSON Schema 通过\n")


async def main():
    print("=== 测试: 结构化输出 ===\n")
    await test_simple_schema()
    await test_nested_schema()
    await test_enum_schema()
    print("✓ 所有结构化输出测试通过")


if __name__ == "__main__":
    anyio.run(main)

#!/usr/bin/env python3
"""测试场景: 控制协议 — 获取服务器信息

验证 get_server_info() 和 get_mcp_status() 控制协议方法。
覆盖功能:
- client.get_server_info() 获取服务器初始化信息
- client.get_mcp_status() 获取 MCP 状态
- 控制协议请求/响应
"""

import asyncio

from claude_agent_sdk import (
    ClaudeSDKClient,
    ResultMessage,
)


async def main():
    print("=== 测试: 控制协议 — 服务器信息 ===\n")

    async with ClaudeSDKClient() as client:
        # 获取服务器信息
        print("--- 获取服务器信息 ---")
        server_info = await client.get_server_info()

        if server_info:
            print(f"✓ 获取到服务器信息")
            if "commands" in server_info:
                print(f"  可用命令数: {len(server_info['commands'])}")
            if "output_style" in server_info:
                print(f"  输出样式: {server_info['output_style']}")
            if "available_output_styles" in server_info:
                print(f"  可用样式: {server_info['available_output_styles']}")
        else:
            print("  服务器信息不可用 (可能不在流式模式)")

        # 获取 MCP 状态
        print("\n--- 获取 MCP 状态 ---")
        try:
            mcp_status = await client.get_mcp_status()
            print(f"✓ MCP 状态: {mcp_status}")
        except Exception as e:
            print(f"  MCP 状态获取异常: {type(e).__name__}: {e}")

        # 发送一个简单查询确认连接正常
        print("\n--- 验证连接正常 ---")
        await client.query("Say 'OK'.")
        async for msg in client.receive_response():
            if isinstance(msg, ResultMessage):
                print(f"结果: {msg.subtype}")
                assert not msg.is_error, "查询失败"

    print("\n✓ 控制协议测试通过")


if __name__ == "__main__":
    asyncio.run(main())

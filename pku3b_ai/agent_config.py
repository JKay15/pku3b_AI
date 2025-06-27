import os
import asyncio
from camel.agents import ChatAgent
from camel.toolkits import MCPToolkit
from camel.models import DeepSeekModel

# ✅ 推荐从环境变量读取
api_key = os.getenv("DEEPSEEK_API_KEY", "sk-0711859f6a804ee4bc935786d4e68fff")

# ✅ 正确写法：字符串格式
model = DeepSeekModel(
    model_type="deepseek-chat",
    model_config_dict={  # ✅ 不要写 model 字段！
        "temperature": 0.7,
        "top_p": 0.9,
        "stream": True,
    },
    api_key=os.getenv("DEEPSEEK_API_KEY", "sk-0711859f6a804ee4bc935786d4e68fff"),
)
from typing import AsyncGenerator

async def astep_pseudo_stream(agent, user_input: str) -> AsyncGenerator[str, None]:
    """调用 ChatAgent 的 astep 方法并伪造流式输出"""
    step_result = await agent.astep(user_input)
    full_text = step_result.msgs[0].content

    # 可自定义分段方式：句子、段落、n个字符一组等
    for segment in full_text.split("。"):  # 可改为其他分段符，如 \n 或固定长度
        if segment.strip():  # 避免空段
            yield segment.strip() + "。"
            await asyncio.sleep(0.05)  # 模拟 token 输出延迟，可调整
async def main():
    # 1️⃣ 初始化 MCP 工具
    mcp_toolkit = MCPToolkit(config_path="./mcp_servers_config.json")
    await mcp_toolkit.connect()
    tools = list(mcp_toolkit.get_tools())

    # 2️⃣ 构造 Agent
    agent = ChatAgent(
        system_message="你是一个工具驱动的助理，请按需调用工具返回 JSON；注意课程索引是从0开始的，而不是从1开始，作业、视频、文档、通知相关的函数如果有两个参数，都是课程索引在前，作业、视频、文档、通知名称在后",
        tools=tools,
        model=model,
    )

    # 3️⃣ 包含 json 关键词，否则 DeepSeek 拒绝响应
    user_input =  "找到我所有课程中和AI有关的课程，然后展示这门课的树形结构信息——显式的给出你调用过的MCP 本地工具"

    # ✅ 调用 async step
    # response = await agent.astep(user_input)
    # print(response.msgs[0].content)
    
    print("👉 模拟流式输出开始:")
    async for segment in astep_pseudo_stream(agent, user_input):
        print(segment, flush=True)


    await mcp_toolkit.disconnect()

if __name__ == "__main__":
    asyncio.run(main())
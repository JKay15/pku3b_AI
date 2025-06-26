# main_dispatch_test.py

from llm_interface.dispatcher import ToolDispatcher
from llm_interface import tools  # ✅ 必须导入以完成 tool 函数绑定（即注册到 tool_registry）
import json


if __name__ == "__main__":
    dispatcher = ToolDispatcher()

    # 你可以替换 name 和 arguments 进行不同功能测试
    name = "list_courses"
    arguments = {
        
    }

    result = dispatcher.dispatch(name, arguments)
    print(json.dumps(result, indent=2, ensure_ascii=False))
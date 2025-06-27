from llm_interface.tool_registry import tool_registry
from llm_interface.auth import load_or_login
from pydantic import BaseModel
from typing import Any
import json
import argparse
import inspect
# ✅ 确保调用所有工具的绑定函数
from llm_interface.tools.core_wrappers import TOOL_BINDINGS  # 这句必须保留以触发绑定逻辑

# ✅ 手动执行绑定（确保这段在 CLI 启动前执行）
for name, func in TOOL_BINDINGS.items():
    if entry := tool_registry.get(name):
        entry.function = func
    else:
        print(f"❌ Tool '{name}' not found in registry!")
# JSON 安全包装器（避免 PyObject 报错）
def json_safe(obj):
    if isinstance(obj, list):
        return [json_safe(o) for o in obj]
    if isinstance(obj, dict):
        return {k: json_safe(v) for k, v in obj.items()}
    if hasattr(obj, "__dict__"):
        return json_safe(vars(obj))
    if hasattr(obj, "__str__"):
        return str(obj)
    return obj

# CLI 主逻辑
def main():
    print("\n🛠 正在检查工具注册状态...\n")
    for name, entry in tool_registry.list_all().items():
        status = "✅ Bound" if entry.function else "❌ Unbound"
        print(f"{status} ▶ {name}")

    print("\n==================== CLI 启动 ====================\n")

    parser = argparse.ArgumentParser()
    parser.add_argument("tool", help="要调用的工具名")
    parser.add_argument("--params", nargs="*", help="以 key=value 格式传入的参数列表")
    args = parser.parse_args()

    tool = tool_registry.get(args.tool)
    if not tool:
        print(f"❌ 工具不存在：{args.tool}")
        return

    if not tool.function:
        print(f"⚠️ 工具未绑定实现函数：{args.tool}")
        return

    input_model = tool.input_model

    # 判断是否为空模型（无字段）
    is_empty_model = not getattr(input_model, "__fields__", None)

    if is_empty_model:
        instance = input_model()
    else:
        if not args.params:
            print(f"⚠️ 工具 {args.tool} 需要参数，但未提供")
            return

        params = {}
        for param in args.params:
            if "=" not in param:
                print(f"❌ 参数格式错误：{param}，应为 key=value")
                return
            key, value = param.split("=", 1)
            params[key] = value

        try:
            instance = input_model(**params)
        except Exception as e:
            print(f"❌ 参数解析失败：{e}")
            return

    try:
        result = tool.function(instance)
        print("\n✅ 工具调用成功：")
        print(json.dumps(result, indent=2, ensure_ascii=False))
    except Exception as e:
        print(f"❌ 工具调用失败：{e}")


if __name__ == "__main__":
    main()

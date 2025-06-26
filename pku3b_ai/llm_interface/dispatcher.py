# llm_interface/dispatcher.py

from pydantic import ValidationError
from typing import Dict, Any
from .tool_registry import tool_registry


class ToolDispatcher:
    def __init__(self):
        self.registry = tool_registry

    def dispatch(self, name: str, arguments: Dict[str, Any]) -> Dict[str, Any]:
        entry = self.registry.get(name)
        if not entry:
            return {"error": f"Tool '{name}' not found."}

        input_model_cls = entry.input_model
        if not input_model_cls:
            return {"error": f"Tool '{name}' does not have a registered input model."}

        # 校验输入参数
        try:
            model = input_model_cls(**arguments)
        except ValidationError as e:
            return {"error": f"Input validation failed.", "details": e.errors()}

        if entry.function is None:
            return {
                "status": "ok",
                "tool": name,
                "message": "Tool matched and validated. (No function registered yet)",
                "parsed_input": model.dict(),
            }

        # 执行函数
        try:
            result = entry.function(model)
            return {
                "status": "ok",
                "tool": name,
                "result": result,
            }
        except Exception as e:
            return {
                "error": "Tool function execution failed.",
                "details": str(e),
            }
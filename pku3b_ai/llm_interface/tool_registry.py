import json
from pathlib import Path
from typing import Type, Optional, Callable, Dict, Any
from pydantic import BaseModel
from .input_models import all_models  # 由 generate_input_models.py 生成的输入模型映射


class ToolEntry(BaseModel):
    name: str
    description: str
    input_model: Optional[Type[BaseModel]] = None
    function: Optional[Callable[[Any], Any]] = None

    class Config:
        arbitrary_types_allowed = True  # 允许非 BaseModel 类型的属性（如函数）


class ToolRegistry:
    def __init__(self, schema_dir: str = "./schema"):
        self.schema_dir = Path(schema_dir)
        self.tools: Dict[str, ToolEntry] = {}
        self.load_all_tools()

    def load_all_tools(self):
        for schema_file in self.schema_dir.glob("*.json"):
            with open(schema_file, "r", encoding="utf-8") as f:
                schema = json.load(f)

            name = schema["name"]
            description = schema.get("description", "")
            model_name = f"{name}_Input"
            input_model = all_models.get(model_name)

            self.tools[name] = ToolEntry(
                name=name,
                description=description,
                input_model=input_model,
            )

    def get(self, name: str) -> Optional[ToolEntry]:
        return self.tools.get(name)

    def list_all(self) -> Dict[str, ToolEntry]:
        return self.tools


# 注册全局工具表
tool_registry = ToolRegistry(schema_dir="./schema")
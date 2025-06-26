import os
import json
from pathlib import Path

SCHEMA_DIR = Path("./schema")
OUTPUT_DIR = Path("./llm_interface")
REGISTRY_PATH = OUTPUT_DIR / "input_models.py"

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)  # ← 自动创建目录

lines = []
registry = []

for file in sorted(SCHEMA_DIR.glob("*.json")):
    name = file.stem
    class_name = f"{name}_Input"
    module_path = OUTPUT_DIR / f"{name}.py"

    with open(file, encoding="utf-8") as f:
        schema = json.load(f)

    properties = schema.get("parameters", {}).get("properties", {})
    required = schema.get("parameters", {}).get("required", [])

    py_fields = []
    for k, v in properties.items():
        t = v.get("type", "string")
        py_type = {"string": "str", "integer": "int", "boolean": "bool"}.get(t, "str")
        default = "" if k in required else " = None"
        py_fields.append(f"    {k}: {py_type}{default}")

    content = f"""from pydantic import BaseModel

class {class_name}(BaseModel):
{chr(10).join(py_fields) or '    pass'}
"""
    module_path.write_text(content, encoding="utf-8")

    lines.append(f"from .{name} import {class_name}")
    registry.append(f'    "{class_name}": {class_name},')

REGISTRY_PATH.write_text(
    "# Auto-generated\n\n" + "\n".join(lines) + "\n\nall_models = {\n" + "\n".join(registry) + "\n}\n",
    encoding="utf-8"
)

print("✅ input_models.py + 所有模型文件生成完成")
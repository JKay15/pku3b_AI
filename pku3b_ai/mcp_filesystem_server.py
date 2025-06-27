import os
from mcp.server.fastmcp import FastMCP

mcp = FastMCP("filesystem")

@mcp.tool()
async def read_file(file_path: str) -> str:
    print(f"[MCP] 📄 Reading file: {file_path}")
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            return f.read()
    except Exception as e:
        return f"[Error reading file] {e}"

@mcp.tool()
async def list_directory(directory_path: str) -> str:
    print(f"[MCP] 📁 Listing directory: {directory_path}")
    try:
        return "\n".join(os.listdir(directory_path))
    except Exception as e:
        return f"[Error listing directory] {e}"

if __name__ == "__main__":
    import sys
    mcp.run(sys.argv[1] if len(sys.argv) > 1 else "stdio")
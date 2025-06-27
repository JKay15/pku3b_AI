from ..tool_registry import tool_registry
from ..auth import load_or_login
from pydantic import BaseModel
from typing import Any
import json

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

# 工具函数定义区

def list_courses_wrapper(_: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(c.get().summary()) for c in bb.list_courses()])

def list_assignments_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).list_assignments()])

def list_unsubmitted_assignments_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).list_unsubmitted_assignments()])

def list_documents_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).list_documents()])

def list_announcements_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).list_announcements()])

def list_course_tree_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    root = bb.course(int(args.course)).build_tree()
    return json_safe(root.summary_tree())

def list_entry_children_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    children = bb.course(int(args.course)).entry(args.entry_id).list_children()
    return json_safe([repr(child) for child in children])

def entry_summary_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    node = bb.course(int(args.course)).entry(args.entry_id)
    return json_safe(node.summary_tree())

def download_assignment_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe(bb.course(int(args.course)).assignment(args.assignment_id).download())

def submit_assignment_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe(bb.course(int(args.course)).assignment(args.assignment_id).submit(path=args.path))

def download_document_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe(bb.course(int(args.course)).entry(args.entry_id).download())

def download_video_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe(bb.course(int(args.course)).entry(args.entry_id).download())

def search_assignments_by_title_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).find_assignments_by_title(args.query)])

def search_documents_by_title_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).find_documents_by_title(args.query)])

def search_videos_by_title_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).find_videos_by_title(args.query)])

def search_announcements_by_title_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    return json_safe([json.loads(h.summary()) for h in bb.course(int(args.course)).find_announcements_by_title(args.query)])

def search_entry_by_title_wrapper(args: BaseModel) -> Any:
    bb = load_or_login()
    nodes = bb.course(int(args.course)).find_entry_by_title(args.query)
    return json_safe([repr(node) for node in nodes])

# 工具绑定表
TOOL_BINDINGS = {
    "list_courses": list_courses_wrapper,
    "list_assignments": list_assignments_wrapper,
    "list_unsubmitted_assignments": list_unsubmitted_assignments_wrapper,
    "list_documents": list_documents_wrapper,
    "list_announcements": list_announcements_wrapper,
    "list_course_tree": list_course_tree_wrapper,
    "list_entry_children": list_entry_children_wrapper,
    "entry_summary": entry_summary_wrapper,
    "download_assignment": download_assignment_wrapper,
    "submit_assignment": submit_assignment_wrapper,
    "download_document": download_document_wrapper,
    "download_video": download_video_wrapper,
    "search_assignments_by_title": search_assignments_by_title_wrapper,
    "search_documents_by_title": search_documents_by_title_wrapper,
    "search_videos_by_title": search_videos_by_title_wrapper,
    "search_entry_by_title": search_entry_by_title_wrapper,
    "search_announcements_by_title": search_announcements_by_title_wrapper,
}

# 绑定函数到工具注册表
for name, func in TOOL_BINDINGS.items():
    if entry := tool_registry.get(name):
        entry.function = func
    else:
        print(f"⚠ Tool '{name}' not found in registry!")

# 可选：验证绑定状态
if __name__ == "__main__":
    for name, entry in tool_registry.list_all().items():
        status = "✅ Bound" if entry.function else "❌ Not Bound"
        print(f"{status} ▶ {name}")
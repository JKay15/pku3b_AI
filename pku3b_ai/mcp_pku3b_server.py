from mcp.server.fastmcp import FastMCP
from pku3b_py import PyClient, cache_size_gb, cache_clean
from mcp.types import ToolAnnotations
from camel.utils import mcp_client

# 初始化 global 变量
client = PyClient()
bb = client.login_blackboard("2000012515", "20020210xjk")
courses_handle = bb.list_courses()
courses_real = [handle.get() for handle in courses_handle]
assignment_registry = {}

def get_course(index: int):
    return courses_real[index]

# ✅ 使用一个统一的 MCP 实例
mcp = FastMCP("pku3b")
# ============================ 🧠 课程树 工具 ============================
course_tree_cache = {}

def get_course_tree(course_index: int):
    if course_index not in course_tree_cache:
        course_tree_cache[course_index] = get_course(course_index).build_tree()
    return course_tree_cache[course_index]

@mcp.tool(description="获取指定课程的内容结构树摘要（图形形式）")
def get_course_tree_summary(course_index: int) -> str:
    tree = get_course_tree(course_index)
    return tree.summary_tree()

@mcp.tool(description="根据标题或 ID 在课程树中查找匹配节点")
def find_node_in_tree_by_title(course_index: int, query: str) -> str:
    tree = get_course_tree(course_index)
    node = tree.find(query)
    if node is None:
        return f"未找到匹配：{query}"
    return f"找到节点：{node.title()}（类型：{node.kind()}）"

@mcp.tool(description="查找课程结构树中所有指定类型的内容节点")
def find_nodes_by_kind(course_index: int, kind: str) -> str:
    """
    kind 可取值：Document / Assignment / Video / Announcement / Folder
    """
    tree = get_course_tree(course_index)
    nodes = tree.find_by_kind(kind)
    if not nodes:
        return f"课程中未找到类型为 {kind} 的节点"
    return f"类型为 {kind} 的节点如下：\n\n" + "\n".join([f"- {n.title()}" for n in nodes])

@mcp.tool(description="根据标题查找节点并返回详细结构信息")
def get_tree_node_detail(course_index: int, query: str) -> str:
    tree = get_course_tree(course_index)
    node = tree.find(query)
    if node is None:
        return f"未找到节点：{query}"

    info = {
        "title": node.title(),
        "id": node.id(),
        "kind": node.kind(),
        "children_count": len(node.children())
    }
    return f"节点详情：\n" + "\n".join(f"{k}: {v}" for k, v in info.items())
# ============================ 🧠 课程通知 工具 ============================
announcement_registry = {}

def get_or_register_announcement(course_index: int, ann_title: str):
    if (course_index, ann_title) in announcement_registry:
        return announcement_registry[(course_index, ann_title)]

    course = get_course(course_index)
    matches = course.find_announcements_by_title(ann_title)
    for a in matches:
        ann = a.get()
        announcement_registry[(course_index, a.title())] = ann
        if a.title() == ann_title:
            return ann
    return None

@mcp.tool(description="模糊查找课程中的通知公告标题，返回匹配项摘要")
def find_announcement_handle_by_title(course_index: int, ann_title: str) -> str:
    course = get_course(course_index)
    matches = course.find_announcements_by_title(ann_title)

    if not matches:
        return f"未找到包含“{ann_title}”的通知公告"

    summaries = []
    for a in matches:
        announcement_registry[(course_index, a.title())] = a.get()
        summaries.append(f"- {a.summary()}")

    return "匹配到的通知如下：\n\n" + "\n".join(summaries)

@mcp.tool(description="获取指定课程中某通知公告的正文内容")
def get_announcement_descriptions(course_index: int, ann_title: str) -> str:
    ann = get_or_register_announcement(course_index, ann_title)
    if ann is None:
        return f"未找到通知公告：{ann_title}"

    desc = ann.descriptions()
    return "\n\n".join(desc) or "该通知没有正文内容。"

@mcp.tool(description="获取指定课程中某通知的附件信息（文件名和下载链接）")
def get_announcement_attachments(course_index: int, ann_title: str) -> str:
    ann = get_or_register_announcement(course_index, ann_title)
    if ann is None:
        return f"未找到通知公告：{ann_title}"

    files = ann.attachments()
    if not files:
        return "该通知没有附件。"
    return "附件列表如下：\n\n" + "\n".join([f"{name} -> {url}" for name, url in files])

@mcp.tool(description="下载指定通知公告的所有附件到目标路径")
def download_announcement_files(course_index: int, ann_title: str, target_dir: str) -> str:
    ann = get_or_register_announcement(course_index, ann_title)
    if ann is None:
        return f"未找到通知公告：{ann_title}"

    try:
        ann.download(target_dir)
        return f"附件已保存至：{target_dir}"
    except Exception as e:
        return f"下载失败：{str(e)}"
# ============================ 🧠 课程文档 工具 ============================
document_registry = {}

def get_or_register_document(course_index: int, doc_title: str):
    if (course_index, doc_title) in document_registry:
        return document_registry[(course_index, doc_title)]

    course = get_course(course_index)
    matches = course.find_documents_by_title(doc_title)
    for d in matches:
        doc = d.get()
        document_registry[(course_index, d.title())] = doc
        if d.title() == doc_title:
            return doc
    return None

@mcp.tool(description="模糊查找指定课程中的文档标题，返回摘要信息")
def find_document_handle_by_title(course_index: int, doc_title: str) -> str:
    course = get_course(course_index)
    matches = course.find_documents_by_title(doc_title)

    if not matches:
        return f"未找到包含“{doc_title}”的文档"

    summaries = []
    for d in matches:
        document_registry[(course_index, d.title())] = d.get()
        summaries.append(f"- {d.summary()}")

    return "匹配到的文档如下：\n\n" + "\n".join(summaries)

@mcp.tool(description="获取指定课程中某文档的正文内容")
def get_document_descriptions(course_index: int, doc_title: str) -> str:
    doc = get_or_register_document(course_index, doc_title)
    if doc is None:
        return f"未找到文档：{doc_title}"

    desc = doc.descriptions()
    return "\n\n".join(desc) or "该文档没有正文段落。"

@mcp.tool(description="获取指定课程中文档的附件信息（文件名和下载链接）")
def get_document_attachments(course_index: int, doc_title: str) -> str:
    doc = get_or_register_document(course_index, doc_title)
    if doc is None:
        return f"未找到文档：{doc_title}"

    files = doc.attachments()
    if not files:
        return "该文档没有附件。"
    return "附件列表如下：\n\n" + "\n".join([f"{name} -> {url}" for name, url in files])

@mcp.tool(description="下载指定课程中某文档的所有附件")
def download_document_files(course_index: int, doc_title: str, target_dir: str) -> str:
    doc = get_or_register_document(course_index, doc_title)
    if doc is None:
        return f"未找到文档：{doc_title}"

    try:
        doc.download(target_dir)
        return f"附件已保存至：{target_dir}"
    except Exception as e:
        return f"下载失败：{str(e)}"

# ============================ 🧠 课程回放 工具 ============================
#  视频缓存（如果不想每次都 get，可以考虑启用）
video_registry = {}

def get_or_register_video(course_index: int, video_title: str):
    """从缓存获取 PyVideo；如未注册则查找匹配并注册"""
    if (course_index, video_title) in video_registry:
        return video_registry[(course_index, video_title)]

    course = get_course(course_index)
    matches = course.find_videos_by_title(video_title)
    for v in matches:
        video = v.get()
        video_registry[(course_index, v.title())] = video
        if v.title() == video_title:
            return video  # 精确匹配优先
    return None

@mcp.tool(description="根据视频标题下载课程视频（可选转为 MP4）")
def download_video_by_title(course_index: int, video_title: str, target_dir: str, to_mp4: bool = True) -> str:
    video = get_or_register_video(course_index, video_title)
    if video is None:
        return f"未找到视频：{video_title}"

    try:
        path = video.download(target_dir, to_mp4)
        return f"视频已成功下载至：{path}"
    except Exception as e:
        return f"下载失败：{str(e)}"
    
@mcp.tool(description="获取指定课程中某个视频的时长")
def get_video_duration(course_index: int, video_title: str) -> str:
    """
    获取视频总时长（例如 '01:22:35'）

    Args:
        course_index (int): 课程编号
        video_title (str): 视频标题关键词或完整标题
    """
    course = get_course(course_index)
    matches = course.find_videos_by_title(video_title)

    if not matches:
        return f"未找到视频：{video_title}"

    # 返回第一个匹配项（大部分情况不会重名）
    v = matches[0]
    return f"视频《{v.title()}》的时长为：{v.time()}"
# ============================ 🧠 课程作业 工具 ============================
def get_or_register_assignment(course_index: int, assignment_title: str):
    """从缓存获取 PyAssignment；如未注册则查找匹配并注册"""
    if (course_index, assignment_title) in assignment_registry:
        return assignment_registry[(course_index, assignment_title)]
    
    course = get_course(course_index)
    matches = course.find_assignments_by_title(assignment_title)
    for a in matches:
        assignment = a.get()
        assignment_registry[(course_index, a.title())] = assignment
        if a.title() == assignment_title:
            return assignment  # 精确匹配优先返回
    
    return None  # 没匹配上

@mcp.tool(description="在指定课程中模糊查找作业标题，返回匹配项的结构摘要（summary），并注册 assignment",)
def find_assignment_handle_by_title(course_index: int, assignment_title: str) -> str:
    """
    获取指定课程中某个作业的描述内容

    Args:
        course_index (int): 课程在课程列表中的编号（从 0 开始）
        assignment_title (str): 作业标题的完整名称或关键词，用于匹配目标作业
    """
    course = get_course(course_index)
    matches = course.find_assignments_by_title(assignment_title)

    if not matches:
        return f"未找到标题中包含“{assignment_title}”的作业。"

    summaries = []
    for a in matches:
        assignment_registry[(course_index, a.title())] = a.get()
        summaries.append(f"- {a.summary()}")

    return "匹配到的作业如下：\n\n" + "\n".join(summaries)


@mcp.tool(
    description="获取指定课程中某个作业的描述内容",
)
def get_assignment_descriptions(course_index: int, assignment_title: str) -> str:
    """
    获取指定课程中某个作业的描述内容

    Args:
        course_index (int): 课程在课程列表中的编号
        assignment_title (str): 要查询的作业标题
    """
    assignment = get_or_register_assignment(course_index, assignment_title)
    if assignment is None:
        return f"未找到作业：{assignment_title}"
    desc = assignment.descriptions()
    return "\n\n".join(desc) or "该作业没有描述内容。"
# 为其添加参数 schema，提供 title 和 description
get_assignment_descriptions.annotations = ToolAnnotations(
    argument_descriptions={
        "course_index": "课程在课程列表中的索引（从0开始）",
        "assignment_title": "作业的完整标题或关键词，用于模糊匹配",
    },
    examples=[
        {
            "input": {
                "course_index": 0,
                "assignment_title": "卷积神经网络"
            },
            "output": "该作业的描述是……"
        }
    ]
)

@mcp.tool(description="获取指定作业的附件信息（文件名和下载链接）")
def get_assignment_attachments(course_index: int, assignment_title: str) -> str:
    """
    获取作业附件（文件名和下载链接）

    Args:
        course_index (int): 课程编号
        assignment_title (str): 作业标题
    """
    assignment = get_or_register_assignment(course_index, assignment_title)
    if assignment is None:
        return f"未找到作业：{assignment_title}"
    files = assignment.attachments()
    if not files:
        return "该作业没有附件。"
    return "附件列表如下：\n\n" + "\n".join([f"{name} -> {url}" for name, url in files])


@mcp.tool(description="获取指定作业的截止时间")
def get_assignment_deadline(course_index: int, assignment_title: str) -> str:
    """
    获取作业的截止时间

    Args:
        course_index (int): 课程编号
        assignment_title (str): 作业标题
    """
    assignment = get_or_register_assignment(course_index, assignment_title)
    if assignment is None:
        return f"未找到作业：{assignment_title}"
    return assignment.deadline_raw() or "该作业无明确截止时间。"


@mcp.tool(description="下载指定作业的所有附件到目标目录")
def download_assignment_files(course_index: int, assignment_title: str, target_dir: str) -> str:
    """
    下载作业附件至指定路径

    Args:
        course_index (int): 课程编号
        assignment_title (str): 作业标题
        target_dir (str): 保存附件的目标目录路径
    """
    assignment = get_or_register_assignment(course_index, assignment_title)
    if assignment is None:
        return f"未找到作业：{assignment_title}"
    assignment.download(target_dir)
    return f"附件已保存至：{target_dir}"


@mcp.tool(description="将指定文件提交至该作业")
def submit_assignment_file(course_index: int, assignment_title: str, file_path: str) -> str:
    """
    提交文件至指定作业

    Args:
        course_index (int): 课程编号
        assignment_title (str): 作业标题
        file_path (str): 要上传的文件路径
    """
    assignment = get_or_register_assignment(course_index, assignment_title)
    if assignment is None:
        return f"未找到作业：{assignment_title}"
    assignment.submit_file(file_path)
    return f"已成功提交文件：{file_path}"

# ============================ 🧠 课程细节 工具 ============================

@mcp.tool()
def get_course_title(course_index: int) -> str:
    """返回指定课程的标题，索引从0开始，而不是从1开始"""
    return get_course(course_index).title()

@mcp.tool()
def get_course_entries(course_index: int) -> str:
    """列出指定课程的左侧菜单项（entry title 列表）"""
    entries = get_course(course_index).list_entry_titles()
    return "该课程左侧菜单项如下：\n\n" + "\n".join(f"- {e}" for e in entries)

@mcp.tool()
def list_submitted_assignments(course_index: int) -> str:
    """列出指定课程中所有已提交的作业标题"""
    assigns = get_course(course_index).list_submitted_assignments()
    return ' '.join([a.title() for a in assigns])

@mcp.tool()
def list_unsubmitted_assignments(course_index: int) -> str:
    """列出指定课程中所有尚未提交的作业标题"""
    assigns = get_course(course_index).list_unsubmitted_assignments()
    return ' '.join([a.title() for a in assigns])

@mcp.tool()
def list_documents(course_index: int) -> str:
    """列出指定课程中所有文档的标题"""
    docs = get_course(course_index).list_documents()
    return "该课程文档如下：\n\n" + "\n".join([f"- {d.title()}" for d in docs])

@mcp.tool()
def list_videos(course_index: int) -> str:
    """列出指定课程中所有视频的标题"""
    videos = get_course(course_index).list_videos()
    return "该课程视频如下：\n\n" + "\n".join([f"- {v.title()}" for v in videos])

@mcp.tool()
def list_announcements(course_index: int) -> str:
    """列出指定课程中所有通知公告的标题"""
    notes = get_course(course_index).list_announcements()
    return "该课程通知如下：\n\n" + "\n".join([f"- {n.title()}" for n in notes])

@mcp.tool()
def list_all_assignments(course_index: int) -> str:
    """列出指定课程中所有作业的标题"""
    assigns = get_course(course_index).list_assignments()
    return "该课程所有作业如下：\n\n" + "\n".join([f"- {a.title()}" for a in assigns])

@mcp.tool()
def get_entry_links(course_index: int) -> str:
    """列出指定课程左侧菜单项及其链接（entry title -> URL）"""
    pairs = get_course(course_index).entries().items()
    return "左侧菜单项与其链接如下：\n\n" + "\n".join([f"{k} -> {v}" for k, v in pairs])

@mcp.tool()
def find_entries_by_keyword(course_index: int, keyword: str) -> str:
    """根据关键词在指定课程中查找匹配的菜单项及其 URL"""
    entries = get_course(course_index).find_entries_by_title(keyword)
    return "\n".join([f"{k} -> {v}" for k, v in entries])

@mcp.tool()
def get_course_summary(course_index: int) -> str:
    """获取指定课程的结构化总结信息（JSON 字符串）"""
    return get_course(course_index).summary()

# ============================ 🧠 Blackboard 工具 ============================

@mcp.tool()
def list_course_titles() -> str:
    """列出所有课程的标题列表（含编号）"""
    titles = bb.course_titles()
    return "以下是所有课程的标题：\n\n" + "\n".join(f"{i + 1}. {t}" for i, t in enumerate(titles))

@mcp.tool()
def get_course_index_map() -> str:
    """返回课程标题与下标的对应关系"""
    titles = bb.course_titles()
    return "\n".join(f"{i}: {t}" for i, t in enumerate(titles))

# ============================ 💾 Client 工具 ============================

@mcp.tool()
async def get_cache_dir() -> str:
    """返回 PyClient 的缓存目录路径"""
    return client.cache_dir()

@mcp.tool()
async def get_cache_size() -> str:
    """返回缓存目录当前占用空间（单位：GB）"""
    return f"{cache_size_gb():.3f} GB"

@mcp.tool()
async def clean_cache() -> str:
    """清理缓存目录并返回释放空间大小（单位：GB）"""
    freed = cache_clean()
    return f"已清理缓存，释放空间：{freed:.3f} GB"

# ============================ 🚀 启动 ============================

if __name__ == "__main__":
    import sys
    mcp.run(sys.argv[1] if len(sys.argv) > 1 else "stdio")
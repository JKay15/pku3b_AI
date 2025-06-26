# 使用文档：pku3b_py

本模块 `pku3b_py` 是对原始爬虫项目 `pku3b` 的结构化封装，旨在提供全球最强、最完整的北京大学教学网（Blackboard）内容获取能力，并对全部教学内容（课程列表、作业、文档、视频、公告）实现结构统一、信息完整的访问接口。该模块是 `pku3b_AI` 项目的核心组成部分，提供高层封装接口供 AI 调用。

---

## 总览结构

- `PyClient`: 登录入口，管理用户缓存与登录态
- `PyBlackboard`: 登录后的教学网对象，可列出课程
- `PyCourseHandle`: 课程句柄（轻量）
- `PyCourse`: 完整课程对象，含内容提取、结构树构建等
- `PyAssignmentHandle` / `PyAssignment`: 作业
- `PyDocumentHandle` / `PyDocument`: 文档
- `PyVideoHandle` / `PyVideo`: 视频（含断点续传与 MP4 合并）
- `PyAnnouncementHandle` / `PyAnnouncement`: 公告
- `PyCourseTreeNode`: 树结构节点，统一管理课程内容

---

## 1. 初始化与登录

```python
from pku3b_py import PyClient
client = PyClient()
bb = client.login_blackboard("学号", "密码")
```

- `cache_dir()`：获取缓存目录路径
- `cache_size_gb()` / `cache_clean()`：查看 / 清理缓存目录大小

---

## 2. 获取课程信息

```python
courses = bb.list_courses()
course = courses[0].get()  # 或 bb.course(0)
```

- `list_courses()`：获取 `PyCourseHandle` 列表
- `course(index)`：便捷方式，直接获取完整 `PyCourse`
- `course_titles()`：仅获取课程标题列表

---

## 3. 课程对象 PyCourse 功能

### 基本信息

- `title()`：课程标题
- `entries()`：左侧菜单 HashMap<标题, URL>
- `list_entry_titles()`：菜单项标题列表
- `list_entry_pairs()`：菜单项 title-URL 对
- `find_entries_by_title(query)`：模糊查找菜单项

### 视频模块

- `list_videos()`：获取所有视频句柄
- `find_videos_by_title(query)`：模糊查找
- `PyVideoHandle.get()` → `PyVideo`
- `download(dst, to_mp4=True)`：支持断点续传 + MP4 转换

### 作业模块

- `list_assignments()` / `find_assignments_by_title(query)`
- `list_unsubmitted_assignments()`：列出所有未提交的作业
- `list_submitted_assignments()`：列出所有已提交的作业
- `PyAssignmentHandle.get()` → `PyAssignment`
- `submit_file(path)`：上传作业

### 文档模块

- `list_documents()` / `find_documents_by_title(query)`
- `PyDocumentHandle.get()` → `PyDocument`
- `download(dst)`：批量下载附件

### 公告模块

- `list_announcements()` / `find_announcements_by_title(query)`
- `PyAnnouncementHandle.get()` → `PyAnnouncement`

### 树结构内容提取

```python
tree = course.build_tree()
print(tree.summary())  # 树状结构概览
```

- `build_tree()`：构建 `PyCourseTreeNode` 结构树
- `find(query)`：递归查找某个标题/ID 节点
- `find_by_kind("Document" / "Assignment" / "Video" / "Announcement")`
- `get_document_handle()` / `get_assignment_handle()` / ...：统一接口
- `children()`：获取子节点
- `summary()`：结构化展示树节点及其子树

---

## 4. 统一句柄属性（表格）

| 模块     | 句柄字段           | 描述                   |
| -------- | ------------------ | ---------------------- |
| 所有内容 | `id()`           | Blackboard 内部唯一 ID |
| 所有内容 | `title()`        | 标题                   |
| 所有内容 | `parent_title()` | 所属菜单标题（可选）   |
| 所有内容 | `section_name()` | 显示模块名称（可选）   |
| 所有内容 | `summary()`      | 格式化展示，用于调试   |
| 内容对象 | `descriptions()` | 正文内容段落           |
| 内容对象 | `attachments()`  | 附件 (name, url) 对    |

---

## 5. 特色功能 & AI 友好性

- **支持所有课程模块的统一访问与搜索**：五类内容均支持标题模糊匹配、统一格式展示与下载
- **完整结构树构建与访问**：`build_tree()` 提供结构化层级遍历，支持递归查找与分类筛选
- **断点续传与 MP4 合并**：视频下载模块集成缓存机制与 `ffmpeg` 合并，可获取完整视频
- **AI 调用友好性**：所有对象提供清晰 getter、格式化摘要 `summary()`、异常稳定返回

---

## 6. 模块注册函数

项目中的 `pymodule` 注册函数位于末尾：

```rust
#[pymodule]
fn pku3b_py(...) -> PyResult<()> { ... }
```

确保所有封装类均被正确导出。

---

## 7. 推荐使用方式

推荐通过 `virtualenv` 构建 Python 环境，使用以下命令：

```bash
pip install maturin
cd ./pku3b_py
maturin develop  # 编译并安装 pku3b_py 模块
```

---

## 8. 后续计划

- 构建 AI 接口调用层，支持自然语言课程信息查询与批量操作（pku3b_AI 子项目）
- 提供 Obsidian/Notion 内容同步功能
- 支持自动分析、提醒、课程笔记生成等智能功能

---

> © 2025 Peking University AI Project

项目主页：https://github.com/JKay15/pku3b_AI

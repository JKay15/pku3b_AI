# 🧠 PKU3b_AI: A Even Better Black Board for PKUers 🎓

[![Crates.io](https://img.shields.io/crates/v/pku3b)](https://crates.io/crates/pku3b)
![Issues]([https://img.shields.io/github/issues-search?query=repo%3AXiongJkay%2Fpku3b_AI%20is%3Aopen&label=issues&color=orange](https://github.com/JKay15/pku3b_AI/issues))
![Closed Issues](https://img.shields.io/github/issues-search?query=repo%3AXiongJkay%2Fpku3b_AI%20is%3Aclosed&label=closed%20issues&color=green)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/XiongJkay/pku3b_AI/build-release.yml)
![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/XiongJkay/pku3b_AI/total)


---

## 📦 项目结构概览

| 模块名称       | 说明                                                                 |
|----------------|----------------------------------------------------------------------|
| `pku3b_AI`      | 顶层项目，整合后端爬虫和智能封装，目标是打造北大教学网最强 AI 应用平台。        |
| `pku3b`         | 后端爬虫核心，Fork 自 [sshwy/pku3b](https://github.com/sshwy/pku3b)，保留通信与下载核心机制。 |
| `pku3b_py`      | Python 接口，基于 PyO3 封装，为 AI 系统提供统一访问入口。                      |

---

## 🚀 关键突破

| 功能模块        | 原版支持 | 我们的增强与创新                                             |
|-----------------|----------|--------------------------------------------------------------|
| 📋 作业系统        | ✅       | ✅ 保留原功能，增加结构化访问和句柄封装                         |
| 🎥 视频下载        | ✅       | ✅ 保留断点续传/mp4 转码，适配 Python 下载 API                  |
| 📄 文档系统        | ❌       | ✅ **新增：首次实现教学文档内容抓取 + 附件下载**                  |
| 📢 通知系统        | ❌       | ✅ **新增：解析课程公告正文 + 图片附件，结构化呈现**              |
| 🌲 内容树构建      | ❌       | ✅ **新增：课程结构树（文档/作业/通知/视频）统一封装，支持遍历和操作** |
| 🧠 Python 封装接口 | ❌       | ✅ **新增：所有内容统一 `.get()` `.download()` `.descriptions()` 等接口** |
| 🤖 AI 适配设计     | ❌       | ✅ **新增：面向 Agent/LLM 设计，适配自动总结、问答、任务管理场景**   |

---

## 🛠️ 后端架构（`pku3b`）

- 使用 Rust 高性能构建，模块职责清晰，性能极高。
- 完整保留原项目的身份认证与通信逻辑。
- 模块划分：
  - `assignment`: 作业内容抓取与提交
  - `video`: 回放列表与断点下载
  - `document`: 课件文档模块（新增）
  - `announcement`: 公告通知模块（新增）
  - `tree`: 树状结构统一组织各类课程内容（新增）
- 各类内容实现统一接口封装（id/title/正文/附件）

---

## 🐍 Python 接口封装（`pku3b_py`）

- 封装 Rust 接口为 Python 类：如 `CourseDocumentHandle`、`CourseAnnouncementHandle`
- 所有内容统一封装成 `CourseContentData`，便于 AI 模型调用和脚本处理。
- 每类内容支持：
  - `.title()` / `.descriptions()` / `.download(path)` 方法
  - 附件自动识别后缀并保存

---

### 🧪 使用示例（Python）

以下为 pku3b_py 的标准使用流程，展示课程访问、模块内容下载、树结构调用等常见场景。

### 🛜 登录教学网

```python
from pku3b_py import PyClient

client = PyClient()
bb = client.login_blackboard("学号", "密码")
```

### 📚 列出课程并进入第一个课程

```python
course = bb.course(0)
print("课程名:", course.title())
```

### 🗂️ 获取课程左侧菜单 entries

```python
print(course.entries())
# 返回如：{"教学资料": "/webapps/xx", "作业提交": "/webapps/yy"}
```

### 📄 下载课程文档

```python
docs = course.list_documents()
for doc_handle in docs:
    doc = doc_handle.get()
    print(doc.title())
    doc.download("./downloads/文档")
```

### 📢 下载课程通知（含正文和附件）

```python
anns = course.list_announcements()
for ann_handle in anns:
    ann = ann_handle.get()
    print("📢", ann.title())
    ann.download("./downloads/通知")
```

### 📝 下载课程作业附件并提交

```python
assignments = course.list_assignments()
for assn_handle in assignments:
    assn = assn_handle.get()
    print("📝", assn.title())
    assn.download("./downloads/作业")
    # assn.submit_file("你的作业路径.pdf")
```

### 🎬 下载课程视频（支持转 mp4）

```python
videos = course.list_videos()
for video_handle in videos:
    video = video_handle.get()
    print("🎬", video.title())
    video.download("./downloads/视频", to_mp4=True)
```

### 🌳 使用内容树精确定位模块

```python
tree = course.build_tree()
root = tree  # 根节点
```

####  🔍 查找“课程通知”模块的 Entry 节点

```python
target = root.find("课程通知")
if target:
    print("找到节点:", target.title())
    for child in target.children():
        print("📌 子节点:", child.title())
        ann = child.get_announcement_handle().get()
        ann.download("./downloads/树状通知")
```

你也可以访问其他模块，如文档、作业、视频：

```python
doc_node = root.find("教学资料").children()[0]
doc = doc_node.get_document_handle().get()
doc.download("./downloads/树状文档")

video_node = root.find_by_kind("Video")[0]
video = video_node.get_video_handle().get()
video.download("./downloads/树状视频")
```

---

## 💻 前端交互界面：Cherry Studio + MCP 接入

本项目前端采用 Cherry Studio 系统作为交互界面，通过 MCP 协议与工具后端进行连接与调用。用户可直接使用自然语言对话操作教学网资源，实现多轮对话、自动工具规划、结构化响应显示等能力。

前端能力包括：

模块 | 功能描述
-----|------------
🧠 多轮对话框架 | 支持与 LLM 进行连续对话，保持上下文
⚙️ 工具自动调用 | 通过 MCP Schema 自动匹配并调用注册工具
📊 结构化结果展示 | 对作业、文档、通知等响应格式美观呈现
💬 模糊查询能力 | 支持关键词模糊匹配课程与作业
🚀 流式响应体验 | 支持实时输出流式回答结果
📁 下载与缓存支持 | 下载视频/文档后自动展示文件地址

未来可拓展模块：

模块 | 功能方向
-----|------------
📅 课程总览 | 所有课程结构/任务概览，支持时序视图和分类导航
📋 作业任务面板 | 自动拉取所有作业 + 智能提醒（DDL 检测）
🔔 通知聚合 | 聚合所有课程通知，按关键词/课程/时间分类筛选
🧠 知识图谱生成器 | 结构化导入 Obsidian/Notion 笔记系统

---

## 🔧 安装与构建（开发者模式）


本系统支持从源码构建并部署完整交互功能，包括 Python 接口、MCP 工具注册服务和前端对话界面。请按如下步骤进行：


### 1️⃣ 克隆项目

```bash
git clone https://github.com/JKay15/pku3b_AI.git
cd pku3b_ai
```

### 2️⃣ 安装 Rust 与构建工具链

```bash
# 安装 Rust 工具链（首次安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# 重启终端后生效 rustc 和 cargo 命令
```

### 3️⃣ 安装 Python 依赖（包括构建器 maturin 和前端工具运行依赖）

```bash
pip install -r requirements.txt
```

### 4️⃣ 构建 Python 模块（基于 maturin）

```bash
cd pku3b_py
maturin develop  # 编译 Rust 模块并生成 Python 接口
cd ..
```

### 5️⃣ 启动 MCP 工具服务

```bash
python pku3b_ai/mcp_pku3b_server.py
```

### 6️⃣ 启动 Cherry Studio 并配置 MCP Server

在 Cherry Studio UI 中添加 MCP Server 设置，URL以运行mcp_pku3b_server.py后的终端输出为准：

![CleanShot 2025-06-27 at 21.59.39@2x.png](https://image-hosting-1319096909.cos.ap-beijing.myqcloud.com/CleanShot%202025-06-27%20at%2021.59.39%402x.png)

![CleanShot 2025-06-27 at 21.58.12@2x.png](https://image-hosting-1319096909.cos.ap-beijing.myqcloud.com/CleanShot%202025-06-27%20at%2021.58.12%402x.png)


---

## 📘 使用文档

本项目提供了完整、结构清晰的 Python 接口使用文档，详见：

👉 [Python库pku3b_py使用说明文档（doc/usage.md）](doc/usage.md)

👉 [MCP工具集说明文档（doc/mcp_tool_summary.md）](doc/mcp_tool_summary.md)


---

## 🤝 致谢原项目

本项目基于北大开源项目：

- 🌟 [sshwy/pku3b](https://github.com/sshwy/pku3b)：由北大学生开发的教学网 CLI 工具，具备优秀的断点下载与命令行交互设计。

我们复用了其**后端通信与视频作业模块**，并在此基础上：
- ✨ 实现了**完整文档与通知模块**
- ✨ 建立了结构化**内容统一数据抽象层**
- ✨ 提供了 **Python 封装接口**，可供 AI 系统调用

---



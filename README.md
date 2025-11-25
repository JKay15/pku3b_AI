# 🧠 PKU3b_AI: An Even Better Blackboard for PKUers 🎓

[![Crates.io](https://img.shields.io/crates/v/pku3b)](https://crates.io/crates/pku3b)
![Issues](https://img.shields.io/github/issues-search?query=repo%3AJKay15%2Fpku3b_AI%20is%3Aopen&label=issues&color=orange)
![Closed Issues](https://img.shields.io/github/issues-search?query=repo%3AJKay15%2Fpku3b_AI%20is%3Aclosed&label=closed%20issues&color=green)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/JKay15/pku3b_AI/build-release.yml)
![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/JKay15/pku3b_AI/total)

> [🇨🇳 **中文说明 (Chinese Version)**](README_zh.md)

---

## 📦 Project Structure Overview

| Module Name    | Description                                                                 |
|----------------|-----------------------------------------------------------------------------|
| `pku3b_AI`     | Top-level project integrating the backend scraper and intelligent encapsulation. Aiming to build the most powerful AI application platform for PKU Blackboard. |
| `pku3b`        | Core backend scraper. Forked from [sshwy/pku3b](https://github.com/sshwy/pku3b), retaining core communication and download mechanisms. |
| `pku3b_py`     | Python interface based on PyO3 encapsulation, providing a unified access point for AI systems. |

![CleanShot 2025-06-27 at 22.28.00@2x.png](https://image-hosting-1319096909.cos.ap-beijing.myqcloud.com/CleanShot%202025-06-27%20at%2022.28.00%402x.png)

---

## 🚀 Key Breakthroughs

| Module          | Original Support | Our Enhancements & Innovations                               |
|-----------------|------------------|--------------------------------------------------------------|
| 📋 Assignments  | ✅               | ✅ Retained functionality, added structured access and handle encapsulation. |
| 🎥 Videos       | ✅               | ✅ Retained resumable downloads/MP4 conversion, adapted for Python Download API. |
| 📄 Documents    | ❌               | ✅ **New: First implementation of document content scraping + attachment downloading.** |
| 📢 Announcements| ❌               | ✅ **New: Parsing course announcement body + image attachments with structured presentation.** |
| 🌲 Content Tree | ❌               | ✅ **New: Unified course structure tree (Docs/Assignments/Notices/Videos) supporting traversal and operations.** |
| 🧠 Python API   | ❌               | ✅ **New: Unified `.get()`, `.download()`, `.descriptions()` interfaces for all content types.** |
| 🤖 AI Adaptation| ❌               | ✅ **New: Designed for Agent/LLM scenarios, supporting auto-summarization, Q&A, and task management.** |

---

## 🛠️ Backend Architecture (`pku3b`)

- Built with **Rust** for high performance and clear module responsibility.
- Completely preserves the authentication and communication logic of the original project.
- **Module Breakdown:**
  - `assignment`: Assignment scraping and submission.
  - `video`: Playback lists and resumable downloads.
  - `document`: Courseware document module (New).
  - `announcement`: Announcement module (New).
  - `tree`: Tree structure for unified organization of course content (New).
- Unified interface encapsulation for all content types (id/title/body/attachments).

---

## 🐍 Python Interface (`pku3b_py`)

- Encapsulates Rust interfaces into Python classes: e.g., `CourseDocumentHandle`, `CourseAnnouncementHandle`.
- All content is unified into `CourseContentData`, facilitating AI model invocation and script processing.
- Each content type supports:
  - `.title()` / `.descriptions()` / `.download(path)` methods.
  - Automatic file extension recognition and saving.

---

### 🧪 Usage Examples (Python)

The following demonstrates the standard workflow of `pku3b_py`, covering course access, module downloading, and tree structure navigation.

### 🛜 Login to Blackboard

```python
from pku3b_py import PyClient

client = PyClient()
# Replace with your actual Student ID and Password
bb = client.login_blackboard("student_id", "password")
```

### 📚 List Courses & Enter the First Course

```python
course = bb.course(0)
print("Course Name:", course.title())
```

### 🗂️ Get Left Menu Entries

```python
print(course.entries())
# Returns e.g.: {"Course Materials": "/webapps/xx", "Assignments": "/webapps/yy"}
```

### 📄 Download Course Documents

```python
docs = course.list_documents()
for doc_handle in docs:
    doc = doc_handle.get()
    print(doc.title())
    doc.download("./downloads/docs")
```

### 📢 Download Announcements (Body + Attachments)

```python
anns = course.list_announcements()
for ann_handle in anns:
    ann = ann_handle.get()
    print("📢", ann.title())
    ann.download("./downloads/announcements")
```

### 📝 Download Assignment Attachments & Submit

```python
assignments = course.list_assignments()
for assn_handle in assignments:
    assn = assn_handle.get()
    print("📝", assn.title())
    assn.download("./downloads/assignments")
    # assn.submit_file("path/to/your/homework.pdf")
```

### 🎬 Download Videos (Support MP4 Conversion)

```python
videos = course.list_videos()
for video_handle in videos:
    video = video_handle.get()
    print("🎬", video.title())
    video.download("./downloads/videos", to_mp4=True)
```

### 🌳 Use Content Tree for Precise Navigation

```python
tree = course.build_tree()
root = tree  # Root node
```

####  🔍 Find "Course Announcements" Node

```python
target = root.find("Course Announcements") # Name may vary based on course settings
if target:
    print("Node Found:", target.title())
    for child in target.children():
        print("📌 Child Node:", child.title())
        ann = child.get_announcement_handle().get()
        ann.download("./downloads/tree_announcements")
```

You can also access other modules like documents, assignments, and videos via the tree:

```python
doc_node = root.find("Course Materials").children()[0]
doc = doc_node.get_document_handle().get()
doc.download("./downloads/tree_docs")

video_node = root.find_by_kind("Video")[0]
video = video_node.get_video_handle().get()
video.download("./downloads/tree_videos")
```

---

## 💻 Frontend Interface: Cherry Studio + MCP

This project uses **Cherry Studio** as the frontend interaction interface, connecting to the backend via the **MCP Protocol**. Users can operate Blackboard resources directly using natural language, enabling capabilities like multi-turn dialogue, automatic tool planning, and structured response display.

**Frontend Capabilities:**

| Module | Description |
|--------|-------------|
| 🧠 Multi-turn Dialogue | Supports continuous conversation with LLMs, maintaining context. |
| ⚙️ Auto Tool Calling | Automatically matches and calls registered tools via MCP Schema. |
| 📊 Structured Display | Beautifully renders responses for assignments, documents, announcements, etc. |
| 💬 Fuzzy Search | Supports keyword fuzzy matching for courses and assignments. |
| 🚀 Streaming Response | Supports real-time streaming output of answers. |
| 📁 Download & Cache | Automatically displays file paths after downloading videos/documents. |

**Future Modules:**

| Module | Direction |
|--------|-----------|
| 📅 Course Overview | Overview of all course structures/tasks, supporting timeline views. |
| 📋 Task Dashboard | Auto-fetch all assignments + Smart reminders (DDL detection). |
| 🔔 Notification Aggregator | Aggregate all course notifications, filtered by keyword/course/time. |
| 🧠 Knowledge Graph | Structured import into Obsidian/Notion note systems. |

---

## 🔧 Installation & Build

The system supports building from source to deploy the full interactive functionality, including the Python interface, MCP tool service, and frontend dialogue interface.

### 1️⃣ Clone Project

```bash
git clone https://github.com/JKay15/pku3b_AI.git
cd pku3b_ai
```

### 2️⃣ Install Rust & Build Toolchain

```bash
# Install Rust toolchain (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Restart terminal to enable rustc and cargo commands
```

### 3️⃣ Install Python Dependencies

Includes the builder `maturin` and frontend tool runtime dependencies.

```bash
pip install -r requirements.txt
```

### 4️⃣ Build Python Module (via Maturin)

```bash
cd pku3b_py
maturin develop  # Compiles Rust modules and generates Python interface
cd ..
```

### 5️⃣ Start MCP Tool Service

```bash
python pku3b_ai/mcp_pku3b_server.py
```

### 6️⃣ Start Cherry Studio & Configure MCP Server

In the Cherry Studio UI, add the MCP Server settings. Use the URL output by the terminal after running `mcp_pku3b_server.py`.

![CleanShot 2025-06-27 at 21.59.39@2x.png](https://image-hosting-1319096909.cos.ap-beijing.myqcloud.com/CleanShot%202025-06-27%20at%2021.59.39%402x.png)

![CleanShot 2025-06-27 at 21.58.12@2x.png](https://image-hosting-1319096909.cos.ap-beijing.myqcloud.com/CleanShot%202025-06-27%20at%2021.58.12%402x.png)

Once connected, you can use natural language to invoke system functions (e.g., downloading assignments, checking announcements).

![CleanShot 2025-06-27 at 22.01.33@2x.png](https://image-hosting-1319096909.cos.ap-beijing.myqcloud.com/CleanShot%202025-06-27%20at%2022.01.33%402x.png)

---

## 📘 Documentation

This project provides comprehensive and structured Python interface documentation:

👉 [Python Library pku3b_py Usage Guide (doc/usage.md)](doc/usage.md)

👉 [MCP Tool Summary (doc/mcp_tool_summary.md)](doc/mcp_tool_summary.md)

---

## 🤝 Acknowledgments

This project is based on the open-source project by a PKU student:

- 🌟 [sshwy/pku3b](https://github.com/sshwy/pku3b): A Blackboard CLI tool developed by a PKU student, featuring excellent resumable download and command-line interaction design.

We reused its **backend communication and video/assignment modules** and extended it with:
- ✨ **Complete document and announcement modules.**
- ✨ **Unified data abstraction layer for structured content.**
- ✨ **Python encapsulation interface** for AI system integration.

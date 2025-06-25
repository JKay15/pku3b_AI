# 🧠 pku3b_AI：全球最强的北大教学网智能爬虫系统

[![Crates.io](https://img.shields.io/crates/v/pku3b)](https://crates.io/crates/pku3b)
![Issues](https://img.shields.io/github/issues-search?query=repo%3AXiongJkay%2Fpku3b_AI%20is%3Aopen&label=issues&color=orange)
![Closed Issues](https://img.shields.io/github/issues-search?query=repo%3AXiongJkay%2Fpku3b_AI%20is%3Aclosed&label=closed%20issues&color=green)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/XiongJkay/pku3b_AI/build-release.yml)
![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/XiongJkay/pku3b_AI/total)

> **AI 赋能教学网信息管理，从爬虫到认知代理的全面革新。**

---

## 📦 项目结构概览

| 模块名称       | 说明                                                                 |
|----------------|----------------------------------------------------------------------|
| `pku3b_AI`      | 顶层项目，整合后端爬虫和智能封装，目标是打造教学网最强 AI 应用平台。        |
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
| 🧠 Python 封装接口 | ❌       | ✅ **新增：所有内容统一 `.get()` `.download()` `.descriptions` 等接口** |
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

## 💻 前端方向（规划中）

我们计划开发以下模块以构建 AI 学习助手原型：

| 模块            | 功能描述                                                |
|-----------------|---------------------------------------------------------|
| 📅 课程总览        | 所有课程结构/任务概览，支持时序视图和分类导航                       |
| 📋 作业任务面板     | 自动拉取所有作业 + 智能提醒（DDL 检测）                         |
| 🔔 通知聚合        | 聚合所有课程通知，按关键词/课程/时间分类筛选                      |
| 🤖 LLM 对话代理     | 与 GPT/Claude 接入，查询“下节课时间/我还有哪些作业”等课程状态        |
| 🧠 知识图谱生成器   | 把课程文档 + 通知结构化导入 Obsidian/Notion 笔记系统             |

---

## 🤝 致谢原项目

本项目基于北大开源项目：

- 🌟 [sshwy/pku3b](https://github.com/sshwy/pku3b)：由北大学生开发的教学网 CLI 工具，具备优秀的断点下载与命令行交互设计。

我们复用了其**后端通信与视频作业模块**，并在此基础上：
- ✨ 实现了**完整文档与通知模块**
- ✨ 建立了结构化**内容统一数据抽象层**
- ✨ 提供了 **Python 封装接口**，可供 AI 系统调用

---

## 🔧 安装与构建（开发者模式）

```bash
# 安装 Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 Python 构建依赖
pip install maturin

# 构建 Python 接口
maturin develop

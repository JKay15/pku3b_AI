# 🧰 MCP 工具函数总览（pku3b）


## 📂 课程结构树

| 函数名 | 功能说明 | 参数 |
|--------|----------|------|
| `get_course_tree` |  | `course_index` |
| `get_course_tree_summary` | 获取指定课程的内容结构树摘要（图形形式） | `course_index` |
| `find_node_in_tree_by_title` | 根据标题或 ID 在课程树中查找匹配节点 | `course_index, query` |
| `get_tree_node_detail` | 根据标题查找节点并返回详细结构信息 | `course_index, query` |

## 📄 文档

| 函数名 | 功能说明 | 参数 |
|--------|----------|------|
| `get_or_register_document` |  | `course_index, doc_title` |
| `find_document_handle_by_title` | 模糊查找指定课程中的文档标题，返回摘要信息 | `course_index, doc_title` |
| `get_document_descriptions` | 获取指定课程中某文档的正文内容 | `course_index, doc_title` |
| `get_document_attachments` | 获取指定课程中文档的附件信息（文件名和下载链接） | `course_index, doc_title` |
| `download_document_files` | 下载指定课程中某文档的所有附件 | `course_index, doc_title, target_dir` |
| `list_documents` |  | `course_index` |

## 📚 课程概况

| 函数名 | 功能说明 | 参数 |
|--------|----------|------|
| `get_course` |  | `index` |
| `get_course_title` |  | `course_index` |
| `get_course_entries` |  | `course_index` |
| `get_entry_links` |  | `course_index` |
| `get_course_summary` |  | `course_index` |
| `list_course_titles` |  | `` |
| `get_course_index_map` |  | `` |

## 📝 作业

| 函数名 | 功能说明 | 参数 |
|--------|----------|------|
| `get_or_register_assignment` |  | `course_index, assignment_title` |
| `find_assignment_handle_by_title` | 在指定课程中模糊查找作业标题，返回匹配项的结构摘要（summary），并注册 assignment | `course_index, assignment_title` |
| `get_assignment_descriptions` | 获取指定课程中某个作业的描述内容 | `course_index, assignment_title` |
| `get_assignment_attachments` | 获取指定作业的附件信息（文件名和下载链接） | `course_index, assignment_title` |
| `get_assignment_deadline` | 获取指定作业的截止时间 | `course_index, assignment_title` |
| `download_assignment_files` | 下载指定作业的所有附件到目标目录 | `course_index, assignment_title, target_dir` |
| `submit_assignment_file` | 将指定文件提交至该作业 | `course_index, assignment_title, file_path` |
| `list_submitted_assignments` |  | `course_index` |
| `list_unsubmitted_assignments` |  | `course_index` |
| `list_all_assignments` |  | `course_index` |

## 📢 通知公告

| 函数名 | 功能说明 | 参数 |
|--------|----------|------|
| `get_or_register_announcement` |  | `course_index, ann_title` |
| `find_announcement_handle_by_title` | 模糊查找课程中的通知公告标题，返回匹配项摘要 | `course_index, ann_title` |
| `get_announcement_descriptions` | 获取指定课程中某通知公告的正文内容 | `course_index, ann_title` |
| `get_announcement_attachments` | 获取指定课程中某通知的附件信息（文件名和下载链接） | `course_index, ann_title` |
| `download_announcement_files` | 下载指定通知公告的所有附件到目标路径 | `course_index, ann_title, target_dir` |
| `list_announcements` |  | `course_index` |

## 📦 其他

| 函数名 | 功能说明 | 参数 |
|--------|----------|------|
| `find_nodes_by_kind` | 查找课程结构树中所有指定类型的内容节点 | `course_index, kind` |
| `find_entries_by_keyword` |  | `course_index, keyword` |

## 📺 视频

| 函数名 | 功能说明 | 参数 |
|--------|----------|------|
| `get_or_register_video` |  | `course_index, video_title` |
| `download_video_by_title` | 根据视频标题下载课程视频（可选转为 MP4） | `course_index, video_title, target_dir, to_mp4` |
| `get_video_duration` | 获取指定课程中某个视频的时长 | `course_index, video_title` |
| `list_videos` |  | `course_index` |